# Phase 0 - Workspace and Boundary Audit

Status: Planned.

Goal: prepare the repo for incremental crate extraction without moving behavior yet.

## Scope

- Convert `server/` into a Cargo workspace while preserving existing command ergonomics:
  - `cd server && cargo run`
  - `cd server && cargo test`
  - `cd server && cargo run --bin ai-matchup`
- Add a shared library target or first internal package that exposes the current server modules
  without changing runtime behavior.
- Replace duplicated module declarations in tool binaries with imports from the shared library
  where possible.
- Add a lightweight dependency-boundary audit document or script that records current imports that
  violate the target direction.
- Decide package names and directory layout before large file moves.

## Suggested Layout

Start conservatively:

```text
server/
  Cargo.toml              # workspace root and server package or virtual workspace
  crates/
    contract/
    protocol/
    rules/
    sim/
    ai/
    tools/
  src/
    main.rs
    lobby/
```

If moving `src/main.rs` immediately is too noisy, keep the current server package at the workspace
root for this phase and only introduce library targets that reduce duplication.

## Boundary Audit Checks

Capture at least these current coupling classes:

- protocol references from sim/rules;
- sim references from protocol/rules/config;
- AI references from `Game` and setup;
- server perf references from sim;
- tokio/axum references outside the server shell;
- direct `lobby` or `main` references from sim/rules/protocol.

This can be a checked-in note, a simple `rg`-based script, or both. The purpose is to make each
later phase's deletion list explicit.

## Tests

- `cd server && cargo test`
- `cd server && cargo run --bin ai-matchup -- --help`
- `cd server && cargo run --bin ai-balance-matrix -- --help`
- `cd server && cargo run --bin ai-perf-harness -- --help`

## Done

- Server commands still work from `server/`.
- Tool binaries no longer compile by copy-pasting the entire module tree if that can be achieved
  without deeper refactors.
- The known dependency violations are documented and mapped to later phases.
- No gameplay, protocol, balance, or lobby behavior changes.

