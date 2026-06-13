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

- This is a pre-alpha refactor with **zero backwards compatibility guarantees**. Old replay
  artifacts, persisted match-history replays, old protocol payloads, old compact snapshot versions,
  and old clients may break. Preserve current live gameplay behavior, not old saved data formats.
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
- AI support for new factions is explicitly out of scope until a phase opts into it. AI slots must
  stay current-faction-only and server/lobby validation must enforce that.
- Prediction/WASM may be disabled for non-default factions until the WASM simulation and adapter
  intentionally support the new faction contracts.
- Faction definitions start as authoritative Rust catalog data. The client should consume a
  mechanically generated or mechanically checked JS mirror; do not hand-maintain divergent faction
  data.
- Runtime and wire entity identity stays global for now. Faction catalogs control which global
  units, buildings, upgrades, abilities, labels, stats, and commands are legal for a player.
- Factions may have completely different economies. The architecture must not assume steel/oil are
  permanent global fields.
- Avoid faction-specific map resource objects for the first new faction unless the approved faction
  brief requires them. Prefer universal map resources that some factions ignore, or non-map
  resource generation through buildings/abilities/timers.

## Approved Design Decisions

- **Compatibility:** no backwards compatibility requirement for replay artifacts, match-history
  replay payloads, compact snapshot versions, old clients, or old protocol payloads.
- **Economy:** factions can have completely different resource sets, so generic resource payloads
  are part of the architecture rather than an optional later migration.
- **Data ownership:** faction catalogs are Rust-authoritative. Client data is generated or
  mechanically checked from the Rust catalog.
- **Entity identity:** keep one global `EntityKind`/wire-kind namespace for now; faction catalogs
  decide availability and rules.
- **Command legality:** the server rejects out-of-faction build/train/research/economy/ability
  commands even if the referenced global kind exists.
- **Starts:** faction starting loadouts define starting entities, resources, supply model, and
  optional opening upgrades/flags.
- **AI:** AI remains current-faction-only until an explicit AI phase implements another faction.
- **Prediction:** non-default factions can disable prediction/WASM until prediction is updated.
- **Client catalog:** keep local client descriptors, but generate or mechanically verify them from
  authoritative Rust data. Do not switch to server-sent command-card descriptors in this plan.
- **Abilities:** build the smallest registry/effect surface needed for known abilities and the
  approved second faction; do not build a generic scripting engine.

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
hardcoded checks should either move into catalog data or remain behind named compatibility helpers,
and client catalog data must become generated or mechanically parity-checked from Rust.

Phase 3 replaces fixed steel/oil economy payload assumptions with a generic resource contract. It
may break old snapshots and replays because this project does not require pre-alpha backwards
compatibility. The current faction should still display and spend Steel, Oil, and Supply exactly as
players expect, but the protocol and HUD should be capable of representing different spendable
resource sets.

Phase 4 moves starting entities, starting resources, supply rules, opening upgrades, and fixture
economy choices into faction loadout definitions. It keeps the current faction's start behavior
unchanged while proving a test fixture can start with a different economy and no steel/oil mining
dependency. This phase should avoid faction-specific map resources unless a brief has already
approved them.

Phase 5 turns existing abilities into registry-backed discovery and projection without changing
their effects. It preserves Smoke, Mortar Fire, Artillery Point Fire, Breakthrough, and legacy
Charge compatibility while routing ids, carriers, target modes, cooldowns, charges, costs, and
command-card affordances through faction-aware definitions. This phase should mostly be parity
tests and command validation, not a new effect engine.

Phase 6 adds the reusable ability effect hooks needed by the approved second faction. It should
generalize only concrete patterns that are actually needed, such as self buffs, targeted world
effects, delayed impacts, area effects, toggles/autocast, or limited charges. Any remaining one-off
effect code is acceptable if the registry clearly documents how Phase 10 should add signature
abilities.

Phase 7 updates the client faction surface: command cards, HUD resources, placement menus,
rendering fallbacks, tooltips, hotkeys, and compact protocol decoding. It should preserve the
current faction's UI while adding fixture-faction coverage for alternate build menus, resources,
and ability buttons. This phase is player-facing, so DOM contract tests and smoke coverage are
required.

Phase 8 is the approval gate for the real second faction. It creates or references the faction
brief plus rules/balance spec, including economy, loadout, production, roster slices, abilities,
art readability, and explicit AI/prediction policy. No implementation code for the real faction
should land until this phase is approved.

Phase 9 implements the second faction's start, economy, and first production path. It should be a
playable but narrow slice: the faction can enter a match, see the right resources, create its basic
production loop, and reject all illegal cross-faction commands. It should keep AI and prediction
disabled for the new faction unless the approved brief says otherwise.

Phase 10 adds the second faction's first combat and signature ability slice. It should include one
baseline combat unit, one signature ability-heavy unit, readable client art, fog-safe events, and
targeted server/client tests. The goal is a short playable match that demonstrates the faction's
mechanical identity without trying to finish the whole roster.

Phase 11 expands the roster as approved, hardens integration, updates docs, and decides rollout.
It verifies mixed-faction match shapes, replay/branch/dev scenario flows under the new non-backcompat
schema, match history, spectators, quickstart, AI restrictions, prediction restrictions, performance,
and balance documentation. This phase is where faction choice becomes ready for regular playtesting.

## Phase Index

0. [Phase 0 - Architecture Inventory and Harness](phase-0.md)
1. [Phase 1 - Faction Identity Contract](phase-1.md)
2. [Phase 2 - Faction-Aware Rules Catalog](phase-2.md)
3. [Phase 3 - Generic Resource Contract](phase-3.md)
4. [Phase 4 - Faction Starting Loadouts](phase-4.md)
5. [Phase 5 - Ability Registry Parity](phase-5.md)
6. [Phase 6 - Ability Effect Hooks](phase-6.md)
7. [Phase 7 - Client Faction Surface](phase-7.md)
8. [Phase 8 - Second Faction Brief and Rules Spec](phase-8.md)
9. [Phase 9 - Second Faction Start and Economy Slice](phase-9.md)
10. [Phase 10 - Second Faction Combat and Signature Ability Slice](phase-10.md)
11. [Phase 11 - Roster Expansion, Integration, and Rollout](phase-11.md)

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
- Prediction/WASM compatibility checks proving non-default factions either disable prediction with
  a clear reason or are intentionally supported by the WASM adapter.
- Generated-client-catalog or catalog-parity tests proving JS descriptors match Rust-authoritative
  faction data.

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
- Do not implement server-sent command-card descriptors in this plan.
- Do not preserve old replay/protocol compatibility during this pre-alpha refactor.
- Do not require the new faction to have AI support before human play unless Phase 11 decides that
  product scope requires it.
