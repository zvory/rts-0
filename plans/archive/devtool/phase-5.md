# Phase 5 - Native Windows Run-All Support

Status: Not started.

## Goal

Make `tool.ps1 run-all` a supported native Windows workflow for migrated test-runner modes. This
phase should remove POSIX assumptions from the Rust implementation and add enough automated and
manual canary coverage to keep Windows support from regressing immediately.

## Scope

- Support Windows temp paths and default cache locations without hard-coded `/tmp` assumptions.
- Handle `.exe` suffixes and executable lookup for Cargo outputs, Chrome, Node, npm, and prebuilt
  server binaries.
- Implement Windows child-process cleanup robustly enough that a failed or interrupted run does not
  leave the server running unexpectedly.
- Support client dependency cache linking through the safest Windows approach available, such as
  junctions, symlinks when permitted, or copy fallback.
- Add Windows Chrome discovery or require a clear `CHROME` override with helpful error output.
- Add Windows CI coverage for the dev tool itself, focusing on unit tests, help output, command
  planning, and lightweight no-network/no-browser behavior.
- Document the Windows manual canary for developers.

## Expected Touch Points

- `server/crates/devtool/src/platform.rs`
- `server/crates/devtool/src/process.rs`
- `server/crates/devtool/src/deps.rs`
- `server/crates/devtool/src/browser.rs`
- `tool.ps1`
- `.github/workflows/main-tests.yml` or a separate lightweight dev-tool workflow
- `README.md`
- `tests/README.md`

## Implementation Notes

Do not make Windows support depend on Git Bash or WSL. The supported path should be PowerShell plus
Cargo, Node, npm, and the normal external tools needed by the suite. If full browser execution is
too expensive or flaky for Windows CI, add a smaller Windows automated canary and document the
manual browser run explicitly.

This phase may need platform-specific process-tree termination. Keep that code behind a narrow
abstraction so Unix behavior from Phase 4 remains stable.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool` on Unix
- `cargo test --manifest-path server/Cargo.toml -p rts-devtool` on Windows CI or a Windows checkout
- `tool.ps1 run-all --help` on Windows
- `tool.ps1 run-all --only-rust` or a documented lighter Windows canary if full Rust coverage is
  too slow for the available machine
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

On a Windows checkout, run `tool.ps1 run-all --help`, one non-browser mode, and one browser-related
help or dry-run path. Confirm no Bash dependency appears in the supported path and that interrupted
runs clean up child processes.

## Handoff Expectations

State exactly which `run-all` modes are supported on Windows and which remain Unix-only or
manual-canary-only. Include any required Windows prerequisites, such as Chrome path setup, Node
version, npm availability, or PowerShell execution policy notes.
