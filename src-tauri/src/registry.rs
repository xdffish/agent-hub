// One command inventory drives both Tauri ACL generation and typed IPC registration.
#[macro_export]
macro_rules! command_registry {
    ($consumer:ident) => {
        $consumer! {
            scan => crate::commands::scan,
            choose_path => crate::access::choose_path,
            add_project => crate::commands::add_project,
            remove_project => crate::commands::remove_project,
            import_skill => crate::commands::import_skill,
            add_skill_to_agents => crate::commands::add_skill_to_agents,
            deploy_skill => crate::commands::deploy_skill,
            remove_resource => crate::commands::remove_resource,
            delete_skill => crate::commands::delete_skill,
            pin_skill => crate::commands::pin_skill,
            check_update => crate::commands::check_update,
            preview_mcp => crate::commands::preview_mcp,
            apply_plan => crate::commands::apply_plan,
            history => crate::commands::history,
            preview_undo => crate::commands::preview_undo,
            export_library => crate::commands::export_library,
            import_library => crate::commands::import_library,
            plugin_guide => crate::commands::plugin_guide,
            reveal_resource => crate::commands::reveal_resource,
            check_mcp => crate::mcp::check_mcp,
            toggle_states => crate::toggles::toggle_states,
            preview_toggle => crate::toggles::preview_toggle,
        }
    };
}
macro_rules! names {
    ($($name:ident => $path:path,)*) => { &[$(stringify!($name)),*] };
}
pub const COMMAND_NAMES: &[&str] = command_registry!(names);
