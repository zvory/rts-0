# Phase 2 - Consolidate Server Invariants

Status: Incomplete

## Objective

Remove a few high-risk duplicate server contracts and give replay/Lab reconstruction one safe
failure boundary. Keep gameplay, wire shapes, fog projection, and ordinary command semantics
unchanged; this is a focused authority cleanup, not a server rewrite.

## Work Themes

- Make `rts-rules` the typed authority for upgrade and ability identities, stable string ids, and
  ability target modes. Remove the competing exhaustive identity registries from `rts-sim` (or
  retain only thin compatibility re-exports where they materially reduce churn).
- Keep simulation-specific ability effect dispatch in the simulation. Declarative catalog data
  should come from rules, and registry lookup must return a normal error or typed absence instead
  of panicking when definitions drift.
- Put the ordinary and Lab-bypass command unit-list caps in one dependency-safe shared location and
  consume those values from both runtime command admission and Lab replay validation. Preserve the
  existing limits and whole-command rejection behavior.
- Add one reusable panic-contained reconstruction boundary for replay seeking, Lab timeline seeks,
  and Lab replay import. Rebuild a candidate game/session, return a structured failure on error or
  panic, and replace authoritative room state only after the candidate succeeds.
- Add focused parity and failure tests that make these authority boundaries difficult to duplicate
  accidentally. Update the relevant server-simulation and hardening design sections to name the
  final owners.

## Non-goals

- Do not broadly narrow or reorganize the `Game` API, decompose `RoomTask`, or restructure tick
  services.
- Do not consolidate AI profile registries in this phase.
- Do not change balance values, ability effects, research availability, command budgets, replay
  formats, protocol payloads, or fog/visibility behavior.
- Do not introduce a generalized registry framework or compatibility layer beyond what current
  callers need.

## Likely Touch Points

- `server/crates/rules/src/faction.rs` and a small adjacent rules module if that gives the typed
  identities a clearer home
- `server/crates/sim/src/game/{ability,upgrade,command}.rs` and direct typed-kind consumers
- `server/crates/contract/src/lib.rs` or another existing dependency-safe contract module for
  shared command-list limits
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/protocol/src/lab_replay.rs`
- `server/src/lobby/replay_session.rs`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/lab/replay.rs`
- focused Rust tests near those modules
- `docs/design/server-sim.md` and `docs/design/hardening.md`

## Verification

- Focused Rust tests proving:
  - every typed upgrade and ability round-trips through its stable id and resolves to one rules row
  - all catalog entries have valid, total simulation handling without a panic-on-drift path
  - runtime admission and replay validation consume the same ordinary/Lab cap values
  - replay and Lab reconstruction leave the prior authoritative state intact after an injected
    error or panic, while successful rebuilds still commit at the requested tick
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-rules -p rts-protocol -p rts-sim -p rts-server`
- `node tests/protocol_parity.mjs`
- `node scripts/check-wiki.mjs`
- `node scripts/check-crate-boundaries.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Focus

In one local session, confirm representative research plus self-targeted and world-targeted
abilities behave as before. Exercise an ordinary replay seek, a Lab rewind/seek, and a Lab replay
import; a rejected or malformed reconstruction should report an error and leave the current room
usable. Spot-check that ordinary and Lab-bypass command limits still accept and reject at the
documented boundaries.

## Handoff

Mark this phase done in its implementation commit. Report the final authority location for typed
rules and command-list caps, any compatibility re-exports retained, every reconstruction caller
moved behind the safe boundary, and the focused failure evidence. Call out remaining duplicate
registries or panic-prone reconstruction paths as explicit follow-up observations without expanding
this phase into broader server cleanup, and name the core research, ability, replay, and Lab flows
the next agent should manually test.
