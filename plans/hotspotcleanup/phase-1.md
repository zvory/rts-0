# Phase 1 - Client Contract Foundation

Status: planned.

## Goal

Create the helper and module pattern for splitting `tests/client_contracts.mjs` while preserving the
stable `node tests/client_contracts.mjs` command. Move only a small, low-risk slice first so later
contract splits can follow an established local shape.

## Scope

- Read `plans/hotspots/extraction-candidates.md` and the `tests/client_contracts.mjs`
  responsibility map.
- Create `tests/client_contracts/` helper modules for shared assertions, fake DOM/Pixi/audio/storage,
  and common fixtures where those helpers are currently copied or tightly clustered in the top-level
  file.
- Move one or two low-risk contract sections into imported domain modules. Prefer sections with few
  mirror-contract hazards, such as settings/profile helpers, frame profiler helpers, score helpers, or
  other isolated pure-client checks.
- Keep `tests/client_contracts.mjs` as the single command entry point and make it import/run the new
  modules.
- Preserve all existing assertions, fixtures, pass/fail output, and dependency-free Node execution.
- Keep protocol/config mirror assertions in the top-level file or move them only if the parity checks
  are run and the new location remains obvious.

## Touch Points

- `tests/client_contracts.mjs`
- new `tests/client_contracts/*.mjs` helper or domain files
- `plans/hotspots/group-map.md` and `scripts/hotspot-analysis.mjs` only if the new paths are not
  already grouped under `tests/client_contracts/`
- `plans/hotspotcleanup/phase-1.md`

## Constraints

- Do not weaken or delete architecture, protocol, config, HUD, state, input, renderer, audio, lab, or
  lobby assertions.
- Do not introduce browser-only test dependencies. The suite must still run directly under Node.
- Do not make a broad helper monolith that simply moves the old top-level burden to another file.
- Do not rename the command or require changes to CI suite selection unless implementation proves it
  is necessary.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if any protocol or compact-code assertions move
- `git diff --check`

## Manual Testing Focus

No browser manual testing should be required for this phase if the moved sections are pure contract
coverage. Manually review failure messages and file names so a future failing assertion still points
to the relevant contract area.

## Handoff

After implementation, mark this phase done and summarize the helper layout, moved sections, commands
run, any assertions deliberately left in the top-level runner, and the recommended next sections for
Phase 2.
