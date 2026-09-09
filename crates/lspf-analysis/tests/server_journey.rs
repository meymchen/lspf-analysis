//! End-to-end protocol journeys over lspf's in-memory Transport.

use std::borrow::Cow;
use std::str::FromStr;
use std::time::Duration;

use bytes::Bytes;
use lspf::testing::ServerJourney;
use lspf::types::{
    Code, DiagnosticSeverity, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, HoverParams, InitializeParams, Position,
    PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    TextDocumentContentChangeWholeDocument, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
};
use lspf::{MemoryFileProvider, RawMessage, RequestId};
use lspf_analysis::config::Settings;
use lspf_analysis::functions::{FunctionHealthParams, FunctionHealthResult};
use lspf_analysis::status::FileHealthParams;

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

/// Waits for the next notification named `wanted`, skipping the others.
///
/// Every analysis publishes diagnostics and a file summary, and a test that
/// cares about one of them should not have to care which arrives first.
async fn next_notification(journey: &mut ServerJourney, wanted: &str) -> Bytes {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), journey.peer().recv())
            .await
            .unwrap_or_else(|_| panic!("the server sends {wanted}"))
            .expect("the testing Transport stays open");
        let RawMessage::Notification { method, params } = message else {
            panic!("expected a notification, got {message:?}");
        };
        if method == wanted {
            return params;
        }
    }
}

async fn diagnostics(journey: &mut ServerJourney) -> PublishDiagnosticsParams {
    let params = next_notification(journey, "textDocument/publishDiagnostics").await;
    serde_json::from_slice(&params).expect("diagnostics params decode")
}

async fn file_health(journey: &mut ServerJourney) -> FileHealthParams {
    let params = next_notification(journey, "lspfAnalysis/fileHealth").await;
    serde_json::from_slice(&params).expect("file health params decode")
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

fn java_uri() -> Uri {
    Uri::from_str("file:///workspace/src/Wide.java").unwrap()
}

fn open_java(uri: &Uri, text: &str) -> RawMessage {
    notification(
        "textDocument/didOpen",
        &DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "java".into(),
                version: 1,
                text: text.into(),
            },
        },
    )
}

/// A Java class wide enough to fail the class pillar.
fn wide_java_class() -> String {
    let mut source = String::from("public class Wide {\n");
    for index in 0..40 {
        source.push_str(&format!("    public int field{index};\n"));
        source.push_str(&format!(
            "    public int get{index}() {{ return field{index}; }}\n"
        ));
    }
    source.push_str("}\n");
    source
}

/// A Java method behind annotations, whose declaration node therefore
/// starts a line above its own name.
const ANNOTATED_JAVA: &str = "public class Ann {
    @Override
    @Nullable
    public int plain(int a, int b) {
        return a + b;
    }
}
";

async fn start() -> ServerJourney {
    ServerJourney::start(lspf_analysis::server(
        MemoryFileProvider::new(),
        Settings::default(),
    ))
    .await
    .unwrap()
}

fn edit(uri: &Uri, version: i32, text: &str) -> RawMessage {
    notification(
        "textDocument/didChange",
        &DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                text_document_identifier: TextDocumentIdentifier { uri: uri.clone() },
                version,
            },
            content_changes: vec![
                TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(
                    TextDocumentContentChangeWholeDocument { text: text.into() },
                ),
            ],
        },
    )
}

async fn no_publication(journey: &mut ServerJourney, milliseconds: u64) {
    assert!(
        tokio::time::timeout(Duration::from_millis(milliseconds), journey.peer().recv())
            .await
            .is_err(),
        "an edit must not publish before its quiet period, or after cancellation"
    );
}

#[tokio::test]
async fn edits_debounce_for_500ms_from_the_last_change() {
    let mut journey = start().await;
    let uri = uri();
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    diagnostics(&mut journey).await;
    journey.peer().send(edit(&uri, 2, &tangled())).unwrap();
    no_publication(&mut journey, 300).await;
    let last_edit = tokio::time::Instant::now();
    journey.peer().send(edit(&uri, 3, SIMPLE)).unwrap();
    no_publication(&mut journey, 300).await;
    let published = diagnostics(&mut journey).await;
    assert!(last_edit.elapsed() >= Duration::from_millis(500));
    assert_eq!(published.version, Some(3));
    assert!(published.diagnostics.is_empty());
    no_publication(&mut journey, 550).await;
    journey.finish().await.unwrap();
}

#[tokio::test]
async fn closing_cancels_pending_edit_publication() {
    let mut journey = start().await;
    let uri = uri();
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    diagnostics(&mut journey).await;
    journey.peer().send(edit(&uri, 2, &tangled())).unwrap();
    no_publication(&mut journey, 100).await;
    journey
        .peer()
        .send(notification(
            "textDocument/didClose",
            &DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
            },
        ))
        .unwrap();
    assert!(diagnostics(&mut journey).await.diagnostics.is_empty());
    no_publication(&mut journey, 550).await;
    journey.finish().await.unwrap();
}

#[tokio::test]
async fn saving_flushes_pending_edits_without_a_second_publication() {
    let mut journey = start().await;
    let uri = uri();
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    diagnostics(&mut journey).await;
    journey.peer().send(edit(&uri, 2, &tangled())).unwrap();
    no_publication(&mut journey, 100).await;
    journey
        .peer()
        .send(notification(
            "textDocument/didSave",
            &serde_json::json!({
                "textDocument": {"uri": uri.as_str()}
            }),
        ))
        .unwrap();
    let published = tokio::time::timeout(Duration::from_millis(300), diagnostics(&mut journey))
        .await
        .unwrap();
    assert_eq!(published.version, Some(2));
    assert_eq!(published.diagnostics.len(), 1);
    no_publication(&mut journey, 550).await;
    journey.finish().await.unwrap();
}

#[tokio::test]
async fn hover_and_panel_requests_wait_for_the_latest_edit() {
    let mut journey = start().await;
    let uri = uri();
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    diagnostics(&mut journey).await;
    journey.peer().send(edit(&uri, 2, &tangled())).unwrap();
    journey
        .peer()
        .send(request(
            81,
            "textDocument/hover",
            &serde_json::json!({
                "textDocument": {"uri": uri.as_str()}, "position": {"line":0, "character":3}
            }),
        ))
        .unwrap();
    journey
        .peer()
        .send(request(
            82,
            "lspfAnalysis/functionHealth",
            &FunctionHealthParams { uri: uri.clone() },
        ))
        .unwrap();
    no_publication(&mut journey, 200).await;
    let last_edit = tokio::time::Instant::now();
    journey
        .peer()
        .send(edit(&uri, 3, "fn newest() {}\n"))
        .unwrap();
    no_publication(&mut journey, 300).await;
    let mut responses = 0;
    let mut published = false;
    while responses < 2 || !published {
        let message = tokio::time::timeout(Duration::from_secs(5), journey.peer().recv())
            .await
            .unwrap()
            .unwrap();
        assert!(last_edit.elapsed() >= Duration::from_millis(500));
        match message {
            RawMessage::Response {
                result: Ok(result),
                ..
            } => {
                assert!(String::from_utf8_lossy(&result).contains("newest"));
                responses += 1;
            }
            RawMessage::Notification { method, params }
                if method == "textDocument/publishDiagnostics" =>
            {
                let diagnostic: PublishDiagnosticsParams = serde_json::from_slice(&params).unwrap();
                assert_eq!(diagnostic.version, Some(3));
                published = true;
            }
            RawMessage::Notification { .. } => {}
            other => panic!("unexpected message: {other:?}"),
        }
    }
    journey.finish().await.unwrap();
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
async fn hover_reports_every_pillar() {
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
        "control flow",
        "cognitive complexity",
        "cyclomatic complexity",
        "size",
        "statements",
        "vocabulary load",
        "working memory",
        "Halstead difficulty",
        "interface",
        "parameters",
    ] {
        assert!(
            markdown.contains(expected),
            "{expected} missing: {markdown}"
        );
    }

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn a_wide_java_class_is_diagnosed_and_hovers_its_own_pillar() {
    let uri = java_uri();
    let mut journey = start().await;
    journey
        .peer()
        .send(open_java(&uri, &wide_java_class()))
        .unwrap();

    let published = diagnostics(&mut journey).await;
    let class = published
        .diagnostics
        .iter()
        .find(|d| d.code == Some(Code::String("class-quality".into())))
        .unwrap_or_else(|| panic!("no class diagnostic: {:?}", published.diagnostics));
    assert_eq!(class.range.start.line, 0, "the `public class` line");
    assert!(message(class).contains("`Wide`"), "{}", message(class));

    // `public class Wide`: the name starts at column 13.
    journey
        .peer()
        .send(request(
            11,
            "textDocument/hover",
            &HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(0, 13),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        ))
        .unwrap();

    let response = journey.peer().recv().await.unwrap();
    let RawMessage::Response {
        id: _,
        result: Ok(result),
    } = response
    else {
        panic!("expected a successful hover response, got {response:?}");
    };
    let value: serde_json::Value = serde_json::from_slice(&result).unwrap();
    let markdown = value["contents"]["value"].as_str().expect("markdown hover");
    for expected in [
        "Wide",
        "class design",
        "weighted methods",
        "public methods",
        "public attributes",
    ] {
        assert!(
            markdown.contains(expected),
            "{expected} missing: {markdown}"
        );
    }

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn an_annotated_java_method_hovers_on_its_name() {
    let uri = java_uri();
    let mut journey = start().await;
    journey
        .peer()
        .send(open_java(&uri, ANNOTATED_JAVA))
        .unwrap();
    let _ = diagnostics(&mut journey).await;

    // `    public int plain(int a, int b) {` is line 4; the name is at
    // column 15, two annotations below where the method's space starts.
    journey
        .peer()
        .send(request(
            12,
            "textDocument/hover",
            &HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(3, 15),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        ))
        .unwrap();

    let response = journey.peer().recv().await.unwrap();
    let RawMessage::Response {
        id: _,
        result: Ok(result),
    } = response
    else {
        panic!("expected a successful hover response, got {response:?}");
    };
    let value: serde_json::Value = serde_json::from_slice(&result).unwrap();
    assert!(
        !value.is_null(),
        "an annotated method should still answer on its name"
    );
    let markdown = value["contents"]["value"].as_str().expect("markdown hover");
    assert!(markdown.contains("plain"), "{markdown}");
    // The highlighted range is the name, on the signature line.
    assert_eq!(value["range"]["start"]["line"], 3);
    assert_eq!(value["range"]["start"]["character"], 15);

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn a_chinese_client_gets_chinese_display_text() {
    // The editor's display language reaches the server as a setting, and
    // decides the text a person reads — but nothing a client keys off.
    let uri = uri();
    let mut journey = start_with_options(serde_json::json!({
        "lspfAnalysis": { "locale": "zh-cn", "health": { "qualityWarn": 100.0 } }
    }))
    .await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();

    // The payloads keep their English vocabulary whatever the reader sees.
    // The summary is read first because it is sent first, and reading past a
    // notification discards it.
    let health = file_health(&mut journey).await;
    assert_eq!(health.grade, "excellent");
    assert_eq!(health.worst[0].weakest_pillar, "interface");

    let published = diagnostics(&mut journey).await;
    let text = message(&published.diagnostics[0]);
    assert!(text.starts_with("函数 `add`："), "{text}");
    assert_eq!(
        published.diagnostics[0].source.as_deref(),
        Some("lspf-analysis"),
        "the source is an identifier, not display text"
    );

    let value = function_health(&mut journey, 40, &uri).await;
    let detail: FunctionHealthResult = serde_json::from_value(value).unwrap();
    assert_eq!(detail.functions[0].grade, "excellent");
    assert_eq!(detail.functions[0].pillars[0].name, "control flow");
    assert_eq!(
        detail.functions[0].pillars[0].measures[0].name,
        "cognitive complexity"
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn hover_off_the_name_leaves_the_editor_alone() {
    // Metrics supplement the editor's own hover, so they must not answer for
    // a parameter or a line inside the body.
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    assert!(diagnostics(&mut journey).await.diagnostics.is_empty());

    for (id, position, what) in [
        (20, Position::new(0, 0), "the `fn` keyword"),
        (21, Position::new(0, 7), "a parameter"),
        (22, Position::new(1, 4), "the body"),
    ] {
        journey
            .peer()
            .send(request(
                id,
                "textDocument/hover",
                &HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri: uri.clone() },
                        position,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ))
            .unwrap();

        let response = journey.peer().recv().await.unwrap();
        let RawMessage::Response {
            result: Ok(result), ..
        } = response
        else {
            panic!("expected a successful hover response, got {response:?}");
        };
        let value: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert!(value.is_null(), "{what} answered with {value}");
    }

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn every_analysis_reports_the_file_total() {
    let uri = uri();
    let mut journey = start().await;
    journey.peer().send(open(&uri, &tangled())).unwrap();

    let health = file_health(&mut journey).await;
    assert_eq!(health.uri, uri);
    assert_eq!(health.functions, 1);
    assert_eq!(health.below, 1, "the one function is below the threshold");
    assert_eq!(health.grade, "poor");
    assert_eq!(health.bands.poor, 1, "{:?}", health.bands);
    let worst = health
        .worst
        .first()
        .expect("a file with a function lists one");
    assert_eq!(worst.name, "tangled");
    assert_eq!(worst.line, 1);
    assert_eq!(worst.weakest_pillar, "control flow");

    // The summary travels alongside the diagnostics, not instead of them.
    assert_eq!(diagnostics(&mut journey).await.diagnostics.len(), 1);

    journey.finish().await.unwrap();
}

/// Sends `lspfAnalysis/functionHealth` and decodes what comes back.
async fn function_health(journey: &mut ServerJourney, id: i32, uri: &Uri) -> serde_json::Value {
    journey
        .peer()
        .send(request(
            id,
            "lspfAnalysis/functionHealth",
            &FunctionHealthParams { uri: uri.clone() },
        ))
        .unwrap();

    let response = journey.peer().recv().await.unwrap();
    let RawMessage::Response {
        id: responded,
        result: Ok(result),
    } = response
    else {
        panic!("expected a successful functionHealth response, got {response:?}");
    };
    assert_eq!(responded, RequestId::Number(id));
    serde_json::from_slice(&result).expect("function health result decode")
}

#[tokio::test]
async fn function_health_reports_every_function_in_detail() {
    // What a client needs to draw its own list: the numbers, not a rendering
    // of them.
    let uri = uri();
    let mut journey = start().await;
    let source = format!("{SIMPLE}\n{}", tangled());
    journey.peer().send(open(&uri, &source)).unwrap();
    assert_eq!(diagnostics(&mut journey).await.diagnostics.len(), 1);

    let value = function_health(&mut journey, 30, &uri).await;
    let detail: FunctionHealthResult =
        serde_json::from_value(value).expect("the wire types decode the wire format");
    assert_eq!(detail.uri, uri);

    let names: Vec<&str> = detail
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    assert_eq!(names, ["add", "tangled"], "source order, not worst first");

    let tangled = &detail.functions[1];
    assert_eq!(tangled.grade, "poor");
    assert_eq!(tangled.weakest_pillar, "control flow");
    assert!(tangled.start_line < tangled.end_line);
    assert_eq!(
        tangled
            .pillars
            .iter()
            .map(|pillar| pillar.name.as_str())
            .collect::<Vec<_>>(),
        ["control flow", "size", "vocabulary load", "interface"]
    );
    let parameters = tangled
        .pillars
        .iter()
        .flat_map(|pillar| pillar.measures.iter())
        .find(|measure| measure.name == "parameters")
        .expect("the interface pillar reports its measure");
    assert_eq!(parameters.value, 5.0, "`tangled` takes five");
    assert_eq!(parameters.threshold, 4.0, "against the shipped default");

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn function_health_follows_the_configured_thresholds() {
    let uri = uri();
    let mut journey = start_with_options(serde_json::json!({
        "lspfAnalysis": { "health": { "parametersThreshold": 1.0 } }
    }))
    .await;
    journey.peer().send(open(&uri, SIMPLE)).unwrap();
    assert!(diagnostics(&mut journey).await.diagnostics.is_empty());

    let value = function_health(&mut journey, 31, &uri).await;
    let detail: FunctionHealthResult = serde_json::from_value(value).unwrap();
    let interface = detail.functions[0]
        .pillars
        .iter()
        .find(|pillar| pillar.name == "interface")
        .expect("every function carries every pillar");
    assert_eq!(interface.measures[0].threshold, 1.0);
    assert_eq!(
        detail.functions[0].weakest_pillar, "interface",
        "the tightened threshold is what now scores worst"
    );

    journey.finish().await.unwrap();
}

#[tokio::test]
async fn function_health_for_an_unanalyzed_document_answers_nothing() {
    // A client can ask about any URI it has; one the server never scored is
    // answered with null rather than an error, as hover answers a position
    // it has nothing to say about.
    let mut journey = start().await;
    let unopened = Uri::from_str("file:///workspace/src/never-opened.rs").unwrap();
    let value = function_health(&mut journey, 32, &unopened).await;
    assert!(value.is_null(), "{value}");

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
