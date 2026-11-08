// Windows 发布版不弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod tray;
mod watch;

use tauri::{Manager, WindowEvent};

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            // 开机自启时只留在托盘里，不弹出主窗口
            Some(vec!["--hidden"]),
        ))
        .manage(commands::AppState::default())
        .manage(watch::WatchState::default())
        .setup(|app| {
            commands::load_locale();
            tray::build(app.handle())?;
            // 点系统通知：打开主窗口，显示待确认的文件或刚才的整理记录
            {
                use tauri_plugin_notification::NotificationExt;
                let handle = app.handle().clone();
                let _ = app
                    .notification()
                    .on_action(move |_| tray::open_pending(&handle));
            }
            watch::spawn(app.handle().clone());
            if std::env::args().any(|a| a == "--hidden")
                && let Some(w) = app.get_webview_window("main")
            {
                let _ = w.hide();
            }
            Ok(())
        })
        // 关闭主窗口时收到托盘里继续运行；从托盘菜单“退出 Foldo”才真正退出
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
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
            commands::watch_status,
            commands::watch_now,
            commands::autostart_enabled,
            commands::set_autostart,
            commands::check_update,
            commands::set_locale,
            commands::open_releases,
            commands::export_rules,
            commands::read_rules_file,
            commands::install_update,
            commands::reveal,
        ])
        .build(tauri::generate_context!())
        .expect("Foldo 启动失败");

    app.run(|app, event| {
        // macOS：点击程序坞图标时重新显示主窗口
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            tray::show_main(app);
        }
        let _ = (app, event);
    });
}
