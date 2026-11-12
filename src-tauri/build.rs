include!("src/registry.rs");
fn main() {
    println!("cargo:rerun-if-changed=src/registry.rs");
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMAND_NAMES)),
    )
    .expect("failed to build scoped application permissions");
}
