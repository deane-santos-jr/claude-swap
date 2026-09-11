#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod cswap;
mod desktop_sessions;
mod launchd;
mod model;
mod terminal;
mod timefmt;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Claude Swap")
            .with_inner_size([760.0, 680.0])
            .with_min_inner_size([600.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native("Claude Swap", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
