//! Design tokens and theme application.
//!
//! One palette type, three modes, no magic colors in the UI. Spacing follows
//! the 4/8 pt scale; touch targets are at least 44 dp.

use std::cell::Cell;
use std::sync::OnceLock;

use egui::{
    Color32, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    InsertFontFamily, Stroke, TextStyle, Theme, ThemePreference, Vec2, Visuals,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Dark,
    Amoled,
    Light,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Тёмная",
            Self::Amoled => "AMOLED",
            Self::Light => "Светлая",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_raised: Color32,
    pub surface_overlay: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub user_bubble: Color32,
    pub on_user: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub scrim: Color32,
}

const DARK: Palette = Palette {
    bg: Color32::from_rgb(10, 11, 15),
    surface: Color32::from_rgb(16, 17, 22),
    surface_raised: Color32::from_rgb(23, 24, 31),
    surface_overlay: Color32::from_rgb(32, 34, 43),
    border: Color32::from_rgb(38, 40, 51),
    border_strong: Color32::from_rgb(55, 58, 72),
    text: Color32::from_rgb(236, 237, 242),
    text_secondary: Color32::from_rgb(160, 165, 180),
    text_tertiary: Color32::from_rgb(111, 116, 132),
    accent: Color32::from_rgb(124, 92, 255),
    accent_hover: Color32::from_rgb(141, 112, 255),
    accent_soft: Color32::from_rgb(43, 38, 78),
    on_accent: Color32::WHITE,
    user_bubble: Color32::from_rgb(46, 42, 74),
    on_user: Color32::from_rgb(240, 238, 255),
    danger: Color32::from_rgb(255, 107, 107),
    success: Color32::from_rgb(74, 222, 128),
    warning: Color32::from_rgb(251, 191, 36),
    scrim: Color32::from_rgba_premultiplied(0, 0, 0, 160),
};

const AMOLED: Palette = Palette {
    bg: Color32::BLACK,
    surface: Color32::from_rgb(8, 8, 10),
    surface_raised: Color32::from_rgb(15, 15, 18),
    surface_overlay: Color32::from_rgb(24, 24, 29),
    border: Color32::from_rgb(31, 31, 38),
    border_strong: Color32::from_rgb(51, 51, 61),
    text: Color32::from_rgb(245, 245, 247),
    text_secondary: Color32::from_rgb(158, 158, 170),
    text_tertiary: Color32::from_rgb(108, 108, 122),
    accent: Color32::from_rgb(135, 105, 255),
    accent_hover: Color32::from_rgb(154, 129, 255),
    accent_soft: Color32::from_rgb(40, 34, 76),
    on_accent: Color32::WHITE,
    user_bubble: Color32::from_rgb(43, 39, 72),
    on_user: Color32::from_rgb(242, 240, 255),
    danger: Color32::from_rgb(255, 107, 107),
    success: Color32::from_rgb(74, 222, 128),
    warning: Color32::from_rgb(251, 191, 36),
    scrim: Color32::from_rgba_premultiplied(0, 0, 0, 190),
};

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(247, 247, 250),
    surface: Color32::from_rgb(255, 255, 255),
    surface_raised: Color32::from_rgb(255, 255, 255),
    surface_overlay: Color32::from_rgb(240, 240, 246),
    border: Color32::from_rgb(226, 226, 234),
    border_strong: Color32::from_rgb(203, 204, 215),
    text: Color32::from_rgb(23, 24, 28),
    text_secondary: Color32::from_rgb(95, 100, 114),
    text_tertiary: Color32::from_rgb(139, 144, 160),
    accent: Color32::from_rgb(91, 75, 214),
    accent_hover: Color32::from_rgb(108, 92, 231),
    accent_soft: Color32::from_rgb(233, 228, 255),
    on_accent: Color32::WHITE,
    user_bubble: Color32::from_rgb(233, 228, 255),
    on_user: Color32::from_rgb(38, 33, 75),
    danger: Color32::from_rgb(217, 45, 32),
    success: Color32::from_rgb(22, 163, 74),
    warning: Color32::from_rgb(180, 83, 9),
    scrim: Color32::from_rgba_premultiplied(0, 0, 0, 96),
};

thread_local! {
    static PALETTE: Cell<Palette> = const { Cell::new(DARK) };
    static MODE: Cell<Mode> = const { Cell::new(Mode::Dark) };
}

/// Current palette. Cheap: one thread-local copy.
pub fn c() -> Palette {
    PALETTE.with(Cell::get)
}

pub fn palette_of(mode: Mode) -> Palette {
    match mode {
        Mode::Dark => DARK,
        Mode::Amoled => AMOLED,
        Mode::Light => LIGHT,
    }
}

/// Install fonts once and apply a mode to the context.
pub fn init(ctx: &egui::Context, mode: Mode) {
    static FONTS: OnceLock<()> = OnceLock::new();
    if FONTS.set(()).is_ok() {
        install_fonts(ctx);
    }
    set_mode(ctx, mode);
}

pub fn set_mode(ctx: &egui::Context, mode: Mode) {
    let palette = palette_of(mode);

    PALETTE.with(|p| p.set(palette));
    MODE.with(|m| m.set(mode));

    let mut style = (*ctx.style_of(Theme::Dark)).clone();
    let mut visuals = match mode {
        Mode::Light => Visuals::light(),
        _ => Visuals::dark(),
    };

    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface_raised;
    visuals.override_text_color = Some(palette.text);
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.danger;
    visuals.selection.bg_fill = palette.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.window_corner_radius = CornerRadius::same(20);
    visuals.menu_corner_radius = CornerRadius::same(14);

    visuals.widgets.inactive.bg_fill = palette.surface_raised;
    visuals.widgets.inactive.weak_bg_fill = palette.surface_raised;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(14);

    visuals.widgets.hovered.bg_fill = palette.surface_overlay;
    visuals.widgets.hovered.weak_bg_fill = palette.surface_overlay;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(14);

    visuals.widgets.active.bg_fill = palette.accent_soft;
    visuals.widgets.active.weak_bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.corner_radius = CornerRadius::same(14);

    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_secondary);

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.spacing.interact_size = Vec2::new(44.0, 44.0);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.indent = 18.0;
    style.animation_time = 0.18;

    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_theme(if mode == Mode::Light {
        ThemePreference::Light
    } else {
        ThemePreference::Dark
    });
    ctx.set_style_of(Theme::Dark, style.clone());
    ctx.set_style_of(Theme::Light, style);

    crate::platform::set_light_system_bars(mode == Mode::Light);
}

fn install_fonts(ctx: &egui::Context) {
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
