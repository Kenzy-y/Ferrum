//! Language support as the UI sees it.
//!
//! - [`LanguagePack`]: what a language declares (files, grammar, server).
//! - [`LanguageRegistry`]: packs by id and by file, filled at startup and
//!   read-only afterwards.
//! - [`LanguageService`]: what the editor asks a language for (hover,
//!   completions, definitions, diagnostics…). The `lsp` crate implements
//!   it over the Language Server Protocol; plugins may implement it
//!   another way. The UI checks [`Capabilities`] and hides what a service
//!   cannot do.
//!
//! The types here are the IDE's own, not the protocol's: positions are
//! [`text::Point`]s (row and byte column), documents are file paths.

mod pack;
mod registry;
mod service;
mod types;

pub use pack::{LanguagePack, ServerContext, ServerFactory};
pub use registry::{LanguageEntry, LanguageRegistry, RegistryError};
pub use service::{
    BoxFuture, Capabilities, CompletionCapabilities, CompletionTrigger, LanguageService,
    ResolveCapability, ServiceError, ServiceEvent,
};
pub use types::{
    CodeLens, Completion, CompletionEdit, CompletionKind, CompletionList, Diagnostic, Hover,
    InlayHint, InlayHintKind, LensAction, Location, ResolveToken, ServerStatus, Severity,
    TextReplacement,
};
