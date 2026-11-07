//! Saving the panel sizes the user drags to, for the next run.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Context, Entity, Subscription};
use workbench::{PanelSizesChanged, Workbench};

use super::Shell;

/// Saves the sizes to `file` (in the background) whenever a drag ends.
pub(super) fn watch(
    fs: Arc<dyn fs::Fs>,
    file: Option<PathBuf>,
    workbench: &Entity<Workbench>,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe(workbench, move |_, _, event: &PanelSizesChanged, cx| {
        let Some(file) = file.clone() else {
            return;
        };
        let fs = fs.clone();
        let state = event.0;
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = state.save(fs.as_ref(), &file).await {
                    tracing::warn!(%error, "cannot save the panel sizes");
                }
            })
            .detach();
    })
}
