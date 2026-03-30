use tauri::{AppHandle, LogicalSize, Manager, Size, WebviewWindow};

const DEFAULT_WINDOW_WIDTH: f64 = 360.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 200.0;
const DEFAULT_WIDGET_WIDTH: f64 = 128.0;
const DEFAULT_WIDGET_HEIGHT: f64 = 44.0;

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
    .always_on_top(true);

    if let Some(main) = app.get_webview_window("main") {
        if let (Ok(pos), Ok(size), Ok(factor)) =
            (main.outer_position(), main.outer_size(), main.scale_factor())
        {
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
}
