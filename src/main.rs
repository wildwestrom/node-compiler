pub mod graph;
mod ui;
use ui::App;

// Just the entry point of this
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_min_inner_size([400.0, 300.0]),
        persistence_path: Some("./app_persist".into()),
        ..Default::default()
    };

    eframe::run_native(
        "Node Compiler",
        native_options,
        Box::new(|cx| Ok(Box::new(App::new(cx)))),
    )
}
