fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["cursor_grab", "cursor_visible"]),
    ))
    .expect("failed to run Tauri build script");
}
