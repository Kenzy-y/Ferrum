//! Project-wide search in the sidebar (`search.svg`). The view asks for
//! searches with [`SearchRequest`]; the application runs them with the
//! `search` crate and feeds the results back.

mod model;
mod view;

pub use self::view::{OpenMatch, ReplaceRequest, SearchRequest, SearchView};
