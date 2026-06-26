# Phase 1 - Tool Facade and Rust CLI Scaffold

Status: Not started.

## Goal

Create the stable developer-facing command facade and the Rust crate that later phases will fill
in. This phase should not migrate `run-all`, `agent-pr`, or `wait-pr` behavior yet; it only creates
the thin launchers, command dispatch shape, help output, and testable Rust module boundaries.

## Scope

- Add a workspace Rust crate for developer tooling, likely under `server/crates/devtool`, with a
  package name such as `rts-devtool` and a binary invoked by the launchers.
- Add a repo-root `tool` launcher for Unix-like environments.
- Add a repo-root `tool.ps1` launcher for Windows PowerShell.
- Add a documented Cargo fallback or Cargo alias so the raw command is available when script
  execution is awkward.
- Implement command registration and help output for `run-all`, `agent-pr`, and `wait-pr`, even if
  those commands initially report "not migrated" or delegate through the compatibility path.
- Add explicit Unix fallback mappings only for commands that should continue to call existing
  scripts during migration.

## Expected Touch Points

- `server/Cargo.toml`
- `server/crates/devtool/Cargo.toml`
- `server/crates/devtool/src/main.rs`
- `server/crates/devtool/src/lib.rs`
- `tool`
- `tool.ps1`
- `.cargo/config.toml` if using a Cargo alias
- `README.md` or `tests/README.md` for the initial command-surface note

## Implementation Notes

Keep the launchers intentionally small. They may locate the repository root and invoke Cargo, but
must not encode suite graphs, PR metadata, GitHub state machines, process supervision, or platform
policy. The Rust crate should define a command enum, argument parsing, and an injectable command
runner seam from the start so later phases can unit-test behavior without actually invoking Cargo,
Node, GitHub, or shell scripts.

The Unix `tool` launcher may fall back to existing scripts for unmigrated commands. The Windows
`tool.ps1` launcher should only advertise and run migrated Rust commands; for a Unix-only fallback,
it should fail with a clear message instead of trying to run Bash.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool`
- `./tool --help`
- `./tool run-all --help`
- `./tool agent-pr --help`
- `./tool wait-pr --help`
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

Confirm that `./tool --help` is readable and that a legacy fallback does not hide errors. On a
Windows checkout, confirm that `tool.ps1 --help` can reach the Rust binary and that unsupported
Unix-only commands fail clearly.

## Handoff Expectations

State the exact crate/package/binary names chosen, the public launcher syntax, and which commands
are Rust-backed versus fallback-only. Note any rough edges around PowerShell execution policy,
Cargo alias support, or launcher path handling before Phase 2 builds on the facade.
