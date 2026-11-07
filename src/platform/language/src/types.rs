//! What language services answer with.

use std::any::Any;
use std::fmt;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use text::Point;

/// How serious a diagnostic is; the order is from most to least.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The code is wrong.
    Error,
    /// The code is suspicious.
    Warning,
    /// Something worth knowing.
    Information,
    /// A suggestion, usually not shown in lists.
    Hint,
}

/// A problem in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Where the problem is.
    pub range: Range<Point>,
    /// How serious it is.
    pub severity: Severity,
    /// What is wrong; may span lines.
    pub message: String,
    /// Who reported it (`rustc`, `clippy`).
    pub source: Option<String>,
    /// The problem's code (`E0308`).
    pub code: Option<String>,
}

/// Information about the symbol at a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Markdown.
    pub contents: String,
    /// The text the information is about.
    pub range: Option<Range<Point>>,
}

/// A place in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The file.
    pub path: PathBuf,
    /// The range in it.
    pub range: Range<Point>,
}

/// Replace `range` with `new_text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReplacement {
    /// The text to replace, in the document as it is now.
    pub range: Range<Point>,
    /// The new text.
    pub new_text: String,
}

/// What a completion inserts; shown as an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "the variants are self-describing")]
pub enum CompletionKind {
    Function,
    Method,
    Field,
    Variable,
    Constant,
    Struct,
    Enum,
    EnumMember,
    Interface,
    Module,
    Keyword,
    Snippet,
    TypeParameter,
    Text,
}

/// Data a service keeps with an item it may later resolve (fill in the
/// details of). Opaque to everyone else.
#[derive(Clone)]
pub struct ResolveToken(pub Arc<dyn Any + Send + Sync>);

impl fmt::Debug for ResolveToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResolveToken")
    }
}

impl PartialEq for ResolveToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ResolveToken {}

/// One completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// What the list shows.
    pub label: String,
    /// What it is.
    pub kind: Option<CompletionKind>,
    /// A short description (a signature, a type).
    pub detail: Option<String>,
    /// Shown right after the label, dimmed (`(…)`, `(use std::fmt)`).
    pub label_detail: Option<String>,
    /// Shown at the right of the row instead of `detail` (`fn(&str)`,
    /// `macro_rules! print`).
    pub description: Option<String>,
    /// Markdown documentation, often only after resolving.
    pub documentation: Option<String>,
    /// Text the typed word is matched against; the label if `None`.
    pub filter_text: Option<String>,
    /// Text the list is sorted by; the label if `None`.
    pub sort_text: Option<String>,
    /// The main edit. Without a range it inserts at the word being
    /// typed.
    pub edit: CompletionEdit,
    /// Edits elsewhere (an import), applied with the main one.
    pub additional_edits: Vec<TextReplacement>,
    /// Whether the service can fill in more details.
    pub resolve: Option<ResolveToken>,
}

/// The main edit of a completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionEdit {
    /// Replaced text; `None`: the word before the cursor.
    pub range: Option<Range<Point>>,
    /// Inserted text (a snippet's text without its markup).
    pub new_text: String,
    /// Where the cursor goes in `new_text`, in order; empty for plain
    /// text (the cursor goes after it).
    pub tab_stops: Vec<crate::TabStop>,
}

/// Completions at a position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionList {
    /// The items, in the service's order.
    pub items: Vec<Completion>,
    /// Typing more may bring other items: ask again instead of only
    /// filtering.
    pub incomplete: bool,
}

/// What an inlay hint stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// The type of a binding.
    Type,
    /// The name of a parameter.
    Parameter,
    /// Anything else.
    Other,
}

/// Text shown inside a line that is not part of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// Where it is shown: before the character at this point.
    pub position: Point,
    /// What is shown.
    pub label: String,
    /// What it stands for.
    pub kind: InlayHintKind,
    /// Leave a space before it.
    pub padding_left: bool,
    /// Leave a space after it.
    pub padding_right: bool,
}

/// What clicking a code lens does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LensAction {
    /// Show these places (implementations, references).
    ShowLocations(Vec<Location>),
    /// Something the IDE cannot do yet; such lenses are not shown.
    Unsupported,
}

/// An actionable note shown with a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    /// The code it is about.
    pub range: Range<Point>,
    /// Its text, once known; `None` until resolved.
    pub title: Option<String>,
    /// What a click does.
    pub action: LensAction,
    /// Set when the service can resolve the title and action later.
    pub resolve: Option<ResolveToken>,
}

/// State of a language service, shown in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    /// Starting up.
    Starting,
    /// Working through the project; answers may be incomplete.
    Indexing {
        /// What it does (`Indexing`, `cargo check`).
        message: Option<String>,
        /// Progress, 0 to 100, if known.
        percentage: Option<u32>,
    },
    /// Ready.
    Ready,
    /// Running, but something is wrong (the project does not load).
    Warning {
        /// What is wrong.
        message: String,
    },
    /// Stopped after an error.
    Failed {
        /// What went wrong: the server's last error output, or the error.
        message: String,
        /// Id of an i18n message saying how to fix it, when the pack
        /// recognizes the failure.
        hint: Option<String>,
    },
    /// The server program does not exist.
    NotFound {
        /// The program that was looked for.
        program: String,
    },
    /// Shut down.
    Stopped,
}
