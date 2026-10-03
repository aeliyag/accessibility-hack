use base64::{engine::general_purpose::STANDARD, Engine};
use std::process::Command;

/// Captures the screen rectangle under the typoscope slit and returns it as a
/// base64 PNG data URI, ready to hand to an in-browser OCR library.
#[tauri::command]
pub async fn capture_slit(x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::env::temp_dir().join(format!(
            "typoscope-capture-{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));

        let status = Command::new("screencapture")
            .args([
                "-x", // no sound
                "-R",
                &format!(
                    "{},{},{},{}",
                    x.round() as i64,
                    y.round() as i64,
                    width.round() as i64,
                    height.round() as i64
                ),
            ])
            .arg(&path)
            .status()
            .map_err(|e| format!("failed to run screencapture: {e}"))?;

        if !status.success() {
            return Err("screencapture exited with a non-zero status".to_string());
        }

        let bytes = std::fs::read(&path).map_err(|e| format!("failed to read capture: {e}"))?;
        let _ = std::fs::remove_file(&path);

        Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    })
    .await
    .map_err(|e| format!("capture task panicked: {e}"))?
}

/// Speaks text aloud using macOS's built-in `say` command.
#[tauri::command]
pub async fn speak_text(text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("No text detected in the slit".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let status = Command::new("say")
            .arg(&text)
            .status()
            .map_err(|e| format!("failed to run say: {e}"))?;
        if !status.success() {
            return Err("say exited with a non-zero status".to_string());
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("speech task panicked: {e}"))?
}
