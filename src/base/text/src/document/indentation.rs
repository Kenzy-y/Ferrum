//! Tab and Shift+Tab: indenting at cursors and indenting whole lines.

use std::ops::RangeInclusive;

use crate::{EditKind, Selection};

use super::{Document, Placement};

/// How one level of indentation is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indent {
    /// Columns per level (and per tab character).
    pub tab_size: usize,
    /// Spaces (`true`) or a tab character per level.
    pub spaces: bool,
}

impl Default for Indent {
    fn default() -> Self {
        Self {
            tab_size: 4,
            spaces: true,
        }
    }
}

impl Document {
    /// Tab: with cursors or selections inside single lines, inserts
    /// indentation up to the next tab stop at each one; with a selection
    /// over several lines, indents those lines by one level.
    pub fn indent(&mut self, indent: Indent) {
        let tab_size = indent.tab_size.max(1);
        if self
            .selections
            .iter()
            .all(|selection| self.single_line(selection))
        {
            let replacements = self
                .selections
                .iter()
                .map(|selection| {
                    let text = if indent.spaces {
                        let column = self.visual_column(selection.start(), tab_size);
                        " ".repeat(tab_size - column % tab_size)
                    } else {
                        "\t".to_owned()
                    };
                    (selection.range(), text)
                })
                .collect();
            self.apply(
                replacements,
                EditKind::Other,
                None,
                &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.end)),
            );
            return;
        }
        let unit = if indent.spaces {
            " ".repeat(tab_size)
        } else {
            "\t".to_owned()
        };
        let replacements = self
            .selected_rows()
            .into_iter()
            .filter(|row| !self.buffer.line_range(*row).is_empty())
            .map(|row| {
                let start = self.buffer.line_range(row).start;
                (start..start, unit.clone())
            })
            .collect();
        self.apply(replacements, EditKind::Other, None, &Placement::Keep);
    }

    /// Shift+Tab: removes one level of indentation from every line with a
    /// cursor or selection.
    pub fn outdent(&mut self, indent: Indent) {
        let tab_size = indent.tab_size.max(1);
        let replacements = self
            .selected_rows()
            .into_iter()
            .filter_map(|row| {
                let start = self.buffer.line_range(row).start;
                let mut width = 0;
                let mut bytes = 0;
                for character in self.buffer.line(row).chars() {
                    match character {
                        ' ' if width < tab_size => {
                            width += 1;
                            bytes += 1;
                        }
                        '\t' if width == 0 => {
                            bytes += 1;
                            break;
                        }
                        _ => break,
                    }
                }
                (bytes > 0).then(|| (start..start + bytes, String::new()))
            })
            .collect();
        self.apply(replacements, EditKind::Other, None, &Placement::Keep);
    }

    fn single_line(&self, selection: &Selection) -> bool {
        self.buffer.offset_to_point(selection.start()).row
            == self.buffer.offset_to_point(selection.end()).row
    }

    /// Rows touched by the selections, each once, in order. A selection
    /// ending at the start of a line does not touch that line.
    fn selected_rows(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = Vec::new();
        for selection in self.selections.iter() {
            for row in self.rows_of(selection) {
                if rows.last().is_none_or(|last| *last < row) {
                    rows.push(row);
                }
            }
        }
        rows
    }

    fn rows_of(&self, selection: &Selection) -> RangeInclusive<usize> {
        let start = self.buffer.offset_to_point(selection.start());
        let end = self.buffer.offset_to_point(selection.end());
        let last = if end.row > start.row && end.column == 0 {
            end.row - 1
        } else {
            end.row
        };
        start.row..=last
    }

    /// Column of `offset` with tabs expanded to `tab_size`.
    fn visual_column(&self, offset: usize, tab_size: usize) -> usize {
        let row = self.buffer.offset_to_point(offset).row;
        let start = self.buffer.line_range(row).start;
        let mut column = 0;
        for character in self.buffer.rope().byte_slice(start..offset).chars() {
            column = if character == '\t' {
                (column / tab_size + 1) * tab_size
            } else {
                column + 1
            };
        }
        column
    }
}
