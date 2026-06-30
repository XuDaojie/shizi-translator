use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub fn register_global_shortcuts(app: &tauri::App) -> Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut().register("Alt+T")
}
