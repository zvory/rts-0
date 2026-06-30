# Phase 6 - Visibility Combat And Effects Checkpoint Coverage

Status: Not started.

## Scope

After Phase 5 has expanded the internal cold checkpoint path over movement, orders, and economy,
extend the same internal checkpoint DTO, import/export helpers, semantic comparator, and focused
Rust tests over visibility-sensitive combat and durable world-effect state. This phase is
behavior-preserving coverage work: restored games must continue from the same authoritative state
and produce the same semantic results, fog-filtered projections, full-world diagnostic projections,
observer analysis output where relevant, and privacy-filtered transient events as the baseline.

Use Phase 4's `Game -> GameCheckpoint -> Game` restore path and Phase 5's expanded comparator rather
than creating a new restore mechanism. If a restored game diverges, treat that as missing durable
state, an incorrect import repair path, or an incomplete comparator/projection check before changing
gameplay behavior. This phase must use projection-audit thinking throughout: it must not leak
fog-hidden entity ids, target ids, positions, ability payloads, remembered occupants, or event data
to a recipient that could not see them through the normal player, spectator, replay/lab-selected, or
full-world policy.

Coverage must include these state families where they exist in the current code:

- live fog and team visibility output, including raw per-player current fog, living-teammate union
  behavior, and any Phase 1 checkpoint policy for whether live grids are serialized or rebuilt on
  import;
- building memory entries and stale-destruction clearing, including observed tick, footprint, owner,
  kind, position, hp/progress state, selected-player union memory dedupe, and the rule that memory
  disappears only after the remembered footprint is scouted;
- trench store state, including deterministic next id, existing trench terrain, discovered-by-player
  memory, remembered-trench projection, and entity-local occupation/digging state that affects
  combat bonuses or `occupiedTrenchId` projection after restore;
- lingering sight and firing reveal sources, including team recipients, expiry ticks, smoke/LOS
  suppression, firing-reveal response delay state on combatants, and post-restore command/combat
  use of revealed targets;
- active smoke clouds and scheduled pending smoke, including store next id, cloud ids, centers,
  radii, spawn/expiry/due ticks, fog blocking, combat LOS blocking, visible smoke projection, and
  owner-visible launch events produced after restore;
- ability runtime state and entity-local ability state, including runtime next id, active
  instances, world objects, projectiles, owner/caster/ability ids, public versus owner-only payload,
  cooldown maps, finite uses, lockout ticks, recast markers, magic anchors, line projectiles,
  breakthrough/recent-smoke status, and mortar-fire wait-until-ready behavior;
- mortar and artillery shell stores, including pending shell owner/attacker/impact point/impact tick,
  scatter continuity, same-team friendly-fire attribution rules, impact damage, firing reveals, and
  fog-gated impact/reveal/under-attack events after restore;
- combat target/cooldown/facing/setup state needed to continue the next tick identically, including
  target ids, attack cooldowns, attack phase, weapon facing and desired facing, emplacement and
  pending redeploy facing, setup/teardown timers, artillery shot count, attack-move no-target grace,
  tank stationary-range ramp, autocast toggles, and firing-reveal response targets;
- lab god mode player set plus mirrored invulnerable flags on lab-owned units/buildings after
  checkpoint import or any import repair pass;
- observer analysis output if it remains authoritative or restore-sensitive under the current
  design. If the output is rebuilt from existing authoritative state, compare the payload after
  restore instead of introducing a new stored analysis object.

Preconditions:

- Phase 4's internal checkpoint export/import path exists and does not use
  `Game::clone_for_replay_keyframe`.
- Phase 5's comparator and scenarios are reusable, including semantic authoritative comparison and
  per-player snapshot comparison after additional ticks.
- Phase 1 through Phase 5 left no unresolved ownership blocker for fog, memory, effects, ability,
  shell, combat, lab god mode, or observer-analysis state covered here.

Explicit non-goals:

- No public checkpoint schema, public Rust checkpoint API, JSON format, wire protocol, endpoint,
  client, snapshot DTO, replay artifact schema, or lab scenario schema change.
- No replay keyframe replacement, replay artifact migration, lab timeline migration, lab scenario
  migration, or lab product behavior change.
- No balance, unit stat, ability rule, combat rule, fog rule, projection policy, or gameplay change.
- No final release audit, public checkpoint-readiness declaration, or broad architecture guardrail
  phase yet.
- No broad UI/client work. Client or protocol files should remain unchanged unless compiler errors
  from existing private test helpers prove otherwise, which should be treated as a blocker to
  re-scope rather than an assumed Phase 6 task.

## Expected Touch Points

- `server/crates/sim/src/game/state.rs` or the Phase 4/5 equivalent checkpoint module: add internal
  DTO fields and import/export handling for the visibility, combat, ability, smoke, trench, shell,
  lab god mode, and projection-sensitive durable state covered here.
- `server/crates/sim/src/game/mod.rs`: add or adjust private/crate-private checkpoint test helpers
  only if needed for canonical semantic views, import repair, event comparison, or test scenario
  construction. Keep public `Game` API signatures stable.
- `server/crates/sim/src/game/fog.rs`, `building_memory.rs`, `trench.rs`, `smoke.rs`,
  `ability_runtime.rs`, `mortar.rs`, and `artillery.rs`: add narrow internal DTO/comparator accessors
  or derives only where the checkpoint path cannot otherwise capture and compare store state.
- `server/crates/sim/src/game/entity/{entity.rs,state.rs,order.rs,store.rs}`: add narrow internal
  DTO/comparator accessors only for entity-local combat, ability, trench occupation/digging,
  cooldown/use/lockout, facing/setup, and reveal-response state that is not already covered by
  Phase 5's entity state view.
- `server/crates/sim/src/game/services/combat/**`: read-only evidence or focused test fixtures for
  target acquisition, firing reveals, attack events, setup/facing, tank range ramp, mortar autocast,
  artillery point fire, and entrenchment combat continuity. Avoid changing combat policy.
- `server/crates/sim/src/game/services/ability_orders.rs` and related ability helpers: read-only
  evidence or focused fixtures for queued ability promotion, cooldown/use/lockout state, smoke
  scheduling, mortar-fire readiness, Ekat runtime objects, and recast validation.
- `server/crates/sim/src/game/snapshot.rs`, `server/crates/sim/src/rules/projection.rs`, and
  `server/crates/sim/src/game/analysis.rs`: compare projection outputs and add privacy-focused
  tests only as needed. Do not change projection policy unless a Phase 6 test exposes an existing
  bug and the user explicitly accepts the behavior change.
- `server/crates/sim/src/game/lab.rs`: read-only evidence or focused tests for lab god mode state
  and import repair. Do not migrate lab scenario import/export to checkpoints.
- Focused tests under `server/crates/sim/src/game/**`, preferably beside the Phase 4/5 checkpoint
  harness/comparator so this phase extends the same proof.
- `docs/design/server-sim.md`, `docs/context/server-sim.md`, and
  `docs/projection-audit-checklist.md` only if the implementation changes an internal checkpoint
  policy, documented section anchor, or projection contract. A pure internal DTO/comparator/test
  coverage expansion should not require documentation edits.
- `plans/game-state/phase-6.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS outside `rts-sim::game` should be out of scope unless compiler errors prove
a private helper must move. Server room code, client code, protocol crates, rule/balance crates,
replay artifact schemas, lab scenario schemas, and public API callers should not need changes.

## Verification

- Extend the Phase 4/5 checkpoint comparator so every covered state family is either compared in a
  canonical semantic view or explicitly proven rebuilt from serialized state according to Phase 1's
  checkpoint policy. The proof must compare after additional ticks, not only immediately after
  import.
- Compare normal per-player fog-filtered snapshots for every player in each visibility/combat/effect
  scenario. Include `visibleTiles`, entities, target ids, weapon facing/setup projection, remembered
  buildings, trenches, smokes, ability objects, resources/ability affordances where relevant, and
  owner-only/debug fields when enabled through existing snapshot options.
- Compare spectator/selected-player snapshots for selected single-player and union views where
  building memory, trench memory, resources, visible smokes, ability objects, combat target ids, or
  event union behavior could diverge.
- Compare full-world snapshots for diagnostic/dev/lab-style projections in at least one scenario
  with hidden enemies plus active smoke/ability/trench/shell state. Full-world comparison is a
  diagnostic proof and must not be used as a substitute for per-player fog checks.
- Include an event privacy check for any scenario that produces events after restore. For each
  attack, mortar/artillery impact, smoke launch, notice, reveal, `target_id`, `toPos`, or ability
  event surface, cover at least one recipient that should receive the data and one recipient that
  should not, using the policies in `docs/projection-audit-checklist.md`.
- Include a fog/building-memory scenario that checkpoints with a known enemy building remembered but
  currently hidden, then continues until the remembered footprint is scouted or remains hidden.
  Baseline and restored games must agree on stale memory projection, hidden live entity withholding,
  selected-player union memory dedupe, and memory clearing timing.
- Include a trench scenario that checkpoints after trench discovery and active occupation or
  dig-progress state. Baseline and restored games must agree on remembered trench terrain, hidden
  occupant withholding, visible `occupiedTrenchId`, entrenchment range/miss/splash benefits, and
  future trench id allocation.
- Include a lingering-sight or firing-reveal scenario that checkpoints while temporary vision is
  active. Baseline and restored games must agree on revealed target command validity, combat
  response delay, team recipients, third-party non-recipients, snapshot visibility, building/trench
  memory refresh, and expiry behavior.
- Include a smoke scenario that checkpoints both a scheduled pending cloud and an active cloud.
  Baseline and restored games must agree on cloud id allocation, due/expiry timing, fog blocking,
  combat LOS blocking, visible cloud projection, hidden enemy withholding inside smoke, and any
  owner-visible smoke-launch events produced after restore.
- Include ability-runtime scenarios for at least one owner-only world object/recast marker and one
  projectile or area runtime object. Baseline and restored games must agree on runtime ids,
  cooldown/use/lockout state, owner-only payload projection, enemy-visible public fields, recast or
  expiry behavior, and future spawned runtime ids.
- Include mortar/artillery scenarios that checkpoint before delayed impact. Baseline and restored
  games must agree on impact timing, damage, friendly-fire attribution, under-attack notice routing,
  firing reveals, fog-gated impact/reveal event payloads, and projectile store cleanup.
- Include combat continuity scenarios that checkpoint with active target ids, nonzero attack
  cooldowns, support-weapon setup/teardown, weapon facing, artillery shot count, tank stationary
  range ramp, and autocast toggles. Baseline and restored games must agree on the next target
  decision, shot timing, setup transition, facing projection, cooldown progression, and events.
- Include a lab god mode checkpoint scenario if lab god mode is in the internal checkpoint contract.
  Baseline and restored games must agree on the god-mode player set, mirrored invulnerable flags
  after import repair, damage immunity, and lab snapshots; do not route lab scenario import/export
  through checkpoints.
- Compare `Game::observer_analysis()` after restore in scenarios that affect its authoritative
  inputs, such as kills/losses, current unit inventory, or active production. If no new stored
  observer-analysis state exists, document that it is rebuilt output and keep the comparison as an
  output check.
- Confirm `Game::clone_for_replay_keyframe`, replay artifact capture/playback, lab timeline
  keyframes, lab scenario import/export, public APIs, wire protocol, client code, and balance values
  did not change.

Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim visibility_combat_checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim projection_privacy
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md docs/projection-audit-checklist.md plans/game-state/phase-6.md plans/game-state/plan.md
```

If final test names do not include `checkpoint`, `visibility_combat_checkpoint`, or
`projection_privacy`, use the narrowest equivalent filters that cover internal checkpoint
import/export, semantic comparator field coverage, per-player/spectator/full-world snapshots,
event privacy, fog/memory/trench/smoke/ability/shell/combat/lab-god-mode scenarios, observer
analysis output, and derived-state rebuild after import. No broad Node suite or full local test
bundle is expected unless implementation changes escape the sim crate or alter protocol-facing
behavior; the PR `./tests/run-all.sh` gate remains the authoritative full-suite check.

## Manual Testing Focus

No broad manual gameplay pass is expected because this phase should expose no public checkpoint or
UI behavior. If a manual check is useful, run one ordinary local match or dev/lab scenario that
exercises hidden enemies, spectator selected vision, smoke, trenches, support-weapon combat,
mortar/artillery impacts, and one runtime ability, then confirm visible gameplay, fog, snapshots,
events, and observer analysis behave as before from player, spectator/selected, and full-world
diagnostic views.

## Handoff

The implementation handoff must name:

- every internal `GameCheckpoint`/DTO field or canonical comparator view added for fog, building
  memory, trenches, lingering sight, firing reveals, smoke, ability runtime, shell stores, combat
  state, lab god mode, and observer analysis output;
- every covered durable field family and any field intentionally excluded because Phase 1 classified
  it as derived or transient, including how it is rebuilt after import if it is not serialized;
- the visibility/fog, building-memory, trench, lingering-sight/firing-reveal, smoke,
  ability-runtime, mortar/artillery, combat-continuity, lab-god-mode, and observer-analysis
  scenarios added;
- how semantic authoritative comparison, per-player fog-filtered snapshot comparison,
  selected-player/spectator comparison, full-world comparison, and event privacy checks run after
  additional ticks;
- which projection-audit cases prove hidden entity ids, positions, target ids, remembered occupants,
  ability payloads, and private events do not leak after restore;
- confirmation that public checkpoint/schema/API surfaces, wire protocol, client code, replay
  keyframes/artifacts, lab timeline keyframes, lab scenario import/export, and balance/gameplay
  values did not change;
- the exact focused Rust test commands, archcheck command, and `git diff --check` command that
  passed;
- remaining checkpoint coverage gaps before any public checkpoint format, replay migration, lab
  migration, final release audit, or architecture guardrail phase is considered.
