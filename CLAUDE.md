# CLAUDE.md

Guidance for working in this repo. **Read only the relevant context capsule first** —
`docs/context/` has small, task-scoped capsules (server-sim, client-ui, protocol, balance,
testing, deployment) that point into the relevant `docs/design/` file and code you actually need.
See [docs/context/README.md](docs/context/README.md) for the capsule index.

**Read only the relevant design file when you are changing a cross-file contract** — the wire
protocol (server ⇄ client), the `Game` API seam, the balance mirror, fog rules, or the hardening
surface. `docs/design/*.md` are the source of truth by contract area; capsules are pointers, not
copies. Keep the relevant design file updated in the same change whenever you alter a contract, and
refresh the capsule's section list if structure shifts.

A server-authoritative Bewegungskrieg server (`server/`, axum + tokio) runs the one authoritative
simulation and also serves the **HTML/CSS/JS + PixiJS** client (`client/`). Clients send commands;
the server simulates at 30 Hz and sends per-player, fog-filtered snapshots.

## Completion Summary

- Once you're done a task, explain in plain language what you've done and the gameplay impact.

## Balance and Gameplay Patch Notes

- For balance or gameplay changes, collect patch-note bullets as you work so the final summary,
  commit message, and any release notes can explain player-facing impact clearly.
- Patch notes should call out changed unit/building stats, economy tuning, combat behavior, UI
  affordances that affect play, and any expected strategic impact.
- Keep patch notes factual and evidence-backed. If the impact is uncertain, say what changed and
  what should be watched in playtests instead of guessing.

## New Unit Workflow

- New-unit work is requirements-gated. If a task references the new unit checklist or asks to start a
  new unit, begin with Phase 0 and Phase 1 only.
- Do not implement Rust, JS, protocol, balance, art, tests, or other implementation files until the
  unit brief and rules/balance spec are complete and the user explicitly authorizes implementation.
- Before that gate, only edit planning, checklist, and design documents. Stop after the brief/spec
  handoff unless the user explicitly says to proceed with implementation code.

## Parallel Worktrees

- For parallel feature work (which is always the case), each terminal/agent must work in its own
  git worktree. Do not run two coding agents in the same checkout.
- Always make a worktree.
- Before making changes, verify the checkout and branch:

  ```bash
  git rev-parse --show-toplevel
  git branch --show-current
  git status --short
  ```

- Use one branch per worktree. Branch names must start with `zvorygin/`.
- Create project worktrees under `/tmp/rts-worktrees` to keep the repo directory clean. Use a
  descriptive directory name that matches the branch:

  ```bash
  mkdir -p /tmp/rts-worktrees
  git worktree add /tmp/rts-worktrees/my-feature -b zvorygin/my-feature main
  ```

- Agents must only edit files inside their assigned worktree. Do not edit the original checkout or
  another agent's worktree.
- Coordinate write ownership before starting. If another agent owns a file or module, do not edit it
  unless explicitly told to. Avoid parallel edits to shared contracts such as protocol, config,
  generated files, or design docs.
- Stage and commit only files belonging to the current task. Never revert unrelated changes.
- When the task is ready for review, push the worktree branch, open an owned PR, arm auto-merge,
  then wait for the PR to merge before claiming completion. Use `scripts/agent-pr.sh` for the
  standard owned-PR body, labels, and auto-merge setup, then `scripts/wait-pr.sh <pr>` to wait until
  GitHub reports the PR merged and the head SHA is reachable from `origin/main`.
- Do not merge, rebase, delete, or otherwise alter another agent's branch/worktree unless explicitly
  asked.
- If running local servers, use different ports per worktree or stop the other server first.

## Git / GitHub

- The default branch is `main`.
- `main` is protected in GitHub: normal updates require a PR, an up-to-date branch, and the required
  `./tests/run-all.sh` check from the `Main test gate` workflow. Admin bypass is reserved for emergency repair and
  explicitly authorized migration work only.
- Ordinary commits run cheap local hooks, currently staged whitespace checks through
  `git diff --cached --check`. The hooks do not run the full local suite by default.
- During development, run only targeted tests that match the files or contracts changed. Use
  GitHub Actions as the authoritative full gate through the PR lifecycle.
- If a cheap local hook fails, fix the staged diff instead of bypassing it unless the task is
  explicitly docs-only and the failure is conclusively unrelated.
- Commit messages should be detailed. Use a clear subject and include a body when the change has
  gameplay impact, contract changes, testing nuance, or non-obvious reasoning.
- Use one `zvorygin/` branch per worktree.
- When work is complete, stage and commit only files that belong to the current task.
- Push the branch to `origin`, open an owned PR, and arm auto-merge. The PR body or labels must make
  ownership, lifecycle mode, auto-merge state, focused verification, and any blockers clear enough
  for another agent to audit. The standard command is:

  ```bash
  scripts/agent-pr.sh --verification "focused check command(s) passed"
  ```

  `scripts/agent-pr.sh` writes the `rts-agent-pr:v1` metadata block, applies `agent-owned` plus
  `automerge` or `needs-human`, and runs `gh pr merge --auto --merge`. If the branch needs human
  input, pass `--no-auto-merge` and explain the blocker in the PR body or handoff.
- After `scripts/agent-pr.sh` opens or updates the PR, run `scripts/wait-pr.sh <pr>`. Do not report
  the task complete until that command confirms GitHub merged the PR and the head SHA is reachable
  from `origin/main`.
- Normal completion states are: merged to `main`; or blocked with the PR link plus the exact failing
  check, merge conflict, GitHub/API failure, or human decision needed. A PR that is merely opened,
  owned, and auto-merge armed is a pending handoff, not completion.
- For serial phase work, do not start the next phase from an assumed merge. Use the same
  `scripts/wait-pr.sh <pr>` gate after opening each PR; it exits successfully only after GitHub
  reports the PR merged and the phase head SHA is reachable from `origin/main`.
- For unattended serial phase execution, use `scripts/phase-runner.sh --pr --wait`. That script is
  the stable compatibility entrypoint for the Rust runner in `server/crates/phaserunner`. Use
  `scripts/phase-runner.sh --pr` only when intentionally stopping after the first owned PR is opened
  and auto-merge is armed; treat that result as a pending handoff until `scripts/wait-pr.sh <pr>`
  confirms the merge.
- To audit outstanding agent PRs, run `scripts/pr-sweep.sh`. It lists open `agent-owned` and
  `zvorygin/*` PRs with owner, age, head SHA, auto-merge state, checks, and flags for stale,
  failed, conflicted, missing-owner, or needs-human states.
- For recovery from failed CI, stale branches, missing auto-merge, closed PRs, GitHub API outages,
  emergency direct pushes, cleanup, canaries, or alternate runners, see
  `docs/pr-first-workflow.md`.
- Do not merge, push to `main`, or bypass branch protection unless the user explicitly authorizes
  emergency or migration repair work.

## Commands

```bash
# Run (serves client + /ws on the configured RTS_ADDR; open the printed URL)
cd server && cargo run            # add --release for the fast build

# Build / lint / format
cd server && cargo build && cargo clippy && cargo fmt
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture

# Targeted tests — start the server first for live Node suites, then (from repo root):
node tests/server_integration.mjs     # dep-free, full server pipeline
node tests/regression.mjs             # dep-free, hardening/DoS/robustness guards
node tests/ai_integration.mjs         # dep-free, AI opponent lobby flow (add/remove/start)
tests/run-all.sh --no-rust            # live Node suites + headless-Chrome smoke with shared deps

# Simulation behavior, including scripted self-play (no running server needed):
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default
```

Do not run broad test bundles by default. Pick the smallest relevant target for the changed area
(for example a focused Rust test, one live Node suite for touched server/client behavior, or an
architecture check for seam changes), then rely on the PR `./tests/run-all.sh`
check for full-suite coverage.

There is **no JS build step** (plain ES modules + PixiJS from CDN). The client is served from
`../client` relative to the server crate, so `cargo run` from `server/` is the whole dev loop.

## Deployed Log Checks

- When investigating behavior that may differ on beta/mainline, including post-deploy regressions,
  WebSocket/lobby failures, match-history recording, server crashes, restarts, or performance
  spikes, check Fly logs early with `scripts/fly-logs.sh beta recent` or
  `scripts/fly-logs.sh mainline recent`.
- For live reproduction, bound tailing so it cannot stream forever:
  `timeout 30 scripts/fly-logs.sh beta tail`.
- `scripts/fly-logs.sh` reads `FLY_API_TOKEN` from the environment, this worktree's ignored
  `.env`, or the main worktree's ignored `.env`. Never commit, print, or paste the token.

## Invariants — do not break these

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

## Conventions

- **Rust:** edition 2021, `cargo fmt`, keep warnings low. Prefer small pure helpers in
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

## Gotchas

- Debug builds have overflow checks **on** (a bad `Build` coord can panic in `cargo run` but
  silently wrap in `--release`) — that's why placement math is `checked_*`. Keep it that way.
- Live Node tests need a **running** server on the test runner's private port; they are live
  integration scripts, not Rust test binaries. Run only the suite that matches the changed area
  unless the user explicitly asks for broader local coverage.
- If a self-play test fails and the reason is not immediately obvious, do **not** sink time into
  speculative debugging first. Start a fresh server on its own port, then use
  the macOS `open` command to open a local self-play spectation replay so the user can inspect the
  failure state directly. Do
  **not** use the Browser skill for this flow. Use
  `open "http://localhost:<port>/dev/selfplay?replay=<artifact_name>"` (for example
  `open "http://localhost:<port>/dev/selfplay?replay=manual_worker_rush_latest"`), not the in-app
  browser.
- A 1-player match is a never-ending sandbox; only 2+ player matches resolve to a winner. Empty
  rooms reset to lobby so a room name is never stuck mid-match.
