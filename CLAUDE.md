# Repository guidance

This is a server-authoritative Bewegungskrieg game. The Rust server in `server/` owns the 30 Hz
simulation and serves the plain JavaScript/PixiJS client in `client/`. Clients send commands; the
server sends per-player, fog-filtered snapshots.

## Development stage

This game is pre-alpha, not a live production service. Build strong foundations for architecture,
authority, security, and maintainability, but do not preserve compatibility for its own sake.
Breaking gameplay, old behavior, protocols, APIs, replays, or local data is acceptable when it
produces a cleaner foundation. Prefer coordinated clean breaks that update all affected code,
docs, and tests over compatibility shims, dual paths, deprecation periods, or elaborate migrations.
Move quickly and do not spend excessive time protecting old versions or transient development
state unless the user explicitly requests compatibility or identifies persistent data that matters.

## Start with relevant context

Read the smallest relevant capsule in `docs/context/` before exploring broadly. The index covers
server simulation, client UI, protocol, balance, testing, deployment, planning, and match history:
`docs/context/README.md`.

When changing a cross-file contract, read and update its source-of-truth file in `docs/design/`.
This applies to the wire protocol, `Game` API seam, balance mirror, fog rules, and hardening surface.
Refresh a capsule's section pointers when the design document's structure changes.

## Scope and evidence

- For requests to investigate, review, audit, scout, or confirm, inspect and report. Keep the pass
  read-only unless the user also requests a change.
- A request to build, change, or fix authorizes the complete normal delivery workflow: make the
  in-scope edits, run focused non-destructive validation, commit and push the task branch, open an
  owned PR, arm auto-merge, and wait for the PR to merge.
- Base claims about current gameplay, balance, deployments, CI, PRs, and merged state on current
  repository, runtime, Git, GitHub, or Fly evidence rather than agent memory.
- Require confirmation for destructive actions, direct pushes to `main`, or material scope
  expansion. Normal task-branch pushes, owned PR creation, and auto-merge do not require a separate
  request or confirmation.

## Editing and Git workflow

Read-only inspection may use the current checkout. Before editing, work in a clean task-specific
worktree based on current `origin/main`; parallel writers must use separate worktrees and branches.

```bash
git rev-parse --show-toplevel
git branch --show-current
git status --short
mkdir -p /tmp/rts-worktrees
git fetch origin main
git worktree add /tmp/rts-worktrees/<task> -b zvorygin/<task> origin/main
```

- Branch names start with `zvorygin/`. Edit only the assigned worktree and coordinate ownership of
  shared contracts, generated files, and design documents.
- Preserve unrelated user changes. Ignore a dirty `playtest_notes.md`; never edit, stage, revert, or
  otherwise manage it.
- Stage and commit only task files. Use a clear commit subject and add a body for gameplay impact,
  contract changes, testing nuance, or non-obvious reasoning.
- Use focused local checks during development. GitHub's `Main test gate` is the authoritative full
  suite for PR delivery.
- For every requested build, change, or fix, run
  `scripts/agent-pr.sh --verification "focused check command(s) passed"`, then
  `scripts/wait-pr.sh <pr>`. Completion means the PR merged and its head is reachable from
  `origin/main`, or a reported blocker identifies the exact failed check, conflict, API failure, or
  human decision needed.
- See `docs/pr-first-workflow.md` for recovery, serial phases, PR audits, and exceptional workflows.

## Focused commands

```bash
# Run the server and client
cd server && cargo run

# Build, lint, and check the simulation seam
cd server && cargo build && cargo clippy
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture

# Live Node suites require a running server on the runner's private port
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
tests/run-all.sh --no-rust

# Simulation and scripted self-play; no running server required
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default
```

Choose the smallest check that covers the changed area. There is no JavaScript build step; PixiJS
is loaded from the CDN, and `cargo run` from `server/` serves the client.

## Contract invariants

- **Wire protocol is mirrored.** `server/crates/protocol/src/lib.rs` ⇄ `server/src/protocol.rs`
  ⇄ `client/src/protocol.js` must agree on every tag, field name, and shape. Change them together
  (and `docs/design/protocol.md`).
- **Balance is mirrored.** `server/crates/rules/src/balance.rs` is authoritative;
  `server/src/config.rs` is a compatibility shim; `client/src/config.js` mirrors the
  UI/render/fog subset (costs, supply, sight, sizes). Change the authoritative Rust values and
  client mirror together.
- **The `Game` API is the seam.** `lobby.rs`/`main.rs` touch the simulation only through the public
  API in `game/mod.rs` (documented in `docs/design/server-sim.md`). Keep the signatures stable; if
  you must change one, update the doc and all callers.
- **`Game::tick()` must be panic-free.** No `unwrap()`/`expect()`/unchecked indexing on the tick
  path; treat stale ids as no-ops. Use checked arithmetic on anything derived from client input
  (a panic kills the whole room task).
- **Fog is authoritative and cheat-proof.** Anything sent per-player — entity views, `target_id`
  tracers, death/positional events — must be gated on visibility/ownership. Never send a player an
  entity or position they can't see.
- **Clients are untrusted.** Validate and bound everything from the wire: command unit lists are
  deduped + capped (`MAX_UNITS_PER_COMMAND`), WebSocket frames are size-limited, placement coords
  are range/overflow-checked. See `docs/design/hardening.md`.
- **Match history is server-only and env-gated.** Only the server writes the `matches` table, and
  only when `RTS_RECORD_MATCHES` is truthy (so local `cargo run` reads but never writes). Detached
  write on `end_match` — never block the room task on the DB. See
  `docs/design/match-history.md`.

## Code conventions

- **Rust:** edition 2021; the owned-PR lifecycle formats only branch-touched Rust files with the
  pinned toolchain. Keep warnings low. Prefer small pure helpers in
  `game/services/`. `systems.rs` is the thin orchestrator that calls services in order. The room
  task is the single owner of its `Game` — no locks around it. Don't panic on the network or tick
  paths; handle errors and keep the room alive.
- **JS:** ES2020 modules, no framework, one small class per file (see
  `docs/design/client-ui.md`). Modules receive their collaborators via **dependency injection**
  from `main.js`; they do **not**
  cross-import each other (only `protocol.js`/`config.js`). PixiJS is the global `PIXI` (v7) — do
  not `import` it.
- **Client teardown:** any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches — omitting it leaks
  listeners/WebGL contexts across rematches.
- **Coordinates:** world pixels on the wire everywhere; tiles only where a field name ends in
  `Tile`.

## Specialized workflows

- Balance or gameplay changes: collect factual patch-note bullets covering changed stats, economy,
  combat behavior, UI affordances, and what should be watched in playtests.
- New units: complete Phase 0 and Phase 1 (unit brief and rules/balance specification) before
  implementation. Until the user explicitly authorizes implementation, edit only planning,
  checklist, and design documents.
- Deployed behavior: use the `fly-logs` skill early for beta/mainline differences, WebSocket or lobby
  failures, match history, crashes, restarts, and performance spikes.
- Graphics or rendering changes: use the project-local `lab-interact` skill to arrange one small
  authoritative scene, capture a clean Pixi PNG, inspect that returned artifact once, and share only
  its returned Tailnet Preview URL. Keep captures under `target/lab-interact/`; do not use
  Browser/Computer Use, commit image bytes,
  or accept a missing-texture fallback as review evidence.
- Planned implementation phases: use the `phase-runner` skill only for an existing phase file.
- Pre-alpha/prototype plans: follow `docs/context/planning.md`. Keep expensive-to-reverse
  architecture and authority/security on the critical path, limit work before the next playtest or
  measured checkpoint to at most three executable phases, and keep speculative hardening/content
  as a non-runnable deferred backlog.
- Testing and self-play: follow `docs/context/testing.md`, including its replay-inspection workflow.
  For a user-requested live AI-vs-AI demo, run the matchup setup and local server with `--release`
  so debug-only simulation invariants do not interrupt play; use a replay only when requested or
  when the release match cannot run.
- Local visual inspection through Tailscale: Tailscale links are the default delivery channel
  whenever the user needs to view a locally served game, browser result, replay, or other visual
  artifact. Lab Interact screenshots and videos already return an opaque Tailnet Preview URL: share
  that URL directly, never offer the raw `target/lab-interact` path, and do not start a separate
  game server for that artifact. Do not infer the user's device: they may be on a phone or desktop.
  Verify Tailscale state with `tailscale status --json`, start the server with `RTS_ADDR=0.0.0.0:8080 cargo run
  --release` if port 8080 is not already served, then provide an `http://<Tailscale-IP>:8080/...`
  link rather than requiring a beta deployment. For a live spectator AI matchup, use the existing
  launch convention with a fresh room name, for example `/?rtsLaunch=match&rtsRoom=mobile-ai-<unique>&rtsRole=spectator&rtsAi=1:ai_2_1&rtsAi=2:ai_turtle&rtsStart=1`.
  This starts an ordinary authoritative AI-vs-AI room. Prefer a Tailscale Serve HTTPS URL only when
  `tailscale serve status` already reports a working endpoint; do not block a preview on the one-time
  tailnet Serve enablement. Never enable Tailscale Funnel or another public tunnel unless the user
  explicitly requests public sharing. For a standalone artifact such as an MP4 or screenshot, use
  `scripts/tailnet-preview [--ttl <duration>|--keep] <file>`; it copies the artifact into the OS
  temporary directory, prints a Tailnet URL on port 8091, enforces a 24-hour default TTL, and never
  changes the game listener on port 8080. For deterministic, seekable review, run `cargo run --release
  --bin ai-matchup -- ai_2_1 ai_turtle --seed <n> --save-replay <name>` from `server/`, then share
  `/?replayArtifact=<name>` through the same local server. Do not kill or replace an existing
  listener merely to create a preview link.

## Completion

Lead with the outcome. Include the evidence needed to support it, material caveats, and the next
action. For gameplay-affecting changes, explain the player-facing impact. Omit filler and repeated
process narration.
