use tauri::WebviewWindow;

#[tauri::command]
pub fn set_click_through(window: WebviewWindow, ignore: bool) -> Result<(), String> {
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|error| error.to_string())
}
