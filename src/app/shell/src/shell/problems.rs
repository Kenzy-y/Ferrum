//! The Problems panel: created with the window, fed by every language
//! service the window starts, and opening what is activated in it.

use std::sync::Arc;

use futures::StreamExt as _;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use i18n::{FluentArgs, I18n};
use language::{LanguageService, ServiceEvent};
use workbench::{FileIcons, OpenProblem, ProblemsTexts, ProblemsView, Workbench};

use super::Shell;

/// The panel, shown by `workbench`, and the subscription that opens its
/// problems.
pub(super) fn create(
    i18n: &Arc<I18n>,
    icons: Arc<FileIcons>,
    workbench: &Entity<Workbench>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<ProblemsView>, Subscription) {
    let position_i18n = i18n.clone();
    let texts = ProblemsTexts {
        empty: i18n.text("problems-empty").into(),
        position: Arc::new(move |line, column| {
            let mut args = FluentArgs::new();
            args.set("line", line);
            args.set("column", column);
            SharedString::from(position_i18n.text_with("problems-position", &args))
        }),
    };
    let problems = cx.new(|cx| ProblemsView::new(icons, texts, cx));
    workbench.update(cx, |workbench, cx| {
        workbench.set_problems(problems.clone(), cx);
    });
    let open = cx.subscribe_in(
        &problems,
        window,
        |shell, _, event: &OpenProblem, window, cx| {
            shell.open_file(
                event.path.clone(),
                Some(event.position..event.position),
                window,
                cx,
            );
        },
    );
    (problems, open)
}

impl Shell {
    /// Passes the diagnostics `service` publishes, for any file, to the
    /// Problems panel until the folder closes.
    pub(super) fn watch_problems(
        &mut self,
        service: &Arc<dyn LanguageService>,
        cx: &mut Context<Self>,
    ) {
        let source = service.name().to_owned();
        let mut events = service.events();
        let problems = self.problems.downgrade();
        self.problem_watches.push(cx.spawn(async move |_, cx| {
            while let Some(event) = events.next().await {
                let ServiceEvent::Diagnostics {
                    path, diagnostics, ..
                } = event
                else {
                    continue;
                };
                let updated = problems.update(cx, |problems, cx| {
                    problems.set_diagnostics(&source, &path, &diagnostics, cx);
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
    }
}
