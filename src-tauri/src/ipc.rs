use crate::model::Result;

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    macro_rules! collect {
        ($($name:ident => $($path:ident)::+,)*) => {tauri_specta::collect_commands![$($($path)::+),*]};
    }
    tauri_specta::Builder::new()
        .commands(crate::command_registry!(collect))
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
}

pub fn export(path: &std::path::Path) -> Result<()> {
    builder()
        .export(specta_typescript::Typescript::default(), path)
        .map_err(|e| format!("IPC 类型生成失败：{e}"))
}
