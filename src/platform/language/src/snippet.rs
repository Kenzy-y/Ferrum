//! Snippets: text with tab stops, in the syntax of the Language Server
//! Protocol (and of VS Code snippets). `$1`, `${1}`, `${1:default}`,
//! `${1|one,two|}` (the first choice is inserted), `$0` the final cursor,
//! variables (`$TM_FILENAME`, `${NAME:default}`, which become their
//! default), and `\$`, `\}`, `\\` escapes.

use std::ops::Range;

/// A place the cursor visits in order. Stops with the same index are
/// edited together (several cursors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStop {
    /// Order of the visit; `0` is the final cursor, visited last.
    pub index: u32,
    /// Byte ranges in the snippet's text; empty for a bare cursor.
    pub ranges: Vec<Range<usize>>,
}

/// A parsed snippet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snippet {
    /// The text to insert.
    pub text: String,
    /// Its tab stops in visiting order (`1`, `2`, …, then `0`).
    pub stops: Vec<TabStop>,
}

impl Snippet {
    /// Parses `source`. Malformed parts are kept as text, so any string
    /// gives a snippet.
    pub fn parse(source: &str) -> Self {
        let mut parser = Parser {
            chars: source.chars().collect(),
            position: 0,
            text: String::with_capacity(source.len()),
            found: Vec::new(),
        };
        parser.body(None);
        let mut stops: Vec<TabStop> = Vec::new();
        for (index, range) in parser.found {
            match stops.iter_mut().find(|stop| stop.index == index) {
                Some(stop) => stop.ranges.push(range),
                None => stops.push(TabStop {
                    index,
                    ranges: vec![range],
                }),
            }
        }
        // 1, 2, … first, the final cursor last.
        stops.sort_by_key(|stop| {
            if stop.index == 0 {
                u32::MAX
            } else {
                stop.index
            }
        });
        Self {
            text: parser.text,
            stops,
        }
    }
}

struct Parser {
    chars: Vec<char>,
    position: usize,
    text: String,
    /// Stops in the order they appear.
    found: Vec<(u32, Range<usize>)>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn next(&mut self) -> Option<char> {
        let char = self.peek()?;
        self.position += 1;
        Some(char)
    }

    /// Text until `end` (or the end of the source), with stops inside.
    fn body(&mut self, end: Option<char>) {
        while let Some(char) = self.peek() {
            if Some(char) == end {
                return;
            }
            self.position += 1;
            match char {
                '\\' => match self.peek() {
                    Some(escaped @ ('$' | '}' | '\\' | ',' | '|')) => {
                        self.position += 1;
                        self.text.push(escaped);
                    }
                    _ => self.text.push('\\'),
                },
                '$' => self.dollar(),
                char => self.text.push(char),
            }
        }
    }

    /// After a `$`: a tab stop, a placeholder, a choice or a variable.
    fn dollar(&mut self) {
        let start = self.position;
        if let Some(index) = self.number() {
            let at = self.text.len();
            self.found.push((index, at..at));
            return;
        }
        if self.peek() == Some('{') {
            self.position += 1;
            if let Some(index) = self.number() {
                let at = self.text.len();
                match self.next() {
                    Some('}') => self.found.push((index, at..at)),
                    Some(':') => {
                        self.body(Some('}'));
                        self.position += 1;
                        self.found.push((index, at..self.text.len()));
                    }
                    Some('|') => {
                        let choice = self.choice();
                        self.text.push_str(&choice);
                        self.found.push((index, at..self.text.len()));
                    }
                    _ => self.literal(start),
                }
                return;
            }
            if self.name().is_some() {
                match self.next() {
                    // A variable's value is not known here: its default.
                    Some(':') => {
                        self.body(Some('}'));
                        self.position += 1;
                    }
                    Some('}') => {}
                    _ => self.literal(start),
                }
                return;
            }
            self.literal(start);
            return;
        }
        if self.name().is_none() {
            self.text.push('$');
        }
    }

    /// The first option of a `|a,b|}` choice.
    fn choice(&mut self) -> String {
        let mut first = String::new();
        let mut taking = true;
        while let Some(char) = self.next() {
            match char {
                '\\' => {
                    if let Some(escaped) = self.next()
                        && taking
                    {
                        first.push(escaped);
                    }
                }
                ',' => taking = false,
                '|' => {
                    if self.peek() == Some('}') {
                        self.position += 1;
                    }
                    break;
                }
                char if taking => first.push(char),
                _ => {}
            }
        }
        first
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.position;
        while self.peek().is_some_and(|char| char.is_ascii_digit()) {
            self.position += 1;
        }
        if start == self.position {
            return None;
        }
        let digits: String = self.chars[start..self.position].iter().collect();
        digits.parse().ok()
    }

    fn name(&mut self) -> Option<()> {
        let start = self.position;
        if !self
            .peek()
            .is_some_and(|char| char.is_ascii_alphabetic() || char == '_')
        {
            return None;
        }
        while self
            .peek()
            .is_some_and(|char| char.is_ascii_alphanumeric() || char == '_')
        {
            self.position += 1;
        }
        (self.position > start).then_some(())
    }

    /// Not a snippet construct after all: the `$` and what followed it are
    /// text.
    fn literal(&mut self, start: usize) {
        self.text.push('$');
        self.position = start;
    }
}

#[cfg(test)]
// A tab stop's ranges are a list that happens to hold one range here.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    #[test]
    fn macro_call_puts_the_cursor_inside() {
        let snippet = Snippet::parse("println!($0)");
        assert_eq!(snippet.text, "println!()");
        assert_eq!(snippet.stops.len(), 1);
        assert_eq!(snippet.stops[0].index, 0);
        assert_eq!(snippet.stops[0].ranges, [9..9]);
    }

    #[test]
    fn placeholders_are_visited_in_order_and_mirrored() {
        // A mirror (`${2}` again) is inserted empty: its text is not
        // copied from the placeholder (servers rarely send mirrors).
        let snippet = Snippet::parse("fn ${2:name}(${1:arg}) -> ${2} {$0}");
        assert_eq!(snippet.text, "fn name(arg) ->  {}");
        let order: Vec<u32> = snippet.stops.iter().map(|stop| stop.index).collect();
        assert_eq!(order, vec![1, 2, 0]);
        assert_eq!(snippet.stops[0].ranges, [8..11]);
        assert_eq!(snippet.stops[1].ranges, vec![3..7, 16..16]);
    }

    #[test]
    fn nested_placeholders_choices_variables_and_escapes() {
        let snippet = Snippet::parse("${1:a ${2:b}} ${3|x,y|} $TM_FILENAME ${NAME:n} \\$5 $");
        assert_eq!(snippet.text, "a b x  n $5 $");
        assert_eq!(snippet.stops[0].ranges, [0..3]);
        assert_eq!(snippet.stops[1].ranges, [2..3]);
        assert_eq!(snippet.stops[2].ranges, [4..5]);
    }
}
