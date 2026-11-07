//! The problems of the project: the diagnostics every language service
//! published, per file, as rows of the Problems panel. Pure logic.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use language::{Diagnostic, Severity};

/// A row of the panel: a file with its problems below it, or a problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProblemRow {
    File {
        path: Arc<Path>,
        count: usize,
    },
    Problem {
        path: Arc<Path>,
        diagnostic: Diagnostic,
    },
}

/// Diagnostics by file and by the service that published them.
#[derive(Debug, Default)]
pub(crate) struct Problems {
    /// File, then service name.
    files: BTreeMap<PathBuf, BTreeMap<String, Vec<Diagnostic>>>,
    /// The rows in order; rebuilt on every change.
    rows: Vec<ProblemRow>,
    errors: usize,
    warnings: usize,
}

impl Problems {
    /// `source` now reports `diagnostics` for `path` (none clears them).
    /// Hints are not listed: they mark code, they are not problems.
    pub(crate) fn set(&mut self, source: &str, path: &Path, diagnostics: &[Diagnostic]) {
        let diagnostics: Vec<Diagnostic> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity != Severity::Hint)
            .cloned()
            .collect();
        if diagnostics.is_empty() {
            if let Some(sources) = self.files.get_mut(path) {
                sources.remove(source);
                if sources.is_empty() {
                    self.files.remove(path);
                }
            }
        } else {
            self.files
                .entry(path.to_path_buf())
                .or_default()
                .insert(source.to_owned(), diagnostics);
        }
        self.rebuild();
    }

    /// Forgets everything `source` reported (its server stopped).
    pub(crate) fn clear_source(&mut self, source: &str) {
        self.files.retain(|_, sources| {
            sources.remove(source);
            !sources.is_empty()
        });
        self.rebuild();
    }

    /// Forgets every problem (the folder closed).
    pub(crate) fn clear(&mut self) {
        self.files.clear();
        self.rebuild();
    }

    pub(crate) fn rows(&self) -> &[ProblemRow] {
        &self.rows
    }

    /// Errors and warnings of the whole project.
    pub(crate) fn counts(&self) -> (usize, usize) {
        (self.errors, self.warnings)
    }

    /// Problems of every severity.
    pub(crate) fn total(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| matches!(row, ProblemRow::Problem { .. }))
            .count()
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        self.errors = 0;
        self.warnings = 0;
        for (path, sources) in &self.files {
            let mut diagnostics: Vec<&Diagnostic> = sources.values().flatten().collect();
            diagnostics.sort_by_key(|diagnostic| {
                (
                    diagnostic.range.start.row,
                    diagnostic.range.start.column,
                    diagnostic.severity,
                )
            });
            let path: Arc<Path> = Arc::from(path.as_path());
            self.rows.push(ProblemRow::File {
                path: path.clone(),
                count: diagnostics.len(),
            });
            for diagnostic in diagnostics {
                match diagnostic.severity {
                    Severity::Error => self.errors += 1,
                    Severity::Warning => self.warnings += 1,
                    Severity::Information | Severity::Hint => {}
                }
                self.rows.push(ProblemRow::Problem {
                    path: path.clone(),
                    diagnostic: diagnostic.clone(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use text::Point;

    use super::*;

    fn diagnostic(row: usize, severity: Severity, message: &str) -> Diagnostic {
        Diagnostic {
            range: Point::new(row, 0)..Point::new(row, 1),
            severity,
            message: message.to_owned(),
            source: None,
            code: None,
        }
    }

    fn shape(problems: &Problems) -> Vec<String> {
        problems
            .rows()
            .iter()
            .map(|row| match row {
                ProblemRow::File { path, count } => format!("{} {count}", path.display()),
                ProblemRow::Problem { diagnostic, .. } => format!("  {}", diagnostic.message),
            })
            .collect()
    }

    #[test]
    fn files_are_listed_with_their_problems_in_order() {
        let mut problems = Problems::default();
        let b = Path::new("/p/b.rs");
        let a = Path::new("/p/a.rs");
        problems.set(
            "ra",
            b,
            &[
                diagnostic(9, Severity::Warning, "late"),
                diagnostic(1, Severity::Error, "early"),
            ],
        );
        problems.set(
            "ra",
            a,
            &[
                diagnostic(0, Severity::Information, "note"),
                diagnostic(2, Severity::Hint, "unused"),
            ],
        );
        assert_eq!(
            shape(&problems),
            vec![
                format!("{} 1", a.display()),
                "  note".to_owned(),
                format!("{} 2", b.display()),
                "  early".to_owned(),
                "  late".to_owned(),
            ]
        );
        assert_eq!(problems.counts(), (1, 1));
        assert_eq!(problems.total(), 3);
    }

    #[test]
    fn sources_report_separately_and_clear_alone() {
        let mut problems = Problems::default();
        let file = Path::new("/p/a.rs");
        problems.set("ra", file, &[diagnostic(0, Severity::Error, "one")]);
        problems.set("other", file, &[diagnostic(1, Severity::Error, "two")]);
        assert_eq!(problems.counts(), (2, 0));
        problems.set("ra", file, &[]);
        assert_eq!(problems.counts(), (1, 0));
        problems.clear_source("other");
        assert!(problems.rows().is_empty());
        assert_eq!(problems.counts(), (0, 0));
    }
}
