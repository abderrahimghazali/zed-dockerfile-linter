# zed-dockerfile-linter

Hadolint-powered Dockerfile linting for [Zed](https://zed.dev/). Diagnostics inline as you type, code-action friendly, no manual install steps once published.

This repo contains two crates:

- `/` (root) is the Zed extension. WebAssembly. Just downloads the LSP binary on first use.
- `/lsp` is `hadolint-lsp`, a small Rust LSP server that shells out to `hadolint --format json` and republishes diagnostics over LSP.

## Releasing

Tag a release with prebuilt `hadolint-lsp` binaries attached as assets named:

- `hadolint-lsp-darwin-aarch64.tar.gz`
- `hadolint-lsp-darwin-x86_64.tar.gz`
- `hadolint-lsp-linux-aarch64.tar.gz`
- `hadolint-lsp-linux-x86_64.tar.gz`
- `hadolint-lsp-windows-x86_64.zip`

Each archive contains a single `hadolint-lsp` (or `hadolint-lsp.exe`) at the top level.

## Local development

You don't need to publish to test. Build the LSP and put it on `PATH`:

```sh
cd lsp
cargo build --release
cp target/release/hadolint-lsp ~/.local/bin/   # or anywhere on PATH
```

The extension calls `worktree.which("hadolint-lsp")` first, so a `PATH` install short-circuits the download path.

Then in Zed: `zed: install dev extension` and point at this repo's root. Open a `Dockerfile` and edit; you should see hadolint diagnostics.

## Requirements

- `hadolint` binary on `PATH`. Install with `brew install hadolint`, `apt install hadolint`, or download from <https://github.com/hadolint/hadolint/releases>.

## How it works

Zed's existing `Dockerfile` extension provides syntax + completions via `dockerfile-language-server`. This extension runs alongside it as a second LSP, contributing only diagnostics. Zed merges diagnostics from all LSPs attached to a language, so both work together without conflict.

## License

MIT. See `LICENSE`.
