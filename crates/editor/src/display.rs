//! A line as it is drawn: tabs expanded to spaces, with offset mapping in
//! both directions. Pure logic, tested without a window.

/// A tab in the source text and where its spaces are in the drawn text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tab {
    /// Byte offset of the tab in the source.
    source: usize,
    /// Byte offset of its first space in the drawn text.
    display: usize,
    /// Number of spaces drawn for it.
    width: usize,
}

/// The drawn text of (part of) a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisplayLine {
    /// The text to shape: no tabs, no line breaks.
    pub(crate) text: String,
    tabs: Vec<Tab>,
}

impl DisplayLine {
    /// Expands tabs in `source` to the next multiple of `tab_size`;
    /// `start_column` is the visual column where `source` starts (non-zero
    /// for a window into a long line).
    pub(crate) fn new(source: &str, tab_size: usize, start_column: usize) -> Self {
        let tab_size = tab_size.max(1);
        if !source.contains('\t') {
            return Self {
                text: source.to_owned(),
                tabs: Vec::new(),
            };
        }
        let mut text = String::with_capacity(source.len() + 8);
        let mut tabs = Vec::new();
        let mut column = start_column;
        for (offset, character) in source.char_indices() {
            if character == '\t' {
                let width = tab_size - column % tab_size;
                tabs.push(Tab {
                    source: offset,
                    display: text.len(),
                    width,
                });
                text.extend(std::iter::repeat_n(' ', width));
                column += width;
            } else {
                text.push(character);
                column += 1;
            }
        }
        Self { text, tabs }
    }

    /// The drawn offset of source offset `source`.
    pub(crate) fn to_display(&self, source: usize) -> usize {
        let before = self.tabs.partition_point(|tab| tab.source < source);
        match before.checked_sub(1).map(|index| self.tabs[index]) {
            // Every byte after a tab is shifted by the tab's extra spaces.
            Some(tab) => tab.display + tab.width + (source - tab.source - 1),
            None => source,
        }
    }

    /// The source offset drawn at `display`; inside a tab's spaces, the
    /// nearer edge of the tab.
    pub(crate) fn to_source(&self, display: usize) -> usize {
        let before = self.tabs.partition_point(|tab| tab.display < display);
        let Some(tab) = before.checked_sub(1).map(|index| self.tabs[index]) else {
            return display;
        };
        let end = tab.display + tab.width;
        if display < end {
            if (display - tab.display) * 2 < tab.width {
                tab.source
            } else {
                tab.source + 1
            }
        } else {
            tab.source + 1 + (display - end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_without_tabs_is_unchanged() {
        let line = DisplayLine::new("let x = 1;", 4, 0);
        assert_eq!(line.text, "let x = 1;");
        assert_eq!(line.to_display(4), 4);
        assert_eq!(line.to_source(4), 4);
    }

    #[test]
    fn tabs_expand_to_the_next_stop() {
        let line = DisplayLine::new("a\tb\t\tc", 4, 0);
        assert_eq!(line.text, "a   b       c");
        assert_eq!(line.to_display(1), 1, "before the first tab");
        assert_eq!(line.to_display(2), 4, "b");
        assert_eq!(line.to_display(5), 12, "c");
        assert_eq!(line.to_source(4), 2);
        assert_eq!(line.to_source(12), 5);
        assert_eq!(line.to_source(2), 1, "left half of a tab");
        assert_eq!(line.to_source(3), 2, "right half of a tab");
    }

    #[test]
    fn a_window_starts_at_its_column() {
        let line = DisplayLine::new("\tx", 4, 2);
        assert_eq!(line.text, "  x");
    }
}
