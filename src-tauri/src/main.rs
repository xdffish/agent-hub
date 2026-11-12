#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod access;
mod app;
mod commands;
mod files;
mod ipc;
mod mcp;
mod model;
mod platform;
mod registry;
mod scan;
mod toggles;
use tauri::Manager;
#[cfg(test)]
mod tests;

fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--export-bindings") {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts");
        ipc::export(&path).expect("failed to export IPC bindings");
        eprintln!(
            "Exported {} typed application commands",
            registry::COMMAND_NAMES.len()
        );
        return;
    }
    let ipc = ipc::builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app::Hub::new())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<app::Hub>().busy() {
                    api.prevent_close();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(ipc.invoke_handler())
        .build(context())
        .expect("Agent Hub failed to start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if app.state::<app::Hub>().busy() {
                    api.prevent_exit();
                }
            }
        });
}
