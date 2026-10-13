//! Desktop window for pagecrop. All image work lives in `pagecrop-core`.

mod app;
mod editor;
mod loader;
mod panels;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let folder = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("pagecrop")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "pagecrop",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app::App::new(cc.egui_ctx.clone(), folder)?))
        }),
    )
}
