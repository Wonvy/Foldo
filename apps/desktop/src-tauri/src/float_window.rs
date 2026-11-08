//! Restore the floating window on a visible screen, including after display changes.
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize};

pub const LABEL: &str = "float";
pub const SIZE: f64 = 84.0;
const MARGIN: f64 = 16.0;

#[derive(Clone, Copy)]
struct Anchor {
    right: bool,
    bottom: bool,
}

#[derive(Default)]
pub struct ResizeState(Mutex<Option<Anchor>>);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
struct Position {
    x: i32,
    y: i32,
}

impl From<PhysicalPosition<i32>> for Position {
    fn from(p: PhysicalPosition<i32>) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<Position> for PhysicalPosition<i32> {
    fn from(p: Position) -> Self {
        Self::new(p.x, p.y)
    }
}

#[derive(Clone, Copy)]
struct Screen {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

impl From<&tauri::Monitor> for Screen {
    fn from(monitor: &tauri::Monitor) -> Self {
        let area = monitor.work_area();
        Self {
            x: area.position.x,
            y: area.position.y,
            width: area.size.width,
            height: area.size.height,
            scale: monitor.scale_factor(),
        }
    }
}

impl Screen {
    fn contains(self, p: Position, size: PhysicalSize<u32>) -> bool {
        i64::from(p.x) >= i64::from(self.x)
            && i64::from(p.y) >= i64::from(self.y)
            && i64::from(p.x) + i64::from(size.width) <= i64::from(self.x) + i64::from(self.width)
            && i64::from(p.y) + i64::from(size.height) <= i64::from(self.y) + i64::from(self.height)
    }

    fn collapsed_size(self) -> PhysicalSize<u32> {
        tauri::LogicalSize::new(SIZE, SIZE).to_physical(self.scale)
    }

    fn top_right(self) -> Position {
        let size = self.collapsed_size();
        let margin = (MARGIN * self.scale).round() as i32;
        Position {
            x: self.x + (self.width as i32 - size.width as i32 - margin).max(0),
            y: self.y + margin.min((self.height as i32 - size.height as i32).max(0)),
        }
    }
}

fn restore(saved: Option<Position>, screens: &[Screen], primary: Screen) -> Position {
    saved
        .filter(|p| screens.iter().any(|s| s.contains(*p, s.collapsed_size())))
        .unwrap_or_else(|| primary.top_right())
}

fn position_file() -> std::path::PathBuf {
    foldo_core::paths::default_data_dir().join("float-position.json")
}

pub fn show(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Ok(mut anchor) = window.state::<ResizeState>().0.lock() {
        *anchor = None;
    }
    let monitors = window.available_monitors()?;
    if let Some(primary) = window
        .primary_monitor()?
        .or_else(|| monitors.first().cloned())
    {
        let screens: Vec<_> = monitors.iter().map(Screen::from).collect();
        let saved = std::fs::read(position_file())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        window.set_position(PhysicalPosition::from(restore(
            saved,
            &screens,
            Screen::from(&primary),
        )))?;
    }
    window.show()
}

pub fn remember(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL)
        && let Ok(position) = window.outer_position()
        && let Ok(bytes) = serde_json::to_vec(&Position::from(position))
    {
        let path = position_file();
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_ok()
        {
            let _ = std::fs::write(path, bytes);
        }
    }
}

fn resize_position(
    position: Position,
    old: PhysicalSize<u32>,
    new: PhysicalSize<u32>,
    screen: Screen,
    anchor: Anchor,
) -> Position {
    let mut x = i64::from(position.x);
    let mut y = i64::from(position.y);
    // Expand toward the screen center so a window at the right/bottom edge stays visible.
    if anchor.right {
        x += i64::from(old.width) - i64::from(new.width);
    }
    if anchor.bottom {
        y += i64::from(old.height) - i64::from(new.height);
    }
    Position {
        x: x.clamp(
            i64::from(screen.x),
            i64::from(screen.x) + i64::from(screen.width.saturating_sub(new.width)),
        ) as i32,
        y: y.clamp(
            i64::from(screen.y),
            i64::from(screen.y) + i64::from(screen.height.saturating_sub(new.height)),
        ) as i32,
    }
}

pub fn resize(window: &tauri::WebviewWindow, width: f64, height: f64) -> tauri::Result<()> {
    let old = window.outer_size()?;
    let position = window.outer_position()?;
    let scale = window.scale_factor()?;
    let size = tauri::LogicalSize::new(width, height);
    let next = window.current_monitor()?.map(|monitor| {
        let screen = Screen::from(&monitor);
        let state = window.state::<ResizeState>();
        let mut saved_anchor = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let anchor = saved_anchor.unwrap_or(Anchor {
            right: i64::from(position.x) * 2 + i64::from(old.width)
                >= i64::from(screen.x) * 2 + i64::from(screen.width),
            bottom: i64::from(position.y) * 2 + i64::from(old.height)
                >= i64::from(screen.y) * 2 + i64::from(screen.height),
        });
        *saved_anchor = (width > SIZE || height > SIZE).then_some(anchor);
        resize_position(
            position.into(),
            old,
            size.to_physical(scale),
            screen,
            anchor,
        )
    });
    window.set_size(size)?;
    if let Some(next) = next {
        window.set_position(PhysicalPosition::from(next))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary() -> Screen {
        Screen {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
            scale: 1.0,
        }
    }

    #[test]
    fn first_open_and_offscreen_positions_use_top_right_work_area() {
        let screen = primary();
        let expected = Position { x: 1820, y: 56 };
        assert_eq!(restore(None, &[screen], screen), expected);
        for p in [
            Position { x: 5000, y: 50 },
            Position { x: 1900, y: 50 },
            Position { x: 500, y: -10 },
        ] {
            assert_eq!(restore(Some(p), &[screen], screen), expected);
        }
    }

    #[test]
    fn saved_position_survives_restart_and_accepts_negative_monitor_coordinates() {
        let main = primary();
        let left = Screen {
            x: -2560,
            y: 0,
            width: 2560,
            height: 1440,
            scale: 1.5,
        };
        let saved = Position { x: -1800, y: 100 };
        let bytes = serde_json::to_vec(&saved).unwrap();
        let loaded = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restore(Some(loaded), &[main, left], main), saved);
        assert_eq!(restore(Some(loaded), &[main], main), main.top_right());
        assert_eq!(left.top_right(), Position { x: -150, y: 24 });
        assert_eq!(
            restore(Some(Position { x: -100, y: 100 }), &[main, left], main),
            main.top_right()
        );
    }

    #[test]
    fn expanding_at_top_right_stays_visible_and_collapsing_restores_position() {
        let screen = primary();
        let start = screen.top_right();
        let collapsed = screen.collapsed_size();
        let expanded = PhysicalSize::new(420, 500);
        let anchor = Anchor {
            right: true,
            bottom: false,
        };
        let position = resize_position(start, collapsed, expanded, screen, anchor);
        assert!(screen.contains(position, expanded));
        assert_eq!(
            resize_position(position, expanded, collapsed, screen, anchor),
            start
        );
    }

    #[test]
    fn expanding_across_screen_center_does_not_change_the_saved_anchor() {
        let screen = primary();
        let start = Position { x: 960, y: 100 };
        let collapsed = screen.collapsed_size();
        let expanded = PhysicalSize::new(340, 380);
        let anchor = Anchor {
            right: true,
            bottom: false,
        };
        let position = resize_position(start, collapsed, expanded, screen, anchor);
        assert_eq!(
            resize_position(position, expanded, collapsed, screen, anchor),
            start
        );
    }
}
