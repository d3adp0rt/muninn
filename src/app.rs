//! Muninn UI shell.
//!
//! M1 in progress: onboarding, model catalog, streaming chat.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui::{self, Align, CornerRadius, Layout, Margin, RichText, ScrollArea, Vec2};

use crate::api::{
    ChatRequest, Client, ClientConfig, ModelInfo, RequestMessage, Role, StreamEvent, Usage,
};
use crate::theme;

pub const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

pub struct MuninnApp {
    screen: Screen,

    base_url: String,
    api_key: String,
    base_focus: bool,
    key_focus: bool,
    connecting: bool,
    error: Option<String>,

    models: Vec<ModelEntry>,
    model: String,
    model_menu: bool,
    model_query: String,

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

struct ModelEntry {
    id: String,
    name: String,
    context: u64,
    prompt_per_m: f64,
    completion_per_m: f64,
    reasoning: bool,
    tools: bool,
    free: bool,
}

struct Message {
    role: Role,
    text: String,
    reasoning: Option<String>,
    cost: Option<f64>,
    tokens: Option<u64>,
    streaming: bool,
    error: bool,
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
        theme::apply(&cc.egui_ctx);
        let _ = crate::runtime::rt();

        let (tx, rx) = channel();

        let mut app = Self {
            screen: Screen::Connect,
            base_url: OPENROUTER_BASE.to_owned(),
            api_key: String::new(),
            base_focus: false,
            key_focus: false,
            connecting: false,
            error: None,
            models: Vec::new(),
            model: "openrouter/free".to_owned(),
            model_menu: false,
            model_query: String::new(),
            title: "New chat".to_owned(),
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

        if let Some(secrets) = crate::secrets::load() {
            app.base_url = secrets.base_url;
            app.api_key = secrets.api_key;
            app.connect(&cc.egui_ctx);
        }

        app
    }

    // ------------------------------------------------------------------
    // Connection
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
                    name: if m.name.is_empty() {
                        String::new()
                    } else {
                        m.name
                    },
                    context: m.context_length.unwrap_or(0),
                    prompt_per_m: prompt * 1_000_000.0,
                    completion_per_m: completion * 1_000_000.0,
                    reasoning: m.supported_parameters.iter().any(|p| p == "reasoning"),
                    tools: m.supported_parameters.iter().any(|p| p == "tools"),
                    free,
                }
            })
            .collect();

        if self.models.is_empty() {
            self.error = Some("Сервер вернул пустой список моделей.".into());
            return;
        }

        self.model = self
            .models
            .iter()
            .find(|m| m.id == "openrouter/free")
            .or_else(|| self.models.iter().find(|m| m.free))
            .or_else(|| self.models.first())
            .map(|m| m.id.clone())
            .unwrap_or_else(|| "openrouter/free".into());

        self.screen = Screen::Chat;
        let _ = crate::secrets::save(&self.base_url, &self.api_key);
    }

    fn model_entry(&self) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == self.model)
    }

    fn is_openrouter(&self) -> bool {
        self.base_url.contains("openrouter.ai")
    }

    // ------------------------------------------------------------------
    // Streaming
    // ------------------------------------------------------------------

    fn send(&mut self, ctx: &egui::Context) {
        if self.streaming {
            return;
        }

        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }

        let mut history: Vec<RequestMessage> = self
            .messages
            .iter()
            .filter(|m| !m.error && !m.text.is_empty())
            .map(|m| RequestMessage::text(m.role, m.text.clone()))
            .collect();

        if self.title == "New chat" {
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

        let mut request = ChatRequest::new(self.model.clone(), std::mem::take(&mut history));
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
        crate::platform::set_ime_text("", 0);
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
    }

    fn fail_last(&mut self, error: String) {
        self.streaming = false;
        self.stream_task = None;
        if let Some(msg) = self.messages.last_mut() {
            msg.streaming = false;
            msg.error = true;
            if msg.text.is_empty() {
                msg.text = format!("⚠ {error}");
            } else {
                msg.text.push_str(&format!("\n\n⚠ {error}"));
            }
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                UiEvent::Models(models) => {
                    self.connecting = false;
                    self.apply_models(models);
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
    // UI
    // ------------------------------------------------------------------

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Muninn").size(19.0).strong());
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {} tok",
                        truncate(&self.title, 28),
                        format_cost(self.chat_cost),
                        format_tokens(self.chat_tokens)
                    ))
                    .size(10.5)
                    .color(theme::MUTED),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let short = self.model.rsplit('/').next().unwrap_or(&self.model);
                let chip = egui::Button::new(
                    RichText::new(format!("{short}  v"))
                        .size(12.5)
                        .color(theme::TEXT),
                )
                .fill(theme::CARD)
                .corner_radius(16);

                if ui.add(chip).on_hover_text(&self.model).clicked() {
                    self.model_menu = true;
                }

                if ui
                    .add(egui::Button::new(RichText::new("+").size(20.0)).frame(false))
                    .on_hover_text("Новый чат")
                    .clicked()
                {
                    self.stop();
                    self.messages.clear();
                    self.title = "New chat".into();
                    self.chat_cost = 0.0;
                    self.chat_tokens = 0;
                }

                if ui
                    .add(egui::Button::new(RichText::new("key").size(11.0)).frame(false))
                    .on_hover_text("Сменить ключ")
                    .clicked()
                {
                    self.stop();
                    crate::secrets::clear();
                    self.screen = Screen::Connect;
                    self.api_key.clear();
                    self.models.clear();
                    self.messages.clear();
                    self.title = "New chat".into();
                    self.chat_cost = 0.0;
                    self.chat_tokens = 0;
                    self.error = None;
                }
            });
        });
    }

    fn bubble(&self, ui: &mut egui::Ui, msg: &Message) {
        let is_user = msg.role == Role::User;
        let viewport = ui.clip_rect().width();
        let max_w = (viewport * if is_user { 0.84 } else { 0.88 }).max(140.0);
        let text_w = (max_w - 28.0).max(100.0);

        if is_user {
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                egui::Frame::new()
                    .fill(theme::ACCENT_SOFT)
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(text_w);
                        ui.add(egui::Label::new(RichText::new(&msg.text).size(15.5)).wrap());
                    });
            });
        } else {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                egui::Frame::new()
                    .fill(if msg.error {
                        theme::SURFACE
                    } else {
                        theme::CARD
                    })
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(text_w);

                        if let Some(reasoning) = &msg.reasoning {
                            let label = if msg.streaming && msg.text.is_empty() {
                                "Thinking…".to_owned()
                            } else {
                                format!("Thinking · {} chars", reasoning.chars().count())
                            };
                            egui::CollapsingHeader::new(
                                RichText::new(label).size(12.0).color(theme::MUTED),
                            )
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(reasoning)
                                            .monospace()
                                            .size(12.5)
                                            .color(theme::MUTED),
                                    )
                                    .wrap(),
                                );
                            });
                            ui.add_space(2.0);
                        }

                        if !msg.text.is_empty() {
                            ui.add(
                                egui::Label::new(RichText::new(&msg.text).size(15.5).color(
                                    if msg.error {
                                        theme::DANGER
                                    } else {
                                        theme::TEXT
                                    },
                                ))
                                .wrap(),
                            );
                        } else if msg.streaming {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("ждём ответ…").size(12.0).color(theme::MUTED),
                                );
                            });
                        }
                    });

                if let (Some(cost), Some(tokens)) = (msg.cost, msg.tokens) {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("copy").size(10.5).color(theme::MUTED),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            crate::platform::clipboard_set(&msg.text);
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} · {} tok",
                                format_cost(cost),
                                format_tokens(tokens)
                            ))
                            .size(10.5)
                            .color(theme::MUTED),
                        );
                    });
                }
            });
        }

        ui.add_space(10.0);
    }

    fn composer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut submit = false;

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(if self.deep_think {
                    "deep think: on"
                } else {
                    "deep think: off"
                })
                .size(11.0)
                .color(if self.deep_think {
                    theme::ACCENT
                } else {
                    theme::MUTED
                }),
            );
            ui.label(RichText::new("·").size(11.0).color(theme::MUTED));
            ui.label(
                RichText::new(if self.web { "web: on" } else { "web: off" })
                    .size(11.0)
                    .color(if self.web {
                        theme::ACCENT
                    } else {
                        theme::MUTED
                    }),
            );
        });

        ui.add_space(6.0);

        egui::Frame::new()
            .fill(theme::CARD)
            .corner_radius(22)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                let edit = egui::TextEdit::multiline(&mut self.draft)
                    .hint_text("Message Muninn…")
                    .desired_rows(1)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::new())
                    .margin(Margin::ZERO);

                let resp = ui.add(edit);

                if resp.has_focus() {
                    ctx.request_repaint_after(Duration::from_millis(50));
                    if !self.ime_focused {
                        crate::platform::set_ime_text(&self.draft, self.draft.len());
                        self.ime_focused = true;
                    } else if let Some(text) = crate::platform::ime_text()
                        && text != self.draft
                    {
                        self.draft = text;
                    }
                } else if self.ime_focused {
                    self.ime_focused = false;
                }

                let enter = resp.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                if enter {
                    submit = true;
                }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("+").size(20.0)).frame(false))
                        .clicked()
                    {
                        // Attachments land here in M2.
                    }
                    if ui
                        .add(egui::Button::new(RichText::new("Mic").size(11.5)).frame(false))
                        .clicked()
                    {
                        // STT lands here in M2.
                    }

                    if ui
                        .selectable_label(self.deep_think, RichText::new("Deep think").size(12.0))
                        .clicked()
                    {
                        self.deep_think = !self.deep_think;
                    }
                    if ui
                        .selectable_label(self.web, RichText::new("Web").size(12.0))
                        .clicked()
                    {
                        self.web = !self.web;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.streaming {
                            let stop = egui::Button::new(
                                RichText::new("Stop").size(12.0).color(theme::TEXT),
                            )
                            .fill(theme::CARD_HOVER)
                            .corner_radius(CornerRadius::same(18))
                            .min_size(Vec2::new(64.0, 36.0));
                            if ui.add(stop).clicked() {
                                self.stop();
                            }
                        } else {
                            let send = egui::Button::new(RichText::new("↑").size(17.0).strong())
                                .fill(if self.draft.trim().is_empty() {
                                    theme::CARD_HOVER
                                } else {
                                    theme::ACCENT
                                })
                                .corner_radius(CornerRadius::same(18))
                                .min_size(Vec2::new(36.0, 36.0));

                            if ui.add(send).clicked() {
                                submit = true;
                            }
                        }
                    });
                });
            });

        if submit && !self.streaming {
            self.send(ctx);
        }
    }

    fn model_picker(&mut self, ctx: &egui::Context) {
        if !self.model_menu {
            return;
        }

        let mut open = self.model_menu;
        let mut chosen: Option<String> = None;

        egui::Window::new("Models")
            .open(&mut open)
            .default_width(360.0)
            .max_height(520.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.model_query)
                        .hint_text("Search models…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);

                let query = self.model_query.to_lowercase();
                let indices: Vec<usize> = self
                    .models
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| {
                        query.is_empty()
                            || m.id.to_lowercase().contains(&query)
                            || m.name.to_lowercase().contains(&query)
                    })
                    .map(|(i, _)| i)
                    .collect();

                ScrollArea::vertical()
                    .max_height(420.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for idx in indices {
                            let m = &self.models[idx];
                            let selected = m.id == self.model;
                            let label = format!(
                                "{}  {}{}{}",
                                if m.name.is_empty() { &m.id } else { &m.name },
                                if m.free { "· free " } else { "" },
                                if m.reasoning { "· think " } else { "" },
                                if m.tools { "· tools" } else { "" },
                            );

                            let resp = ui.selectable_label(selected, label);
                            if resp.clicked() {
                                chosen = Some(m.id.clone());
                            }
                            resp.on_hover_text(format!(
                                "{}\ncontext: {}\nprice: ${:.3}/M in · ${:.3}/M out",
                                m.id,
                                format_tokens(m.context),
                                m.prompt_per_m,
                                m.completion_per_m
                            ));
                        }
                    });
            });

        if let Some(id) = chosen {
            self.model = id;
            open = false;
        }
        self.model_menu = open;
    }

    fn onboarding(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(48.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("MUNINN").size(34.0).strong());
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Приватный клиент OpenRouter")
                            .size(14.0)
                            .color(theme::MUTED),
                    );
                });

                ui.add_space(36.0);
                ui.horizontal(|ui| {
                    let side = (ui.available_width() * 0.08).max(8.0);
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - side).max(200.0));

                        field_label(ui, "BASE URL");
                        let base = egui::TextEdit::singleline(&mut self.base_url)
                            .desired_width(f32::INFINITY)
                            .hint_text(OPENROUTER_BASE)
                            .margin(Margin::symmetric(12, 10));
                        let base_resp = ui.add(base);
                        ime_into(
                            ui,
                            &base_resp,
                            &mut self.base_url,
                            &mut self.base_focus,
                            false,
                        );

                        ui.add_space(12.0);
                        field_label(ui, "API KEY");

                        let key = egui::TextEdit::singleline(&mut self.api_key)
                            .desired_width(f32::INFINITY)
                            .hint_text("sk-or-v1-…")
                            .password(true)
                            .margin(Margin::symmetric(12, 10));
                        let key_resp = ui.add(key);
                        ime_into(ui, &key_resp, &mut self.api_key, &mut self.key_focus, true);

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Вставить из буфера").clicked()
                                && let Some(text) = crate::platform::clipboard_get()
                            {
                                self.api_key = text.trim().to_owned();
                                if self.key_focus {
                                    crate::platform::set_ime_text(
                                        &self.api_key,
                                        self.api_key.len(),
                                    );
                                }
                            }
                            if ui.button("Вставить base URL").clicked()
                                && let Some(text) = crate::platform::clipboard_get()
                            {
                                self.base_url = text.trim().to_owned();
                                if self.base_focus {
                                    crate::platform::set_ime_text(
                                        &self.base_url,
                                        self.base_url.len(),
                                    );
                                }
                            }
                        });

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Забыть сохранённый ключ")
                                        .size(10.5)
                                        .color(theme::MUTED),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            crate::secrets::clear();
                            self.api_key.clear();
                            self.error = None;
                        }

                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(
                                "Ключ остаётся на устройстве. Без аккаунта и телеметрии.",
                            )
                            .size(10.5)
                            .color(theme::MUTED),
                        );
                        ui.label(
                            RichText::new(
                                "Бесплатные модели имеют лимиты. Ключ: openrouter.ai/keys",
                            )
                            .size(10.5)
                            .color(theme::MUTED),
                        );

                        ui.add_space(18.0);

                        let can_connect = !self.connecting && !self.api_key.trim().is_empty();
                        if ui
                            .add_enabled(
                                can_connect,
                                egui::Button::new(
                                    RichText::new(if self.connecting {
                                        "Подключаюсь…"
                                    } else {
                                        "Подключиться"
                                    })
                                    .size(15.0),
                                )
                                .fill(if can_connect {
                                    theme::ACCENT
                                } else {
                                    theme::CARD
                                })
                                .corner_radius(14)
                                .min_size(Vec2::new(ui.available_width(), 46.0)),
                            )
                            .clicked()
                        {
                            self.connect(ctx);
                        }

                        if self.connecting {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("проверяю ключ и загружаю модели")
                                        .size(11.5)
                                        .color(theme::MUTED),
                                );
                            });
                        }

                        if let Some(error) = &self.error {
                            ui.add_space(10.0);
                            ui.label(RichText::new(error).size(12.0).color(theme::DANGER));
                        }
                    });
                });
                ui.add_space(48.0);
            });
    }
}

impl eframe::App for MuninnApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events(ui.ctx());

        let insets = crate::platform::system_insets(ui);

        if insets.top > 0.5 {
            egui::Panel::top("muninn_safe_top")
                .exact_size(insets.top)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::SURFACE))
                .show(ui, |_| {});
        }

        if insets.bottom > 0.5 {
            egui::Panel::bottom("muninn_safe_bottom")
                .exact_size(insets.bottom)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::BG))
                .show(ui, |_| {});
        }

        let side = (12.0 + insets.left.max(insets.right)).round() as i8;
        let ctx = ui.ctx().clone();

        match self.screen {
            Screen::Connect => {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(theme::BG)
                            .inner_margin(Margin::symmetric(side, 0)),
                    )
                    .show(ui, |ui| self.onboarding(ui, &ctx));
            }
            Screen::Chat => {
                egui::Panel::top("muninn_header")
                    .exact_size(58.0)
                    .frame(
                        egui::Frame::new()
                            .fill(theme::SURFACE)
                            .inner_margin(Margin::symmetric(14, 8)),
                    )
                    .show(ui, |ui| self.header(ui));

                egui::Panel::bottom("muninn_composer")
                    .frame(
                        egui::Frame::new()
                            .fill(theme::BG)
                            .inner_margin(Margin::symmetric(10, 8)),
                    )
                    .show(ui, |ui| self.composer(ui, &ctx));

                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(theme::BG)
                            .inner_margin(Margin::symmetric(side, 0)),
                    )
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.add_space(12.0);

                                if self.messages.is_empty() {
                                    ui.add_space(ui.available_height() * 0.12);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new("MUNINN")
                                                .size(30.0)
                                                .strong()
                                                .color(theme::TEXT),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new("Память. Ветки. Агент.")
                                                .size(13.0)
                                                .color(theme::MUTED),
                                        );
                                    });
                                }

                                for msg in &self.messages {
                                    self.bubble(ui, msg);
                                }

                                ui.add_space(6.0);
                            });
                    });

                self.model_picker(&ctx);
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::BG;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).color(theme::MUTED).strong());
    ui.add_space(2.0);
}

/// Mirror the Android IME text state into a text field.
fn ime_into(
    ui: &egui::Ui,
    resp: &egui::Response,
    text: &mut String,
    focused: &mut bool,
    secret: bool,
) {
    if resp.has_focus() {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
        if !*focused {
            crate::platform::set_ime_text(text, text.len());
            *focused = true;
        } else if let Some(ime) = crate::platform::ime_text()
            && ime != *text
        {
            *text = ime;
        }
    } else if *focused {
        *focused = false;
    }

    let _ = secret;
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
