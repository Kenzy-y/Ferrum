//! The outline: named items of a file (modules, types, functions, tables)
//! with their nesting.

use std::ops::Range;

use ropey::Rope;
use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Query, QueryCursor};

use crate::{Grammar, SyntaxTree};

/// What an outline item is. The outline query names it in the capture of
/// the item: `@outline.struct`, `@outline.function`, …
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// `@outline.module`.
    Module,
    /// `@outline.struct`.
    Struct,
    /// `@outline.enum`.
    Enum,
    /// `@outline.union`.
    Union,
    /// `@outline.trait` (interfaces).
    Trait,
    /// `@outline.impl` (implementation blocks).
    Impl,
    /// `@outline.function` (methods too).
    Function,
    /// `@outline.constant`.
    Constant,
    /// `@outline.static` (global variables).
    Static,
    /// `@outline.type` (type aliases).
    TypeAlias,
    /// `@outline.macro`.
    Macro,
    /// `@outline.table` (sections of configuration files).
    Table,
    /// `@outline.key` (keys of configuration files).
    Key,
}

impl SymbolKind {
    /// Every kind.
    pub const ALL: [Self; 13] = [
        Self::Module,
        Self::Struct,
        Self::Enum,
        Self::Union,
        Self::Trait,
        Self::Impl,
        Self::Function,
        Self::Constant,
        Self::Static,
        Self::TypeAlias,
        Self::Macro,
        Self::Table,
        Self::Key,
    ];

    /// The name after `outline.` in captures.
    pub fn capture_name(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Function => "function",
            Self::Constant => "constant",
            Self::Static => "static",
            Self::TypeAlias => "type",
            Self::Macro => "macro",
            Self::Table => "table",
            Self::Key => "key",
        }
    }

    fn from_capture_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.capture_name() == name)
    }
}

/// One item of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem {
    /// What it is.
    pub kind: SymbolKind,
    /// The name, with its context words (`impl Display for Cli`).
    pub name: String,
    /// Byte range of the whole item.
    pub range: Range<usize>,
    /// Byte offset of the name: where going to the item puts the cursor.
    pub name_start: usize,
    /// Items it is nested in.
    pub depth: usize,
}

/// A compiled outline query and the meaning of its captures.
pub(crate) struct OutlineQuery {
    query: Query,
    /// Kind of each capture index that captures a whole item.
    kinds: Vec<Option<SymbolKind>>,
    name: u32,
    context: Option<u32>,
}

/// Longest name kept for an item; longer ones (a key with a huge quoted
/// string) are cut.
const MAX_NAME_BYTES: usize = 256;

impl OutlineQuery {
    pub(crate) fn new(query: Query) -> Result<Self, String> {
        let mut kinds = Vec::new();
        for name in query.capture_names() {
            let kind = match name.strip_prefix("outline.") {
                Some(kind) => Some(
                    SymbolKind::from_capture_name(kind)
                        .ok_or_else(|| format!("unknown outline kind `{kind}`"))?,
                ),
                None => None,
            };
            kinds.push(kind);
        }
        let name = query
            .capture_index_for_name("name")
            .ok_or_else(|| "no `@name` capture".to_owned())?;
        let context = query.capture_index_for_name("context");
        Ok(Self {
            query,
            kinds,
            name,
            context,
        })
    }
}

/// The outline of the whole tree, in text order, with the depth of each
/// item from how items contain one another. Linear in the size of the
/// tree: run it in the background.
pub fn outline(grammar: &Grammar, tree: &SyntaxTree, rope: &Rope) -> Vec<OutlineItem> {
    let Some(outline) = &grammar.outline else {
        return Vec::new();
    };
    let _span = tracing::debug_span!("syntax.outline", grammar = grammar.id()).entered();
    let mut cursor = QueryCursor::new();
    let text = |node: tree_sitter::Node<'_>| rope.byte_slice(node.byte_range()).chunks();
    let mut matches = cursor.matches(&outline.query, tree.tree.root_node(), text);
    let mut items = Vec::new();
    while let Some(found) = matches.next() {
        let mut item = None;
        let mut words: Vec<(usize, Range<usize>)> = Vec::new();
        for capture in found.captures() {
            let range = capture.node.byte_range();
            if let Some(Some(kind)) = outline.kinds.get(capture.index as usize) {
                item = Some((*kind, range));
            } else if capture.index == outline.name || Some(capture.index) == outline.context {
                words.push((range.start, range));
            }
        }
        let Some((kind, range)) = item else {
            continue;
        };
        words.sort_by_key(|(start, _)| *start);
        let name_start = words
            .iter()
            .find(|(start, _)| *start >= range.start)
            .map_or(range.start, |(start, _)| *start);
        let name = words
            .iter()
            .map(|(_, range)| name_text(rope, range.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        if name.is_empty() {
            continue;
        }
        items.push(OutlineItem {
            kind,
            name,
            range,
            name_start,
            depth: 0,
        });
    }
    nest(items)
}

/// The text of a name on one line, cut to [`MAX_NAME_BYTES`].
fn name_text(rope: &Rope, range: Range<usize>) -> String {
    let slice = rope.byte_slice(range);
    let mut name = String::new();
    for character in slice.chars() {
        if name.len() + character.len_utf8() > MAX_NAME_BYTES {
            name.push('…');
            break;
        }
        name.push(if character.is_whitespace() {
            ' '
        } else {
            character
        });
    }
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sorts items by position, drops duplicates (several patterns for one
/// item) and sets each depth to the number of items containing it.
fn nest(mut items: Vec<OutlineItem>) -> Vec<OutlineItem> {
    items.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then(right.range.end.cmp(&left.range.end))
    });
    items.dedup_by(|later, earlier| later.range == earlier.range && later.kind == earlier.kind);
    let mut open: Vec<Range<usize>> = Vec::new();
    for item in &mut items {
        while open
            .last()
            .is_some_and(|parent| parent.end <= item.range.start || parent.end < item.range.end)
        {
            open.pop();
        }
        item.depth = open.len();
        open.push(item.range.clone());
    }
    items
}

/// The items containing `offset`, outermost first (breadcrumbs).
pub fn items_at(items: &[OutlineItem], offset: usize) -> Vec<&OutlineItem> {
    let mut path: Vec<&OutlineItem> = Vec::new();
    // Items are in text order: only those starting before `offset` can
    // contain it.
    let candidates = items.partition_point(|item| item.range.start <= offset);
    for item in &items[..candidates] {
        if item.range.end >= offset {
            path.retain(|outer| outer.depth < item.depth);
            path.push(item);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, range: Range<usize>) -> OutlineItem {
        OutlineItem {
            kind: SymbolKind::Function,
            name: name.to_owned(),
            name_start: range.start,
            range,
            depth: 0,
        }
    }

    #[test]
    fn nesting_follows_containment() {
        let items = nest(vec![
            item("b", 10..20),
            item("a", 0..50),
            item("c", 12..15),
            item("c", 12..15),
            item("d", 30..40),
            item("e", 60..70),
        ]);
        let shape: Vec<(&str, usize)> = items
            .iter()
            .map(|item| (item.name.as_str(), item.depth))
            .collect();
        assert_eq!(shape, [("a", 0), ("b", 1), ("c", 2), ("d", 1), ("e", 0)]);

        let path: Vec<&str> = items_at(&items, 13)
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(path, ["a", "b", "c"]);
        let path: Vec<&str> = items_at(&items, 35)
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(path, ["a", "d"], "siblings that ended are left out");
        assert!(items_at(&items, 55).is_empty());
    }

    #[test]
    fn names_are_one_line_and_bounded() {
        let rope = Rope::from_str("impl<T>\n    Display");
        assert_eq!(name_text(&rope, 0..rope.len_bytes()), "impl<T> Display");
        let long = Rope::from_str(&"x".repeat(1000));
        assert!(name_text(&long, 0..1000).len() <= MAX_NAME_BYTES + '…'.len_utf8());
    }
}
