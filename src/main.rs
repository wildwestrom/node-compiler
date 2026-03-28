mod ui;
use ui::App;

// Just the entry point of this
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Node Compiler",
        native_options,
        Box::new(|cx| Ok(Box::new(App::new(cx)))),
    )
}
