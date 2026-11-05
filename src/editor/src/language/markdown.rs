//! The little Markdown that language servers send in hovers and
//! completion documentation: paragraphs, fenced code blocks and rules.
//! Inline markup is reduced to its text; code blocks in the editor's
//! language are highlighted with its grammar.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use syntax::{Grammar, HighlightSpan, SyntaxTree};
use text::Buffer;

/// Longest a code block of a popup may take to parse.
const CODE_PARSE_TIMEOUT: Duration = Duration::from_millis(20);

/// A block of a popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    /// A paragraph of text.
    Text(String),
    /// Code, with highlights when its language is known.
    Code {
        language: Option<String>,
        text: String,
        spans: Vec<HighlightSpan>,
    },
    /// A horizontal rule.
    Rule,
}

/// The blocks of `markdown`.
pub(crate) fn parse(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut code: Option<(Option<String>, Vec<&str>)> = None;
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Text(inline(&paragraph.join(" "))));
            paragraph.clear();
        }
    };
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(fence) = trimmed.strip_prefix("```") {
            if let Some((language, lines)) = code.take() {
                blocks.push(Block::Code {
                    language,
                    text: lines.join("\n"),
                    spans: Vec::new(),
                });
            } else {
                flush(&mut paragraph, &mut blocks);
                let language = fence.trim();
                code = Some((
                    (!language.is_empty()).then(|| language.to_owned()),
                    Vec::new(),
                ));
            }
            continue;
        }
        if let Some((_, lines)) = &mut code {
            lines.push(line);
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks);
        } else if is_rule(trimmed) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
        } else {
            paragraph.push(trimmed.trim_start_matches('#').trim_start());
        }
    }
    flush(&mut paragraph, &mut blocks);
    if let Some((language, lines)) = code {
        blocks.push(Block::Code {
            language,
            text: lines.join("\n"),
            spans: Vec::new(),
        });
    }
    // A rule at the edges separates nothing.
    while blocks.last() == Some(&Block::Rule) {
        blocks.pop();
    }
    while blocks.first() == Some(&Block::Rule) {
        blocks.remove(0);
    }
    blocks
}

fn is_rule(line: &str) -> bool {
    line.len() >= 3 && (line.chars().all(|c| c == '-') || line.chars().all(|c| c == '*'))
}

/// Text of inline markup: emphasis and code marks dropped, links reduced
/// to their text, escapes resolved.
fn inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '`' | '*' => {}
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
            }
            '[' => {
                let mut label = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == ']' {
                        closed = true;
                        break;
                    }
                    label.push(c);
                }
                if closed && chars.peek() == Some(&'(') {
                    // A link: its text only.
                    for c in chars.by_ref() {
                        if c == ')' {
                            break;
                        }
                    }
                    out.push_str(&inline(&label));
                } else {
                    out.push('[');
                    out.push_str(&label);
                    if closed {
                        out.push(']');
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Highlights the code blocks written in `language_id` (or without a
/// language) with `grammar`. Runs in the background.
pub(crate) fn highlight_code(blocks: &mut [Block], grammar: &Arc<Grammar>, language_id: &str) {
    for block in blocks {
        let Block::Code {
            language,
            text,
            spans,
        } = block
        else {
            continue;
        };
        if language
            .as_deref()
            .is_some_and(|language| language != language_id)
        {
            continue;
        }
        let buffer = Buffer::from_text(text);
        let cancel = AtomicBool::new(false);
        if let Ok(tree) =
            SyntaxTree::parse(grammar, buffer.rope(), None, CODE_PARSE_TIMEOUT, &cancel)
        {
            *spans = syntax::highlights(grammar, &tree, buffer.rope(), 0..text.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_code_rules_and_paragraphs() {
        let blocks = parse(
            "```rust\nfn main()\n```\n\n---\n\nRuns the **program**\nwith `args`.\n\nSee [docs](https://x).",
        );
        assert_eq!(
            blocks,
            vec![
                Block::Code {
                    language: Some("rust".to_owned()),
                    text: "fn main()".to_owned(),
                    spans: Vec::new(),
                },
                Block::Rule,
                Block::Text("Runs the program with args.".to_owned()),
                Block::Text("See docs.".to_owned()),
            ]
        );
    }

    #[test]
    fn keeps_plain_brackets_and_drops_edge_rules() {
        assert_eq!(
            parse("---\na[0] \\* b\n---"),
            vec![Block::Text("a[0] * b".to_owned())]
        );
    }
}
