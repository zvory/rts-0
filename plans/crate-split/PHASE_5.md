# Phase 5 - Server Shell, Tools, and Perf Composition

Status: Planned.

Goal: make the axum/tokio server and developer tools compose the split crates cleanly.

## Scope

- Keep `rts-server` focused on:
  - static client serving;
  - WebSocket upgrade and frame bounds;
  - client message parsing;
  - lobby/room lifecycle;
  - snapshot fanout;
  - dev endpoints;
  - map save endpoints;
  - server-side perf logging.
- Move AI/tool binaries to depend on crates instead of binary-local module declarations.
- Decide whether dev scenarios live in server, tools, or a small dev-support crate.
- Keep compacting-for-wire and resource elision in server/protocol boundary code, not sim core, if
  it is transport-specific.
- Keep crash replay artifact writing in server/lobby or dev-support, while sim owns only the data
  needed to reconstruct a match.
- Re-home `perf.rs` so server owns reporting, while sim exposes only generic phase/snapshot timing
  hooks.

## Design Notes

This phase is where build caching should become visibly better. `ai-matchup`, `ai-balance-matrix`,
and `ai-perf-harness` should link against `rts-sim`/`rts-ai` instead of compiling private copies of
the same source modules.

The server shell can depend on everything because it composes the runtime. That is acceptable as
long as the lower-level crates do not depend back upward.

## Tests

- `cd server && cargo build --bins`
- `cd server && cargo test`
- `cd server && cargo run --bin ai-matchup -- --help`
- `cd server && cargo run --bin ai-balance-matrix -- --help`
- `cd server && cargo run --bin ai-perf-harness -- --help`
- Live Node integration suites with a running server.

## Done

- No binary declares duplicate `mod game; mod protocol; mod rules;` trees.
- `rts-server` is the only crate importing axum/tokio WebSocket/server machinery.
- Tool binaries are thin CLI wrappers around library crates.
- Perf logs still provide the existing room/tick/snapshot visibility.

