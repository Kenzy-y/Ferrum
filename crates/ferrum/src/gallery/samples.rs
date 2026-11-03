//! Sample data shown by the gallery. These are example file names, shortcut
//! labels and code, not interface copy, so they are not localized.

/// Kinds of syntax tokens in [`CODE_SAMPLE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Syntax {
    Plain,
    Punctuation,
    Keyword,
    Type,
    Function,
    Macro,
    Attribute,
    String,
    Number,
    Constant,
    Lifetime,
    Comment,
    DocComment,
    InlayHint,
}

use Syntax::{
    Attribute as A, Comment as C, Constant as K, DocComment as D, Function as F, InlayHint as H,
    Keyword as W, Lifetime as L, Macro as M, Number as N, Plain as P, Punctuation as U,
    String as S, Type as T,
};

/// A Rust snippet that uses every syntax token, one `Vec` of spans per line.
pub(crate) const CODE_SAMPLE: &[&[(Syntax, &str)]] = &[
    &[(D, "/// A small tool for building great things.")],
    &[(A, "#[derive(Parser, Debug)]")],
    &[(W, "struct "), (T, "Cli"), (U, "<"), (L, "'a"), (U, "> {")],
    &[
        (P, "    config: "),
        (U, "&"),
        (L, "'a "),
        (T, "str"),
        (U, ","),
    ],
    &[(U, "}")],
    &[],
    &[
        (W, "const "),
        (K, "MAX_RETRIES"),
        (U, ": "),
        (T, "u32"),
        (U, " = "),
        (N, "3"),
        (U, ";"),
    ],
    &[],
    &[
        (W, "fn "),
        (F, "main"),
        (U, "() -> "),
        (T, "Result"),
        (U, "<()> {"),
    ],
    &[(C, "    // Load the configuration file.")],
    &[
        (W, "    let "),
        (P, "cli"),
        (H, ": Cli"),
        (U, " = "),
        (T, "Cli"),
        (U, "::"),
        (F, "parse"),
        (U, "();"),
    ],
    &[
        (W, "    let "),
        (P, "path"),
        (U, " = "),
        (S, "\"forge.toml\""),
        (U, ";"),
    ],
    &[
        (M, "    println!"),
        (U, "("),
        (S, "\"retries: {}\""),
        (U, ", "),
        (K, "MAX_RETRIES"),
        (U, ");"),
    ],
    &[(F, "    Ok"), (U, "(())")],
    &[(U, "}")],
];

/// Index of the line drawn as the current line.
pub(crate) const ACTIVE_LINE: usize = 10;

/// File names used in tabs and list rows.
pub(crate) const FILE_MAIN: &str = "main.rs";
pub(crate) const FILE_LIB: &str = "lib.rs";
pub(crate) const FILE_CARGO: &str = "Cargo.toml";
pub(crate) const FOLDER_SRC: &str = "src";
pub(crate) const FOLDER_TARGET: &str = "target";

/// Letter shown as the Rust file icon.
pub(crate) const RUST_FILE_LETTER: &str = "R";

/// Keyboard shortcut labels.
pub(crate) const SHORTCUT_PALETTE: &str = "Ctrl Shift P";
pub(crate) const SHORTCUT_SEARCH: &str = "Ctrl K";

/// Git status letters.
pub(crate) const STATUS_MODIFIED: &str = "M";
pub(crate) const STATUS_ADDED: &str = "A";
pub(crate) const STATUS_DELETED: &str = "D";

/// Count shown in badges.
pub(crate) const SAMPLE_COUNT: &str = "4";
