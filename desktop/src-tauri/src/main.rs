// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io;

use tauri::{Manager, Url, Window, WindowEvent};

const SERVER_URL: &str = "https://rts-0-zvorygin.fly.dev/";

#[tauri::command]
fn cursor_grab(window: Window, grab: bool) -> Result<(), String> {
    window.set_cursor_grab(grab).map_err(|err| err.to_string())
}

#[tauri::command]
fn cursor_visible(window: Window, visible: bool) -> Result<(), String> {
    window
        .set_cursor_visible(visible)
        .map_err(|err| err.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![cursor_grab, cursor_visible])
        .enable_macos_default_menu(false)
        .setup(|app| {
            let server_url = Url::parse(SERVER_URL)?;
            let main_window = app.get_webview_window("main").ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "main webview window is missing")
            })?;

            let cursor_window = main_window.clone();
            main_window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Focused(false)) {
                    let _ = cursor_window.set_cursor_grab(false);
                    let _ = cursor_window.set_cursor_visible(true);
                }
            });

            main_window.navigate(server_url)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bewegungskrieg desktop shell");
}
