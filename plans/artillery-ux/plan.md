# Artillery UX Implementation Plan

## Purpose

Implement the artillery UX requirements in [requirements.md](requirements.md): separate Point Fire
and Blanket Fire orders, automatic in-place setup or redeploy for artillery fire commands, range
locking instead of raw out-of-range rejection, queued planning support, client previews, and the
25-tile artillery minimum range balance update. The final behavior should let players choose a
target point or direction without making artillery walk itself into range. Server simulation remains
authoritative for command admission, locked target storage, setup/redeploy, queue promotion,
ammunition, reloads, deterministic shell placement, fog, and damage.

## Phase Summaries

### [Phase 1 - Mirrored Blanket Fire Contract Skeleton](phase-1.md)

Add the `blanketFire` ability and order-stage vocabulary across Rust protocol metadata, Rust
faction catalogs, JS protocol constants, and client rules mirror without exposing the command card
button yet. Add the client-visible blanket radius balance constant and keep Point Fire behavior
unchanged. This phase gives later server and client phases a real mirrored command identity instead
of a hidden Point Fire flag.

### [Phase 2 - Authoritative Target Locking And Point Fire Setup](phase-2.md)

Teach the server to convert raw Point Fire clicks into stored effective fire points by locking each
artillery piece to its valid range band along the origin-to-click ray. Change immediate and queued
Point Fire so packed or misaligned artillery accepts the fire order, sets up or redeploys in place,
then begins firing after deployment instead of requiring a separate setup command first. Apply the
artillery minimum range balance update from 15 tiles to 25 tiles with the Rust and client mirrors
updated together.

### [Phase 3 - Blanket Fire Server Runtime](phase-3.md)

Implement Blanket Fire as its own terminal artillery fire order using the mirrored `blanketFire`
identity from phase 1. The order stores the locked blanket center, owns setup or redeploy like Point
Fire, spends the same ammunition, uses the same reload and shell delay, and samples deterministic
impact points inside a 15-tile radius around the center. This phase should prove replay-stable
sampling, terminal queue behavior, stale target safety, and fog/event parity with existing
artillery fire.

### [Phase 4 - Command Card And World Targeting UX](phase-4.md)

Expose Blanket Fire in the artillery command card as a separate targeted button and keep Point Fire
as a distinct targeted command. Add advisory client-side target locking, setup/redeploy cone
previews, and command feedback that mark the stored effective point rather than the raw cursor
where the client can mirror the server lock. Blanket Fire feedback must include the 15-tile blanket
radius while preserving existing Point Fire and support-weapon affordances.

### [Phase 5 - Queued Planning, Minimap Targeting, And Reconciliation](phase-5.md)

Extend queued artillery planning so Point Fire and Blanket Fire target from available future queued
positions and frozen setup previews instead of only the current artillery position. Make minimap
targeting issue the same fire semantics as world targeting, even if minimap hover cannot show every
per-gun preview. Reconcile local previews with authoritative `orderPlan` snapshots and clear stale
planning feedback when selection, Stop/Hold, replacement orders, rejection, or queue changes make it
misleading.

### [Phase 6 - Integration, Documentation, And Playtest Hardening](phase-6.md)

Close gaps after the server and client phases land by aligning docs, tests, patch notes, and manual
playtest coverage. Verify that protocol, balance, server-sim, and client-ui docs all describe the
same command semantics, target locking, queueing, and visibility behavior. Add or adjust focused
regression coverage for mixed selections, stale queued states, replay determinism, and final
client/server preview agreement.

## Overall Constraints

- Keep [requirements.md](requirements.md) as the product behavior source for this plan. If a phase
  discovers a requirement conflict or missing product decision, stop that phase as blocked instead
  of inventing new gameplay.
- Preserve server authority. The client may preview, but the server owns locked effective targets,
  setup/redeploy decisions, queue admission and promotion, shell sampling, cost spending, cooldowns,
  fog-gated events, and damage.
- Do not make artillery walk, path, or stage itself to put a raw click in range. Target locking
  happens along the ray from the artillery origin or planned origin; if no valid in-map point exists
  in the range band, that gun ignores the command.
- Store locked effective fire points, not raw clicked points, in authoritative orders and queued
  stages. Blanket Fire stores a locked center; sampled impacts are not re-clamped to range or cone.
- Keep Point Fire and Blanket Fire as separate mirrored command identities. Do not implement
  Blanket Fire as a client alias or as Point Fire with an untracked hidden flag.
- Keep both fire modes terminal per artillery unit. Later queued unit orders must not append behind
  an accepted Point Fire or Blanket Fire stage for that same gun.
- Keep `Game::tick()` panic-free. Stale ids, dead artillery, under-construction artillery, invalid
  coordinates, impossible map clamps, unaffordable ammunition, and cleared queues must be safe
  no-ops or existing notices.
- Maintain fog guarantees. `artilleryTarget`, `artilleryImpact`, `artilleryFiring`, attack reveals,
  and order-plan projection must not reveal hidden enemy positions beyond the established artillery
  rules.
- Update mirrored contracts together. Protocol vocabulary changes touch `server/crates/protocol`,
  `server/src/protocol.rs` if needed, `client/src/protocol*.js`, and `docs/design/protocol.md`.
  Balance changes touch Rust rules, client mirrors, generated catalog expectations, and
  `docs/design/balance.md`.
- Respect client architecture. Client modules should receive collaborators through existing
  injection paths, and any new listeners or GPU resources must be cleaned up through `destroy()`.
- Collect factual patch-note bullets during each gameplay phase: new Blanket Fire command, Point
  Fire auto-setup, range locking, 25-tile minimum range, queued fire planning, and any playtest risk.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- `node tests/protocol_parity.mjs` after ability/order-stage vocabulary, compact code, or protocol
  docs changes.
- `node scripts/check-faction-catalog-parity.mjs` after client-visible ability descriptors,
  balance constants, faction catalogs, or mirrored config changes.
- `node scripts/check-wiki.mjs` after visible rules, catalog, upgrade, or ability metadata changes.
- Focused Rust tests for command admission, order queue promotion, artillery setup/redeploy,
  target locking, deterministic Blanket Fire sampling, and shell execution.
- Focused client contract tests for command-card buttons, hotkeys, input targeting, minimap
  targeting, command feedback, and renderer preview view-model behavior.
- `node scripts/check-client-architecture.mjs` after client module or wiring changes.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` after
  sim service boundary or dependency changes.
- `node scripts/check-docs-health.mjs` for plan/doc-heavy phases or docs-only changes.
- `git diff --check`.

## Handoff Requirement

After implementing each phase, the implementing agent must provide a compact handoff message that
states whether the phase completed or blocked, what changed, what verification ran, gameplay impact,
what the next executor should know, and what a human should manually test later. Manual testing
notes should name core gameplay scenarios, not an exhaustive matrix. Each phase branch must be
pushed as an owned PR with auto-merge armed, and the executor must wait for a definite PR merge with
the phase head reachable from `origin/main` before reporting the phase complete or starting the next
phase.

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge.

```bash
scripts/phase-runner.sh --plan artillery-ux 1 --pr --wait
scripts/phase-runner.sh --plan artillery-ux 2 --pr --wait
scripts/phase-runner.sh --plan artillery-ux 3 --pr --wait
scripts/phase-runner.sh --plan artillery-ux 4 --pr --wait
scripts/phase-runner.sh --plan artillery-ux 5 --pr --wait
scripts/phase-runner.sh --plan artillery-ux 6 --pr --wait
```
