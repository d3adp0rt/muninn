//! The first Muninn screen: a modern chat shell.
//!
//! This is intentionally a thin shell for M0 — it exists to validate the
//! Android stack (GameActivity, IME, wgpu, edge-to-edge insets) before the
//! real domain model lands.

use eframe::egui::{
    self, Align, CornerRadius, Layout, Margin, RichText, ScrollArea, Vec2,
};

use crate::theme;

pub struct MuninnApp {
    draft: String,
    messages: Vec<Message>,
    model: String,
    models: Vec<&'static str>,
    deep_think: bool,
    web: bool,
    cost: f64,
}

struct Message {
    role: Role,
    text: String,
    reasoning: Option<String>,
    cost: Option<f64>,
    tokens: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    User,
    Assistant,
}

impl MuninnApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let messages = vec![
            Message {
                role: Role::Assistant,
                text: "Привет. Я Muninn — локальный клиент OpenRouter.\n\nКлюч хранится только на этом устройстве. История, ветки и агентский workspace — тоже.".into(),
                reasoning: Some(
                    "Пользователь только что открыл приложение. Нужно коротко представиться и не мешать."
                        .into(),
                ),
                cost: Some(0.000_014),
                tokens: Some(64),
            },
            Message {
                role: Role::User,
                text: "Проверим ввод: кириллица, emoji 🐦‍⬛ и курсор в середине строки.".into(),
                reasoning: None,
                cost: None,
                tokens: None,
            },
            Message {
                role: Role::Assistant,
                text: "Ввод работает. Это сообщение — проверка markdown, переносов и пузырей.\n\n`inline code`, **bold**, and a quiet line of secondary text.".into(),
                reasoning: None,
                cost: Some(0.000_031),
                tokens: Some(96),
            },
        ];

        Self {
            draft: String::new(),
            messages,
            model: "openrouter/free".into(),
            models: vec![
                "openrouter/free",
                "openrouter/auto",
                "anthropic/claude-sonnet-4.5",
                "openai/gpt-5.2",
            ],
            deep_think: true,
            web: false,
            cost: 0.000_045,
        }
    }

    fn send(&mut self) {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.draft.clear();
        self.messages.push(Message {
            role: Role::User,
            text,
            reasoning: None,
            cost: None,
            tokens: None,
        });
        self.messages.push(Message {
            role: Role::Assistant,
            text: "M0 shell: сеть ещё не подключена. Следующий шаг — SSE-стрим OpenRouter."
                .into(),
            reasoning: self
                .deep_think
                .then(|| "Запрос получен, но транспорт ещё не реализован.".into()),
            cost: Some(0.0),
            tokens: Some(12),
        });
        self.cost += 0.0;
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Muninn").size(19.0).strong());
            ui.label(RichText::new("M0").size(11.0).color(theme::MUTED));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let chip = egui::Button::new(
                    RichText::new(format!("{}  ▾", self.model))
                        .size(12.5)
                        .color(theme::TEXT),
                )
                .fill(theme::CARD)
                .corner_radius(16);

                if ui.add(chip).clicked() {
                    let idx = self
                        .models
                        .iter()
                        .position(|m| *m == self.model)
                        .unwrap_or(0);
                    self.model = self.models[(idx + 1) % self.models.len()].to_owned();
                }

                ui.label(
                    RichText::new(format!("${:.6}", self.cost))
                        .size(11.5)
                        .color(theme::MUTED),
                );
            });
        });
    }

    fn bubble(&self, ui: &mut egui::Ui, msg: &Message) {
        let is_user = msg.role == Role::User;

        if is_user {
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                egui::Frame::new()
                    .fill(theme::ACCENT_SOFT)
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width((ui.available_width() * 0.82).max(120.0));
                        ui.label(RichText::new(&msg.text).size(15.5).color(theme::TEXT));
                    });
            });
        } else {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                egui::Frame::new()
                    .fill(theme::CARD)
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width((ui.available_width() * 0.86).max(120.0));

                        if let Some(reasoning) = &msg.reasoning {
                            let tokens = msg.tokens.unwrap_or(0);
                            egui::CollapsingHeader::new(
                                RichText::new(format!("Thinking · {tokens} tok"))
                                    .size(12.0)
                                    .color(theme::MUTED),
                            )
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(reasoning)
                                        .monospace()
                                        .size(12.5)
                                        .color(theme::MUTED),
                                );
                            });
                            ui.add_space(2.0);
                        }

                        ui.label(RichText::new(&msg.text).size(15.5));
                    });

                if let (Some(cost), Some(tokens)) = (msg.cost, msg.tokens) {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("${cost:.6} · {tokens} tok"))
                            .size(10.5)
                            .color(theme::MUTED),
                    );
                }
            });
        }

        ui.add_space(10.0);
    }

    fn empty_state(&self, ui: &mut egui::Ui) {
        ui.add_space(ui.available_height() * 0.18);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("MUNINN").size(34.0).strong().color(theme::TEXT));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Память. Ветки. Агент.")
                    .size(14.0)
                    .color(theme::MUTED),
            );
        });
    }

    fn composer(&mut self, ui: &mut egui::Ui) {
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
                    .color(if self.web { theme::ACCENT } else { theme::MUTED }),
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

                let enter = resp.has_focus()
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                    });
                if enter {
                    submit = true;
                }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("＋").size(17.0)).frame(false))
                        .clicked()
                    {
                        // Attachments land here in M2.
                    }
                    if ui
                        .add(egui::Button::new(RichText::new("🎙").size(14.0)).frame(false))
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

                        if !self.draft.trim().is_empty() {
                            ui.label(
                                RichText::new(format!("{} chars", self.draft.chars().count()))
                                    .size(10.5)
                                    .color(theme::MUTED),
                            );
                        }
                    });
                });
            });

        if submit {
            self.send();
        }
    }
}

impl eframe::App for MuninnApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("muninn_header")
            .exact_size(56.0)
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
            .show(ui, |ui| self.composer(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new("ЛОКАЛЬНАЯ ИСТОРИЯ · ШИФРОВАНИЕ ВКЛ")
                                    .size(9.5)
                                    .color(theme::MUTED),
                            );
                        });
                        ui.add_space(14.0);

                        for msg in &self.messages {
                            self.bubble(ui, msg);
                        }

                        if self.messages.is_empty() {
                            self.empty_state(ui);
                        }

                        ui.add_space(6.0);
                    });
            });
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
