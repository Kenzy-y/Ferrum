//! Requests about a document: positions out, answers back in the IDE's
//! terms. Answers are converted where the future is awaited.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use language::{
    BoxFuture, Capabilities, CodeLens, Completion, CompletionList, CompletionTrigger, Hover,
    InlayHint, Location, ResolveToken, ServiceError,
};
use lsp_types as lsp;
use lsp_types::request::{
    CodeLensRequest, CodeLensResolve, Completion as CompletionRequest, GotoDefinition,
    HoverRequest, InlayHintRequest, ResolveCompletionItem,
};
use text::Point;
use text::ropey::Rope;

use super::Inner;
use crate::client::Client;
use crate::convert::Converter;
use crate::sync::{Encoding, to_position, to_range};
use crate::uri::path_to_uri;

/// What a request about one document needs.
struct Prepared {
    client: Client,
    encoding: Encoding,
    document: lsp::TextDocumentIdentifier,
    text: Rope,
}

impl Prepared {
    fn position(&self, point: Point) -> lsp::TextDocumentPositionParams {
        lsp::TextDocumentPositionParams {
            text_document: self.document.clone(),
            position: to_position(point, &self.text, self.encoding),
        }
    }
}

fn prepare(
    inner: &Inner,
    path: &Path,
    supported: impl FnOnce(&Capabilities) -> bool,
) -> Result<Prepared, ServiceError> {
    let text = super::lock(&inner.documents)
        .get(path)
        .map(|document| document.text.rope().clone())
        .ok_or(ServiceError::NotRunning)?;
    let state = inner.state();
    let running = state.running.as_ref().ok_or(ServiceError::NotRunning)?;
    if !supported(&state.capabilities) {
        return Err(ServiceError::Unsupported);
    }
    let uri = path_to_uri(path).ok_or(ServiceError::Unsupported)?;
    Ok(Prepared {
        client: running.client.clone(),
        encoding: running.encoding,
        document: lsp::TextDocumentIdentifier::new(uri),
        text,
    })
}

fn failed<T: Send + 'static>(error: ServiceError) -> BoxFuture<Result<T, ServiceError>> {
    Box::pin(async move { Err(error) })
}

/// Runs `convert` on the answer with the texts of the open documents.
fn converted<T, R>(
    inner: &Arc<Inner>,
    encoding: Encoding,
    request: impl Future<Output = Result<R, ServiceError>> + Send + 'static,
    convert: impl FnOnce(&Converter<'_>, &Inner, R) -> T + Send + 'static,
) -> BoxFuture<Result<T, ServiceError>>
where
    T: Send + 'static,
    R: Send + 'static,
{
    let inner = inner.clone();
    Box::pin(async move {
        let answer = request.await?;
        let texts = inner.texts();
        let converter = Converter {
            encoding,
            texts: &texts,
        };
        Ok(convert(&converter, &inner, answer))
    })
}

pub(super) fn hover(
    inner: &Arc<Inner>,
    path: &Path,
    point: Point,
) -> BoxFuture<Result<Option<Hover>, ServiceError>> {
    let prepared = match prepare(inner, path, |capabilities| capabilities.hover) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared.client.request::<HoverRequest>(&lsp::HoverParams {
        text_document_position_params: prepared.position(point),
        work_done_progress_params: lsp::WorkDoneProgressParams::default(),
    });
    let path = path.to_path_buf();
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, _, hover| hover.and_then(|hover| converter.hover(&path, hover)),
    )
}

pub(super) fn completions(
    inner: &Arc<Inner>,
    path: &Path,
    point: Point,
    trigger: CompletionTrigger,
) -> BoxFuture<Result<CompletionList, ServiceError>> {
    let mut resolvable = false;
    let prepared = match prepare(inner, path, |capabilities| {
        resolvable = capabilities
            .completion
            .as_ref()
            .is_some_and(|completion| completion.resolve.resolve);
        capabilities.completion.is_some()
    }) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let context = match trigger {
        CompletionTrigger::Invoked => lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        },
        CompletionTrigger::Character(character) => lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(character),
        },
        CompletionTrigger::Incomplete => lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS,
            trigger_character: None,
        },
    };
    let request = prepared
        .client
        .request::<CompletionRequest>(&lsp::CompletionParams {
            text_document_position: prepared.position(point),
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
            context: Some(context),
        });
    let path = path.to_path_buf();
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, _, response| {
            response.map_or_else(CompletionList::default, |response| {
                let mut list = converter.completions(&path, response, resolvable);
                // Keep the path with each item, to resolve it later.
                for item in &mut list.items {
                    if let Some(token) = &item.resolve
                        && let Some(original) = token.0.downcast_ref::<lsp::CompletionItem>()
                    {
                        item.resolve =
                            Some(ResolveToken(Arc::new((path.clone(), original.clone()))));
                    }
                }
                list
            })
        },
    )
}

pub(super) fn resolve_completion(
    inner: &Arc<Inner>,
    completion: Completion,
) -> BoxFuture<Result<Completion, ServiceError>> {
    let Some((path, item)) = completion
        .resolve
        .as_ref()
        .and_then(|token| token.0.downcast_ref::<(PathBuf, lsp::CompletionItem)>())
        .cloned()
    else {
        return Box::pin(async move { Ok(completion) });
    };
    let prepared = match prepare(inner, &path, |capabilities| {
        capabilities
            .completion
            .as_ref()
            .is_some_and(|completion| completion.resolve.resolve)
    }) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared.client.request::<ResolveCompletionItem>(&item);
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, _, item| {
            let mut resolved = converter.completion(&path, item, false);
            // The edit is the one the list showed; servers resolve details.
            resolved.edit = completion.edit;
            resolved
        },
    )
}

pub(super) fn definition(
    inner: &Arc<Inner>,
    path: &Path,
    point: Point,
) -> BoxFuture<Result<Vec<Location>, ServiceError>> {
    let prepared = match prepare(inner, path, |capabilities| capabilities.definition) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared
        .client
        .request::<GotoDefinition>(&lsp::GotoDefinitionParams {
            text_document_position_params: prepared.position(point),
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
        });
    converted(
        inner,
        prepared.encoding,
        request,
        |converter, _, response| {
            response.map_or_else(Vec::new, |response| converter.locations(response))
        },
    )
}

pub(super) fn inlay_hints(
    inner: &Arc<Inner>,
    path: &Path,
    range: Range<Point>,
) -> BoxFuture<Result<Vec<InlayHint>, ServiceError>> {
    let prepared = match prepare(inner, path, |capabilities| capabilities.inlay_hints) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared
        .client
        .request::<InlayHintRequest>(&lsp::InlayHintParams {
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            text_document: prepared.document.clone(),
            range: to_range(&range, &prepared.text, prepared.encoding),
        });
    let path = path.to_path_buf();
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, _, hints| {
            hints
                .into_iter()
                .flatten()
                .map(|hint| converter.inlay_hint(&path, hint))
                .collect()
        },
    )
}

pub(super) fn code_lenses(
    inner: &Arc<Inner>,
    path: &Path,
) -> BoxFuture<Result<Vec<CodeLens>, ServiceError>> {
    let prepared = match prepare(inner, path, |capabilities| capabilities.code_lens.is_some()) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared
        .client
        .request::<CodeLensRequest>(&lsp::CodeLensParams {
            text_document: prepared.document.clone(),
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
        });
    let path = path.to_path_buf();
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, inner, lenses| {
            lenses
                .into_iter()
                .flatten()
                .map(|lens| {
                    let mut lens = converter.code_lens(&path, lens, inner.adapter.as_ref());
                    if let Some(token) = &lens.resolve
                        && let Some(original) = token.0.downcast_ref::<lsp::CodeLens>()
                    {
                        lens.resolve =
                            Some(ResolveToken(Arc::new((path.clone(), original.clone()))));
                    }
                    lens
                })
                .collect()
        },
    )
}

pub(super) fn resolve_code_lens(
    inner: &Arc<Inner>,
    lens: CodeLens,
) -> BoxFuture<Result<CodeLens, ServiceError>> {
    let Some((path, original)) = lens
        .resolve
        .as_ref()
        .and_then(|token| token.0.downcast_ref::<(PathBuf, lsp::CodeLens)>())
        .cloned()
    else {
        return Box::pin(async move { Ok(lens) });
    };
    let prepared = match prepare(inner, &path, |capabilities| {
        capabilities
            .code_lens
            .is_some_and(|code_lens| code_lens.resolve)
    }) {
        Ok(prepared) => prepared,
        Err(error) => return failed(error),
    };
    let request = prepared.client.request::<CodeLensResolve>(&original);
    converted(
        inner,
        prepared.encoding,
        request,
        move |converter, inner, resolved| {
            converter.code_lens(&path, resolved, inner.adapter.as_ref())
        },
    )
}
