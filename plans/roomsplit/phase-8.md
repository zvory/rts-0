# Phase 8 - Guardrails And Documentation Closeout

Status: done.

## Goal

Document the final room-task split and add guardrails so the root actor file and child modules do not
grow back into context-heavy hotspots.

## Scope

- Update `docs/design/server-sim.md` and `docs/context/server-sim.md` with the final
  `room_task.rs` and `room_task/*` module responsibilities.
- Update `scripts/check-lobby-architecture.mjs` with explicit size and boundary checks for the root
  actor file and room-task child modules.
- Set budgets from the achieved final sizes after Phases 1-7, not from the pre-split 8k-line file.
- Keep or update existing checks that route snapshot projection through `projection.rs` and lab
  mutation through the approved room-task lab boundary.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if the new
  `server/src/lobby/room_task/**` paths are not grouped under room-runtime ownership.
- Rerun hotspot analysis and record whether the room-runtime logical group remains trackable after
  the split.

## Touch Points

- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- `scripts/check-lobby-architecture.mjs`
- `scripts/hotspot-analysis.mjs` if grouping needs the new child paths
- `plans/hotspots/group-map.md` if grouping needs the new child paths
- `plans/roomsplit/phase-8.md`

## Constraints

- Do not use this phase to move more runtime code unless a tiny checker/doc mismatch requires it.
- Do not bless oversized files as acceptable. If a child module is still too large, document a
  follow-up instead of hiding the problem with a high budget.
- Keep docs aligned with the actual code that landed in earlier phases.
- Keep guardrails specific enough to prevent root-file growth without blocking legitimate small
  additions to child modules.

## Verification

- `node scripts/check-lobby-architecture.mjs`
- `node scripts/hotspot-analysis.mjs --base-ref HEAD --recent-days 14 --limit 0 --output /tmp/rts-roomsplit-hotspots.json`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `git diff --check`

## Manual Testing Focus

Manually smoke the core room flows after the full split: normal match start, live spectator join,
post-match replay, persisted replay room, lab room mutation, dev scenario pause/step, branch staging
and launch, empty-room reset, and drain warning behavior.

## Handoff

After implementation, mark this phase done and summarize the final module map, size budgets added,
hotspot-analysis result, commands run, manual smoke results or remaining checks, and any follow-up
room-runtime simplification that should become a separate plan.

Implementation note: `scripts/hotspot-analysis.mjs` already groups `server/src/lobby/**` under
`server-lobby-runtime`, so the split `room_task/**` files remain trackable as one room-runtime
ownership area without a script or group-map change.
