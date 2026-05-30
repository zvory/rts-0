# CLAUDE.md

Guidance for working in this repo. **Read `DESIGN.md` first** — it is the source of truth for the
architecture, the JSON wire protocol, every module's contract, the `Game` API seam, the balance
table, and the hardening limits (§7). Keep `DESIGN.md` updated in the same change whenever you
alter a contract.

A server-authoritative Bewegungskrieg server (`server/`, axum + tokio) runs the one authoritative
simulation and also serves the **HTML/CSS/JS + PixiJS** client (`client/`). Clients send commands;
the server simulates at 30 Hz and sends per-player, fog-filtered snapshots.

## Parallel Worktrees

- For parallel feature work, each terminal/agent must work in its own git worktree. Do not run two
  coding agents in the same checkout.
- Before making changes, verify the checkout and branch:

  ```bash
  git rev-parse --show-toplevel
  git branch --show-current
  git status --short
  ```

- Use one branch per worktree. Branch names must start with `zvorygin/`.
- Create worktrees from the repo root, for example:

  ```bash
  git worktree add ../rts-my-feature -b zvorygin/my-feature main
  ```

- Agents must only edit files inside their assigned worktree. Do not edit the original checkout or
  another agent's worktree.
- Coordinate write ownership before starting. If another agent owns a file or module, do not edit it
  unless explicitly told to. Avoid parallel edits to shared contracts such as protocol, config,
  generated files, or design docs.
- Stage and commit only files belonging to the current task. Never revert unrelated changes.
- When the task is complete, merge the worktree branch directly into `main` and push `main` to
  `origin`. Do not create a PR unless the user explicitly asks for one.
- Do not merge, rebase, or delete another agent's branch/worktree unless explicitly asked. Only merge
  the branch assigned to the current task.
- If running local servers, use different ports per worktree or stop the other server first.

## Git / GitHub

- The default branch is `main`.
- committing has a commit hook that silently runs all tests.
- Work directly on `main` for simple single-agent changes. For parallel worktree changes, use one
  `zvorygin/` branch per worktree.
- When work is complete, stage and commit only files that belong to the current task.
- Do not create, open, or update PRs for repo work unless the user explicitly asks for a PR.
- If work was done on a branch, the AI should merge that branch into `main` directly and push `main`
  to `origin` without opening a PR.
- Before merging, make sure the worktree is clean, update `main` from `origin/main`, and merge only
  the current task branch.

## Commands

```bash
# Run (serves client + /ws on :8080; open http://localhost:8080)
cd server && cargo run            # add --release for the fast build

# Build / lint / format
cd server && cargo build && cargo clippy && cargo fmt

# Tests — start the server first, then (from repo root):
node tests/server_integration.mjs     # dep-free, full server pipeline
node tests/regression.mjs             # dep-free, hardening/DoS/robustness guards
node tests/ai_integration.mjs         # dep-free, AI opponent lobby flow (add/remove/start)
cd tests && npm install && node client_smoke.mjs   # headless-Chrome client smoke

# Simulation behavior, including scripted self-play (no running server needed): cd server && cargo test
```

There is **no JS build step** (plain ES modules + PixiJS from CDN). The client is served from
`../client` relative to the server crate, so `cargo run` from `server/` is the whole dev loop.

## Invariants — do not break these

- **Wire protocol is mirrored.** `server/src/protocol.rs` ⇄ `client/src/protocol.js` must agree on
  every tag, field name, and shape. Change both together (and `DESIGN.md §2`).
- **Balance is mirrored.** `server/src/config.rs` is authoritative; `client/src/config.js` mirrors
  the UI/render/fog subset (costs, supply, sight, sizes). Change both together.
- **The `Game` API is the seam.** `lobby.rs`/`main.rs` touch the simulation only through the public
  API in `game/mod.rs` (documented in `DESIGN.md §3.1`). Keep the signatures stable; if you must
  change one, update the doc and all callers.
- **`Game::tick()` must be panic-free.** No `unwrap()`/`expect()`/unchecked indexing on the tick
  path; treat stale ids as no-ops. Use checked arithmetic on anything derived from client input
  (a panic kills the whole room task).
- **Fog is authoritative and cheat-proof.** Anything sent per-player — entity views, `target_id`
  tracers, death/positional events — must be gated on visibility/ownership. Never send a player an
  entity or position they can't see.
- **Clients are untrusted.** Validate and bound everything from the wire: command unit lists are
  deduped + capped (`MAX_UNITS_PER_COMMAND`), WebSocket frames are size-limited, placement coords
  are range/overflow-checked. See `DESIGN.md §7`.

## Conventions

- **Rust:** edition 2021, `cargo fmt`, keep warnings low. Prefer small pure helpers in
  `game/services/`. `systems.rs` is the thin orchestrator that calls services in order. The room
  task is the single owner of its `Game` — no locks around it. Don't panic on the network or tick
  paths; handle errors and keep the room alive.
- **JS:** ES2020 modules, no framework, one small class per file (see `DESIGN.md §4`). Modules
  receive their collaborators via **dependency injection** from `main.js`; they do **not**
  cross-import each other (only `protocol.js`/`config.js`). PixiJS is the global `PIXI` (v7) — do
  not `import` it.
- **Client teardown:** any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches — omitting it leaks
  listeners/WebGL contexts across rematches.
- **Coordinates:** world pixels on the wire everywhere; tiles only where a field name ends in
  `Tile`.

## Gotchas

- Debug builds have overflow checks **on** (a bad `Build` coord can panic in `cargo run` but
  silently wrap in `--release`) — that's why placement math is `checked_*`. Keep it that way.
- Tests need a **running** server on `:8080`; they are not `cargo test` (they're Node scripts that
  drive the live server/client end to end). After any change, run all three and confirm green.
- A 1-player match is a never-ending sandbox; only 2+ player matches resolve to a winner. Empty
  rooms reset to lobby so a room name is never stuck mid-match.
