use serde::Deserialize;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Deserialize)]
struct HadolintIssue {
    line: u32,
    column: u32,
    level: String,
    code: String,
    message: String,
}

struct Backend {
    client: Client,
}

impl Backend {
    async fn lint(&self, uri: Url, text: String) {
        match run_hadolint(&text).await {
            Ok(issues) => {
                let diagnostics = issues.into_iter().map(to_diagnostic).collect();
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
            }
            Err(err) => {
                self.client
                    .log_message(MessageType::ERROR, format!("hadolint failed: {err}"))
                    .await;
            }
        }
    }
}

async fn run_hadolint(text: &str) -> std::io::Result<Vec<HadolintIssue>> {
    let mut child = Command::new("hadolint")
        .args(["--format", "json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).await?;
    }

    let output = child.wait_with_output().await?;
    if output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn to_diagnostic(issue: HadolintIssue) -> Diagnostic {
    let severity = match issue.level.as_str() {
        "error" => DiagnosticSeverity::ERROR,
        "warning" => DiagnosticSeverity::WARNING,
        "info" => DiagnosticSeverity::INFORMATION,
        "style" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::WARNING,
    };
    let line = issue.line.saturating_sub(1);
    let col = issue.column.saturating_sub(1);
    Diagnostic {
        range: Range {
            start: Position::new(line, col),
            end: Position::new(line, col + 1),
        },
        severity: Some(severity),
        code: Some(NumberOrString::String(issue.code)),
        source: Some("hadolint".into()),
        message: issue.message,
        ..Default::default()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "hadolint-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.lint(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.pop() {
            self.lint(params.text_document.uri, change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            self.lint(params.text_document.uri, text).await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
