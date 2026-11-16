//! Visual language for Muninn.
//!
//! Dark, quiet, Claude/DeepSeek-inspired: near-black surfaces, soft bubbles,
//! a single violet accent, generous spacing.

use egui::{Color32, CornerRadius, FontFamily, FontId, TextStyle, Vec2, Visuals};

pub const BG: Color32 = Color32::from_rgb(11, 12, 16);
pub const SURFACE: Color32 = Color32::from_rgb(17, 18, 24);
pub const CARD: Color32 = Color32::from_rgb(24, 25, 33);
pub const CARD_HOVER: Color32 = Color32::from_rgb(33, 34, 45);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(58, 42, 120, 255);
pub const TEXT: Color32 = Color32::from_rgb(233, 234, 239);
pub const MUTED: Color32 = Color32::from_rgb(148, 151, 163);
pub const BORDER: Color32 = Color32::from_rgb(38, 39, 49);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();

    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = Color32::from_rgb(8, 9, 12);
    v.faint_bg_color = CARD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgba_premultiplied(70, 52, 150, 255);
    v.widgets.inactive.bg_fill = CARD;
    v.widgets.inactive.weak_bg_fill = CARD;
    v.widgets.inactive.corner_radius = CornerRadius::same(14);
    v.widgets.hovered.bg_fill = CARD_HOVER;
    v.widgets.hovered.weak_bg_fill = CARD_HOVER;
    v.widgets.hovered.corner_radius = CornerRadius::same(14);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.corner_radius = CornerRadius::same(14);

    style.visuals = v;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();

    ctx.set_style_of(egui::Theme::Dark, style);
}
