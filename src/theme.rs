//! Visual language for Muninn.
//!
//! Dark, quiet, Claude/DeepSeek-inspired: near-black surfaces, soft bubbles,
//! a single violet accent, generous spacing.

use egui::{
    Color32, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    InsertFontFamily, TextStyle, Vec2, Visuals,
};

pub const BG: Color32 = Color32::from_rgb(10, 11, 15);
pub const SURFACE: Color32 = Color32::from_rgb(16, 17, 22);
pub const CARD: Color32 = Color32::from_rgb(23, 24, 31);
pub const CARD_HOVER: Color32 = Color32::from_rgb(32, 34, 43);
pub const ACCENT: Color32 = Color32::from_rgb(108, 92, 231);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(43, 38, 78);
pub const TEXT: Color32 = Color32::from_rgb(236, 237, 242);
pub const MUTED: Color32 = Color32::from_rgb(138, 143, 158);
pub const BORDER: Color32 = Color32::from_rgb(37, 39, 49);
pub const DANGER: Color32 = Color32::from_rgb(255, 110, 110);
pub const USER_BUBBLE: Color32 = Color32::from_rgb(45, 42, 70);

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);

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
    style.spacing.button_padding = Vec2::new(10.0, 7.0);
    style.spacing.interact_size = Vec2::new(36.0, 36.0);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style_of(egui::Theme::Dark, style);
}

fn install_fonts(ctx: &egui::Context) {
    // Inter is the UI face; Noto Emoji and egui's icon font are fallbacks.
    ctx.add_font(FontInsert {
        name: "inter".to_owned(),
        data: FontData::from_static(include_bytes!("../assets/fonts/InterVariable.ttf")),
        families: vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    });

    ctx.add_font(FontInsert {
        name: "noto-emoji".to_owned(),
        data: FontData::from_static(include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf")),
        families: vec![
            InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Lowest,
            },
            InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Lowest,
            },
        ],
    });

    ctx.add_font(FontInsert {
        name: "emoji-icon-font".to_owned(),
        data: FontData::from_static(include_bytes!("../assets/fonts/emoji-icon-font.ttf")),
        families: vec![
            InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Lowest,
            },
            InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Lowest,
            },
        ],
    });
}
