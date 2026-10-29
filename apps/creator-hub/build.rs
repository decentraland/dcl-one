fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["mp_hello", "mp_log"])),
    )
    .expect("tauri-build");
}
