//! Protocol answers in the IDE's terms.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use language::{
    CodeLens, Completion, CompletionEdit, CompletionKind, CompletionList, Diagnostic, Hover,
    InlayHint, InlayHintKind, LensAction, Location, ResolveToken, Severity, TextReplacement,
};
use lsp_types as lsp;
use text::ropey::Rope;

use crate::adapter::{LocationConverter, LspAdapter};
use crate::sync::{Encoding, from_range};
use crate::uri::uri_to_path;

/// Converts positions with the texts of open documents (UTF-16 columns
/// need the line).
pub(crate) struct Converter<'a> {
    pub(crate) encoding: Encoding,
    pub(crate) texts: &'a HashMap<PathBuf, Rope>,
}

impl LocationConverter for Converter<'_> {
    fn location(&self, location: &lsp::Location) -> Option<Location> {
        let path = uri_to_path(&location.uri)?;
        let range = from_range(location.range, self.texts.get(&path), self.encoding);
        Some(Location { path, range })
    }
}

impl Converter<'_> {
    fn range(&self, path: &PathBuf, range: lsp::Range) -> std::ops::Range<text::Point> {
        from_range(range, self.texts.get(path), self.encoding)
    }

    pub(crate) fn diagnostic(&self, path: &PathBuf, diagnostic: lsp::Diagnostic) -> Diagnostic {
        let severity = match diagnostic.severity {
            Some(lsp::DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(lsp::DiagnosticSeverity::INFORMATION) => Severity::Information,
            Some(lsp::DiagnosticSeverity::HINT) => Severity::Hint,
            // Errors, and the protocol's default for a missing severity.
            _ => Severity::Error,
        };
        Diagnostic {
            range: self.range(path, diagnostic.range),
            severity,
            message: diagnostic.message,
            source: diagnostic.source,
            code: diagnostic.code.map(|code| match code {
                lsp::NumberOrString::Number(number) => number.to_string(),
                lsp::NumberOrString::String(text) => text,
            }),
        }
    }

    pub(crate) fn hover(&self, path: &PathBuf, hover: lsp::Hover) -> Option<Hover> {
        let contents = match hover.contents {
            lsp::HoverContents::Scalar(marked) => marked_string(marked),
            lsp::HoverContents::Array(items) => items
                .into_iter()
                .map(marked_string)
                .collect::<Vec<_>>()
                .join("\n\n"),
            lsp::HoverContents::Markup(markup) => markup.value,
        };
        if contents.trim().is_empty() {
            return None;
        }
        Some(Hover {
            contents,
            range: hover.range.map(|range| self.range(path, range)),
        })
    }

    pub(crate) fn completions(
        &self,
        path: &PathBuf,
        response: lsp::CompletionResponse,
        resolvable: bool,
    ) -> CompletionList {
        let (items, incomplete) = match response {
            lsp::CompletionResponse::Array(items) => (items, false),
            lsp::CompletionResponse::List(list) => (list.items, list.is_incomplete),
        };
        CompletionList {
            items: items
                .into_iter()
                .map(|item| self.completion(path, item, resolvable))
                .collect(),
            incomplete,
        }
    }

    pub(crate) fn completion(
        &self,
        path: &PathBuf,
        item: lsp::CompletionItem,
        resolvable: bool,
    ) -> Completion {
        let snippet = item.insert_text_format == Some(lsp::InsertTextFormat::SNIPPET);
        let plain = |text: String| if snippet { plain_snippet(&text) } else { text };
        let edit = match &item.text_edit {
            Some(lsp::CompletionTextEdit::Edit(edit)) => CompletionEdit {
                range: Some(self.range(path, edit.range)),
                new_text: plain(edit.new_text.clone()),
            },
            Some(lsp::CompletionTextEdit::InsertAndReplace(edit)) => CompletionEdit {
                range: Some(self.range(path, edit.insert)),
                new_text: plain(edit.new_text.clone()),
            },
            None => CompletionEdit {
                range: None,
                new_text: plain(
                    item.insert_text
                        .clone()
                        .unwrap_or_else(|| item.label.clone()),
                ),
            },
        };
        let additional_edits = item
            .additional_text_edits
            .iter()
            .flatten()
            .map(|edit| TextReplacement {
                range: self.range(path, edit.range),
                new_text: edit.new_text.clone(),
            })
            .collect();
        Completion {
            label: item.label.clone(),
            kind: item.kind.and_then(completion_kind),
            detail: item.detail.clone(),
            documentation: item.documentation.clone().map(documentation),
            filter_text: item.filter_text.clone(),
            sort_text: item.sort_text.clone(),
            edit,
            additional_edits,
            resolve: resolvable.then(|| ResolveToken(Arc::new(item))),
        }
    }

    pub(crate) fn locations(&self, response: lsp::GotoDefinitionResponse) -> Vec<Location> {
        match response {
            lsp::GotoDefinitionResponse::Scalar(location) => {
                self.location(&location).into_iter().collect()
            }
            lsp::GotoDefinitionResponse::Array(locations) => locations
                .iter()
                .filter_map(|location| self.location(location))
                .collect(),
            lsp::GotoDefinitionResponse::Link(links) => links
                .into_iter()
                .filter_map(|link| {
                    self.location(&lsp::Location::new(
                        link.target_uri,
                        link.target_selection_range,
                    ))
                })
                .collect(),
        }
    }

    pub(crate) fn inlay_hint(&self, path: &PathBuf, hint: lsp::InlayHint) -> InlayHint {
        let label = match hint.label {
            lsp::InlayHintLabel::String(label) => label,
            lsp::InlayHintLabel::LabelParts(parts) => {
                parts.into_iter().map(|part| part.value).collect()
            }
        };
        let kind = match hint.kind {
            Some(lsp::InlayHintKind::TYPE) => InlayHintKind::Type,
            Some(lsp::InlayHintKind::PARAMETER) => InlayHintKind::Parameter,
            _ => InlayHintKind::Other,
        };
        let position = self
            .range(path, lsp::Range::new(hint.position, hint.position))
            .start;
        InlayHint {
            position,
            label,
            kind,
            padding_left: hint.padding_left.unwrap_or(false),
            padding_right: hint.padding_right.unwrap_or(false),
        }
    }

    pub(crate) fn code_lens(
        &self,
        path: &PathBuf,
        lens: lsp::CodeLens,
        adapter: &dyn LspAdapter,
    ) -> CodeLens {
        let range = self.range(path, lens.range);
        match &lens.command {
            Some(command) => CodeLens {
                range,
                title: Some(command.title.clone()),
                action: adapter.lens_action(command, self),
                resolve: None,
            },
            None => CodeLens {
                range,
                title: None,
                action: LensAction::Unsupported,
                resolve: Some(ResolveToken(Arc::new(lens))),
            },
        }
    }
}

fn marked_string(marked: lsp::MarkedString) -> String {
    match marked {
        lsp::MarkedString::String(text) => text,
        lsp::MarkedString::LanguageString(code) => {
            format!("```{}\n{}\n```", code.language, code.value)
        }
    }
}

fn documentation(documentation: lsp::Documentation) -> String {
    match documentation {
        lsp::Documentation::String(text) => text,
        lsp::Documentation::MarkupContent(markup) => markup.value,
    }
}

fn completion_kind(kind: lsp::CompletionItemKind) -> Option<CompletionKind> {
    use lsp::CompletionItemKind as Lsp;
    Some(match kind {
        Lsp::FUNCTION | Lsp::CONSTRUCTOR => CompletionKind::Function,
        Lsp::METHOD => CompletionKind::Method,
        Lsp::FIELD | Lsp::PROPERTY => CompletionKind::Field,
        Lsp::VARIABLE => CompletionKind::Variable,
        Lsp::CONSTANT | Lsp::VALUE => CompletionKind::Constant,
        Lsp::STRUCT | Lsp::CLASS => CompletionKind::Struct,
        Lsp::ENUM => CompletionKind::Enum,
        Lsp::ENUM_MEMBER => CompletionKind::EnumMember,
        Lsp::INTERFACE => CompletionKind::Interface,
        Lsp::MODULE | Lsp::FILE | Lsp::FOLDER => CompletionKind::Module,
        Lsp::KEYWORD => CompletionKind::Keyword,
        Lsp::SNIPPET => CompletionKind::Snippet,
        Lsp::TYPE_PARAMETER => CompletionKind::TypeParameter,
        Lsp::TEXT => CompletionKind::Text,
        _ => return None,
    })
}

/// The text of a snippet without its tab stops: `${1:name}` becomes
/// `name`, `$0` and `$1` disappear, `\$` becomes `$`. Snippet sessions are
/// not supported yet (ROADMAP), and servers are told so, but some send
/// snippets anyway.
pub(crate) fn plain_snippet(snippet: &str) -> String {
    let mut plain = String::with_capacity(snippet.len());
    let mut chars = snippet.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '\\' => {
                if let Some(next) = chars.next() {
                    plain.push(next);
                }
            }
            '$' => match chars.peek() {
                Some('{') => {
                    chars.next();
                    while chars.peek().is_some_and(char::is_ascii_digit) {
                        chars.next();
                    }
                    if chars.peek() == Some(&':') {
                        chars.next();
                    }
                    // The placeholder text up to the matching brace.
                    let mut depth = 1;
                    for inner in chars.by_ref() {
                        match inner {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        plain.push(inner);
                    }
                }
                Some(next) if next.is_ascii_digit() => {
                    while chars.peek().is_some_and(char::is_ascii_digit) {
                        chars.next();
                    }
                }
                _ => plain.push('$'),
            },
            _ => plain.push(char),
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn snippets_lose_their_tab_stops() {
        assert_eq!(plain_snippet("println!(\"$1\")$0"), "println!(\"\")");
        assert_eq!(
            plain_snippet("fn ${1:name}(${2:args}) {}"),
            "fn name(args) {}"
        );
        assert_eq!(plain_snippet("cost: \\$5"), "cost: $5");
        assert_eq!(plain_snippet("a $ b"), "a $ b");
    }

    #[test]
    fn converts_answers() {
        let texts = HashMap::new();
        let converter = Converter {
            encoding: Encoding::Utf8,
            texts: &texts,
        };
        let path = PathBuf::from("main.rs");
        let hover = lsp::Hover {
            contents: lsp::HoverContents::Array(vec![
                lsp::MarkedString::LanguageString(lsp::LanguageString {
                    language: "rust".to_owned(),
                    value: "fn main()".to_owned(),
                }),
                lsp::MarkedString::String("Entry point.".to_owned()),
            ]),
            range: None,
        };
        assert_eq!(
            converter.hover(&path, hover).unwrap().contents,
            "```rust\nfn main()\n```\n\nEntry point."
        );

        let item = lsp::CompletionItem {
            label: "push".to_owned(),
            kind: Some(lsp::CompletionItemKind::METHOD),
            insert_text: Some("push($1)".to_owned()),
            insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
            ..Default::default()
        };
        let completion = converter.completion(&path, item, true);
        assert_eq!(completion.kind, Some(CompletionKind::Method));
        assert_eq!(completion.edit.new_text, "push()");
        assert!(completion.edit.range.is_none() && completion.resolve.is_some());

        let uri = lsp::Uri::from_str(if cfg!(windows) {
            "file:///C:/a.rs"
        } else {
            "file:///a.rs"
        })
        .unwrap();
        let range = lsp::Range::new(lsp::Position::new(1, 2), lsp::Position::new(1, 4));
        let link = lsp::LocationLink {
            origin_selection_range: None,
            target_uri: uri,
            target_range: range,
            target_selection_range: range,
        };
        let locations = converter.locations(lsp::GotoDefinitionResponse::Link(vec![link]));
        assert_eq!(locations[0].range.start, text::Point::new(1, 2));

        let diagnostic = lsp::Diagnostic {
            message: "mismatched types".to_owned(),
            code: Some(lsp::NumberOrString::String("E0308".to_owned())),
            ..Default::default()
        };
        let diagnostic = converter.diagnostic(&path, diagnostic);
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code.as_deref(), Some("E0308"));
    }
}
