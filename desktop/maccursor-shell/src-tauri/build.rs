fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "maccursor_start",
            "maccursor_configure",
            "maccursor_stop",
            "maccursor_diagnostics",
        ]),
    ))
    .expect("failed to run Tauri build script")
}
