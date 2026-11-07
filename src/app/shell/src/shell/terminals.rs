//! The terminals of the window: created with it, started in the open
//! folder, following settings and the keyboard layout.

use std::sync::Arc;

use gpui::{AppContext as _, Context, Entity, Window};
use i18n::I18n;
use settings::SettingsStore;
use workbench::{TerminalPanel, Workbench};

use super::Shell;

/// The terminal panel, shown by `workbench`.
pub(super) fn create(
    i18n: &Arc<I18n>,
    fs: Arc<dyn fs::Fs>,
    settings: &SettingsStore,
    workbench: &Entity<Workbench>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Entity<TerminalPanel> {
    let runner: Arc<dyn process::ProcessRunner> = Arc::new(process::RealProcessRunner);
    let i18n = i18n.clone();
    let terminal_settings = settings.get::<terminal::TerminalSettings>(None);
    let panel = cx.new(|cx| TerminalPanel::new(runner, fs, i18n, terminal_settings, window, cx));
    workbench.update(cx, |workbench, cx| {
        workbench.set_terminal(panel.clone(), cx);
    });
    panel
}
