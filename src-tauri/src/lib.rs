mod mouse_hook;
mod tray;
mod tts;

#[cfg(target_os = "macos")]
mod overlay_panel;

#[cfg(not(target_os = "macos"))]
mod overlay_window;

use mouse_hook::start_global_mouse_stream;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![tts::capture_slit, tts::speak_text]);

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let window = app.get_webview_window("main").expect("main window not found");

            #[cfg(target_os = "macos")]
            overlay_panel::configure(window.clone())?;

            #[cfg(not(target_os = "macos"))]
            overlay_window::configure(window.clone())?;

            start_global_mouse_stream(window);
            tray::setup_tray(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
