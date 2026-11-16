//! Muninn UI.
//!
//! Design notes:
//! - one screen at a time; secondary surfaces are bottom sheets, never floating
//!   windows (they end up under the status bar on tall phones);
//! - every text field goes through [`ime_sync`], because winit's Android backend
//!   does not forward IME commits;
//! - the composer owns the bottom safe area and grows above the keyboard.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Margin, Pos2, Rect, RichText,
    ScrollArea, Sense, Stroke, Ui, Vec2,
};

use crate::api::{
    ChatRequest, Client, ClientConfig, ModelInfo, RequestMessage, Role, StreamEvent, Usage,
};
use crate::platform::Insets;
use crate::theme;
use serde::{Deserialize, Serialize};

pub const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

pub struct MuninnApp {
    screen: Screen,

    base_url: String,
    api_key: String,
    base_focus: bool,
    key_focus: bool,
    base_ime: String,
    key_ime: String,
    search_ime: String,
    composer_ime: String,
    base_sel: (usize, usize),
    key_sel: (usize, usize),
    search_sel: (usize, usize),
    composer_sel: (usize, usize),
    connecting: bool,
    error: Option<String>,

    models: Vec<ModelEntry>,
    model: String,
    sheet: Sheet,
    model_query: String,
    model_search_focus: bool,
    model_filter: ModelFilter,

    chats: Vec<ChatSnapshot>,
    active: usize,
    theme_mode: theme::Mode,
    drawer_open: bool,
    chat_query: String,
    chat_search_focus: bool,
    chat_search_ime: String,
    chat_search_sel: (usize, usize),

    title: String,
    messages: Vec<Message>,
    draft: String,
    ime_focused: bool,
    deep_think: bool,
    web: bool,
    streaming: bool,
    chat_cost: f64,
    chat_tokens: u64,

    tx: Sender<UiEvent>,
    rx: Receiver<UiEvent>,
    stream_task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Connect,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sheet {
    None,
    Models,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelFilter {
    All,
    Free,
    Think,
    Tools,
    Vision,
}

impl ModelFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "Все",
            Self::Free => "Free",
            Self::Think => "Think",
            Self::Tools => "Tools",
            Self::Vision => "Vision",
        }
    }

    fn accepts(self, model: &ModelEntry) -> bool {
        match self {
            Self::All => true,
            Self::Free => model.free,
            Self::Think => model.reasoning,
            Self::Tools => model.tools,
            Self::Vision => model.vision,
        }
    }
}

struct ModelEntry {
    id: String,
    name: String,
    context: u64,
    prompt_per_m: f64,
    reasoning: bool,
    tools: bool,
    vision: bool,
    free: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Message {
    role: Role,
    text: String,
    reasoning: Option<String>,
    cost: Option<f64>,
    tokens: Option<u64>,
    streaming: bool,
    error: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct ChatSnapshot {
    id: u64,
    title: String,
    model: String,
    cost: f64,
    tokens: u64,
    updated_at: u64,
    messages: Vec<Message>,
}

#[derive(Default, Serialize, Deserialize)]
struct Workspace {
    chats: Vec<ChatSnapshot>,
    active: usize,
    #[serde(default)]
    theme: theme::Mode,
}

enum UiEvent {
    Models(Vec<ModelInfo>),
    ConnectError(String),
    Delta(String),
    Reasoning(String),
    Usage(Usage),
    Finished,
    StreamError(String),
}

impl MuninnApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let _ = crate::runtime::rt();

        let (tx, rx) = channel();

        let mut app = Self {
            screen: Screen::Connect,
            base_url: OPENROUTER_BASE.to_owned(),
            api_key: String::new(),
            base_focus: false,
            key_focus: false,
            base_ime: String::new(),
            key_ime: String::new(),
            search_ime: String::new(),
            composer_ime: String::new(),
            base_sel: (0, 0),
            key_sel: (0, 0),
            search_sel: (0, 0),
            composer_sel: (0, 0),
            connecting: false,
            error: None,
            models: Vec::new(),
            model: "openrouter/free".to_owned(),
            sheet: Sheet::None,
            model_query: String::new(),
            model_search_focus: false,
            model_filter: ModelFilter::All,
            chats: Vec::new(),
            active: 0,
            theme_mode: theme::Mode::Dark,
            drawer_open: false,
            chat_query: String::new(),
            chat_search_focus: false,
            chat_search_ime: String::new(),
            chat_search_sel: (0, 0),
            title: "Новый чат".to_owned(),
            messages: Vec::new(),
            draft: String::new(),
            ime_focused: false,
            deep_think: false,
            web: false,
            streaming: false,
            chat_cost: 0.0,
            chat_tokens: 0,
            tx,
            rx,
            stream_task: None,
        };

        app.load_workspace();
        theme::init(&cc.egui_ctx, app.theme_mode);

        if let Some(secrets) = crate::secrets::load() {
            app.base_url = secrets.base_url;
            app.api_key = secrets.api_key;
            app.connect(&cc.egui_ctx);
        }

        app
    }

    // ------------------------------------------------------------------
    // Networking
    // ------------------------------------------------------------------

    fn connect(&mut self, ctx: &egui::Context) {
        if self.connecting {
            return;
        }

        let base = self.base_url.trim().trim_end_matches('/').to_owned();
        let key = self.api_key.trim().to_owned();

        if key.is_empty() {
            self.error = Some("Вставь API-ключ.".into());
            return;
        }

        self.connecting = true;
        self.error = None;

        let tx = self.tx.clone();
        let ctx = ctx.clone();

        crate::runtime::rt().spawn(async move {
            let client = match Client::new(ClientConfig {
                base_url: base.clone(),
                api_key: key,
            }) {
                Ok(client) => client,
                Err(err) => {
                    let _ = tx.send(UiEvent::ConnectError(err.to_string()));
                    ctx.request_repaint();
                    return;
                }
            };

            // `/models` is public on OpenRouter, so it cannot validate a key.
            if base.contains("openrouter.ai")
                && let Err(err) = client.key_info().await
            {
                let _ = tx.send(UiEvent::ConnectError(friendly_error(&err.to_string())));
                ctx.request_repaint();
                return;
            }

            match client.list_models().await {
                Ok(models) => {
                    let _ = tx.send(UiEvent::Models(models));
                }
                Err(err) => {
                    let _ = tx.send(UiEvent::ConnectError(friendly_error(&err.to_string())));
                }
            }
            ctx.request_repaint();
        });
    }

    fn apply_models(&mut self, models: Vec<ModelInfo>) {
        self.models = models
            .into_iter()
            .map(|m| {
                let prompt = m.pricing.prompt.parse::<f64>().unwrap_or(0.0);
                let completion = m.pricing.completion.parse::<f64>().unwrap_or(0.0);
                let free = (prompt == 0.0 && completion == 0.0) || m.id.ends_with(":free");
                ModelEntry {
                    id: m.id,
                    name: m.name,
                    context: m.context_length.unwrap_or(0),
                    prompt_per_m: prompt * 1_000_000.0,
                    reasoning: m.supported_parameters.iter().any(|p| p == "reasoning"),
                    tools: m.supported_parameters.iter().any(|p| p == "tools"),
                    vision: m.architecture.input_modalities.iter().any(|m| m == "image"),
                    free,
                }
            })
            .collect();

        if self.models.is_empty() {
            self.error = Some("Сервер вернул пустой список моделей.".into());
            return;
        }

        if !self.models.iter().any(|m| m.id == self.model) {
            self.model = self
                .models
                .iter()
                .find(|m| m.id == "openrouter/free")
                .or_else(|| self.models.iter().find(|m| m.free))
                .or_else(|| self.models.first())
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "openrouter/free".into());
        }

        self.screen = Screen::Chat;
        let _ = crate::secrets::save(&self.base_url, &self.api_key);
    }

    fn model_entry(&self) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == self.model)
    }

    fn model_display_name(&self) -> String {
        let raw = self
            .model_entry()
            .map(|m| {
                if m.name.is_empty() {
                    m.id.clone()
                } else {
                    m.name.clone()
                }
            })
            .unwrap_or_else(|| self.model.clone());

        raw.split_once(':')
            .map(|(_, rest)| rest.trim().to_owned())
            .unwrap_or(raw)
    }

    fn is_openrouter(&self) -> bool {
        self.base_url.contains("openrouter.ai")
    }

    fn send(&mut self, ctx: &egui::Context) {
        if self.streaming {
            return;
        }

        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }

        if self.title == "Новый чат" {
            self.title = text.chars().take(48).collect();
        }

        self.messages.push(Message {
            role: Role::User,
            text,
            reasoning: None,
            cost: None,
            tokens: None,
            streaming: false,
            error: false,
        });

        // The user turn must be part of the request; build history after
        // pushing it, and skip error bubbles.
        let history: Vec<RequestMessage> = self
            .messages
            .iter()
            .filter(|m| !m.error && !m.text.is_empty())
            .map(|m| RequestMessage::text(m.role, m.text.clone()))
            .collect();

        let mut request = ChatRequest::new(self.model.clone(), history);
        request.stream = Some(true);

        if self.deep_think && self.model_entry().is_some_and(|m| m.reasoning) {
            request.reasoning = Some(crate::api::Reasoning {
                effort: Some("medium".into()),
                ..Default::default()
            });
        }

        if self.web && self.is_openrouter() {
            request.plugins = Some(vec![crate::api::Plugin {
                id: "web".into(),
                params: Default::default(),
            }]);
        }

        self.messages.push(Message {
            role: Role::Assistant,
            text: String::new(),
            reasoning: None,
            cost: None,
            tokens: None,
            streaming: true,
            error: false,
        });

        self.draft.clear();
        self.composer_ime.clear();
        self.save_current();
        crate::platform::set_ime_text("", 0, 0);
        hide_keyboard(ctx);
        self.streaming = true;

        let client = match Client::new(ClientConfig {
            base_url: self.base_url.trim().trim_end_matches('/').to_owned(),
            api_key: self.api_key.trim().to_owned(),
        }) {
            Ok(client) => client,
            Err(err) => {
                self.fail_last(err.to_string());
                return;
            }
        };

        let tx = self.tx.clone();
        let ctx = ctx.clone();

        self.stream_task = Some(crate::runtime::rt().spawn(async move {
            use futures_util::StreamExt;

            let mut stream = client.stream_chat(request);
            while let Some(item) = stream.next().await {
                match item {
                    Ok(StreamEvent::Content(text)) => {
                        let _ = tx.send(UiEvent::Delta(text));
                    }
                    Ok(StreamEvent::Reasoning(text)) => {
                        let _ = tx.send(UiEvent::Reasoning(text));
                    }
                    Ok(StreamEvent::Usage(usage)) => {
                        let _ = tx.send(UiEvent::Usage(usage));
                    }
                    Ok(StreamEvent::Done) => break,
                    Ok(_) => {}
                    Err(err) => {
                        let _ = tx.send(UiEvent::StreamError(friendly_error(&err.to_string())));
                        break;
                    }
                }
                ctx.request_repaint();
            }

            let _ = tx.send(UiEvent::Finished);
            ctx.request_repaint();
        }));
    }

    fn stop(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        self.streaming = false;
        if let Some(msg) = self.messages.last_mut() {
            msg.streaming = false;
            if msg.text.is_empty() {
                msg.text = "[остановлено]".into();
            }
        }
        self.save_current();
    }

    fn fail_last(&mut self, error: String) {
        self.streaming = false;
        self.stream_task = None;
        if let Some(msg) = self.messages.last_mut() {
            msg.streaming = false;
            msg.error = true;
            if msg.text.is_empty() {
                msg.text = error;
            } else {
                msg.text.push_str(&format!("\n\n{error}"));
            }
        }
        self.save_current();
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                UiEvent::Models(models) => {
                    self.connecting = false;
                    self.apply_models(models);
                    hide_keyboard(ctx);
                }
                UiEvent::ConnectError(error) => {
                    self.connecting = false;
                    self.error = Some(error);
                }
                UiEvent::Delta(text) => {
                    if let Some(msg) = self.messages.last_mut() {
                        msg.text.push_str(&text);
                    }
                }
                UiEvent::Reasoning(text) => {
                    if let Some(msg) = self.messages.last_mut() {
                        msg.reasoning
                            .get_or_insert_with(String::new)
                            .push_str(&text);
                    }
                }
                UiEvent::Usage(usage) => {
                    let cost = usage.cost.unwrap_or(0.0);
                    self.chat_cost += cost;
                    self.chat_tokens += usage.total_tokens;
                    if let Some(msg) = self.messages.last_mut() {
                        msg.cost = usage.cost;
                        msg.tokens = Some(usage.total_tokens);
                    }
                }
                UiEvent::Finished => {
                    self.streaming = false;
                    self.stream_task = None;
                    if let Some(msg) = self.messages.last_mut() {
                        msg.streaming = false;
                        if msg.text.is_empty() && !msg.error {
                            msg.text = "[пустой ответ]".into();
                        }
                    }
                    self.save_current();
                }
                UiEvent::StreamError(error) => {
                    self.fail_last(error);
                }
            }
        }

        if self.streaming {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    // ------------------------------------------------------------------
    // Actions
    // ------------------------------------------------------------------

    fn open_sheet(&mut self, ctx: &egui::Context, sheet: Sheet) {
        self.sheet = sheet;
        self.model_search_focus = false;
        self.drawer_open = false;
        hide_keyboard(ctx);
    }

    fn close_sheet(&mut self, ctx: &egui::Context) {
        self.sheet = Sheet::None;
        self.model_search_focus = false;
        hide_keyboard(ctx);
    }

    fn new_chat(&mut self, ctx: &egui::Context) {
        self.stop();
        self.save_current();
        self.chats.push(self.empty_snapshot());
        self.active = self.chats.len() - 1;
        self.load_active();
        self.sheet = Sheet::None;
        hide_keyboard(ctx);
        self.persist();
    }

    fn change_key(&mut self, ctx: &egui::Context) {
        self.stop();
        self.save_current();
        crate::secrets::clear();
        self.screen = Screen::Connect;
        self.api_key.clear();
        self.base_url = OPENROUTER_BASE.to_owned();
        self.connecting = false;
        self.models.clear();
        self.messages.clear();
        self.title = "Новый чат".into();
        self.chat_cost = 0.0;
        self.chat_tokens = 0;
        self.sheet = Sheet::None;
        self.error = None;
        hide_keyboard(ctx);
    }

    // ------------------------------------------------------------------
    // Persistence
    // ------------------------------------------------------------------

    fn load_workspace(&mut self) {
        if let Some(bytes) = crate::secrets::load_blob("chats.bin")
            && let Ok(workspace) = serde_json::from_slice::<Workspace>(&bytes)
        {
            self.chats = workspace.chats;
            self.active = workspace.active;
            self.theme_mode = workspace.theme;
        }

        if self.chats.is_empty() {
            self.chats.push(self.empty_snapshot());
            self.active = 0;
        }

        self.active = self.active.min(self.chats.len().saturating_sub(1));
        self.load_active();
    }

    fn empty_snapshot(&self) -> ChatSnapshot {
        ChatSnapshot {
            id: next_id(),
            title: "Новый чат".to_owned(),
            model: if self.model.is_empty() {
                "openrouter/free".to_owned()
            } else {
                self.model.clone()
            },
            cost: 0.0,
            tokens: 0,
            updated_at: now_secs(),
            messages: Vec::new(),
        }
    }

    fn load_active(&mut self) {
        let Some(chat) = self.chats.get(self.active) else {
            return;
        };

        self.title = chat.title.clone();
        self.model = if chat.model.is_empty() {
            "openrouter/free".to_owned()
        } else {
            chat.model.clone()
        };
        self.chat_cost = chat.cost;
        self.chat_tokens = chat.tokens;
        self.messages = chat.messages.clone();

        for message in &mut self.messages {
            message.streaming = false;
        }
        // A stream interrupted before the first token leaves an empty bubble.
        self.messages.retain(|message| {
            !(message.role == Role::Assistant
                && message.text.is_empty()
                && message.reasoning.is_none()
                && !message.error)
        });
    }

    fn save_current(&mut self) {
        if self.chats.is_empty() {
            self.chats.push(self.empty_snapshot());
            self.active = 0;
        }

        let id = self
            .chats
            .get(self.active)
            .map(|chat| chat.id)
            .unwrap_or_else(next_id);

        let snapshot = ChatSnapshot {
            id,
            title: self.title.clone(),
            model: self.model.clone(),
            cost: self.chat_cost,
            tokens: self.chat_tokens,
            updated_at: now_secs(),
            messages: self.messages.clone(),
        };
        self.chats[self.active] = snapshot;
        self.persist();
    }

    fn persist(&self) {
        let workspace = Workspace {
            chats: self.chats.clone(),
            active: self.active,
            theme: self.theme_mode,
        };
        if let Ok(bytes) = serde_json::to_vec(&workspace) {
            let _ = crate::secrets::save_blob("chats.bin", &bytes);
        }
    }

    fn switch_chat(&mut self, ctx: &egui::Context, index: usize) {
        if index >= self.chats.len() || index == self.active {
            return;
        }
        self.stop();
        self.save_current();
        self.active = index;
        self.load_active();
        self.sheet = Sheet::None;
        hide_keyboard(ctx);
        self.persist();
    }

    // ------------------------------------------------------------------
    // Chat UI
    // ------------------------------------------------------------------

    fn header(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        ui.horizontal(|ui| {
            if menu_button(ui).clicked() {
                self.drawer_open = true;
                self.chat_search_focus = false;
                hide_keyboard(&ctx);
            }

            ui.add_space(2.0);

            let title_width = (ui.available_width() - 166.0).max(70.0);
            ui.allocate_ui_with_layout(
                Vec2::new(title_width, 40.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(truncate(&self.title, 24)).size(16.0).strong(),
                        )
                        .truncate(),
                    );
                    let subtitle = if self.chat_tokens == 0 && self.chat_cost == 0.0 {
                        "начни разговор".to_owned()
                    } else {
                        format!(
                            "{} · {} tok",
                            format_cost(self.chat_cost),
                            format_tokens(self.chat_tokens)
                        )
                    };
                    ui.label(
                        RichText::new(subtitle)
                            .size(10.5)
                            .color(theme::c().text_secondary),
                    );
                },
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if model_chip(ui, &self.model_display_name(), 150.0).clicked() {
                    self.open_sheet(&ctx, Sheet::Models);
                }
            });
        });
    }

    fn bubble(&self, ui: &mut Ui, msg: &Message) {
        let is_user = msg.role == Role::User;
        let viewport = ui.clip_rect().width();
        let max_w = (viewport * if is_user { 0.84 } else { 0.94 }).max(160.0);
        let text_w = (max_w - if is_user { 24.0 } else { 34.0 }).max(120.0);

        if is_user {
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                egui::Frame::new()
                    .fill(theme::c().user_bubble)
                    .corner_radius(20)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(text_w);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&msg.text)
                                    .size(15.0)
                                    .color(theme::c().on_user),
                            )
                            .wrap(),
                        );
                    });
            });
            ui.add_space(14.0);
            return;
        }

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(text_w);
            ui.horizontal(|ui| {
                avatar(ui);
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Muninn")
                        .size(10.5)
                        .color(theme::c().text_secondary),
                );
            });
            ui.add_space(5.0);

            if let Some(reasoning) = &msg.reasoning {
                let label = if msg.streaming && msg.text.is_empty() {
                    "Размышления…".to_owned()
                } else {
                    format!("Размышления · {} симв.", reasoning.chars().count())
                };

                egui::Frame::new()
                    .fill(theme::c().surface_raised)
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0, theme::c().border))
                    .inner_margin(Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        egui::CollapsingHeader::new(
                            RichText::new(label)
                                .size(10.5)
                                .color(theme::c().text_secondary),
                        )
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(reasoning)
                                        .monospace()
                                        .size(12.0)
                                        .color(theme::c().text_secondary),
                                )
                                .wrap(),
                            );
                        });
                    });
                ui.add_space(8.0);
            }

            if msg.error {
                egui::Frame::new()
                    .fill(theme::c().surface)
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0, theme::c().danger))
                    .inner_margin(Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&msg.text).size(13.5).color(theme::c().danger),
                            )
                            .wrap(),
                        );
                    });
            } else if !msg.text.is_empty() {
                crate::markdown::render(ui, &msg.text, text_w, msg.streaming);
            } else if msg.streaming {
                ui.horizontal(|ui| {
                    typing_dot(ui);
                    ui.label(
                        RichText::new("печатает…")
                            .size(11.0)
                            .color(theme::c().text_secondary),
                    );
                });
            }

            if !msg.streaming && !msg.text.is_empty() {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if link_button(ui, "копировать").clicked() {
                        crate::platform::clipboard_set(&msg.text);
                    }
                    if let (Some(cost), Some(tokens)) = (msg.cost, msg.tokens) {
                        ui.label(
                            RichText::new(format!(
                                "{} · {} tok",
                                format_cost(cost),
                                format_tokens(tokens)
                            ))
                            .size(9.5)
                            .color(theme::c().text_secondary),
                        );
                    }
                });
            }
        });

        ui.add_space(18.0);
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let top = ((ui.available_height() - 320.0) * 0.22).clamp(12.0, 140.0);
        ui.add_space(top);

        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 28.0, theme::c().accent_soft);
            ui.painter()
                .circle_stroke(rect.center(), 27.0, Stroke::new(1.0, theme::c().border));
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "M",
                FontId::proportional(22.0),
                theme::c().accent,
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Чем помочь?").size(26.0).strong());
            ui.add_space(6.0);
            ui.label(
                RichText::new("История и файлы остаются на устройстве.")
                    .size(13.0)
                    .color(theme::c().text_secondary),
            );
        });

        ui.add_space(26.0);

        let width = ((ui.available_width() - 12.0) / 2.0).max(120.0);
        let rows = [
            [
                ("Объясни код", "Разбери фрагмент"),
                ("Составь план", "Шаги и сроки"),
            ],
            [
                ("Переведи", "С сохранением стиля"),
                ("Напиши письмо", "Деловое или личное"),
            ],
        ];

        for row in rows {
            ui.horizontal(|ui| {
                for (title, subtitle) in row {
                    if suggestion_card(ui, title, subtitle, width).clicked() {
                        self.draft = format!("{title}: ");
                        ui.ctx()
                            .memory_mut(|m| m.request_focus(Id::unique("composer")));
                    }
                }
            });
            ui.add_space(8.0);
        }
    }

    fn composer(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut submit = false;

        let composer_id = Id::unique("composer");
        let focused = ui.memory(|m| m.focused() == Some(composer_id));

        egui::Frame::new()
            .fill(theme::c().surface_raised)
            .corner_radius(20)
            .stroke(Stroke::new(
                if focused { 1.5 } else { 1.0 },
                if focused {
                    theme::c().accent
                } else {
                    theme::c().border
                },
            ))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                let edit = egui::TextEdit::multiline(&mut self.draft)
                    .id(composer_id)
                    .hint_text("Сообщение…")
                    .desired_rows(1)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(14.5))
                    .frame(egui::Frame::new())
                    .margin(Margin::symmetric(4, 6));

                let resp = ui.add_sized([ui.available_width(), 38.0], edit);
                ime_sync(
                    ui,
                    &resp,
                    &mut self.draft,
                    &mut self.ime_focused,
                    &mut self.composer_ime,
                    &mut self.composer_sel,
                    true,
                    false,
                );

                if resp.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                {
                    submit = true;
                }

                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    if self.model_entry().is_some_and(|m| m.reasoning)
                        && toggle_pill(ui, "Think", self.deep_think)
                    {
                        self.deep_think = !self.deep_think;
                    }
                    if self.is_openrouter() && toggle_pill(ui, "Web", self.web) {
                        self.web = !self.web;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.streaming {
                            if stop_button(ui).clicked() {
                                self.stop();
                            }
                        } else if send_button(ui, !self.draft.trim().is_empty()).clicked() {
                            submit = true;
                        }
                    });
                });
            });

        if submit && !self.streaming {
            self.send(ctx);
        }
    }

    // ------------------------------------------------------------------
    // Onboarding
    // ------------------------------------------------------------------

    fn onboarding(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(36.0);
                ui.vertical_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 28.0, theme::c().accent_soft);
                    ui.painter().circle_stroke(
                        rect.center(),
                        27.0,
                        Stroke::new(1.0, theme::c().border),
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "M",
                        FontId::proportional(22.0),
                        theme::c().accent,
                    );

                    ui.add_space(12.0);
                    ui.label(RichText::new("Muninn").size(28.0).strong());
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Приватный клиент OpenRouter")
                            .size(13.0)
                            .color(theme::c().text_secondary),
                    );
                });

                ui.add_space(28.0);

                let card_width = (ui.available_width() - 32.0).min(390.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - card_width) * 0.5).max(0.0));
                    ui.allocate_ui_with_layout(
                        Vec2::new(card_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(theme::c().surface)
                                .corner_radius(20)
                                .stroke(Stroke::new(1.0, theme::c().border))
                                .inner_margin(Margin::same(16))
                                .show(ui, |ui| {
                                    field_label(ui, "BASE URL");
                                    let base = egui::TextEdit::singleline(&mut self.base_url)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(OPENROUTER_BASE)
                                        .margin(Margin::symmetric(12, 12));
                                    let base_resp = ui.add(base);
                                    ime_sync(
                                        ui,
                                        &base_resp,
                                        &mut self.base_url,
                                        &mut self.base_focus,
                                        &mut self.base_ime,
                                        &mut self.base_sel,
                                        false,
                                        false,
                                    );

                                    ui.add_space(12.0);
                                    field_label(ui, "API KEY");
                                    let key = egui::TextEdit::singleline(&mut self.api_key)
                                        .desired_width(f32::INFINITY)
                                        .hint_text("sk-or-v1-…")
                                        .password(true)
                                        .margin(Margin::symmetric(12, 12));
                                    let key_resp = ui.add(key);
                                    ime_sync(
                                        ui,
                                        &key_resp,
                                        &mut self.api_key,
                                        &mut self.key_focus,
                                        &mut self.key_ime,
                                        &mut self.key_sel,
                                        false,
                                        true,
                                    );

                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("Вставить ключ").size(12.0),
                                                )
                                                .fill(theme::c().surface_raised)
                                                .corner_radius(12)
                                                .min_size(Vec2::new(0.0, 40.0)),
                                            )
                                            .clicked()
                                            && let Some(text) = crate::platform::clipboard_get()
                                        {
                                            self.api_key = text.trim().to_owned();
                                            if self.key_focus {
                                                crate::platform::set_ime_text(
                                                    &self.api_key,
                                                    self.api_key.len(),
                                                    self.api_key.len(),
                                                );
                                            }
                                        }

                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("OpenRouter").size(12.0),
                                                )
                                                .fill(theme::c().surface_raised)
                                                .corner_radius(12)
                                                .min_size(Vec2::new(0.0, 40.0)),
                                            )
                                            .clicked()
                                        {
                                            self.base_url = OPENROUTER_BASE.to_owned();
                                            if self.base_focus {
                                                crate::platform::set_ime_text(
                                                    &self.base_url,
                                                    self.base_url.len(),
                                                    self.base_url.len(),
                                                );
                                            }
                                        }
                                    });

                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        let (dot, _) = ui.allocate_exact_size(
                                            Vec2::splat(7.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            dot.center(),
                                            3.0,
                                            theme::c().success,
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Ключ шифруется Android Keystore",
                                            )
                                            .size(10.5)
                                            .color(theme::c().text_secondary),
                                        );
                                    });
                                    ui.label(
                                        RichText::new(
                                            "Бесплатные модели лимитированы. Ключ: openrouter.ai/keys",
                                        )
                                        .size(10.5)
                                        .color(theme::c().warning),
                                    );

                                    ui.add_space(16.0);

                                    let can_connect =
                                        !self.connecting && !self.api_key.trim().is_empty();
                                    if ui
                                        .add_enabled(
                                            can_connect,
                                            egui::Button::new(
                                                RichText::new(if self.connecting {
                                                    "Подключаюсь…"
                                                } else {
                                                    "Подключиться"
                                                })
                                                .size(15.0)
                                                .color(if can_connect {
                                                    theme::c().on_accent
                                                } else {
                                                    theme::c().text_tertiary
                                                }),
                                            )
                                            .fill(if can_connect {
                                                theme::c().accent
                                            } else {
                                                theme::c().surface_raised
                                            })
                                            .corner_radius(14)
                                            .min_size(Vec2::new(ui.available_width(), 48.0)),
                                        )
                                        .clicked()
                                    {
                                        hide_keyboard(ctx);
                                        self.connect(ctx);
                                    }

                                    if self.connecting {
                                        ui.add_space(10.0);
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label(
                                                RichText::new(
                                                    "проверяю ключ и загружаю модели",
                                                )
                                                .size(11.0)
                                                .color(theme::c().text_secondary),
                                            );
                                        });
                                    }

                                    if let Some(error) = &self.error {
                                        ui.add_space(10.0);
                                        ui.label(
                                            RichText::new(error)
                                                .size(11.5)
                                                .color(theme::c().danger),
                                        );
                                    }

                                    ui.add_space(6.0);
                                    if link_button(ui, "Забыть сохранённый ключ").clicked() {
                                        crate::secrets::clear();
                                        self.api_key.clear();
                                        self.error = None;
                                    }
                                });
                        },
                    );
                });

                ui.add_space(36.0);
            });
    }

    // ------------------------------------------------------------------
    // Bottom sheets
    // ------------------------------------------------------------------

    fn sheets(&mut self, ctx: &egui::Context, screen: Rect, insets: Insets) {
        if self.sheet == Sheet::None {
            return;
        }

        let bottom = screen.bottom() - insets.bottom;
        let max_h = (bottom - screen.top() - 16.0).max(220.0);
        let wanted = match self.sheet {
            Sheet::Models => screen.height() * 0.72,
            Sheet::Settings => 420.0,
            Sheet::None => 0.0,
        };
        let height = wanted.min(620.0).min(max_h);
        let rect = Rect::from_min_max(
            Pos2::new(screen.left(), bottom - height),
            Pos2::new(screen.right(), bottom),
        );

        let mut dismiss = false;
        egui::Area::new(Id::unique("sheet_overlay"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(r, 0, theme::c().scrim);
                if resp.clicked() {
                    dismiss = true;
                }
            });

        let mut close = false;
        egui::Area::new(Id::unique("sheet"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_width(rect.width());
                ui.set_height(rect.height());
                ui.set_clip_rect(rect);
                egui::Frame::new()
                    .fill(theme::c().surface)
                    .corner_radius(CornerRadius {
                        nw: 24,
                        ne: 24,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(Margin {
                        left: 16,
                        right: 16,
                        top: 8,
                        bottom: 12,
                    })
                    .show(ui, |ui| {
                        let (handle, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 14.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            Rect::from_center_size(handle.center(), Vec2::new(36.0, 4.0)),
                            2,
                            theme::c().border,
                        );

                        match self.sheet {
                            Sheet::Models => self.models_sheet(ui, &mut close),
                            Sheet::Settings => self.settings_sheet(ui, &mut close),
                            Sheet::None => {}
                        }
                    });
            });

        if dismiss || close {
            self.close_sheet(ctx);
        }
    }

    fn models_sheet(&mut self, ui: &mut Ui, close: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Модели").size(18.0).strong());
            ui.label(
                RichText::new(format!("{}", self.models.len()))
                    .size(10.0)
                    .color(theme::c().text_secondary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if close_button(ui).clicked() {
                    *close = true;
                }
            });
        });

        ui.add_space(8.0);

        let search = egui::TextEdit::singleline(&mut self.model_query)
            .hint_text("Поиск по названию или id…")
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(12, 9));
        let resp = ui.add(search);
        ime_sync(
            ui,
            &resp,
            &mut self.model_query,
            &mut self.model_search_focus,
            &mut self.search_ime,
            &mut self.search_sel,
            false,
            false,
        );

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for filter in [
                ModelFilter::All,
                ModelFilter::Free,
                ModelFilter::Think,
                ModelFilter::Tools,
                ModelFilter::Vision,
            ] {
                if filter_chip(ui, filter.label(), self.model_filter == filter).clicked() {
                    self.model_filter = filter;
                }
            }
        });

        ui.add_space(6.0);

        let indices = self.filtered_models();
        ui.label(
            RichText::new(format!("{} моделей", indices.len()))
                .size(10.0)
                .color(theme::c().text_secondary),
        );
        ui.add_space(4.0);

        let list_height = (ui.available_height() - 6.0).max(120.0);
        let mut chosen: Option<String> = None;

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(list_height)
                .show_rows(ui, 64.0, indices.len(), |ui, range| {
                    for i in range {
                        let idx = indices[i];
                        let model = &self.models[idx];
                        let selected = model.id == self.model;
                        let row = model_row(ui, model, selected, ui.available_width());
                        if row.clicked() {
                            chosen = Some(model.id.clone());
                        }
                    }
                });
        });

        if let Some(id) = chosen {
            self.model = id;
            *close = true;
        }
    }

    fn filtered_models(&self) -> Vec<usize> {
        let query = self.model_query.trim().to_lowercase();

        self.models
            .iter()
            .enumerate()
            .filter(|(_, model)| {
                let matches_query = query.is_empty()
                    || model.id.to_lowercase().contains(&query)
                    || model.name.to_lowercase().contains(&query);
                matches_query && self.model_filter.accepts(model)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn drawer(&mut self, ctx: &egui::Context, screen: Rect, insets: Insets) {
        let t = ctx.animate_bool(Id::unique("drawer"), self.drawer_open);
        if !self.drawer_open && t <= 0.001 {
            return;
        }

        let width = (screen.width() * 0.86).min(340.0);
        let bottom = screen.bottom() - insets.bottom;

        let mut dismiss = false;
        let scrim_alpha = (theme::c().scrim.a() as f32 * t) as u8;
        egui::Area::new(Id::unique("drawer_scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, scrim_alpha),
                );
                if resp.clicked() {
                    dismiss = true;
                }
            });

        let x = -width * (1.0 - t);
        let rect = Rect::from_min_size(
            Pos2::new(x, screen.top()),
            Vec2::new(width, bottom - screen.top()),
        );

        let mut close = false;
        let mut chosen: Option<usize> = None;
        let mut do_new = false;
        let mut do_settings = false;
        let mut do_key = false;

        egui::Area::new(Id::unique("drawer"))
            .order(egui::Order::Foreground)
            .constrain(false)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_width(rect.width());
                ui.set_height(rect.height());
                ui.set_clip_rect(rect);

                egui::Frame::new()
                    .fill(theme::c().surface)
                    .corner_radius(CornerRadius {
                        nw: 0,
                        ne: 18,
                        sw: 0,
                        se: 18,
                    })
                    .inner_margin(Margin {
                        left: 16,
                        right: 16,
                        top: (insets.top as i8).saturating_add(8),
                        bottom: (insets.bottom as i8).saturating_add(8),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter().circle_filled(
                                dot.center(),
                                3.0,
                                if self.screen == Screen::Chat {
                                    theme::c().success
                                } else {
                                    theme::c().warning
                                },
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new("MUNINN")
                                    .size(11.0)
                                    .color(theme::c().accent)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if close_button(ui).clicked() {
                                    close = true;
                                }
                            });
                        });

                        ui.add_space(8.0);

                        let search = egui::TextEdit::singleline(&mut self.chat_query)
                            .hint_text("Поиск по чатам")
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(12, 10));
                        let resp = ui.add(search);
                        ime_sync(
                            ui,
                            &resp,
                            &mut self.chat_query,
                            &mut self.chat_search_focus,
                            &mut self.chat_search_ime,
                            &mut self.chat_search_sel,
                            false,
                            false,
                        );

                        ui.add_space(8.0);
                        let new_chat = egui::Button::new(
                            RichText::new("+  Новый чат")
                                .size(14.0)
                                .color(theme::c().on_accent),
                        )
                        .fill(theme::c().accent)
                        .corner_radius(14)
                        .min_size(Vec2::new(ui.available_width(), 44.0));
                        if ui.add(new_chat).clicked() {
                            do_new = true;
                        }

                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("ЧАТЫ")
                                .size(9.0)
                                .color(theme::c().text_tertiary),
                        );
                        ui.add_space(4.0);

                        let footer_space = 112.0;
                        let list_height = (ui.available_height() - footer_space).max(120.0);
                        let query = self.chat_query.trim().to_lowercase();
                        let indices: Vec<usize> = (0..self.chats.len())
                            .rev()
                            .filter(|index| {
                                let chat = &self.chats[*index];
                                let title = if *index == self.active {
                                    &self.title
                                } else {
                                    &chat.title
                                };
                                let preview = if *index == self.active {
                                    self.messages.last().map(|m| m.text.as_str()).unwrap_or("")
                                } else {
                                    chat.messages.last().map(|m| m.text.as_str()).unwrap_or("")
                                };
                                query.is_empty()
                                    || title.to_lowercase().contains(&query)
                                    || preview.to_lowercase().contains(&query)
                            })
                            .collect();

                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .max_height(list_height)
                                .show(ui, |ui| {
                                    for index in indices {
                                        let chat = &self.chats[index];
                                        let active = index == self.active;
                                        let title = if active { &self.title } else { &chat.title };
                                        let preview = if active {
                                            self.messages
                                                .last()
                                                .map(|m| m.text.as_str())
                                                .unwrap_or("Пустой чат")
                                        } else {
                                            chat.messages
                                                .last()
                                                .map(|m| m.text.as_str())
                                                .unwrap_or("Пустой чат")
                                        };
                                        let time = if active {
                                            "сейчас".to_owned()
                                        } else {
                                            relative_time(chat.updated_at)
                                        };

                                        if drawer_chat_row(
                                            ui,
                                            title,
                                            preview,
                                            &time,
                                            active,
                                            ui.available_width(),
                                        )
                                        .clicked()
                                        {
                                            chosen = Some(index);
                                        }
                                    }
                                });
                        });

                        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                            if menu_row(ui, "Сменить ключ", "Keystore и автоподключение").clicked()
                            {
                                do_key = true;
                            }
                            if menu_row(ui, "Настройки", "тема, ключ, о приложении").clicked()
                            {
                                do_settings = true;
                            }
                        });
                    });
            });

        if dismiss || close {
            self.drawer_open = false;
            hide_keyboard(ctx);
        }
        if do_new {
            self.new_chat(ctx);
            self.drawer_open = false;
            return;
        }
        if do_settings {
            self.drawer_open = false;
            self.sheet = Sheet::Settings;
            hide_keyboard(ctx);
            return;
        }
        if do_key {
            self.drawer_open = false;
            self.change_key(ctx);
            return;
        }
        if let Some(index) = chosen {
            self.switch_chat(ctx, index);
            self.drawer_open = false;
        }
    }

    fn settings_sheet(&mut self, ui: &mut Ui, close: &mut bool) {
        let ctx = ui.ctx().clone();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Настройки").size(18.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if close_button(ui).clicked() {
                    *close = true;
                }
            });
        });

        ui.add_space(10.0);
        ui.label(
            RichText::new("ТЕМА")
                .size(9.0)
                .color(theme::c().text_tertiary),
        );
        ui.add_space(6.0);

        let card_width = ((ui.available_width() - 16.0) / 3.0).max(80.0);
        ui.horizontal(|ui| {
            for mode in [theme::Mode::Dark, theme::Mode::Amoled, theme::Mode::Light] {
                if theme_card(ui, mode, self.theme_mode == mode, card_width).clicked() {
                    self.theme_mode = mode;
                    theme::set_mode(&ctx, mode);
                    self.persist();
                }
            }
        });

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);

        if menu_row(ui, "Сменить ключ", "Keystore и автоподключение").clicked()
        {
            self.change_key(&ctx);
            *close = true;
        }
        if menu_row(ui, "О приложении", "Muninn 0.1 · локально, без телеметрии").clicked()
        {
            // Reserved.
        }
        ui.add_space(8.0);
    }
}

impl eframe::App for MuninnApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.drain_events(ui.ctx());

        if crate::platform::take_back_requested() {
            if self.drawer_open {
                self.drawer_open = false;
                hide_keyboard(ui.ctx());
            } else if self.sheet != Sheet::None {
                self.close_sheet(ui.ctx());
            }
        }
        crate::platform::set_back_handled(self.drawer_open || self.sheet != Sheet::None);

        let screen = ui.max_rect();
        let insets = crate::platform::system_insets(ui);

        if insets.top > 0.5 {
            egui::Panel::top("muninn_safe_top")
                .exact_size(insets.top)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::c().surface))
                .show(ui, |_| {});
        }

        if insets.bottom > 0.5 {
            egui::Panel::bottom("muninn_safe_bottom")
                .exact_size(insets.bottom)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::c().bg))
                .show(ui, |_| {});
        }

        let side = (12.0 + insets.left.max(insets.right)).round() as i8;
        let ctx = ui.ctx().clone();

        match self.screen {
            Screen::Connect => {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(theme::c().bg)
                            .inner_margin(Margin::symmetric(side, 0)),
                    )
                    .show(ui, |ui| self.onboarding(ui, &ctx));
            }
            Screen::Chat => {
                egui::Panel::top("muninn_header")
                    .exact_size(60.0)
                    .show_separator_line(true)
                    .frame(
                        egui::Frame::new()
                            .fill(theme::c().surface)
                            .inner_margin(Margin::symmetric(12, 8)),
                    )
                    .show(ui, |ui| self.header(ui));

                egui::Panel::bottom("muninn_composer")
                    .frame(
                        egui::Frame::new()
                            .fill(theme::c().bg)
                            .inner_margin(Margin::symmetric(10, 8)),
                    )
                    .show(ui, |ui| self.composer(ui, &ctx));

                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(theme::c().bg)
                            .inner_margin(Margin::symmetric(side, 0)),
                    )
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.add_space(10.0);

                                if self.messages.is_empty() {
                                    self.empty_state(ui);
                                }

                                for msg in &self.messages {
                                    self.bubble(ui, msg);
                                }

                                ui.add_space(4.0);
                            });
                    });
            }
        }

        self.drawer(&ctx, screen, insets);
        self.sheets(&ctx, screen, insets);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::c().bg;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }
}

// ----------------------------------------------------------------------
// Widgets
// ----------------------------------------------------------------------

fn avatar(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 12.0, theme::c().accent_soft);
    ui.painter()
        .circle_stroke(rect.center(), 11.5, Stroke::new(1.0, theme::c().border));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "M",
        FontId::proportional(11.0),
        theme::c().accent,
    );
}

fn typing_dot(ui: &mut Ui) {
    let time = ui.input(|i| i.time);
    for index in 0..3 {
        let phase = ((time * 3.0 + index as f64 * 0.6).sin() * 0.5 + 0.5) * 0.75 + 0.25;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 14.0), Sense::hover());
        ui.painter().circle_filled(
            rect.center(),
            2.2,
            theme::c().accent.gamma_multiply(phase as f32),
        );
    }
}

fn menu_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    let color = if resp.hovered() {
        theme::c().text
    } else {
        theme::c().text_secondary
    };
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 10, theme::c().surface_overlay);
    }
    let p = ui.painter();
    let c = rect.center();
    for dy in [-6.0, 0.0, 6.0] {
        p.line_segment(
            [
                Pos2::new(c.x - 8.0, c.y + dy),
                Pos2::new(c.x + 8.0, c.y + dy),
            ],
            Stroke::new(1.6, color),
        );
    }
    resp
}

fn close_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    let color = if resp.hovered() {
        theme::c().text
    } else {
        theme::c().text_secondary
    };
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 9, theme::c().surface_overlay);
    }
    let c = rect.center();
    let p = ui.painter();
    p.line_segment(
        [
            Pos2::new(c.x - 5.0, c.y - 5.0),
            Pos2::new(c.x + 5.0, c.y + 5.0),
        ],
        Stroke::new(1.5, color),
    );
    p.line_segment(
        [
            Pos2::new(c.x + 5.0, c.y - 5.0),
            Pos2::new(c.x - 5.0, c.y + 5.0),
        ],
        Stroke::new(1.5, color),
    );
    resp
}

fn send_button(ui: &mut Ui, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    let fill = if enabled {
        if resp.hovered() {
            theme::c().accent_hover
        } else {
            theme::c().accent
        }
    } else {
        theme::c().surface_overlay
    };
    ui.painter().circle_filled(rect.center(), 22.0, fill);
    let color = if enabled {
        theme::c().on_accent
    } else {
        theme::c().text_secondary
    };
    let c = rect.center();
    let p = ui.painter();
    p.line_segment(
        [Pos2::new(c.x, c.y + 6.0), Pos2::new(c.x, c.y - 6.0)],
        Stroke::new(1.7, color),
    );
    p.line_segment(
        [Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x, c.y - 6.0)],
        Stroke::new(1.7, color),
    );
    p.line_segment(
        [Pos2::new(c.x + 4.0, c.y - 2.0), Pos2::new(c.x, c.y - 6.0)],
        Stroke::new(1.7, color),
    );
    resp
}

fn stop_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    let fill = if resp.hovered() {
        theme::c().border_strong
    } else {
        theme::c().surface_overlay
    };
    ui.painter().circle_filled(rect.center(), 22.0, fill);
    ui.painter().rect_filled(
        egui::Rect::from_center_size(rect.center(), Vec2::splat(12.0)),
        2,
        theme::c().text,
    );
    resp
}

fn toggle_pill(ui: &mut Ui, label: &str, active: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0, 34.0), Sense::click());
    let fill = if active {
        theme::c().accent_soft
    } else if resp.hovered() {
        theme::c().surface_overlay
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        Stroke::NONE
    } else {
        Stroke::new(1.0, theme::c().border)
    };
    ui.painter()
        .rect(rect, 13, fill, stroke, egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        if active {
            theme::c().accent
        } else {
            theme::c().text_secondary
        },
    );
    resp.clicked()
}

fn filter_chip(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0, 32.0), Sense::click());
    let fill = if active {
        theme::c().accent_soft
    } else if resp.hovered() {
        theme::c().surface_overlay
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        Stroke::NONE
    } else {
        Stroke::new(1.0, theme::c().border)
    };
    ui.painter()
        .rect(rect, 13, fill, stroke, egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        if active {
            theme::c().accent
        } else {
            theme::c().text_secondary
        },
    );
    resp
}

fn suggestion_card(ui: &mut Ui, title: &str, subtitle: &str, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 68.0), Sense::click());
    let fill = if resp.hovered() {
        theme::c().surface_overlay
    } else {
        theme::c().surface_raised
    };
    ui.painter().rect(
        rect,
        16,
        fill,
        Stroke::new(1.0, theme::c().border),
        egui::StrokeKind::Inside,
    );

    let dot = Pos2::new(rect.left() + 16.0, rect.top() + 20.0);
    ui.painter().circle_filled(dot, 4.0, theme::c().accent);

    let p = ui.painter().with_clip_rect(rect);
    p.text(
        Pos2::new(rect.left() + 28.0, rect.top() + 11.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(13.0),
        theme::c().text,
    );
    p.text(
        Pos2::new(rect.left() + 28.0, rect.top() + 32.0),
        Align2::LEFT_TOP,
        subtitle,
        FontId::proportional(10.0),
        theme::c().text_secondary,
    );
    resp
}

fn model_chip(ui: &mut Ui, label: &str, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::click());
    let fill = if resp.hovered() {
        theme::c().surface_overlay
    } else {
        theme::c().surface_raised
    };
    ui.painter().rect_filled(rect, 20, fill);
    let p = ui.painter().with_clip_rect(rect);
    p.text(
        Pos2::new(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        truncate(label, 19),
        FontId::proportional(12.5),
        theme::c().text,
    );
    let c = Pos2::new(rect.right() - 14.0, rect.center().y - 1.0);
    p.line_segment(
        [Pos2::new(c.x - 3.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.0)],
        Stroke::new(1.4, theme::c().text_secondary),
    );
    p.line_segment(
        [Pos2::new(c.x + 3.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.0)],
        Stroke::new(1.4, theme::c().text_secondary),
    );
    resp
}

fn model_row(ui: &mut Ui, model: &ModelEntry, selected: bool, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click());
    let fill = if selected {
        theme::c().accent_soft
    } else if resp.hovered() {
        theme::c().surface_overlay
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect.shrink(2.0), 12, fill);
    }

    let p = ui.painter().with_clip_rect(rect);
    let name = if model.name.is_empty() {
        &model.id
    } else {
        &model.name
    };

    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 9.0),
        Align2::LEFT_TOP,
        truncate(name, 30),
        FontId::proportional(13.5),
        theme::c().text,
    );
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 28.0),
        Align2::LEFT_TOP,
        truncate(&model.id, 38),
        FontId::proportional(9.5),
        theme::c().text_secondary,
    );

    let mut badges = Vec::new();
    if model.free {
        badges.push("free");
    }
    if model.reasoning {
        badges.push("think");
    }
    if model.tools {
        badges.push("tools");
    }
    if model.vision {
        badges.push("vision");
    }
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 45.0),
        Align2::LEFT_TOP,
        badges.join(" · "),
        FontId::proportional(9.0),
        if selected {
            theme::c().accent
        } else {
            theme::c().text_secondary
        },
    );

    let price = if model.free {
        "free".to_owned()
    } else {
        format!("${:.2}/M", model.prompt_per_m)
    };
    p.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 9.0),
        Align2::RIGHT_TOP,
        price,
        FontId::proportional(10.0),
        if model.free {
            theme::c().accent
        } else {
            theme::c().text_secondary
        },
    );
    p.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 28.0),
        Align2::RIGHT_TOP,
        format!("{} ctx", format_tokens(model.context)),
        FontId::proportional(9.0),
        theme::c().text_secondary,
    );

    resp
}

fn theme_card(ui: &mut Ui, mode: theme::Mode, selected: bool, width: f32) -> egui::Response {
    let palette = mode_palette(mode);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 76.0), Sense::click());
    let fill = if selected {
        theme::c().accent_soft
    } else if resp.hovered() {
        theme::c().surface_overlay
    } else {
        theme::c().surface_raised
    };
    let stroke = if selected {
        Stroke::new(1.5, theme::c().accent)
    } else {
        Stroke::new(1.0, theme::c().border)
    };
    ui.painter()
        .rect(rect, 14, fill, stroke, egui::StrokeKind::Inside);

    let swatch = egui::Rect::from_min_size(
        egui::Pos2::new(rect.left() + 12.0, rect.top() + 12.0),
        Vec2::new(28.0, 28.0),
    );
    ui.painter()
        .circle_filled(swatch.center(), 14.0, palette.bg);
    ui.painter()
        .circle_stroke(swatch.center(), 13.0, Stroke::new(1.0, palette.border));
    ui.painter()
        .circle_filled(swatch.center() + Vec2::new(6.0, 6.0), 5.0, palette.accent);

    ui.painter().text(
        egui::Pos2::new(rect.left() + 12.0, rect.top() + 50.0),
        Align2::LEFT_TOP,
        mode.label(),
        FontId::proportional(11.5),
        theme::c().text,
    );
    resp
}

fn mode_palette(mode: theme::Mode) -> theme::Palette {
    theme::palette_of(mode)
}

fn drawer_chat_row(
    ui: &mut Ui,
    title: &str,
    preview: &str,
    time: &str,
    selected: bool,
    width: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click());
    let fill = if selected {
        theme::c().accent_soft
    } else if resp.hovered() {
        theme::c().surface_overlay
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect.shrink(2.0), 12, fill);
    }

    let p = ui.painter().with_clip_rect(rect);
    p.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 9.0),
        Align2::LEFT_TOP,
        truncate(title, 28),
        FontId::proportional(13.0),
        theme::c().text,
    );
    p.text(
        Pos2::new(rect.right() - 10.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        time,
        FontId::proportional(9.0),
        theme::c().text_tertiary,
    );
    p.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 32.0),
        Align2::LEFT_TOP,
        truncate(preview.replace('\n', " ").trim(), 30),
        FontId::proportional(10.5),
        theme::c().text_secondary,
    );
    resp
}

fn menu_row(ui: &mut Ui, title: &str, subtitle: &str) -> egui::Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(2.0), 12, theme::c().surface_overlay);
    }
    let p = ui.painter().with_clip_rect(rect);
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 10.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(14.0),
        theme::c().text,
    );
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 30.0),
        Align2::LEFT_TOP,
        subtitle,
        FontId::proportional(9.5),
        theme::c().text_secondary,
    );
    resp
}

fn link_button(ui: &mut Ui, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(9.5),
        theme::c().text_secondary,
    );
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 4.0, 18.0), Sense::click());
    ui.painter().galley(
        Pos2::new(rect.left() + 2.0, rect.center().y - galley.size().y * 0.5),
        galley,
        theme::c().text_secondary,
    );
    resp
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn field_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(9.5)
            .color(theme::c().text_secondary)
            .strong(),
    );
    ui.add_space(2.0);
}

/// Mirror the Android IME text and selection into an egui text field.
///
/// The IME is the source of truth for soft-keyboard edits; egui is the source
/// for taps, arrows and hardware keys. Each side wins only when it actually
/// changed since the last frame, so neither can revert the other.
#[allow(clippy::too_many_arguments)]
fn ime_sync(
    ui: &Ui,
    resp: &egui::Response,
    text: &mut String,
    focused: &mut bool,
    last_ime: &mut String,
    last_sel: &mut (usize, usize),
    multiline: bool,
    password: bool,
) {
    if !resp.has_focus() {
        *focused = false;
        return;
    }

    ui.ctx().request_repaint_after(Duration::from_millis(50));

    if !*focused {
        crate::platform::configure_ime(multiline, password);
        let end = text.len();
        crate::platform::set_ime_text(text, end, end);
        *last_ime = text.clone();
        *last_sel = (end, end);
        *focused = true;
        return;
    }

    let egui_sel = egui_selection(ui.ctx(), resp.id, text);
    let Some((ime_text, ime_sel)) = crate::platform::ime_state() else {
        return;
    };

    if ime_text != *last_ime {
        // Soft-keyboard edit or paste.
        *last_ime = ime_text.clone();
        if ime_text != *text {
            *text = ime_text;
        }
        set_egui_selection(ui.ctx(), resp.id, text, ime_sel);
        *last_sel = ime_sel;
        ui.ctx().request_repaint();
        return;
    }

    if let Some(sel) = egui_sel
        && sel != *last_sel
    {
        // Tap, arrow key or hardware edit. Push both text and selection.
        crate::platform::set_ime_text(text, sel.0, sel.1);
        *last_ime = text.clone();
        *last_sel = sel;
    }
}

fn egui_selection(ctx: &egui::Context, id: Id, text: &str) -> Option<(usize, usize)> {
    let state = egui::widgets::text_edit::TextEditState::load(ctx, id)?;
    let range = state.cursor.char_range()?;
    let a = char_to_byte(text, range.primary.index.0);
    let b = char_to_byte(text, range.secondary.index.0);
    Some((a.min(b), a.max(b)))
}

fn set_egui_selection(ctx: &egui::Context, id: Id, text: &str, selection: (usize, usize)) {
    let start = byte_to_char(text, selection.0);
    let end = byte_to_char(text, selection.1);
    let mut state = egui::widgets::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(ctx, id);
}

fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn byte_to_char(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    text[..byte].chars().count()
}

fn hide_keyboard(ctx: &egui::Context) {
    if let Some(id) = ctx.memory(|m| m.focused()) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(false));
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn format_cost(cost: f64) -> String {
    if cost <= 0.0 {
        "$0".to_owned()
    } else if cost < 0.01 {
        format!("${cost:.6}")
    } else {
        format!("${cost:.4}")
    }
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn friendly_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid api key")
    {
        "Ключ не принят (401). Проверь, что это ключ OpenRouter и он активен.".into()
    } else if lower.contains("402") || lower.contains("insufficient") {
        "На ключе недостаточно кредитов (402).".into()
    } else if lower.contains("429") || lower.contains("rate limit") {
        "Слишком много запросов (429). Бесплатные модели часто лимитированы — попробуй позже."
            .into()
    } else if lower.contains("404") {
        "Эндпоинт не найден. Проверь base URL (обычно .../api/v1).".into()
    } else if lower.contains("network") || lower.contains("connect") {
        format!("Сеть недоступна: {raw}")
    } else {
        raw.to_owned()
    }
}

fn relative_time(timestamp: u64) -> String {
    let now = now_secs();
    let delta = now.saturating_sub(timestamp);
    if delta < 60 {
        "сейчас".to_owned()
    } else if delta < 3600 {
        format!("{} мин", delta / 60)
    } else if delta < 86_400 {
        format!("{} ч", delta / 3600)
    } else if delta < 86_400 * 7 {
        format!("{} д", delta / 86_400)
    } else {
        format!("{} нед", delta / (86_400 * 7))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn next_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}
