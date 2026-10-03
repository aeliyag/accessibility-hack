use tauri::Manager;

#[tauri::command]
pub fn prepare_camera_prompt(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        // Queue these before the command returns so the event loop applies them
        // before the webview starts getUserMedia.
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = app.set_dock_visibility(true);
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

#[tauri::command]
pub fn restore_overlay_policy(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_dock_visibility(false);
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}
