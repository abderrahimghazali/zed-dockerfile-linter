use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

const HADOLINT_VERSION: &str = "2.14.0";

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
    hadolint: Mutex<Option<PathBuf>>,
}

impl Backend {
    async fn hadolint_path(&self) -> std::result::Result<PathBuf, String> {
        {
            let lock = self.hadolint.lock().await;
            if let Some(p) = lock.as_ref() {
                return Ok(p.clone());
            }
        }
        let path = resolve_hadolint(&self.client).await?;
        let mut lock = self.hadolint.lock().await;
        *lock = Some(path.clone());
        Ok(path)
    }

    async fn lint(&self, uri: Url, text: String) {
        let path = match self.hadolint_path().await {
            Ok(p) => p,
            Err(err) => {
                self.client
                    .log_message(MessageType::ERROR, format!("hadolint unavailable: {err}"))
                    .await;
                return;
            }
        };
        match run_hadolint(&path, &text).await {
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

async fn resolve_hadolint(client: &Client) -> std::result::Result<PathBuf, String> {
    if let Ok(path) = which::which("hadolint") {
        return Ok(path);
    }
    let cache_dir = dirs::cache_dir().ok_or("no cache directory on this system")?;
    let install_dir = cache_dir
        .join("hadolint-lsp")
        .join(format!("hadolint-{HADOLINT_VERSION}"));
    let bin_name = if cfg!(windows) {
        "hadolint.exe"
    } else {
        "hadolint"
    };
    let cached = install_dir.join(bin_name);
    if cached.exists() {
        return Ok(cached);
    }
    client
        .log_message(
            MessageType::INFO,
            format!("hadolint not on PATH, downloading v{HADOLINT_VERSION} to {}", install_dir.display()),
        )
        .await;
    download_hadolint(&install_dir).await?;
    Ok(cached)
}

async fn download_hadolint(dir: &Path) -> std::result::Result<(), String> {
    let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "hadolint-macos-arm64",
        ("macos", _) => "hadolint-macos-x86_64",
        ("linux", "x86_64") => "hadolint-linux-x86_64",
        ("linux", "aarch64") => "hadolint-linux-arm64",
        ("windows", _) => "hadolint-windows-x86_64.exe",
        (os, arch) => return Err(format!("no upstream hadolint binary for {os}/{arch}")),
    };
    let url = format!(
        "https://github.com/hadolint/hadolint/releases/download/v{HADOLINT_VERSION}/{asset}"
    );
    let bytes = tokio::task::spawn_blocking(move || -> std::result::Result<Vec<u8>, String> {
        let response = ureq::get(&url).call().map_err(|e| format!("{e}"))?;
        let mut buf = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| format!("{e}"))?;
        Ok(buf)
    })
    .await
    .map_err(|e| format!("download task panicked: {e}"))??;

    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("create_dir_all: {e}"))?;
    let bin_name = if cfg!(windows) {
        "hadolint.exe"
    } else {
        "hadolint"
    };
    let bin_path = dir.join(bin_name);
    tokio::fs::write(&bin_path, &bytes)
        .await
        .map_err(|e| format!("write: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&bin_path)
            .await
            .map_err(|e| format!("metadata: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&bin_path, perms)
            .await
            .map_err(|e| format!("chmod: {e}"))?;
    }
    Ok(())
}

async fn run_hadolint(bin: &Path, text: &str) -> std::io::Result<Vec<HadolintIssue>> {
    let mut child = Command::new(bin)
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
    let (service, socket) = LspService::new(|client| Backend {
        client,
        hadolint: Mutex::new(None),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
