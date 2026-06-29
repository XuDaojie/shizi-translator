#[tauri::command]
fn greet(name: &str) -> String {
    format!("你好, {}! 欢迎使用 Shizi 翻译助手", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
