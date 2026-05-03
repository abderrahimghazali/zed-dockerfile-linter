# hadolint-lsp

A small LSP server that wraps [hadolint](https://github.com/hadolint/hadolint) so editors can show Dockerfile lint diagnostics over LSP.

## Build

```sh
cargo build --release
```

The binary lands at `target/release/hadolint-lsp`.

## Use

Most editors expect an LSP command. Point at the binary; it speaks LSP over stdio. It only implements `textDocument/publishDiagnostics`, triggered on `didOpen`, `didChange`, and `didSave`.

`hadolint` must be on `PATH`.

## Behavior

For each text update, the server runs:

```
hadolint --format json -
```

with the document content piped on stdin. The JSON output is converted to LSP diagnostics with `source = "hadolint"` and the rule code (e.g. `DL3008`) in the `code` field.

Hadolint exits non-zero when issues are found; that is expected and ignored.
