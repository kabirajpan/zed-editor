use zed_text_editor::gui::GuiApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Zed Editor - GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "Zed Editor",
        options,
        Box::new(|cc| {
            // Setup custom theme
            zed_text_editor::gui::theme::setup_theme(&cc.egui_ctx);
            Ok(Box::new(GuiApp::new(cc)))
        }),
    )
}
