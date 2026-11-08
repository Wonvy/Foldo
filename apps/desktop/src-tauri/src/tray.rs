//! 托盘（菜单栏）图标：显示自动整理状态，切换模式，立即整理，打开主窗口或悬浮窗，退出。

use crate::commands::{current_config, toggle_float_window, write_config};
use crate::watch::{self, WatchStatus};
use foldo_core::config::AutoMode;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub struct TrayItems {
    status: MenuItem<Wry>,
    modes: [CheckMenuItem<Wry>; 3],
    /// 文字固定的菜单项：(菜单项, 原文)，切换语言时重新翻译
    fixed: Vec<(MenuItem<Wry>, &'static str)>,
    /// “配置方案”子菜单，方案变化时重建
    profiles: Submenu<Wry>,
}

const MODE_LABELS: [&str; 3] = ["不自动整理", "发现文件时提醒我", "自动整理…"];

const MODES: [AutoMode; 3] = [AutoMode::Off, AutoMode::Notify, AutoMode::Run];

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    // 有待确认的文件时可以点，点了打开主窗口并扫描
    use foldo_core::i18n::tr;
    let status = MenuItem::with_id(app, "status", tr("自动整理：关闭"), false, None::<&str>)?;
    let run_now = MenuItem::with_id(app, "run_now", tr("立即整理"), true, None::<&str>)?;
    let m_off = CheckMenuItem::with_id(
        app,
        "mode_off",
        tr(MODE_LABELS[0]),
        true,
        true,
        None::<&str>,
    )?;
    let m_notify = CheckMenuItem::with_id(
        app,
        "mode_notify",
        tr(MODE_LABELS[1]),
        true,
        false,
        None::<&str>,
    )?;
    let m_run = CheckMenuItem::with_id(
        app,
        "mode_run",
        tr(MODE_LABELS[2]),
        true,
        false,
        None::<&str>,
    )?;
    let profiles = Submenu::with_id(app, "profiles", tr("配置方案"), false)?;
    let open = MenuItem::with_id(app, "open", tr("打开 Foldo"), true, None::<&str>)?;
    let float = MenuItem::with_id(app, "float", tr("悬浮窗"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr("退出 Foldo"), true, None::<&str>)?;
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
            &profiles,
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
            "mode_run" => request_auto(app),
            "open" => show_main(app),
            "float" => {
                let _ = toggle_float_window(app);
            }
            "quit" => app.exit(0),
            id => {
                if let Some(i) = id
                    .strip_prefix("profile:")
                    .and_then(|i| i.parse::<usize>().ok())
                {
                    switch_profile_at(app, i);
                }
            }
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
        fixed: vec![
            (run_now, "立即整理"),
            (open, "打开 Foldo"),
            (float, "悬浮窗"),
            (quit, "退出 Foldo"),
        ],
        profiles,
    });
    refresh_profiles(app);
    Ok(())
}

/// 按配置重建“配置方案”子菜单：每个方案一项，当前方案打勾；没有方案时整项不可用。
pub fn refresh_profiles(app: &AppHandle) {
    let Some(t) = app.try_state::<TrayItems>() else {
        return;
    };
    let Ok(file) = current_config() else {
        return;
    };
    if let Ok(items) = t.profiles.items() {
        for item in items {
            let _ = t.profiles.remove(&item);
        }
    }
    for (i, p) in file.profiles.iter().enumerate() {
        let on = file.profile.as_deref() == Some(p.name.as_str());
        if let Ok(item) =
            CheckMenuItem::with_id(app, format!("profile:{i}"), &p.name, true, on, None::<&str>)
        {
            let _ = t.profiles.append(&item);
        }
    }
    let _ = t.profiles.set_enabled(!file.profiles.is_empty());
}

fn switch_profile_at(app: &AppHandle, index: usize) {
    let Ok(file) = current_config() else {
        return;
    };
    if let Some(name) = file.profiles.get(index).map(|p| p.name.clone()) {
        let _ = crate::commands::switch_profile_to(app, &name);
    }
}

/// 切换语言后重新设置菜单文字。
pub fn relabel(app: &AppHandle) {
    let Some(t) = app.try_state::<TrayItems>() else {
        return;
    };
    for (item, zh) in &t.fixed {
        let _ = item.set_text(foldo_core::i18n::tr(zh));
    }
    let _ = t.profiles.set_text(foldo_core::i18n::tr("配置方案"));
    for (item, zh) in t.modes.iter().zip(MODE_LABELS) {
        let _ = item.set_text(foldo_core::i18n::tr(zh));
    }
    let status = app
        .state::<crate::watch::WatchState>()
        .status
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default();
    refresh(app, &status);
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
        "notify" if st.pending > 0 => foldo_core::t!("有 {a0} 个文件可以整理", a0 = st.pending),
        "notify" => foldo_core::t!("自动整理：发现文件时提醒"),
        "run" if st.pending > 0 => foldo_core::t!("自动整理中 · {a0} 个待确认", a0 = st.pending),
        "run" => match &st.last_check {
            Some(t) => foldo_core::t!("自动整理中 · {t} 检查过", t = t),
            None => foldo_core::t!("自动整理中"),
        },
        _ => foldo_core::t!("自动整理：关闭"),
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

/// 从托盘选“自动整理”：不直接开启，打开主窗口让用户先确认（列出监听的文件夹和会立刻整理的文件）。
fn request_auto(app: &AppHandle) {
    let current = current_config()
        .map(|c| c.settings.auto)
        .unwrap_or_default();
    // 菜单上的勾选先恢复成当前模式，确认后才会变
    check_mode(app, current);
    if current != AutoMode::Run {
        show_main(app);
        let _ = app.emit("confirm-auto", ());
    }
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
