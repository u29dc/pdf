# pdf

Personal Rust CLI for PDF metadata cleanup and lossless optimization.

## Status

- Legacy vault script was removed from the vault.
- Tool is now implemented as a Rust binary: `pdf`.
- Global install target: `~/.tools/pdf/pdf`.
- Shell command exposed via dotfiles: `:pdf`.

## Commands

```bash
pdf tools
pdf optimize <path>
pdf optimize <path> --estimate-size
pdf optimize <path> --apply
pdf optimize <path> --apply --estimate-size
pdf optimize <path> --json
```

## Behavior

- `optimize` accepts either:
  - one PDF file
  - one directory (recursive PDF scan)
- default run is dry-run planning (no file writes)
- `--apply` writes in place
- signed PDFs are skipped (signature-token guard)
- password-protected PDFs are skipped
- metadata normalization:
  - keep only `Title`, `CreationDate`, `ModDate`
  - set `Title` from filename stem
  - remove catalog `/Metadata` (XMP reference)
  - set PDF version to `1.7`
- optimization:
  - stream compression + modern save options (object streams + xref streams)
  - threshold gate via:
    - `--min-size-savings-bytes` (default `1024`)
    - `--min-size-savings-percent` (default `0.5`)
- apply mode creates a backup snapshot unless `--no-backup` is set

## Output Artifacts

- reports: `~/.tools/pdf/reports/*.json`
- backups: `~/.tools/pdf/backups/<timestamp>/...`

## Build and Install

```bash
bun install
bun run util:check
bun run build
```

`bun run build` compiles release binary and copies it to:

- `${PDF_HOME:-${TOOLS_HOME:-$HOME/.tools}/pdf}/pdf`

## Quality Gate

`bun run util:check` runs:

- format check (`cargo fmt --check`)
- clippy (`-D warnings`)
- type/build checks (`cargo check`)
- tests (`cargo test`)
- dependency audit (`cargo audit`)
- release build (`cargo build --release`)
