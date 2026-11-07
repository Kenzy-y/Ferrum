//! Problems: the diagnostics of the whole project in the bottom panel,
//! and their counts in the status bar and on the panel tab. The owner
//! passes every language service's diagnostics to [`ProblemsView`].

mod model;
mod view;

pub use self::view::{OpenProblem, ProblemsTexts, ProblemsView};
