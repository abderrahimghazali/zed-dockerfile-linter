# zed-dockerfile-linter

Hadolint-powered Dockerfile linting for [Zed](https://zed.dev/). Diagnostics inline as you type, no manual install steps.

This repo contains two crates:

- `/` (root) is the Zed extension. WebAssembly. Downloads `hadolint-lsp` from this repo's GitHub releases on first use.
- `/lsp` is `hadolint-lsp`, a small Rust LSP that wraps the `hadolint` binary and republishes its findings as LSP diagnostics.

## Requirements

- The official Zed `Dockerfile` extension (provides the `Dockerfile` language definition this one attaches to). Install it from `zed: extensions`.
- That's it. `hadolint` itself is downloaded automatically the first time you open a Dockerfile, if it isn't already on `PATH`.

## How `hadolint` is resolved

On first lint, `hadolint-lsp` looks for the binary in this order:

1. `hadolint` on `PATH` (e.g. `brew install hadolint`, `apt install hadolint`).
2. `<cache>/hadolint-lsp/hadolint-<version>/hadolint` from a previous auto-download.
3. Otherwise downloads the upstream release for the current platform and stores it under `<cache>/hadolint-lsp/`.

`<cache>` is `~/Library/Caches` on macOS, `~/.cache` on Linux, `%LOCALAPPDATA%` on Windows. The pinned version is set in `lsp/src/main.rs`.

macOS arm64 note: hadolint does not publish a native arm64 build, so the auto-downloader fetches the x86_64 binary and relies on Rosetta. If you'd rather have a native build, `brew install hadolint` produces one and `PATH` lookup picks it up first.

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
cargo install --path . --force   # installs to ~/.cargo/bin
```

The Zed extension calls `worktree.which("hadolint-lsp")` first, so a `PATH` install short-circuits the GitHub-release download.

Then in Zed: `Cmd+Shift+P` → `zed: install dev extension` and point at this repo's root. Open a Dockerfile; diagnostics should appear.

## How it works

Zed's official `Dockerfile` extension provides syntax + completions via `dockerfile-language-server`. This extension runs alongside it as a second LSP, contributing only diagnostics. Zed merges diagnostics from all LSPs attached to a language, so both work together without conflict.

## License

MIT. See `LICENSE`.
