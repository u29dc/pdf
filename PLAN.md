# PDF Tool Migration Plan

Status: implemented on February 20, 2026.

## 1. Goal

Create a fast personal Rust CLI named `pdf`, installable as a single binary to `~/.tools/pdf/pdf`, exposed via `:pdf`, with one domain action:

- `optimize`: sanitize metadata and reduce PDF file size (lossless-first), for one file or a directory tree.

## 2. Constraints

- Keep command surface minimal.
- Default execution must be safe (dry-run/plan).
- Must work from anywhere on macOS.
- Must support both vault and non-vault paths.
- Must remain personal-use pragmatic, not a generalized enterprise framework.

## 3. Existing State

- Source script moved from vault to this repo:
  - `cleanup.py` (legacy implementation)
- Vault script folder removed from vault root and preserved here:
  - `_vault_scripts_from_vault/`
- Existing global tool pattern reference:
  - `~/Git/koe/package.json`
  - `~/Git/cho/package.json`
  - `~/Git/dot/shell/zshrc`

## 4. Target CLI Contract (Minimal)

### 4.1 Commands

- `pdf tools`
- `pdf optimize <path> [--apply] [--json]`

### 4.2 Behavior

- `pdf tools`
  - lists available commands (currently only `optimize`)
  - supports `--json` for machine-readable output
- `pdf optimize <path>`
  - accepts file or directory
  - directory scan is recursive
  - dry-run by default
  - `--apply` writes changes in place
  - skips hidden files and non-PDF files
  - skips password-protected PDFs and signature-sensitive PDFs where detection is reliable

### 4.3 Output

- Human-readable summary in text mode.
- JSON envelope in `--json` mode:
  - success: `{ ok: true, data, meta }`
  - failure: `{ ok: false, error, meta }`

## 5. Technical Approach

### 5.1 Runtime and Core Crates

- `clap` for CLI parsing.
- `rayon` for parallel planning/processing.
- `serde` + `serde_json` for JSON output.
- `anyhow` or `thiserror` for structured error handling.
- `walkdir` + `ignore` for fast recursive traversal.

### 5.2 PDF Processing Strategy

- Stage A: metadata cleanup in Rust pipeline:
  - keep minimal allowed info fields
  - normalize title from file name
  - remove XMP metadata stream when possible
- Stage B: lossless optimization:
  - preferred: `qpdf` subprocess integration (object streams + stream recompression) when installed
  - fallback: metadata-only rewrite with explicit warning

Note: exact low-level PDF operation support will be validated during implementation because Rust PDF crate capabilities vary by feature completeness.

### 5.3 Safety and File Writes

- Default dry-run.
- `--apply` performs atomic writes (`temp file -> fsync -> rename`).
- Backup snapshot before apply (zip or mirrored backup directory).

## 6. Repo and Build Plan

### 6.1 Repository Baseline

- Initialize git repo in `~/Git/PDF`.
- Keep legacy Python script during migration window.
- Add initial Rust scaffold:
  - `Cargo.toml`
  - `src/main.rs`
  - `src/cli.rs`
  - `src/optimize.rs`
  - `src/report.rs`

### 6.2 Global Install Pattern

Mirror `koe`/`cho` muscle-memory workflow with `package.json` script:

- build release binary
- copy binary to `${PDF_HOME:-${TOOLS_HOME:-$HOME/.tools}/pdf}/pdf`

Planned command:

- `bun run build`

### 6.3 Shell Exposure

Update `~/Git/dot/shell/zshrc`:

- add `export PDF_HOME="$TOOLS_HOME/pdf"`
- add `:pdf() { "$PDF_HOME/pdf" "$@"; }`

## 7. Execution Phases

### Phase 1: Scaffold + Compatibility

- Create Rust CLI with:
  - `tools`
  - `optimize` dry-run file discovery
- Preserve `cleanup.py` as reference only.

### Phase 2: Metadata + Optimization Engine

- Implement metadata cleanup path.
- Integrate optimization path and per-file change estimation.
- Add signed/password-protected skip logic.

### Phase 3: Apply Mode + Backups

- Implement atomic writes and backup generation.
- Add summary reports (`json` + markdown optional).

### Phase 4: Globalization

- Add `package.json` build/install script.
- Update dotfiles (`PDF_HOME`, `:pdf` function).
- Validate from arbitrary directories with `:pdf optimize <path>`.

### Phase 5: Decommission Python

- Verify parity for core use cases.
- Remove or archive Python implementation.

## 8. Validation Checklist

- `pdf tools` lists `optimize`.
- `pdf tools --json` produces valid JSON envelope.
- `pdf optimize <single-file>` dry-run works.
- `pdf optimize <folder>` recursive dry-run works.
- `pdf optimize <path> --apply` writes safely and produces backup.
- Measurable size reduction on representative invoice PDFs.
- Runtime faster than legacy Python on the same folder set.

## 9. Risks and Mitigations

- Rust PDF crate limitations:
  - Mitigation: use `qpdf` integration for robust optimization path.
- Signature invalidation risk:
  - Mitigation: detect and skip signed PDFs by default.
- Unexpected file growth on specific PDFs:
  - Mitigation: keep threshold gate and dry-run estimates before apply.
- Operational drift with shell aliases:
  - Mitigation: codify `PDF_HOME` and `:pdf` in dotfiles with same pattern as `:cho`/`:koe`.
