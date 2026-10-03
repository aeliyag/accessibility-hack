use tauri::WebviewWindow;

pub fn configure(window: WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.primary_monitor()? {
        let size = monitor.size();
        let position = monitor.position();
        window.set_size(*size)?;
        window.set_position(*position)?;
    }

    Ok(())
}
