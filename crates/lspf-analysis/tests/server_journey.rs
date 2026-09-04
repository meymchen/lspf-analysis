//! End-to-end protocol journeys over lspf's in-memory Transport.

use std::borrow::Cow;
use std::str::FromStr;
use std::time::Duration;

use bytes::Bytes;
use lspf::testing::ServerJourney;
use lspf::types::{
    DiagnosticSeverity, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, HoverParams, InitializeParams, Position,
    PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    TextDocumentContentChangeWholeDocument, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
};
use lspf::{MemoryFileProvider, RawMessage, RequestId};
use lspf_analysis::config::Settings;

const SIMPLE: &str = "fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";

/// A function bad enough on all three pillars to fail the shipped threshold.
fn tangled() -> String {
    let mut source = String::from("fn tangled(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {\n");
    source.push_str("    let mut total = 0;\n");
    for index in 0..6 {
        source.push_str(&format!(
            "    if a > {index} {{
        if b > {index} {{
            if c > {index} {{
                if d > {index} {{
                    let step{index} = a + b + c + d + e;
                    total = total + step{index};
                }}
            }}
        }}
    }}\n"
        ));
    }
    source.push_str("    total\n}\n");
    source
}

fn notification(method: &'static str, params: &impl serde::Serialize) -> RawMessage {
    RawMessage::Notification {
        method: Cow::Borrowed(method),
        params: Bytes::from(serde_json::to_vec(params).expect("notification params serialize")),
    }
}

fn request(id: i32, method: &'static str, params: &impl serde::Serialize) -> RawMessage {
    RawMessage::Request {
        id: RequestId::Number(id),
        method: Cow::Borrowed(method),
        params: Bytes::from(serde_json::to_vec(params).expect("request params serialize")),
    }
}

async fn diagnostics(journey: &mut ServerJourney) -> PublishDiagnosticsParams {
    let message = tokio::time::timeout(Duration::from_secs(5), journey.peer().recv())
        .await
        .expect("the server publishes diagnostics")
        .expect("the testing Transport stays open");
    let RawMessage::Notification { method, params } = message else {
        panic!("expected diagnostics notification, got {message:?}");
    };
    assert_eq!(method, "textDocument/publishDiagnostics");
    serde_json::from_slice(&params).expect("diagnostics params decode")
}

fn uri() -> Uri {
    Uri::from_str("file:///workspace/src/lib.rs").unwrap()
}

fn open(uri: &Uri, text: &str) -> RawMessage {
    notification(
        "textDocument/didOpen",
        &DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "rust".into(),
                version: 1,
                text: text.into(),
            },
        },
    )
}

async fn start() -> ServerJourney {
    ServerJourney::start(lspf_analysis::server(
        MemoryFileProvider::new(),
        Settings::default(),
    ))
    .await
    .unwrap()
}

/// Starts a connection whose `initialize` carried `initializationOptions`.
async fn start_with_options(options: serde_json::Value) -> ServerJourney {
    let params = InitializeParams {
        initialization_options: Some(options),
        ..InitializeParams::default()
    };
    ServerJourney::start_with(
        lspf_analysis::server(MemoryFileProvider::new(), Settings::default()),
        params,
    )
    .await
    .unwrap()
}

fn message(diagnostic: &lspf::types::Diagnostic) -> String {
    match &diagnostic.message {
        lspf::types::Message::String(text) => text.clone(),
        other => panic!("expected a plain message, got {other:?}"),
    }
}

#[tokio::test]
async fn opening_an_unhealthy_file_warns_on_the_signature_line() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, &tangled())).unwrap();

    let published = diagnostics(&mut journey).await;
    assert_eq!(published.uri, uri);
    assert_eq!(published.version, Some(1));
    assert_eq!(published.diagnostics.len(), 1);

    let diagnostic = &published.diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Warning));
    assert_eq!(diagnostic.source.as_deref(), Some("lspf-analysis"));
    assert_eq!(diagnostic.range.start, Position::new(0, 0));
    assert_eq!(
        diagnostic.range.end.line, 0,
        "the range stays on the signature line"
    );
    let text = message(diagnostic);
    assert!(text.contains("tangled"), "{text}");
    assert!(text.contains("quality"), "{text}");

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn opening_a_healthy_file_reports_nothing() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();

    let published = diagnostics(&mut journey).await;
    assert!(
        published.diagnostics.is_empty(),
        "{:?}",
        published.diagnostics
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn editing_the_problem_away_clears_the_diagnostic() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, &tangled())).unwrap();
    assert_eq!(diagnostics(&mut journey).await.diagnostics.len(), 1);

    journey
        .peer()
        .send(notification(
            "textDocument/didChange",
            &DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    text_document_identifier: TextDocumentIdentifier { uri: uri.clone() },
                    version: 2,
                },
                content_changes: vec![
                    TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                        TextDocumentContentChangeWholeDocument {
                            text: SIMPLE.into(),
                        },
                    ),
                ],
            },
        ))
        .unwrap();

    let published = diagnostics(&mut journey).await;
    assert_eq!(published.version, Some(2));
    assert!(
        published.diagnostics.is_empty(),
        "{:?}",
        published.diagnostics
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn closing_a_document_clears_its_diagnostics() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, &tangled())).unwrap();
    assert_eq!(diagnostics(&mut journey).await.diagnostics.len(), 1);

    journey
        .peer()
        .send(notification(
            "textDocument/didClose",
            &DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            },
        ))
        .unwrap();

    let published = diagnostics(&mut journey).await;
    assert_eq!(published.uri, uri);
    assert!(published.diagnostics.is_empty());

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn raising_the_threshold_republishes_open_documents() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    assert!(diagnostics(&mut journey).await.diagnostics.is_empty());

    journey
        .peer()
        .send(notification(
            "workspace/didChangeConfiguration",
            &DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lspfAnalysis": { "health": { "qualityWarn": 100.0 } }
                }),
            },
        ))
        .unwrap();

    let published = diagnostics(&mut journey).await;
    assert_eq!(published.uri, uri);
    assert_eq!(
        published.diagnostics.len(),
        1,
        "nothing scores a perfect 100%, so the open file now warns"
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn hover_reports_the_four_numbers() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    assert!(diagnostics(&mut journey).await.diagnostics.is_empty());

    journey
        .peer()
        .send(request(
            10,
            "textDocument/hover",
            &HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(0, 4),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        ))
        .unwrap();

    let response = journey.peer().recv().await.unwrap();
    let RawMessage::Response {
        id,
        result: Ok(result),
    } = response
    else {
        panic!("expected a successful hover response, got {response:?}");
    };
    assert_eq!(id, RequestId::Number(10));

    let value: serde_json::Value = serde_json::from_slice(&result).unwrap();
    let markdown = value["contents"]["value"].as_str().expect("markdown hover");
    assert_eq!(value["contents"]["kind"], "markdown");
    for expected in [
        "add",
        "quality",
        "complexity",
        "method length",
        "working memory",
    ] {
        assert!(
            markdown.contains(expected),
            "{expected} missing: {markdown}"
        );
    }

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn an_unsupported_language_is_left_alone() {
    let uri = Uri::from_str("file:///workspace/notes.txt").unwrap();
    let mut journey = start().await;
    journey
        .peer()
        .send(notification(
            "textDocument/didOpen",
            &DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "plaintext".into(),
                    version: 1,
                    text: "just some prose\n".into(),
                },
            },
        ))
        .unwrap();

    // Nothing is published at all, so the next thing the peer sees is the
    // shutdown handshake rather than a diagnostics notification.
    let quiet = tokio::time::timeout(Duration::from_millis(200), journey.peer().recv()).await;
    assert!(quiet.is_err(), "unexpected traffic: {quiet:?}");

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn initialization_options_configure_the_first_analysis() {
    // Without this, the very first publish after didOpen would silently use
    // the defaults and ignore what the client asked for at startup.
    let uri = uri();
    let mut journey = start_with_options(serde_json::json!({
        "lspfAnalysis": { "health": { "qualityWarn": 100.0 } }
    }))
    .await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();

    let published = diagnostics(&mut journey).await;
    assert_eq!(
        published.diagnostics.len(),
        1,
        "the startup threshold should already be in force"
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn did_change_configuration_overrides_initialization_options() {
    let uri = uri();
    let mut journey = start_with_options(serde_json::json!({
        "lspfAnalysis": { "health": { "qualityWarn": 100.0 } }
    }))
    .await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    assert_eq!(diagnostics(&mut journey).await.diagnostics.len(), 1);

    journey
        .peer()
        .send(notification(
            "workspace/didChangeConfiguration",
            &DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lspfAnalysis": { "health": { "qualityWarn": 1.0 } }
                }),
            },
        ))
        .unwrap();

    let published = diagnostics(&mut journey).await;
    assert!(
        published.diagnostics.is_empty(),
        "the later threshold wins: {:?}",
        published.diagnostics
    );

    journey.finish().await.unwrap();
}
