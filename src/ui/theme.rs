use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0x4a, 0x6c, 0xf7);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x5a, 0x7c, 0xff);
pub const DANGER: Color32 = Color32::from_rgb(0x8a, 0x2a, 0x2a);
pub const AUTO_GREEN: Color32 = Color32::from_rgb(0x3a, 0x60, 0x48);
pub const PANEL_BG: Color32 = Color32::from_rgb(0x1a, 0x1c, 0x20);
pub const SIDEBAR_BG: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);
pub const SUBTLE_TEXT: Color32 = Color32::from_rgb(0x8a, 0x8f, 0x99);
pub const ERROR_TEXT: Color32 = Color32::from_rgb(0xff, 0xb0, 0xb0);

/// Bundled Noto Sans CJK JP (subset) — covers Japanese plus common Latin/digits.
/// Licensed under SIL OFL 1.1; see assets/fonts/NotoSans-LICENSE.txt.
const FONT_JP: &[u8] = include_bytes!("../../assets/fonts/NotoSansJP-Regular.otf");

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(0x1d, 0x1f, 0x23);
    visuals.window_fill = Color32::from_rgb(0x1d, 0x1f, 0x23);
    visuals.extreme_bg_color = Color32::from_rgb(0x15, 0x17, 0x1a);
    visuals.faint_bg_color = Color32::from_rgb(0x23, 0x26, 0x2d);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2a, 0x2d, 0x33));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x2e, 0x32, 0x3a);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x2e, 0x32, 0x3a);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3b, 0x41, 0x4c));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x3a, 0x3f, 0x48);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x3a, 0x3f, 0x48);
    visuals.widgets.active.bg_fill = Color32::from_rgb(0x4a, 0x4f, 0x58);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0x4a, 0x4f, 0x58);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT_HOVER;

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    let radius = CornerRadius::same(4);
    style.visuals.widgets.noninteractive.corner_radius = radius;
    style.visuals.widgets.inactive.corner_radius = radius;
    style.visuals.widgets.hovered.corner_radius = radius;
    style.visuals.widgets.active.corner_radius = radius;
    ctx.set_style(style);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto_jp".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_JP)),
    );
    // Prepend so JP coverage is preferred where the default font lacks glyphs.
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "noto_jp".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("noto_jp".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Render a button styled as the primary (accent) button.
pub fn primary_button(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    let btn = egui::Button::new(label).fill(ACCENT);
    ui.add(btn)
}

pub fn primary_button_enabled(
    ui: &mut egui::Ui,
    enabled: bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let btn = egui::Button::new(label).fill(ACCENT);
    ui.add_enabled(enabled, btn)
}

pub fn danger_button(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    let btn = egui::Button::new(label).fill(DANGER);
    ui.add(btn)
}
