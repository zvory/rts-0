# Faction Architecture Plan

## Purpose

Add support for multiple factions without turning every new tech tree, roster, or ability into a
cross-codebase rewrite. The immediate target is a second faction with its own tech tree, unit
roster, and ability-heavy mechanics while continuing to use the game's current Steel, Oil, and
Supply economy. The longer-term target is a repeatable faction catalog where additional factions
can be added through explicit data and small behavior hooks instead of scattered `EntityKind`
special cases.

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
- The generated or mechanically checked client catalog path is a hard gate: Phase 2 is not complete
  until there is a named command or test that fails when Rust catalog data and JS descriptors drift.
- Runtime and wire entity identity stays global for now. Faction catalogs control which global
  units, buildings, upgrades, abilities, labels, stats, and commands are legal for a player.
- A global entity, upgrade, or ability id may be shared across factions only when its gameplay
  semantics are identical for every faction that can use it. If behavior, stats, production role, or
  ability meaning diverges, add a distinct global id and gate it through faction catalogs.
- Steel, Oil, and Supply remain global game resources for this plan. Faction catalogs may define
  different starting amounts, costs, supply use, and production dependencies, but they must not
  introduce new resource payload shapes or faction-specific map resource objects.
- Faction assignment must be explicit for every lifecycle path before real second-faction content
  ships. Normal lobby selection may stay hidden, but dev/test starts, quickstart, replay, branches,
  AI seats, self-play, dev scenarios, and match-history replay must each have a documented source of
  faction truth and fail-closed behavior.
- A future generic-resource migration is allowed, but it should be its own self-contained plan after
  real faction mechanics prove the need. Do not pre-abstract snapshots, replay analysis, HUD rows,
  map resources, or scoring for hypothetical resources during this faction rollout.

## Approved Design Decisions

- **Compatibility:** no backwards compatibility requirement for replay artifacts, match-history
  replay payloads, compact snapshot versions, old clients, or old protocol payloads.
- **Faction names:** the existing faction is **Kriegsia** and its canonical id is `kriegsia`.
  The first real new faction is **Ekaterina** and its reserved id is `ekaterina`. Earlier Phase 1/2
  work introduced `steel_vanguard` as a temporary current-faction id; Phase 3A must rename that id
  before later phases build durable replay, command, hotkey, or prediction contracts on it.
- **Economy:** all factions use the existing Steel, Oil, and Supply resource contract for now.
  Different faction flavor can be represented through costs, starting stockpiles, production rules,
  labels, or later balance changes; truly generic resources are deferred to a separate future plan.
- **Data ownership:** faction catalogs are Rust-authoritative. Client data is generated or
  mechanically checked from the Rust catalog.
- **Entity identity:** keep one global `EntityKind`/wire-kind namespace for now; faction catalogs
  decide availability and rules.
- **Command legality:** the server rejects out-of-faction build/train/research/economy/ability
  commands even if the referenced global kind exists.
- **Starts:** faction starting loadouts define starting entities, resources, supply model, and
  optional opening upgrades/flags.
- **AI:** AI remains current-faction-only until an explicit AI phase implements another faction.
- **Prediction:** prediction stays enabled for supported local-player factions only. It must be
  disabled when the local player is on an unsupported faction, but remote opponents using an
  unsupported faction do not by themselves disable the local player's prediction path.
- **Client catalog:** keep local client descriptors, but generate or mechanically verify them from
  authoritative Rust data. Do not switch to server-sent command-card descriptors in this plan.
- **Abilities:** Phase 7 must not depend on the real second-faction brief. It may only add hooks
  justified by existing current-faction abilities or architecture fixtures; Phase 11 may add tightly
  scoped hooks for the approved second-faction signature ability if needed.
- **Command identity:** command ids used by command cards and hotkey profiles must include enough
  namespace to avoid collisions across faction-specific build/train/research/ability actions.
  Global tactical commands and global production commands remain un-namespaced for now. Custom
  hotkey bindings are stored per faction; grid mode may stay global because it follows rendered
  slot position rather than command identity.

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

Phase 3 is split into four executor-sized guardrail phases before faction-specific starts or UI
become visible. Phase 3A corrects the canonical current-faction id to `kriegsia`, reserves
`ekaterina`, adds the server validation contract, and updates the lifecycle matrix. Phase 3B makes
AI and prediction fail closed; Phase 3C defines command ids and per-faction hotkeys; Phase 3D
hardens replay, branch, and dev lifecycle tests.

Phase 4 keeps Steel, Oil, and Supply as the global resource contract and hardens faction-aware
resource policy around that existing shape. It removes generic-resource work from the critical path
while ensuring costs, affordability, supply, score values, replay analysis, and HUD assumptions are
documented as intentionally steel/oil/supply-shaped. This phase should add pressure against hidden
cross-faction economy bugs without changing current gameplay or snapshot resource payloads.

Phase 5 moves starting entities, starting Steel/Oil/Supply values, supply rules, opening upgrades,
and fixture start choices into faction loadout definitions. It keeps the current faction's start
behavior unchanged while proving a test fixture can use a different loadout and command set within
the existing economy. This phase should not add faction-specific map resources.

Phase 5.5 tightens the architectural seams discovered after the first implemented faction phases.
It makes lower-level catalog access fail closed for unknown factions, narrows old global start
resource compatibility APIs, strengthens catalog/checker guardrails, and prepares Phase 6 to
consolidate ability metadata instead of adding another source of truth. This phase should not add
Ekaterina gameplay or change Kriegsia behavior.

Phase 6 turns existing abilities into registry-backed discovery and projection without changing
their effects. It preserves Smoke, Mortar Fire, Artillery Point Fire, Breakthrough, and legacy
Charge compatibility while routing ids, carriers, target modes, cooldowns, charges, costs, and
command-card affordances through faction-aware definitions. This phase should mostly be parity
tests and command validation, not a new effect engine.

Phase 7 adds only reusable ability effect hooks justified by existing current-faction abilities or
test fixtures. It must not depend on the real second-faction brief or speculate about unapproved
mechanics; self buffs, targeted world effects, delayed impacts, area effects, toggles/autocast, or
limited charges should be added only when current parity or fixture coverage needs them. Any
remaining one-off effect code is acceptable if the registry clearly documents how Phase 11 should
add tightly scoped signature ability hooks later.

Phase 8 updates the client faction surface: command cards, Steel/Oil/Supply HUD assumptions,
placement menus, rendering fallbacks, tooltips, hotkeys, and compact protocol decoding. It should
preserve the current faction's UI while adding fixture-faction coverage for alternate build menus
and ability buttons within the existing resource model. This phase is player-facing, so DOM
contract tests and smoke coverage are required.

Phase 8.5 cleans up architecture drift found after the guardrail, ability, and client-surface
phases before the real second-faction brief starts. It restores the faction-assumption ratchet,
centralizes replay faction/loadout validation, documents the Point Fire ability extension policy,
and decides whether the checked client catalog mirror is sufficient for Phase 10. This phase should
not add Ekaterina gameplay or change Kriegsia behavior.

Phase 9 is the approval gate for the real second faction. It creates or references the faction
brief plus rules/balance spec, including Steel/Oil/Supply usage, loadout, production, roster slices,
abilities, art readability, and explicit AI/prediction policy. No implementation code for the real
faction should land until this phase is approved.

Phase 10 implements the second faction's start, Steel/Oil/Supply tuning, and first production path.
It should be a playable but narrow slice: the faction can enter a match, see the normal resource
HUD, create its basic production loop, and reject all illegal cross-faction commands. It should
keep AI and prediction disabled for the new faction unless the approved brief says otherwise.

Phase 11 adds the second faction's first combat and signature ability slice. It should include one
baseline combat unit, one signature ability-heavy unit, readable client art, fog-safe events, and
targeted server/client tests. The goal is a short playable match that demonstrates the faction's
mechanical identity without trying to finish the whole roster.

Phase 12 expands the roster as approved, hardens integration, updates docs, and decides rollout.
It verifies mixed-faction match shapes, replay/branch/dev scenario flows under the new non-backcompat
schema, match history, spectators, quickstart, AI restrictions, prediction restrictions, performance,
and balance documentation. This phase is where faction choice becomes ready for regular playtesting.

## Phase Index

0. [Phase 0 - Architecture Inventory and Harness](phase-0.md)
1. [Phase 1 - Faction Identity Contract](phase-1.md)
2. [Phase 2 - Faction-Aware Rules Catalog](phase-2.md)
3. [Phase 3 - Assignment, Lifecycle, and Command Identity Guardrails](phase-3.md)
   - [Phase 3A - Canonical Faction Validation and Lifecycle Matrix](phase-3a.md)
   - [Phase 3B - AI and Prediction Fail-Closed Policy](phase-3b.md)
   - [Phase 3C - Command Identity and Per-Faction Hotkeys](phase-3c.md)
   - [Phase 3D - Replay, Branch, and Dev Lifecycle Tests](phase-3d.md)
4. [Phase 4 - Steel/Oil Resource Policy Hardening](phase-4.md)
5. [Phase 5 - Faction Starting Loadouts](phase-5.md)
5.5. [Phase 5.5 - Architecture Course Correction Guardrails](phase-5.5.md)
6. [Phase 6 - Ability Registry Parity](phase-6.md)
7. [Phase 7 - Ability Effect Hooks](phase-7.md)
8. [Phase 8 - Client Faction Surface](phase-8.md)
8.5. [Phase 8.5 - Architecture Cleanup Before Second-Faction Spec](phase-8.5.md)
9. [Phase 9 - Second Faction Brief and Rules Spec](phase-9.md)
10. [Phase 10 - Second Faction Start and Economy Slice](phase-10.md)
11. [Phase 11 - Second Faction Combat and Signature Ability Slice](phase-11.md)
12. [Phase 12 - Roster Expansion, Integration, and Rollout](phase-12.md)

## Supporting Artifacts

- [Faction Lifecycle Matrix](lifecycle-matrix.md) — assignment, AI, prediction, replay, branch,
  spectator, dev-tool, and match-history source-of-truth tracker.

## Testing Strategy

The first implementation work is test infrastructure, not faction content. The expected harnesses
are:

- Rust rule/catalog contract tests that compare the existing faction catalog to today's hardcoded
  behavior.
- Protocol parity tests that fail when faction ids, kind ids, ability ids, or existing
  Steel/Oil/Supply payloads are not mirrored.
- Snapshot and replay tests that prove faction identity survives live match start, replay start,
  branch start, and compact snapshot paths where relevant.
- A faction lifecycle matrix that every phase updates when it touches match creation, playback,
  spectators, dev tooling, AI, prediction, or replay branches.
- Client command-card descriptor tests for build/train/research/ability availability.
- Focused server integration tests for mixed-faction starts and illegal cross-faction commands.
- Fog/security tests for new ability events and resource-node visibility.
- Architecture checks or reports that flag new direct `EntityKind::Worker` or current-tech-tree
  special cases outside approved compatibility modules. Direct Steel/Oil/Supply references remain
  allowed only in the documented global economy modules and mirrors.
- Prediction/WASM compatibility checks proving unsupported local-player factions disable prediction
  with a clear reason, while supported local-player factions may keep prediction enabled even if an
  opponent uses an unsupported faction.
- Generated-client-catalog or catalog-parity tests proving JS descriptors match Rust-authoritative
  faction data.
- Hotkey profile tests proving faction-specific command ids do not collide and custom bindings are
  isolated per faction.

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
- Do not implement generic resources, arbitrary HUD resource rows, faction-specific map resources,
  or generic replay-analysis resource vectors as part of this faction plan.
- Do not require the new faction to have AI support before human play unless Phase 12 decides that
  product scope requires it.

## Deferred Generic Resources

Generic resources are intentionally deferred. If a later faction requires non-Steel/Oil resources,
create a separate plan that owns the full migration across `PlayerState`, affordability/refund
helpers, snapshots, compact transport, start-map resources, spectator resources, replay artifacts,
match-history replay payloads, replay analysis, score semantics, HUD rendering, client command-card
costs, protocol parity, and prediction/WASM compatibility. That migration should be driven by a
specific approved faction mechanic rather than speculative abstraction.
