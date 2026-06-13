# Faction Architecture Plan

## Purpose

Add support for multiple factions without turning every new tech tree, economy model, or ability
into a cross-codebase rewrite. The immediate target is a second faction with its own tech tree,
unit roster, ability-heavy mechanics, and possibly no steel/oil mining. The longer-term target is a
repeatable faction catalog where additional factions can be added through explicit data and small
behavior hooks instead of scattered `EntityKind` and resource special cases.

This plan follows the multi-phase planning convention in `docs/context/planning.md` and
`plans/README.md`: this `plan.md` is the entry point, each phase has its own file, and every phase
must be implemented, committed, merged to `main`, and pushed before the next phase starts.

## Core Constraints

- Preserve the current faction's gameplay unless a phase explicitly changes it.
- Keep the server authoritative for economy, production, fog, combat, abilities, match outcome, and
  faction validation.
- Add test harnesses and architecture checks before relying on broad refactors.
- Keep protocol mirrors synchronized: `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`,
  `server/crates/sim/src/protocol.rs`, `client/src/protocol.js`, and `docs/design/protocol.md`.
- Keep balance mirrors synchronized: `server/crates/rules/src/defs.rs` /
  `server/crates/rules/src/balance.rs`, compatibility shims, `client/src/config.js`, and
  `docs/design/balance.md`.
- Keep fog authoritative. New faction mechanics, ability events, resource reveals, and remembered
  buildings must not leak hidden enemy state.
- Keep `Game::tick()` panic-free. New faction hooks must treat stale ids, missing definitions, and
  invalid client input as no-ops or notices.
- Do not implement a new faction's full unit roster until the faction brief and rules/balance spec
  are approved. Use placeholder fixture factions only where a phase needs test coverage for the
  faction architecture itself.
- Prefer small registries and typed helpers over stringly scattered checks. If a phase adds an
  exception, add a follow-up or ratchet so the exception does not become the new pattern.
- AI support for new factions is not required for the first human-playable faction unless a phase
  explicitly opts into it.

## Phase Summaries

Phase 0 creates a faction architecture test harness and static inventory before behavior changes.
It records current assumptions around entity kinds, resources, production, start loadouts, command
cards, protocol codes, and AI coupling as explicit tests or reports. The goal is to make existing
single-faction behavior measurable so later phases can prove they preserved it.

Phase 1 introduces faction identity and match-start contracts without changing the current
playable experience. It adds a canonical default faction, carries faction ids through lobby/start/
replay surfaces, and verifies old FFA behavior still starts as today's current faction. This phase
creates the contract all later faction data hangs from.

Phase 2 moves unit, building, upgrade, and tech-tree queries behind faction-aware catalog APIs. It
keeps the existing faction's rules unchanged, but stops production/build/research code from
assuming one global tech tree. This phase is where architectural cleanliness matters most: old
hardcoded checks should either move into catalog data or remain behind named compatibility helpers.

Phase 3 refactors economy and starting loadouts so a faction can opt into different starting
entities and resource usage. It must keep the current steel/oil/supply behavior exactly compatible
while adding a test fixture faction that can start without mining steel or oil. This phase should
not yet build the real new faction; it proves the engine can support one.

Phase 4 expands the ability system into a faction-ready ability registry and reusable effect
surface. It keeps Smoke, Mortar Fire, Artillery Point Fire, and Breakthrough behavior intact while
making discovery, cooldown projection, charges, costs, command-card affordances, and protocol ids
work for additional abilities. This is the main prerequisite for an ability-heavy faction.

Phase 5 updates the client faction surface: command cards, HUD resources, placement menus,
rendering fallbacks, tooltips, hotkeys, and compact protocol decoding. It should preserve the
current faction's UI while adding fixture-faction coverage for alternate build menus, resources,
and ability buttons. This phase is player-facing, so DOM contract tests and smoke coverage are
required.

Phase 6 implements the approved second faction incrementally using the new contracts. It should
start with the faction brief and rules/balance spec, then add the minimal playable vertical slice
before expanding the roster. Each unit, building, upgrade, and ability must land with targeted
server/client tests and factual patch-note bullets.

Phase 7 handles integration hardening, AI decisions, performance, documentation, and rollout. It
decides whether AI can play the new faction now or should be restricted to the current faction, then
tests mixed-faction matches, replay compatibility, fog/security boundaries, and balance docs. This
phase is where the feature becomes safe to make generally selectable.

## Phase Index

0. [Phase 0 - Architecture Inventory and Harness](phase-0.md)
1. [Phase 1 - Faction Identity Contract](phase-1.md)
2. [Phase 2 - Faction-Aware Rules Catalog](phase-2.md)
3. [Phase 3 - Economy and Starting Loadouts](phase-3.md)
4. [Phase 4 - Ability Registry and Effect Hooks](phase-4.md)
5. [Phase 5 - Client Faction Surface](phase-5.md)
6. [Phase 6 - Second Faction Vertical Slice](phase-6.md)
7. [Phase 7 - Integration, AI, Rollout, and Documentation](phase-7.md)

## Testing Strategy

The first implementation work is test infrastructure, not faction content. The expected harnesses
are:

- Rust rule/catalog contract tests that compare the existing faction catalog to today's hardcoded
  behavior.
- Protocol parity tests that fail when faction ids, kind ids, ability ids, or resource payloads are
  not mirrored.
- Snapshot and replay tests that prove faction identity survives live match start, replay start,
  branch start, and compact snapshot paths where relevant.
- Client command-card descriptor tests for build/train/research/ability availability.
- Focused server integration tests for mixed-faction starts and illegal cross-faction commands.
- Fog/security tests for new ability events and alternate resource visibility.
- Architecture checks or reports that flag new direct `EntityKind::Worker`, `steel`, `oil`, or
  current-tech-tree special cases outside approved compatibility modules.

Broad test bundles should still be avoided during development. Each phase document names focused
verification, and the final merge-ready commit should rely on the normal hook for full-suite
coverage unless the phase is docs-only.

## Handoff Rules

After implementing each phase, the implementing agent must provide a handoff message for the next
agent. The handoff must summarize what changed, list verification commands and results, identify
the next phase or discovered follow-up, and name the core features that should be manually tested.
Manual testing notes should cover the changed gameplay/UI surface, not an exhaustive test matrix.

Each phase document must be marked done in the same implementation commit that completes that
phase. Do not mark later phases complete early.

## Non-Goals

- Do not add shared-control, diplomacy, or teams as part of faction support.
- Do not rewrite the renderer or client framework.
- Do not make generic ECS or data-driven scripting a prerequisite for the first new faction.
- Do not make the browser authoritative for faction mechanics.
- Do not require the new faction to have AI support before human play unless Phase 7 decides that
  product scope requires it.

