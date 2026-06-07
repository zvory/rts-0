// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, io};

use tauri::{Manager, Url, WebviewWindow};

const SERVER_URL: &str = "https://bewegungskrieg.net/";
const DESKTOP_URL_ENV: &str = "RTS_DESKTOP_URL";
const OPEN_DEVTOOLS_ENV: &str = "RTS_TAURI_OPEN_DEVTOOLS";

fn desktop_url() -> Result<Url, Box<dyn std::error::Error>> {
    let raw = env::var(DESKTOP_URL_ENV).unwrap_or_else(|_| SERVER_URL.to_string());
    Ok(Url::parse(&raw)?)
}

#[cfg(any(debug_assertions, feature = "desktop-debug-tools"))]
fn maybe_open_devtools(window: &WebviewWindow) {
    if env::var(OPEN_DEVTOOLS_ENV).is_ok_and(|value| value != "0") {
        window.open_devtools();
    }
}

#[cfg(not(any(debug_assertions, feature = "desktop-debug-tools")))]
fn maybe_open_devtools(_window: &WebviewWindow) {}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let server_url = desktop_url()?;
            let main_window = app.get_webview_window("main").ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "main webview window is missing")
            })?;

            main_window.navigate(server_url)?;
            maybe_open_devtools(&main_window);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bewegungskrieg desktop shell");
}
