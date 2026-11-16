#![cfg(feature = "desktop")]

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([420.0, 880.0])
            .with_title("Muninn"),
        ..Default::default()
    };

    eframe::run_native(
        "Muninn",
        options,
        Box::new(|cc| Ok(Box::new(muninn::MuninnApp::new(cc)))),
    )
}
