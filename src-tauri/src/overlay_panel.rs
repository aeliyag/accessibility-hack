use tauri::WebviewWindow;
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt,
};

tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true,
        }
    })
}

pub fn configure(window: WebviewWindow) -> tauri::Result<()> {
    let panel = window.to_panel::<OverlayPanel>()?;

    panel.set_level(PanelLevel::Floating.value());

    panel
        .add_style_mask(StyleMask::empty().nonactivating_panel().into())
        .expect("failed to make overlay panel non-activating");

    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );

    panel.set_hides_on_deactivate(false);

    if let Some(monitor) = window.primary_monitor()? {
        let size = monitor.size();
        let position = monitor.position();
        window.set_size(*size)?;
        window.set_position(*position)?;
    }

    Ok(())
}
