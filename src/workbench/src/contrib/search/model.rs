//! Search results as the rows of a tree: each file with matches, its
//! matching lines below it while it is expanded.

use std::path::Path;

use search::{FileMatches, LineMatch};

/// A file with matches.
#[derive(Debug, Clone)]
pub(crate) struct FileResult {
    pub(crate) matches: FileMatches,
    pub(crate) count: usize,
    pub(crate) expanded: bool,
}

/// A row of the results tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultRow {
    /// The file at this index of [`Results::files`].
    File(usize),
    /// A line of a file: file index, line index.
    Line(usize, usize),
}

/// Results of the current search.
#[derive(Debug, Default)]
pub(crate) struct Results {
    files: Vec<FileResult>,
    rows: Vec<ResultRow>,
    matches: usize,
}

impl Results {
    pub(crate) fn clear(&mut self) {
        self.files.clear();
        self.rows.clear();
        self.matches = 0;
    }

    /// Adds the next file; it starts expanded.
    pub(crate) fn push(&mut self, matches: FileMatches) {
        let index = self.files.len();
        let count = matches.count();
        self.matches += count;
        self.rows.push(ResultRow::File(index));
        self.rows
            .extend((0..matches.lines.len()).map(|line| ResultRow::Line(index, line)));
        self.files.push(FileResult {
            matches,
            count,
            expanded: true,
        });
    }

    /// Expands or collapses the file at `file`; returns the row of the
    /// file afterwards.
    pub(crate) fn set_expanded(&mut self, file: usize, expanded: bool) -> Option<usize> {
        let result = self.files.get_mut(file)?;
        if result.expanded != expanded {
            result.expanded = expanded;
            self.rebuild_rows();
        }
        self.rows
            .iter()
            .position(|row| *row == ResultRow::File(file))
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        for (index, file) in self.files.iter().enumerate() {
            self.rows.push(ResultRow::File(index));
            if file.expanded {
                self.rows
                    .extend((0..file.matches.lines.len()).map(|line| ResultRow::Line(index, line)));
            }
        }
    }

    pub(crate) fn rows(&self) -> &[ResultRow] {
        &self.rows
    }

    pub(crate) fn file(&self, index: usize) -> &FileResult {
        &self.files[index]
    }

    pub(crate) fn line(&self, file: usize, line: usize) -> (&Path, &LineMatch) {
        let result = &self.files[file];
        (&result.matches.path, &result.matches.lines[line])
    }

    /// Matches in every file.
    pub(crate) fn match_count(&self) -> usize {
        self.matches
    }

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use search::Preview;

    use super::*;

    fn file(name: &str, lines: &[(usize, usize)]) -> FileMatches {
        FileMatches {
            path: PathBuf::from(name),
            lines: lines
                .iter()
                .map(|&(line, matches)| LineMatch {
                    line,
                    ranges: (0..matches).map(|index| index..index + 1).collect(),
                    preview: Preview {
                        text: String::new(),
                        highlights: Vec::new(),
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn files_are_followed_by_their_lines() {
        let mut results = Results::default();
        results.push(file("main.rs", &[(3, 1), (20, 3)]));
        results.push(file("config.rs", &[(7, 1)]));
        assert_eq!(
            results.rows(),
            &[
                ResultRow::File(0),
                ResultRow::Line(0, 0),
                ResultRow::Line(0, 1),
                ResultRow::File(1),
                ResultRow::Line(1, 0),
            ]
        );
        assert_eq!(results.match_count(), 5);
        assert_eq!(results.file_count(), 2);
        assert_eq!(results.file(0).count, 4);
    }

    #[test]
    fn collapsing_hides_the_lines_of_a_file() {
        let mut results = Results::default();
        results.push(file("main.rs", &[(3, 1), (20, 1)]));
        results.push(file("config.rs", &[(7, 1)]));
        assert_eq!(results.set_expanded(0, false), Some(0));
        assert_eq!(
            results.rows(),
            &[
                ResultRow::File(0),
                ResultRow::File(1),
                ResultRow::Line(1, 0)
            ]
        );
        assert_eq!(results.set_expanded(1, false), Some(1));
        assert_eq!(results.set_expanded(0, true), Some(0));
        assert_eq!(results.rows().len(), 4);
        // A collapsed file keeps counting.
        assert_eq!(results.match_count(), 3);
    }

    #[test]
    fn clearing_forgets_everything() {
        let mut results = Results::default();
        results.push(file("main.rs", &[(3, 1)]));
        results.clear();
        assert!(results.is_empty());
        assert!(results.rows().is_empty());
        assert_eq!(results.match_count(), 0);
    }
}
