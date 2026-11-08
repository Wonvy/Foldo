//! 托盘（菜单栏）图标：显示自动整理状态，切换模式，立即整理，打开主窗口或悬浮窗，退出。

use crate::commands::{current_config, toggle_float_window, write_config};
use crate::watch::{self, WatchStatus};
use foldo_core::config::AutoMode;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub struct TrayItems {
    status: MenuItem<Wry>,
    modes: [CheckMenuItem<Wry>; 3],
}

const MODES: [AutoMode; 3] = [AutoMode::Off, AutoMode::Notify, AutoMode::Run];

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    // 有待确认的文件时可以点，点了打开主窗口并扫描
    let status = MenuItem::with_id(app, "status", "自动整理：关闭", false, None::<&str>)?;
    let run_now = MenuItem::with_id(app, "run_now", "立即整理", true, None::<&str>)?;
    let m_off = CheckMenuItem::with_id(app, "mode_off", "不自动整理", true, true, None::<&str>)?;
    let m_notify = CheckMenuItem::with_id(
        app,
        "mode_notify",
        "发现文件时提醒我",
        true,
        false,
        None::<&str>,
    )?;
    let m_run = CheckMenuItem::with_id(
        app,
        "mode_run",
        "自动整理（回收站操作仍需确认）",
        true,
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "open", "打开 Foldo", true, None::<&str>)?;
    let float = MenuItem::with_id(app, "float", "悬浮窗", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Foldo", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &run_now,
            &PredefinedMenuItem::separator(app)?,
            &m_off,
            &m_notify,
            &m_run,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &float,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("foldo")
        .tooltip("Foldo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "run_now" => run_now_action(app),
            "status" => open_pending(app),
            "mode_off" => set_mode(app, AutoMode::Off),
            "mode_notify" => set_mode(app, AutoMode::Notify),
            "mode_run" => set_mode(app, AutoMode::Run),
            "open" => show_main(app),
            "float" => {
                let _ = toggle_float_window(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                show_main(tray.app_handle());
            }
        });
    // macOS 菜单栏用单色模板图标，系统会按深浅色自动着色；其他平台用彩色应用图标
    if cfg!(target_os = "macos") {
        builder = builder
            .icon(Image::from_bytes(include_bytes!(
                "../icons/tray-template.png"
            ))?)
            .icon_as_template(true);
    } else if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    app.manage(TrayItems {
        status,
        modes: [m_off, m_notify, m_run],
    });
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn check_mode(app: &AppHandle, mode: AutoMode) {
    if let Some(t) = app.try_state::<TrayItems>() {
        for (item, m) in t.modes.iter().zip(MODES) {
            let _ = item.set_checked(m == mode);
        }
    }
}

/// 根据后台状态更新托盘菜单的文字和勾选。
pub fn refresh(app: &AppHandle, st: &WatchStatus) {
    let Some(t) = app.try_state::<TrayItems>() else {
        return;
    };
    let text = match st.mode {
        "notify" if st.pending > 0 => format!("有 {} 个文件可以整理", st.pending),
        "notify" => "自动整理：发现文件时提醒".to_string(),
        "run" if st.pending > 0 => format!("自动整理中 · {} 个待确认", st.pending),
        "run" => match &st.last_check {
            Some(t) => format!("自动整理中 · {t} 检查过"),
            None => "自动整理中".to_string(),
        },
        _ => "自动整理：关闭".to_string(),
    };
    let _ = t.status.set_text(text);
    let _ = t.status.set_enabled(st.pending > 0);
    let mode = match st.mode {
        "notify" => AutoMode::Notify,
        "run" => AutoMode::Run,
        _ => AutoMode::Off,
    };
    check_mode(app, mode);
}

/// 打开主窗口并显示待确认的文件（点托盘状态或系统通知时）。
pub fn open_pending(app: &AppHandle) {
    show_main(app);
    let _ = app.emit("open-pending", ());
}

/// 从托盘切换自动整理模式：写入配置并立即生效。
pub fn set_mode(app: &AppHandle, mode: AutoMode) {
    check_mode(app, mode);
    if let Ok(mut c) = current_config() {
        c.settings.auto = mode;
        if write_config(&c).is_ok() {
            watch::reset_failures(app);
            watch::wake();
            let _ = app.emit("config-changed", ());
        }
    }
}

/// “立即整理”：自动模式下马上检查一次；其他模式打开主窗口并扫描。
fn run_now_action(app: &AppHandle) {
    let auto = current_config()
        .map(|c| c.settings.auto)
        .unwrap_or_default();
    if auto == AutoMode::Run {
        watch::wake();
    } else {
        show_main(app);
        let _ = app.emit("tidy-scan", ());
    }
}
