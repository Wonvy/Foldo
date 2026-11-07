// Windows 发布版不弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::add_protect,
            commands::get_yaml,
            commands::save_yaml,
            commands::categories,
            commands::suggested_folders,
            commands::scan,
            commands::scan_paths,
            commands::try_rule,
            commands::retarget,
            commands::execute_plan,
            commands::history,
            commands::batch_ops,
            commands::undo_preview,
            commands::undo,
            commands::fields,
            commands::inspect,
            commands::test_rule,
            commands::debug_file,
            commands::toggle_float,
            commands::float_visible,
            commands::float_resize,
            commands::show_main,
            commands::reveal,
        ])
        .run(tauri::generate_context!())
        .expect("Foldo 启动失败");
}
