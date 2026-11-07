// Windows 发布版不弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_rules,
            commands::create_example_rules,
            commands::validate_rules,
            commands::save_rules,
            commands::make_plan,
            commands::execute_plan,
            commands::history,
            commands::batch_ops,
            commands::undo,
            commands::reveal,
        ])
        .run(tauri::generate_context!())
        .expect("Foldo 启动失败");
}
