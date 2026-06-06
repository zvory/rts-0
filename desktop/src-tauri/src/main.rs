// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io;

use tauri::{Manager, Url};

const SERVER_URL: &str = "https://rts-0-zvorygin.fly.dev/";

fn main() {
    tauri::Builder::default()
        .enable_macos_default_menu(false)
        .setup(|app| {
            let server_url = Url::parse(SERVER_URL)?;
            let main_window = app.get_webview_window("main").ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "main webview window is missing")
            })?;

            main_window.navigate(server_url)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bewegungskrieg desktop shell");
}
