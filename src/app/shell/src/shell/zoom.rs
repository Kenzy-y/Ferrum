//! Interface zoom (`[appearance] zoom`, Ctrl + / Ctrl − / Ctrl 0).

use gpui::{Context, Window};
use settings::LayerKind;
use theme::ActiveTheme as _;

use super::Shell;
use super::setting_writes::SettingWrite;

/// Zooms closer than this are the same zoom.
const SAME_ZOOM: f32 = 0.001;

impl Shell {
    /// Applies `step` to the current zoom and saves it in the user
    /// settings; the default zoom removes the value instead.
    pub(super) fn change_zoom(
        &mut self,
        step: fn(f32) -> f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = cx.theme().zoom;
        let next = step(current);
        if (next - current).abs() < SAME_ZOOM {
            return;
        }
        // At once, so holding the keys feels immediate; the write follows.
        apply_zoom(next, cx);
        let value = ((next - theme::zoom::DEFAULT).abs() >= SAME_ZOOM)
            .then(|| toml::Value::Float((f64::from(next) * 100.0).round() / 100.0));
        self.write_setting(
            SettingWrite {
                layer: LayerKind::User,
                path: vec!["appearance".to_owned(), "zoom".to_owned()],
                value,
            },
            window,
            cx,
        );
    }

    /// Applies `[appearance] zoom` from the settings.
    pub(super) fn apply_zoom_setting(&self, cx: &mut Context<Self>) {
        apply_zoom(
            self.settings.get::<theme::AppearanceSettings>(None).zoom,
            cx,
        );
    }
}

/// Rebuilds the theme at `zoom`, unless it already is.
fn apply_zoom(zoom: f32, cx: &mut gpui::App) {
    let zoom = theme::zoom::clamp(zoom);
    let current = theme::shared_theme(cx);
    if (current.zoom - zoom).abs() < SAME_ZOOM {
        return;
    }
    match (*current).clone().with_zoom(zoom) {
        Ok(zoomed) => {
            theme::set_theme(zoomed, cx);
            ui::sync_component_theme(cx);
        }
        Err(error) => tracing::error!("{error}"),
    }
}
