# Phase 2 - Responsibility Maps

Status: done.

## Goal

Turn the Phase 1 hotspot list into responsibility maps that explain what each large/churned file is
actually doing. The output should distinguish mechanical extraction seams from real architectural
coupling and from areas that are already owned by active plans.

## Scope

- Read the relevant context capsules before mapping source areas:
  - `docs/context/server-sim.md` for simulation services and command flow;
  - `docs/context/client-ui.md` for client match, HUD, state, input, and renderer modules;
  - `docs/context/protocol.md` for protocol mirrors;
  - `docs/context/balance.md` for balance mirrors;
  - `docs/context/testing.md` for large client and simulation test files.
- Create `plans/hotspots/responsibility-map.md`.
- For each selected hotspot, document:
  - primary responsibilities;
  - internal sections or clusters;
  - public entry points and exported helpers;
  - important collaborators;
  - cross-file or mirrored contracts;
  - existing tests that protect the behavior;
  - likely mechanical extraction seams;
  - likely design-coupled seams that should not be moved mechanically;
  - ownership conflicts with active plans.
- Include an architectural-group map that assigns files to stable groups such as room runtime,
  command service, protocol mirror, balance mirror, client match shell, client input, client HUD,
  contract tests, sim tests, AI tests, and styling.
- Treat `server/src/lobby/room_task.rs` as read-only unless the user explicitly says this plan
  should take over from active room cleanup.

## Expected Touch Points

- `plans/hotspots/responsibility-map.md`
- `plans/hotspots/phase-2.md`

Do not modify runtime source, tests, protocol files, client modules, CSS, or design docs in this
phase.

## Mapping Priorities

Start with the top hotspots from Phase 1. Unless the Phase 1 evidence says otherwise, include at
least:

- `tests/client_contracts.mjs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/src/lobby/room_task.rs`
- protocol mirror files such as `server/crates/protocol/src/lib.rs` and `client/src/protocol.js`
- balance mirror files such as `server/crates/rules/src/balance.rs` and `client/src/config.js`
- client shell files such as `client/src/match.js`, `client/src/hud.js`, and `client/src/state.js`
- one representative large sim or AI test file if tests dominate the hotspot list

## Analysis Questions

- Which files are too large because they contain several independent responsibilities?
- Which files are large but cohesive enough that splitting them would mostly hide complexity?
- Which code is acting as an orchestration shell and should probably remain a shell with smaller
  injected collaborators?
- Which test files can be split by contract area without weakening coverage?
- Which mirrored contract files need a group-level cleanup plan rather than individual file splits?
- Which seams would reduce the amount a future model or human must load at once?

## Verification

- `git status --short`
- `git diff --check`
- Check every mapped file still exists on current `origin/main`.
- Spot-check that each cited responsibility or collaborator is backed by current source references,
  not stale memory or old paths.

## Manual Review Focus

Review whether the maps describe real responsibilities in plain language. The key question is
whether another engineer could read the map and know where a safe extraction boundary might be
without loading the whole hotspot file first.

## Handoff

After implementation, mark this phase done and summarize the mapped hotspots, the cleanest
mechanical seams, the riskiest coupled seams, and any active-plan ownership constraints. Tell the
next phase which extraction candidates should be ranked first.
