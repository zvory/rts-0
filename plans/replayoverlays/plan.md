# Replay Analysis Overlays Plan

## Purpose

Add replay analysis overlays that make fights and strategic state easier to read without weakening
server authority, fog guarantees, or replay seek correctness. The first player-facing target is an
army value overlay that shows visible on-screen unit value in steel and oil for each player. Later
phases build the shared overlay system and the server-backed analysis data needed for production,
units, units lost, and resources lost tabs.

## Core Model

- Replay playback remains server-authoritative. The browser reads the current replay snapshot and
  replay analysis payloads; it never reconstructs hidden simulation truth by guessing.
- Overlay UI state that should survive replay seeking must live outside a single `Match` instance
  or be explicitly carried through `App.onStart()`, because replay seeks resend `start` and rebuild
  the client match.
- Viewport-specific overlays use the current camera and projected snapshot only. Global tabs use
  server-authored analysis state so arbitrary seeking produces correct numbers immediately.
- Fog remains authoritative. A viewer only sees analysis for the replay vision mode the server has
  selected for that viewer unless a later phase deliberately defines an all-player spectator-only
  analysis contract.
- Balance values remain mirrored. Any client-side value calculation must use the existing mirrored
  `STATS` cost table or a new synchronized protocol/rules field.

## Phase Summaries

Phase 1 builds the replay overlay shell and persistence surface without adding gameplay analysis.
It introduces a replay-only DOM overlay module, tab state that survives seek-triggered `Match`
recreation, and teardown paths that match the existing client lifecycle. The outcome is a stable
place to mount analysis tabs and viewport overlays.

Phase 2 implements the first useful overlay: visible on-screen army value by player. It computes
unit steel and oil value from the current replay snapshot, current camera viewport, and mirrored
client costs, with no protocol or server changes. The outcome is immediate fight-readability for
visible units under the selected replay vision.

Phase 3 adds the replay analysis protocol and server runtime state for global tab data. It defines
seek-safe analysis snapshots for per-player unit counts, production progress, units lost, and
resources lost, then attaches them to replay playback without exposing hidden live-match data. The
outcome is an authoritative analysis stream that is rebuilt with replay seeks and can support tabs
that cannot be derived from the browser's two-snapshot buffer.

Phase 4 builds the production and unit inventory tabs on top of the server-backed analysis payload.
It renders who is producing what, progress, queue depth, and current unit composition while honoring
the selected replay vision and replay lifecycle. The outcome is a replay observer panel for reading
macro state without clicking individual buildings.

Phase 5 builds the losses and resources-lost tabs and hardens the full overlay system. It renders
units lost, army value lost, and resource-spend/loss summaries, then verifies seek behavior,
vision changes, teardown, responsive layout, and performance. The outcome is a cohesive replay
analysis suite ready for normal match-history and post-match replay use.

## Phase Index

1. [Phase 1 - Replay Overlay Shell](phase-1.md)
2. [Phase 2 - Viewport Army Value Overlay](phase-2.md)
3. [Phase 3 - Server Analysis Contract](phase-3.md)
4. [Phase 4 - Production and Units Tabs](phase-4.md)
5. [Phase 5 - Losses, Resources Lost, and Hardening](phase-5.md)

## Overall Constraints

- Do not implement analysis by replaying command logs in the browser. Replay seek authority belongs
  to the server-side `ReplaySession`.
- Do not add analysis fields to normal active-player snapshots unless the design explicitly proves
  they are owner-safe and needed outside replay/spectator contexts.
- Do not leak owner-only production fields through ordinary fog-filtered player snapshots. Replay
  analysis data must be tied to replay/spectator semantics and documented in `docs/design/protocol.md`.
- Keep the first army-value overlay client-only unless its definition changes to include hidden or
  global army value.
- Keep every overlay module lifecycle explicit: listeners, timers, and DOM nodes need `destroy()`,
  and `Match.destroy()` must call it.
- Preserve the existing replay branch flow. Branch staging freezes or tears down match UI, so
  overlays must hide or destroy cleanly when a replay branch is created.
- Avoid broad test bundles during implementation. Use focused client architecture, protocol parity,
  Rust replay/session tests, and targeted browser smoke checks according to the touched files.
- For gameplay/balance-facing values, collect patch-note bullets during implementation. The likely
  player-facing changes are observer UI affordances rather than unit stat changes.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.

