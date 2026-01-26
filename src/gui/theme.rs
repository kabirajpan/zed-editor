use egui::{Color32, Style, Visuals};

pub fn setup_theme(ctx: &egui::Context) {
    let mut style = Style::default();
    style.visuals = Visuals::dark();

    // Custom colors
    style.visuals.window_fill = Color32::from_rgb(30, 30, 30);
    style.visuals.extreme_bg_color = Color32::from_rgb(20, 20, 20);
    style.visuals.code_bg_color = Color32::from_rgb(25, 25, 25);

    ctx.set_style(style);
}

pub const BACKGROUND: Color32 = Color32::from_rgb(30, 30, 30);
pub const LINE_NUMBER_BG: Color32 = Color32::from_rgb(40, 40, 40);
pub const LINE_NUMBER_FG: Color32 = Color32::from_rgb(100, 100, 100);
pub const TEXT_COLOR: Color32 = Color32::from_rgb(200, 200, 200);
pub const CURSOR_COLOR: Color32 = Color32::from_rgb(255, 255, 255);
// Fixed: Use const-friendly function
pub const SELECTION_COLOR: Color32 = Color32::from_rgba_premultiplied(80, 120, 204, 80);
pub const STATUS_BAR_BG: Color32 = Color32::from_rgb(40, 40, 40);
