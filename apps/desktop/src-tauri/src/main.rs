// Windows 发布版不弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod float_window;
mod regex_window;
mod tray;
mod updates;
mod watch;

use tauri::{Manager, WindowEvent};

fn main() {
    let app = tauri::Builder::default()
        // 必须先拦截重复启动，避免创建第二套托盘、文件监听和定时任务。
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // 手动打开时唤起已有窗口；开机自启仍保持静默。
            if !args.iter().skip(1).any(|arg| arg == "--hidden") {
                tray::show_main(app);
            }
        }))
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
        .manage(float_window::ResizeState::default())
        .manage(regex_window::Sessions::default())
        .manage(watch::WatchState::default())
        .setup(|app| {
            commands::load_locale();
            // 启动时清理一次过期的历史记录
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    commands::prune_history(&handle.state::<commands::AppState>())
                });
            }
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
            if matches!(event, WindowEvent::Destroyed) {
                regex_window::forget(window.app_handle(), window.label());
            }
            if window.label() == float_window::LABEL
                && matches!(event, WindowEvent::CloseRequested { .. })
            {
                float_window::remember(window.app_handle());
            }
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                for (label, child) in window.app_handle().webview_windows() {
                    if label.starts_with("regex-") {
                        let _ = child.close();
                    }
                }
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            regex_window::open_regex_lab,
            regex_window::regex_session,
            regex_window::apply_regex_lab,
            regex_window::close_regex_lab,
            regex_window::regex_file_samples,
            commands::get_config,
            commands::save_config,
            commands::add_protect,
            commands::get_yaml,
            commands::save_yaml,
            commands::categories,
            commands::dropped_extensions,
            commands::suggested_folders,
            commands::scan,
            commands::scan_paths,
            commands::scan_progress,
            commands::cancel_scan,
            commands::try_rule,
            commands::retarget,
            commands::execute_plan,
            commands::history,
            commands::stats,
            commands::switch_profile,
            commands::batch_ops,
            commands::undo_preview,
            commands::undo,
            commands::fields,
            commands::inspect,
            commands::test_rule,
            commands::test_regex,
            commands::debug_file,
            commands::toggle_float,
            commands::float_visible,
            commands::float_resize,
            float_window::float_icon_size,
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
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            float_window::remember(app);
        }
        // macOS：点击程序坞图标时重新显示主窗口
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            tray::show_main(app);
        }
        let _ = (app, event);
    });
}
