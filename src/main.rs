#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use discord_message_downloader::App;
use eframe::egui;

#[tokio::main]

async fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_resizable(true)
            .with_inner_size([800.0, 600.0])
            .with_icon(eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon.png")[..]).expect("Failed to load icon")),
        ..Default::default()
    };
    eframe::run_native("Discord Message Downloader", native_options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
