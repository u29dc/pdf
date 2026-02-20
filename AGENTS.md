## 1. Documentation

- `https://bun.sh/docs/llms.txt` (script runner, lockfile, workspace behavior)
- `https://doc.rust-lang.org/book/` (language and ownership model)
- `https://doc.rust-lang.org/cargo/` (build, test, release workflow)
- `https://docs.rs/clap/latest/clap/` (CLI contracts and argument parsing)
- `https://docs.rs/lopdf/latest/lopdf/` (PDF parse/rewrite APIs)

## 2. Repository Structure

```text
.
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── runner.rs
│   ├── scanner.rs
│   ├── pdf_ops.rs
│   ├── model.rs
│   └── output.rs
├── .husky/
├── Cargo.toml
├── package.json
├── commitlint.config.js
├── lint-staged.config.js
├── PLAN.md
└── AGENTS.md
```

## 3. Stack

| Layer        | Choice                           | Notes                                          |
| ------------ | -------------------------------- | ---------------------------------------------- |
| Runtime      | Rust (edition 2024)              | Primary implementation language                |
| CLI          | `clap`                           | Subcommands: `tools`, `optimize`               |
| PDF engine   | `lopdf`                          | Metadata rewrite and save options              |
| Parallelism  | `rayon`                          | File-level parallel planning and apply         |
| Tooling      | Bun                              | Script orchestration via `package.json`        |
| Commit gates | Husky + lint-staged + commitlint | Enforce `util:check` and commit message policy |

## 4. Commands

- `bun install` - install JS tooling dependencies
- `bun run util:check` - full quality gate (format, lint, types, test, audit, release build)
- `bun run util:format` - run `cargo fmt --all`
- `bun run util:lint` - run clippy with `-D warnings`
- `bun run util:types` - run `cargo check --all-targets --all-features`
- `bun run util:test` - run full Rust test suite
- `bun run util:audit` - run dependency audit
- `bun run build` - compile release binary and copy to `${PDF_HOME:-${TOOLS_HOME:-$HOME/.tools}/pdf}/pdf`
- `bun run pdf -- optimize <path> --estimate-size` - analyze one PDF or tree without writing
- `bun run pdf -- optimize <path> --apply` - apply in-place rewrite with backup snapshots

## 5. Architecture

- `src/main.rs` parses CLI input, routes subcommands, and emits human/JSON output with clear exit codes.
- `src/cli.rs` defines command and flag contracts, including optimization thresholds and concurrency controls.
- `src/runner.rs` orchestrates end-to-end runs: path resolution, recursive scan, parallel analysis/apply, backups, and report writes.
- `src/scanner.rs` expands one file or directory input into a deterministic sorted PDF target list.
- `src/pdf_ops.rs` performs signature/password guards, metadata normalization, optimization estimation, and threshold-gated writes.
- `src/model.rs` defines run/file report models and summary derivation.
- `src/output.rs` formats terminal and JSON envelopes for `tools`, `optimize`, and error flows.

## 6. Quality

- Run `bun run util:check` before merge or push.
- Keep Rust warnings at zero; clippy is configured as a hard gate.
- Preserve commit format `type(scope): subject` with lowercase subject and no trailing period.
- Keep `README.md` and `CLAUDE.md` as symlinks to `AGENTS.md`; update one source of truth only.
- Validate dry-run (`optimize` without `--apply`) and apply mode, including signed/password-protected skip paths.
