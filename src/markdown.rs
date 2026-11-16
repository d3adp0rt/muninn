//! Markdown renderer for chat messages.
//!
//! Deliberately a layout renderer, not HTML: paragraphs, headings, lists,
//! quotes, links, inline code, fenced code blocks, simple tables.

use egui::{
    self, Color32, FontFamily, FontId, Label, Margin, RichText, ScrollArea, Sense, Stroke,
    TextFormat, TextWrapMode, Ui, Vec2,
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::theme;

pub fn render(ui: &mut Ui, text: &str, width: f32, streaming: bool) {
    let mut renderer = Renderer::new(ui, width);
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;

    for event in Parser::new_ext(text, options) {
        renderer.event(event);
    }
    renderer.finish();

    if streaming {
        let time = ui.input(|i| i.time) as f32;
        let alpha = ((time * 2.6).sin() * 0.5 + 0.5) * 0.75 + 0.25;
        ui.add_space(3.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(2.0, 13.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 1, theme::ACCENT.gamma_multiply(alpha));
    }
}

#[derive(Clone, Copy)]
struct InlineStyle {
    size: f32,
    weight: f32,
    italic: bool,
    strike: bool,
    link: bool,
    code: bool,
}

impl Default for InlineStyle {
    fn default() -> Self {
        Self {
            size: 14.5,
            weight: 400.0,
            italic: false,
            strike: false,
            link: false,
            code: false,
        }
    }
}

struct CodeBlock {
    lang: String,
    text: String,
}

#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: String,
    in_cell: bool,
}

struct Renderer<'u> {
    ui: &'u mut Ui,
    width: f32,
    spans: Vec<(String, TextFormat)>,
    style: InlineStyle,
    heading: Option<HeadingLevel>,
    quote: usize,
    lists: Vec<(bool, u64)>,
    item_prefix: Option<String>,
    code: Option<CodeBlock>,
    table: Option<Table>,
}

impl<'u> Renderer<'u> {
    fn new(ui: &'u mut Ui, width: f32) -> Self {
        Self {
            ui,
            width,
            spans: Vec::new(),
            style: InlineStyle::default(),
            heading: None,
            quote: 0,
            lists: Vec::new(),
            item_prefix: None,
            code: None,
            table: None,
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some(code) = self.code.as_mut() {
                    code.text.push_str(&text);
                } else if let Some(table) = self.table.as_mut()
                    && table.in_cell
                {
                    table.cell.push_str(&text);
                } else {
                    self.push(&text);
                }
            }
            Event::Code(code) => {
                let previous = self.style.code;
                self.style.code = true;
                self.push(&code);
                self.style.code = previous;
            }
            Event::SoftBreak => self.push(" "),
            Event::HardBreak => self.push("\n"),
            Event::Rule => {
                self.flush();
                self.ui.add_space(4.0);
                self.ui.separator();
                self.ui.add_space(4.0);
            }
            Event::TaskListMarker(done) => {
                self.push(if done { "[x] " } else { "[ ] " });
            }
            Event::InlineMath(text) | Event::DisplayMath(text) => {
                let previous = self.style.code;
                self.style.code = true;
                self.push(&text);
                self.style.code = previous;
            }
            Event::FootnoteReference(label) => {
                self.push(&format!("[^{label}]"));
            }
            Event::Html(_) | Event::InlineHtml(_) => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.start_block(),
            Tag::Heading { level, .. } => {
                self.start_block();
                self.heading = Some(level);
                self.style.size = heading_size(level);
                self.style.weight = 650.0;
            }
            Tag::BlockQuote(_) => {
                self.quote += 1;
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some(CodeBlock {
                    lang,
                    text: String::new(),
                });
            }
            Tag::List(start) => {
                self.lists.push((start.is_some(), start.unwrap_or(1)));
            }
            Tag::Item => {
                let indent = "  ".repeat(self.lists.len().saturating_sub(1));
                let marker = match self.lists.last_mut() {
                    Some((true, next)) => {
                        let n = *next;
                        *next += 1;
                        format!("{n}. ")
                    }
                    _ => "• ".to_owned(),
                };
                self.item_prefix = Some(format!("{indent}{marker}"));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table::default());
            }
            Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    table.cell.clear();
                    table.in_cell = true;
                }
            }
            Tag::Strong => self.style.weight = 600.0,
            Tag::Emphasis => self.style.italic = true,
            Tag::Strikethrough => self.style.strike = true,
            Tag::Link { .. } => self.style.link = true,
            Tag::Image { .. } => self.style.link = true,
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.flush();
                self.heading = None;
                self.style.size = 14.5;
                self.style.weight = 400.0;
            }
            TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.render_code(&code);
                }
            }
            TagEnd::List(_) => {
                self.lists.pop();
                self.ui.add_space(2.0);
            }
            TagEnd::Item => {
                self.flush();
                self.item_prefix = None;
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.render_table(&table);
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = self.table.as_mut()
                    && !table.row.is_empty()
                {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    table.in_cell = false;
                    let cell = std::mem::take(&mut table.cell);
                    table.row.push(cell.trim().to_owned());
                }
            }
            TagEnd::Strong => {
                self.style.weight = if self.heading.is_some() { 650.0 } else { 400.0 }
            }
            TagEnd::Emphasis => self.style.italic = false,
            TagEnd::Strikethrough => self.style.strike = false,
            TagEnd::Link | TagEnd::Image => self.style.link = false,
            _ => {}
        }
    }

    fn start_block(&mut self) {
        if !self.spans.is_empty() {
            self.flush();
        }

        if let Some(prefix) = self.item_prefix.take() {
            let format = self.format();
            self.spans.push((prefix, format));
        }

        if self.quote > 0 {
            let format = TextFormat {
                color: theme::MUTED,
                ..self.format()
            };
            self.spans.push(("│ ".repeat(self.quote), format));
        }
    }

    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let format = self.format();
        if let Some((last_text, last_format)) = self.spans.last_mut()
            && *last_format == format
        {
            last_text.push_str(text);
            return;
        }
        self.spans.push((text.to_owned(), format));
    }

    fn format(&self) -> TextFormat {
        TextFormat {
            font_id: FontId::new(
                self.style.size,
                if self.style.code {
                    FontFamily::Monospace
                } else {
                    FontFamily::Proportional
                },
            ),
            color: if self.style.link {
                theme::ACCENT
            } else if self.quote > 0 {
                theme::MUTED
            } else {
                theme::TEXT
            },
            background: if self.style.code {
                theme::CARD_HOVER
            } else {
                Color32::TRANSPARENT
            },
            italics: self.style.italic,
            underline: if self.style.link {
                Stroke::new(1.0, theme::ACCENT)
            } else {
                Stroke::NONE
            },
            strikethrough: if self.style.strike {
                Stroke::new(1.0, theme::MUTED)
            } else {
                Stroke::NONE
            },
            coords: egui::epaint::text::VariationCoords::new([(b"wght", self.style.weight)]),
            ..Default::default()
        }
    }

    fn flush(&mut self) {
        if self.spans.is_empty() {
            return;
        }

        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = self.width;
        for (text, format) in self.spans.drain(..) {
            job.append(&text, 0.0, format);
        }

        if self.heading.is_some() {
            self.ui.add_space(8.0);
        }
        self.ui.add(Label::new(job).wrap());
        self.ui
            .add_space(if self.heading.is_some() { 8.0 } else { 6.0 });
    }

    fn render_code(&mut self, code: &CodeBlock) {
        let lang = code.lang.trim();
        let body = code.text.trim_end_matches('\n');

        self.ui.add_space(2.0);
        egui::Frame::new()
            .fill(theme::BG)
            .corner_radius(10)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(Margin::symmetric(10, 8))
            .show(self.ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if lang.is_empty() { "code" } else { lang })
                            .size(9.5)
                            .color(theme::MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if text_button(ui, "copy") {
                            crate::platform::clipboard_set(body);
                        }
                    });
                });
                ui.add_space(2.0);
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.add(
                        Label::new(RichText::new(body).monospace().size(12.0))
                            .wrap_mode(TextWrapMode::Extend),
                    );
                });
            });
        self.ui.add_space(8.0);
    }

    fn render_table(&mut self, table: &Table) {
        for row in &table.rows {
            ui_row(self.ui, row);
        }
        self.ui.add_space(6.0);
    }

    fn finish(&mut self) {
        self.flush();
    }
}

fn ui_row(ui: &mut Ui, row: &[String]) {
    ui.horizontal_wrapped(|ui| {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                ui.add_space(14.0);
            }
            ui.label(RichText::new(cell).size(12.0).color(theme::TEXT));
        }
    });
}

fn text_button(ui: &mut Ui, label: &str) -> bool {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(9.5), theme::MUTED);
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 4.0, 18.0), Sense::click());
    ui.painter().galley(
        egui::Pos2::new(rect.left() + 2.0, rect.center().y - galley.size().y * 0.5),
        galley,
        theme::MUTED,
    );
    resp.clicked()
}

fn heading_size(level: HeadingLevel) -> f32 {
    match level {
        HeadingLevel::H1 => 21.0,
        HeadingLevel::H2 => 19.0,
        HeadingLevel::H3 => 17.0,
        HeadingLevel::H4 => 16.0,
        HeadingLevel::H5 => 15.0,
        HeadingLevel::H6 => 14.5,
    }
}
