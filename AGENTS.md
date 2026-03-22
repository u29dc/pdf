> `pdf` is a Rust CLI with a JSON-first command surface that introspects itself with `tools` and rewrites PDFs with `optimize`, using `lopdf` for metadata cleanup and `qpdf` for size optimization and output validation.

## 1. Documentation

- Primary references: [Bun docs](https://bun.sh/docs/llms.txt), [Cargo Book](https://doc.rust-lang.org/cargo/), [Rust Book](https://doc.rust-lang.org/book/), [Clap docs](https://docs.rs/clap/latest/clap/), [lopdf docs](https://docs.rs/lopdf/latest/lopdf/), [qpdf manual](https://qpdf.readthedocs.io/)
- Local source-of-truth files: [`src/cli.rs`](src/cli.rs), [`src/tool_registry.rs`](src/tool_registry.rs), [`src/pdf_ops.rs`](src/pdf_ops.rs), [`src/runner.rs`](src/runner.rs), [`tests/cli_contract.rs`](tests/cli_contract.rs), [`package.json`](package.json), [`.cargo/config.toml`](.cargo/config.toml)
- There is no `docs/` tree or nested `AGENTS.md`; use the code, tests, and package scripts as the operational reference.

## 2. Repository Structure

```text
.
├── src/
│   ├── main.rs              CLI entrypoint and exit code handling
│   ├── cli.rs               clap contract for `tools` and `optimize`
│   ├── runner.rs            path resolution, parallel planning, backups, reports
│   ├── pdf_ops.rs           metadata rewrite and qpdf-driven optimization
│   ├── scanner.rs           visible-path discovery and target guards
│   ├── tool_registry.rs     manual tool catalog and JSON schemas
│   └── {model,output,error}.rs
├── tests/cli_contract.rs    JSON envelope and discoverability contract
├── .cargo/config.toml       macOS `target-cpu=native` overrides
├── .husky/_/                husky shims only; repo hook entrypoints are absent
└── AGENTS.md                canonical repo-level agent instructions
```

- Keep [`README.md`](README.md) and [`CLAUDE.md`](CLAUDE.md) as symlinks to [`AGENTS.md`](AGENTS.md).
- Treat `target/`, runtime reports, backups, and temporary `.pdf-*.tmp.pdf` files as generated artifacts.

## 3. Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Runtime | Rust 2024 binary crate | single package rooted at [`src/main.rs`](src/main.rs) |
| CLI contract | `clap` + manual registry | [`src/cli.rs`](src/cli.rs) and [`src/tool_registry.rs`](src/tool_registry.rs) must stay in sync |
| PDF pipeline | `lopdf` + external `qpdf` | metadata rewrite happens in-process; optimization and validation shell out |
| Parallelism | `rayon` | file planning and apply work run through a configurable thread pool |
| Tooling | Bun + Cargo | Bun wraps quality gates and local install; Cargo builds and tests |
| Tests | Rust unit + integration tests | [`tests/cli_contract.rs`](tests/cli_contract.rs) locks the JSON-first interface |

## 4. Commands

- `bun install` - install JS tooling and Husky support files
- `cargo run -- tools [name]` - inspect the machine-discoverable tool catalog during iteration
- `cargo run -- optimize <path> [--estimate-size] [--apply]` - exercise the real CLI from source
- `cargo test --all` - run unit tests plus the CLI contract integration test
- `bun run util:check` - run the aggregated completion gate: format check, clippy, cargo check, tests, audit, and release build
- `bun run build` - build `target/release/pdf` and copy it to `${PDF_HOME:-${TOOLS_HOME:-$HOME/.tools}/pdf}/pdf`
- `bun run pdf -- <args>` - execute the release binary in `./target/release/pdf`; this does not build it first

## 5. Architecture

- [`src/main.rs`](src/main.rs) parses CLI input, defaults to JSON output, routes `tools` and `optimize`, and maps failures to exit code `1` or blocked conditions to exit code `2`.
- [`src/tool_registry.rs`](src/tool_registry.rs) is a hand-maintained contract layer for tool names, schemas, examples, and output fields; changes to flags or report fields must update it explicitly.
- [`src/runner.rs`](src/runner.rs) resolves relative paths against the current working directory, sorts scan targets deterministically, analyzes files in parallel, creates backups before apply-mode writes, and persists run reports.
- [`src/scanner.rs`](src/scanner.rs) rejects symlink targets and hidden top-level targets, while recursive directory scans silently skip hidden entries and non-PDF files.
- [`src/pdf_ops.rs`](src/pdf_ops.rs) checks raw bytes for signature tokens before opening the document, treats encrypted/password-protected files as skipped, normalizes title/docinfo/XMP/version metadata, and stages optimized temp files through `qpdf --check`.
- Apply semantics are threshold-gated twice: optimized output replaces the source only when both byte and percentage thresholds pass; metadata-only fallback happens only after an optimized temp file exists but is below threshold. If optimized staging or `qpdf --check` fails, the file is marked failed and metadata-only fallback is not attempted.

## 6. Runtime and State

- Home directory resolution: `PDF_HOME` -> `TOOLS_HOME/pdf` -> `$HOME/.tools/pdf`.
- Persistent runtime state: reports under `.../reports/<timestamp>-{plan|apply}.json`; backups under `.../backups/<timestamp>/` with the original absolute source path reproduced under that timestamped root.
- Temporary files are created next to the source PDF with prefixes like `.pdf-estimate-`, `.pdf-optimized-`, `.pdf-metadata-`, and `.pdf-metadata-stage-`.
- `qpdf` on `PATH` is required for any `--estimate-size` run and for all `--apply` runs, because apply mode enables size estimation internally.
- Empty scans are valid: a directory with no visible PDFs returns success, writes a report, and reports `summary.total = 0`.
- The emitted JSON envelope reports the correct `reportPath`, but the persisted report file is written before that field is backfilled, so inside the saved JSON `reportPath` is currently an empty string.
- [`.cargo/config.toml`](.cargo/config.toml) sets `target-cpu=native` for Apple targets, so locally built release binaries are host-optimized artifacts rather than portable generic macOS builds.

## 7. Conventions

- Default stdout contract is one JSON object with `{ ok, data | error, meta }`; human-readable output exists only behind `--text`.
- When the compiled binary is invoked directly, success-path JSON commands are expected to keep stderr empty. [`tests/cli_contract.rs`](tests/cli_contract.rs) asserts this for `tools` and `optimize`.
- Keep [`src/cli.rs`](src/cli.rs), [`src/tool_registry.rs`](src/tool_registry.rs), [`src/model.rs`](src/model.rs), and [`tests/cli_contract.rs`](tests/cli_contract.rs) aligned whenever flags, tool names, output fields, or example commands change.
- Preserve the `planned_actions` string constants in [`src/model.rs`](src/model.rs) unless you are intentionally changing the report contract for downstream consumers.
- Prefer scoped Conventional Commits to match repository history, but note that the current [`commitlint.config.js`](commitlint.config.js) does not enforce scope presence.

## 8. Constraints

- Never point `optimize` at a symlink target or a hidden path; the scanner rejects those inputs before planning.
- Treat `optimize --apply` as destructive in-place rewrite. Backups are created before writes unless `--no-backup` is set, and a backup directory may be created even when all apply attempts later fail.
- Do not assume every valid-looking PDF optimizes cleanly. `qpdf --check` failures surface as `optimizationError` in plan mode and `applyError` in apply mode.
- Do not hand-edit `target/`, runtime reports, backup trees, or leftover `.pdf-*.tmp.pdf` artifacts; regenerate or rerun the command instead.
- Do not rely on Git hooks for enforcement in the current repo state. `.husky/_/` exists, but top-level `.husky/pre-commit` and `.husky/commit-msg` scripts are not present.

## 9. Validation

- Required gate: `bun run util:check`. It is intentionally aggregated rather than fail-fast, so read the full output even after the first failure.
- Minimum targeted checks for CLI contract changes: `cargo test --test cli_contract`, `cargo run -- tools`, and `cargo run -- tools pdf.optimize`.
- Minimum targeted checks for scan/path handling changes: `cargo test scanner::tests:: -- --nocapture` or `cargo test --all`.
- Minimum targeted checks for optimization changes: run `cargo run -- optimize <visible-pdf-or-dir> --estimate-size` and, if write behavior changed, rerun with `--apply` inside a disposable directory with `PDF_HOME` pointed at a temp location.
- After touching repo docs, verify `README.md` and `CLAUDE.md` still resolve to [`AGENTS.md`](AGENTS.md) with `ls -l AGENTS.md CLAUDE.md README.md`.
