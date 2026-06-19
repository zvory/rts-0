# Phase 7 - MVP Hardening and Documentation

## Phase Status

- [x] Done.

## Objective

Harden the complete lab MVP, update source-of-truth documentation, add precise guardrails, and
prepare the feature for normal use without broadening scope.

## Work

- Run through the complete MVP workflow and close integration gaps found during implementation:
  lab creation, map selection, full/team vision, setup operations, issue-as commands, import/export,
  empty-room reset, reconnect, and normal-mode isolation.
- Update `plans/lab/architecture.md` if any hypotheses are confirmed, rejected, or refined by the
  implementation.
- Update `docs/design/server-sim.md`, `docs/design/client-ui.md`, `docs/design/protocol.md`, and
  relevant context capsules to describe the final lab boundaries.
- Add or tighten guardrails only for stable boundaries. Good candidates are prohibiting lobby code
  from calling sim internals for lab mutations, keeping lab snapshot calls routed through
  projection policy, and preventing lab panels from being imported by `Match`.
- Add focused end-to-end checks where practical: protocol parity, sim lab ops, room authorization,
  client architecture, scenario round trip, and one live Node smoke path.
- Confirm non-lab flows still pass their focused baselines from `plans/lab/room/mode-matrix.md`.
- Document known non-MVP follow-ups: pause/step/seek, lab flags, timeline/keyframes, public
  scenario storage, multi-operator semantics, visual iteration, and possible `/dev/scenario`
  migration.
- Remove temporary scaffolding, debug affordances, or broad allowlist entries introduced during
  earlier phases.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md`
- `docs/context/server-sim.md`
- `docs/context/client-ui.md`
- `docs/context/protocol.md`
- `plans/lab/architecture.md`
- `plans/lab/mvp/*.md`
- `scripts/check-lobby-architecture.mjs`
- `scripts/check-client-architecture.mjs`
- focused server/client tests from prior phases

## Implementation Checklist

- [x] Complete manual MVP smoke and record the exact flow in the handoff.
- [x] Update design docs and context capsules for the shipped lab boundaries.
- [x] Add precise guardrails for stable lab boundaries.
- [x] Remove temporary debug routes, compatibility shims, or broad checker allowlists.
- [x] Re-run focused lab, protocol, room, client, and scenario tests.
- [x] Re-run focused non-lab baselines touched by lab work.
- [x] Mark this phase done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-lobby-architecture.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

Use exact focused test names if any broad filter would match too much or too little.

## Manual Test Focus

Create a lab, select a non-default map if available, spawn opposing units, switch vision modes,
edit resources/research, issue real move/attack commands, export JSON, reload JSON in a fresh lab,
then verify normal lobby start, replay playback, replay branch launch, and one dev scenario still
work.

## Handoff Expectations

Summarize what the MVP now supports, what was deliberately left out, and what follow-up plan should
come next. Include exact verification results, manual smoke notes, guardrails added, and any
remaining risk that should be watched in playtests.
