//! The lspf-analysis language server: source analysis as LSP feedback.
//!
//! The analysis it performs today is code health. Every time a document is
//! opened, edited, or saved, the server parses it, scores each function with
//! [`lspf_analysis_core::health`], and publishes a diagnostic for every
//! function below the quality threshold. Hovering over a function reports
//! its four numbers.
//!
//! Further analyses are meant to join it here, each contributing its own
//! diagnostics over the shared document pipeline in [`document`].

pub mod config;
pub mod diagnostics;
pub mod document;
pub mod formats;
pub mod hover;
pub mod status;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use lspf::types::{
    Contents, DidChangeConfigurationNotification as DidChangeConfiguration,
    DidChangeConfigurationParams, DidChangeTextDocumentNotification as DidChangeTextDocument,
    DidChangeTextDocumentParams, DidCloseTextDocumentNotification as DidCloseTextDocument,
    DidCloseTextDocumentParams, DidOpenTextDocumentNotification as DidOpenTextDocument,
    DidOpenTextDocumentParams, DidSaveTextDocumentNotification as DidSaveTextDocument,
    DidSaveTextDocumentParams, Hover, HoverParams, MarkupContent, MarkupKind,
    PublishDiagnosticsParams, Uri,
};
use lspf::{CancellationToken, FileProvider, LspError, Server, ServerContext};
use lspf_analysis_core::health::FileHealth;

use config::Settings;
use document::{analyze, language_for, uri_to_path};
use status::FileHealthNotification;

/// What the server carries across one connection.
///
/// The framework owns the documents; this is the little that is genuinely
/// ours: the settings the client sent, and which documents are open. lspf
/// gives no way to enumerate open documents, so the set is tracked here in
/// order to republish everything when the configuration changes.
#[derive(Default)]
pub struct State {
    /// What `workspace/didChangeConfiguration` last supplied, if anything.
    configured: RwLock<Option<Settings>>,
    /// What to use when the client configures nothing at all.
    fallback: Settings,
    open: RwLock<HashMap<String, Uri>>,
}

impl State {
    /// Resolves the settings in force, most specific source first:
    /// `workspace/didChangeConfiguration`, then the `initializationOptions`
    /// the client sent at startup, then whatever the process started with.
    fn settings(&self, ctx: &ServerContext) -> Settings {
        if let Some(configured) = self.configured.read().ok().and_then(|held| held.clone()) {
            return configured;
        }
        if let Some(settings) = ctx
            .workspace()
            .initialization_options()
            .and_then(Settings::from_value)
        {
            return settings;
        }
        self.fallback.clone()
    }
}

/// One document, scored.
struct Analyzed {
    report: FileHealth,
    /// The exact text the report was computed from. Diagnostic ranges are
    /// resolved against it, because the document may have moved on by the
    /// time the analysis returns.
    text: String,
    /// The version that text came from, captured with it so a concurrent
    /// edit cannot leave these diagnostics labelled with a newer revision.
    version: Option<i32>,
}

/// Parses and scores a document off the protocol task.
async fn report_for(ctx: &ServerContext, uri: &Uri, settings: &Settings) -> Option<Analyzed> {
    let document = ctx.documents().get(uri)?;
    let path = uri_to_path(uri);
    let language = language_for(document.language_id(), &path)?;
    let text = document.text();
    let version = document.version();

    let config = settings.health.clone();
    let source = text.clone();
    // Parsing is synchronous CPU work; keep it off the protocol task.
    let report = tokio::task::spawn_blocking(move || analyze(language, source, &path, &config))
        .await
        .ok()??;
    Some(Analyzed {
        report,
        text,
        version,
    })
}

/// Scores a document, publishes its diagnostics, and reports the file total.
async fn publish(ctx: ServerContext, uri: Uri, settings: Settings) {
    let Some(analyzed) = report_for(&ctx, &uri, &settings).await else {
        return;
    };
    let encoding = ctx.documents().position_encoding();
    let diagnostics = diagnostics::build(&analyzed.report, &analyzed.text, encoding, &settings);

    // The file-level score has no line to sit on, so it travels separately;
    // a client that does not listen for it simply ignores it.
    let _ = ctx
        .client()
        .notify::<FileHealthNotification>(status::summarize(&uri, &analyzed.report, &settings));

    let _ = ctx.publish_diagnostics(PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: analyzed.version,
    });
}

async fn did_open(state: Arc<State>, ctx: ServerContext, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    if let Ok(mut open) = state.open.write() {
        open.insert(uri.as_str().to_string(), uri.clone());
    }
    let settings = state.settings(&ctx);
    publish(ctx, uri, settings).await;
}

async fn did_change(state: Arc<State>, ctx: ServerContext, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.text_document_identifier.uri;
    let settings = state.settings(&ctx);
    publish(ctx, uri, settings).await;
}

async fn did_save(state: Arc<State>, ctx: ServerContext, params: DidSaveTextDocumentParams) {
    let settings = state.settings(&ctx);
    publish(ctx, params.text_document.uri, settings).await;
}

async fn did_close(state: Arc<State>, ctx: ServerContext, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    if let Ok(mut open) = state.open.write() {
        open.remove(uri.as_str());
    }
    // A closed document keeps whatever was last published until the server
    // says otherwise, so clear it explicitly.
    let _ = ctx.publish_diagnostics(PublishDiagnosticsParams {
        uri,
        diagnostics: Vec::new(),
        version: None,
    });
}

async fn did_change_configuration(
    state: Arc<State>,
    ctx: ServerContext,
    params: DidChangeConfigurationParams,
) {
    let Some(settings) = Settings::from_value(&params.settings) else {
        return;
    };
    if let Ok(mut current) = state.configured.write() {
        if current.as_ref() == Some(&settings) {
            return;
        }
        *current = Some(settings.clone());
    }
    // New thresholds mean everything on screen is now stale.
    let open: Vec<Uri> = state
        .open
        .read()
        .map(|open| open.values().cloned().collect())
        .unwrap_or_default();
    for uri in open {
        publish(ctx.clone(), uri, settings.clone()).await;
    }
}

async fn hover(
    state: Arc<State>,
    ctx: ServerContext,
    params: HoverParams,
    _ct: CancellationToken,
) -> Result<Option<Hover>, LspError> {
    let uri = params.text_document_position_params.text_document.uri;
    let settings = state.settings(&ctx);
    let Some(analyzed) = report_for(&ctx, &uri, &settings).await else {
        return Ok(None);
    };
    let encoding = ctx.documents().position_encoding();
    let position = params.text_document_position_params.position;
    let Some((function, range)) =
        hover::function_at(&analyzed.report, &analyzed.text, position, encoding)
    else {
        return Ok(None);
    };

    Ok(Some(Hover {
        contents: Contents::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover::render(function),
        }),
        // The name, and nothing else: the editor highlights this while the
        // hover is up, and other providers contribute their own.
        range: Some(range),
    }))
}

/// Builds the language server.
///
/// `initial` is the floor the server falls back to when the client sends no
/// configuration of its own. A client's `initializationOptions` override it,
/// and `workspace/didChangeConfiguration` overrides those.
pub fn server(file_provider: impl FileProvider, initial: Settings) -> Server<State> {
    let state = State {
        configured: RwLock::new(None),
        fallback: initial,
        open: RwLock::new(HashMap::new()),
    };
    Server::builder(state)
        .file_provider(file_provider)
        .feature(lspf::features::hover(), hover)
        .notification::<DidOpenTextDocument, _, _>(did_open)
        .notification::<DidChangeTextDocument, _, _>(did_change)
        .notification::<DidSaveTextDocument, _, _>(did_save)
        .notification::<DidCloseTextDocument, _, _>(did_close)
        .notification::<DidChangeConfiguration, _, _>(did_change_configuration)
        .build()
        .expect("the static registrations are valid")
}
