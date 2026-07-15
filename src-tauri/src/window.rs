use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalRect, PhysicalSize, Size,
    WebviewUrl, WebviewWindow,
};

const DEFAULT_WINDOW_WIDTH: f64 = 360.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 200.0;
const DEFAULT_WIDGET_WIDTH: f64 = 128.0;
const DEFAULT_WIDGET_HEIGHT: f64 = 44.0;

fn clamp_position_to_work_area(
    position: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    work_area: &PhysicalRect<i32, u32>,
) -> PhysicalPosition<i32> {
    let work_left = i64::from(work_area.position.x);
    let work_top = i64::from(work_area.position.y);
    let work_right = work_left + i64::from(work_area.size.width);
    let work_bottom = work_top + i64::from(work_area.size.height);
    let window_width = i64::from(window_size.width);
    let window_height = i64::from(window_size.height);

    let max_x = (work_right - window_width).max(work_left);
    let max_y = (work_bottom - window_height).max(work_top);
    let x = i64::from(position.x).clamp(work_left, max_x) as i32;
    let y = i64::from(position.y).clamp(work_top, max_y) as i32;

    PhysicalPosition::new(x, y)
}

fn build_main_window(app: &AppHandle, widget_mode: bool) -> Result<WebviewWindow, String> {
    let (width, height) = main_window_dimensions(widget_mode);

    tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("FamVoice")
        .inner_size(width, height)
        .resizable(false)
        .maximizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .center()
        .build()
        .map_err(|e| e.to_string())
}

fn keep_window_on_screen(window: &WebviewWindow) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| {
            window
                .available_monitors()
                .ok()
                .and_then(|monitors| monitors.into_iter().next())
        });

    let Some(monitor) = monitor else {
        return Ok(());
    };

    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let safe_position = clamp_position_to_work_area(position, size, monitor.work_area());

    if safe_position != position {
        window
            .set_position(safe_position)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(windows)]
fn show_without_activation(window: &WebviewWindow) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindowAsync, SW_SHOWNOACTIVATE};

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    unsafe {
        ShowWindowAsync(hwnd.0 as _, SW_SHOWNOACTIVATE);
    }
    Ok(())
}

#[cfg(not(windows))]
fn show_without_activation(window: &WebviewWindow) -> Result<(), String> {
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())
}

pub(crate) fn ensure_main_window_visible(
    app: &AppHandle,
    widget_mode: bool,
    focus: bool,
) -> Result<(), String> {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => build_main_window(app, widget_mode)?,
    };

    if let Err(error) = window.set_always_on_top(true) {
        eprintln!("[FamVoice] Failed to restore always-on-top state: {error}");
    }
    if let Err(error) = keep_window_on_screen(&window) {
        eprintln!("[FamVoice] Failed to keep main window on screen: {error}");
    }

    if focus {
        window.unminimize().map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        show_without_activation(&window)?;
    }

    Ok(())
}

pub(crate) fn main_window_dimensions(widget_mode: bool) -> (f64, f64) {
    if widget_mode {
        (DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT)
    } else {
        (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
    }
}

pub(crate) fn set_main_window_size(
    window: &WebviewWindow,
    width: f64,
    height: f64,
    center: bool,
) -> Result<(), String> {
    window.set_resizable(true).map_err(|e| e.to_string())?;
    window
        .set_min_size(None::<Size>)
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(None::<Size>)
        .map_err(|e| e.to_string())?;

    let size = LogicalSize::new(width, height);
    window.set_size(size).map_err(|e| e.to_string())?;
    window
        .set_min_size(Some(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(Some(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window.set_resizable(false).map_err(|e| e.to_string())?;
    let _ = window.set_maximizable(false);

    if center {
        let _ = window.center();
    }

    Ok(())
}

pub(crate) fn apply_main_window_mode(
    app: &AppHandle,
    widget_mode: bool,
    center: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    let (width, height) = main_window_dimensions(widget_mode);
    set_main_window_size(&window, width, height, center)
}

pub(crate) fn resize_main_window(app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    set_main_window_size(&window, width, height, false)
}

pub(crate) fn close_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.close();
    }
}

pub(crate) fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("Settings")
    .inner_size(340.0, 520.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true);

    if let Some(main) = app.get_webview_window("main") {
        if let (Ok(pos), Ok(size), Ok(factor)) = (
            main.outer_position(),
            main.outer_size(),
            main.scale_factor(),
        ) {
            let settings_width = 340.0 * factor;
            let settings_height = 520.0 * factor;
            let gap = 12.0 * factor;
            let mut x = pos.x as f64 + size.width as f64 + gap;
            let mut y = pos.y as f64 + (size.height as f64 / 2.0) - (settings_height / 2.0);

            if let Ok(Some(monitor)) = main.current_monitor() {
                let m_pos = monitor.position();
                let m_size = monitor.size();
                let m_right = (m_pos.x + m_size.width as i32) as f64;
                let m_top = m_pos.y as f64;
                let m_bottom = (m_pos.y + m_size.height as i32) as f64;

                if x + settings_width > m_right {
                    x = pos.x as f64 - settings_width - gap;
                }

                if y < m_top {
                    y = m_top;
                } else if y + settings_height > m_bottom {
                    y = m_bottom - settings_height;
                }
            }

            builder = builder.position(x, y);
        } else {
            builder = builder.center();
        }
    } else {
        builder = builder.center();
    }

    builder.build().map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_window_dimensions_use_compact_widget_size() {
        assert_eq!(
            main_window_dimensions(true),
            (DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT)
        );
        assert_eq!(
            main_window_dimensions(false),
            (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
        );
    }

    #[test]
    fn clamp_position_keeps_window_inside_work_area() {
        let work_area = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(1920, 1040),
        };

        assert_eq!(
            clamp_position_to_work_area(
                PhysicalPosition::new(1900, 1030),
                PhysicalSize::new(128, 44),
                &work_area,
            ),
            PhysicalPosition::new(1792, 996)
        );
    }

    #[test]
    fn clamp_position_supports_negative_monitor_coordinates() {
        let work_area = PhysicalRect {
            position: PhysicalPosition::new(-1920, -100),
            size: PhysicalSize::new(1920, 1080),
        };

        assert_eq!(
            clamp_position_to_work_area(
                PhysicalPosition::new(-2400, -300),
                PhysicalSize::new(128, 44),
                &work_area,
            ),
            PhysicalPosition::new(-1920, -100)
        );
    }

    #[test]
    fn clamp_position_leaves_visible_window_unchanged() {
        let work_area = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(1920, 1040),
        };

        assert_eq!(
            clamp_position_to_work_area(
                PhysicalPosition::new(640, 360),
                PhysicalSize::new(128, 44),
                &work_area,
            ),
            PhysicalPosition::new(640, 360)
        );
    }

    #[test]
    fn clamp_position_handles_window_larger_than_work_area() {
        let work_area = PhysicalRect {
            position: PhysicalPosition::new(100, 50),
            size: PhysicalSize::new(80, 30),
        };

        assert_eq!(
            clamp_position_to_work_area(
                PhysicalPosition::new(400, 400),
                PhysicalSize::new(128, 44),
                &work_area,
            ),
            PhysicalPosition::new(100, 50)
        );
    }
}
