# Ekaterina Rules and Balance Spec

Status: Approved for Phase 10 and Phase 11 implementation scope.

## Scope

This spec defines the first playable Ekaterina slices. It is intentionally narrow:

- Phase 10: start loadout, Steel/Oil/Supply tuning, first production path, client visibility,
  dev-only assignment, and illegal cross-faction command rejection.
- Phase 11: baseline combat unit, signature ability-heavy unit, readable art, fog-safe events, and
  focused tests.
- Later phases: additional roster, upgrades, advanced abilities, AI, prediction, and normal lobby
  selection.

No arbitrary resource system, faction-specific map resource object, new resource protocol field, or
generic HUD resource-row migration is approved by this spec.

## Faction and Loadout

- Faction id: `ekaterina`.
- Standard loadout id: `ekaterina.standard`.
- Starting resources: 85 Steel, 0 Oil.
- Starting entities:
  - 1 completed `ekaterina_command_post`.
  - 4 completed `ekaterina_engineer`.
- Opening upgrades: none.
- Supply model:
  - Command Post provides 8 supply.
  - Supply Cache provides 8 supply.
  - Hard cap remains 200.
- Map resources:
  - Uses existing Steel and Oil patches.
  - No Ekaterina-only map resource nodes.
  - No alternate gather payload shape.

The slightly higher starting Steel offsets weaker early combat and the need to build a Workshop
before the first combat unit.

## Phase 10 Roster

| id | role | hp | damage | range tiles | cooldown ticks | speed px/tick | sight tiles | steel | oil | supply | build ticks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `ekaterina_engineer` | worker/gather/build | 35 | 3 | 1 | 26 | 2.0 | 7 | 50 | 0 | 1 | 360 |
| `ekaterina_conscript` | baseline infantry | 38 | 4 | 4 | 18 | 1.7 | 8 | 45 | 0 | 1 | 300 |

The Engineer should share the existing worker command family semantically, but must use its own
global entity id because its stats, labels, art, and faction availability differ from Kriegsia
Worker. It gathers existing resources and builds only Ekaterina buildings.

The Conscript is intentionally weaker than Kriegsia Rifleman in an equal fight. Its purpose in
Phase 10 is to prove production, command-card, selection, attack, fog, and replay surfaces for a
real non-default faction.

## Phase 10 Buildings

| id | role | hp | sight tiles | footprint | steel | oil | supply provided | build ticks |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |
| `ekaterina_command_post` | start building / trains Engineers | 520 | 9 | 3x3 | 200 | 0 | 8 | 400 |
| `ekaterina_supply_cache` | supply | 95 | 4 | 2x2 | 80 | 0 | 8 | 260 |
| `ekaterina_workshop` | first production | 260 | 6 | 3x2 | 140 | 35 | 0 | 520 |

Build requirements:

- Command Post: buildable by Engineer; no prerequisite.
- Supply Cache: buildable by Engineer; requires Command Post.
- Workshop: buildable by Engineer; requires Command Post.

Train requirements:

- Engineer: trained at Command Post.
- Conscript: trained at Workshop.
- Signal Team: Phase 11, trained at Workshop after its ability hook is implemented.

## Phase 11 Roster

| id | role | hp | damage | range tiles | cooldown ticks | speed px/tick | sight tiles | steel | oil | supply | build ticks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `ekaterina_signal_team` | signature support infantry | 42 | 2 | 4 | 24 | 1.45 | 9 | 90 | 25 | 2 | 420 |

The Signal Team is not a general-purpose combat upgrade. Its weapon is deliberately weak; its value
comes from the approved signature ability below.

## Signature Ability

Approved ability id: `markTarget`.

Carrier: `ekaterina_signal_team`.

Target mode: world point.

Initial effect: delayed world effect that creates a visible temporary marker at the target point
and then applies a small area damage pulse.

Rules target:

- Range: 8 tiles.
- Minimum range: none.
- Cooldown: 25 seconds.
- Charges: none.
- Cost: 15 Steel, 0 Oil.
- Queueable: yes.
- Autocast: no.
- Launch delay: 60 ticks.
- Area radius: 1.25 tiles.
- Damage: 20 normal damage to units only; no building damage in the first slice.
- Friendly fire: yes, same support-weapon contract as Mortar and Artillery area damage.
- Fog/events: marker and impact events are sent only to recipients who can see the point by
  authoritative fog at the relevant event time, or who own/allied-own the caster if the caster is
  visible through normal projection.

Hook policy:

- Prefer the existing delayed-world-effect hook shape.
- Do not add a generic scripting engine.
- If current delayed-world infrastructure cannot express the marker plus impact cleanly, Phase 11
  may add a narrow named `markTarget` effect path with faction validation, cost validation,
  cooldown validation, and fog-safe event tests.
- No new protocol field is approved. Add a new mirrored ability id/code only if the existing
  ability command/event shapes are sufficient.

## Command Ids and Hotkeys

Faction-specific command ids use the established namespace:

- `ekaterina.build.<kind>`
- `ekaterina.train.<kind>`
- `ekaterina.research.<upgrade>`
- `ekaterina.ability.<ability>`

Expected initial bindings:

- Engineer build menu: existing global worker build-menu command.
- Return from build menu: existing global worker return command.
- Build Supply Cache: `ekaterina.build.ekaterina_supply_cache`, suggested key `Q`.
- Build Workshop: `ekaterina.build.ekaterina_workshop`, suggested key `W`.
- Train Engineer: `ekaterina.train.ekaterina_engineer`, suggested key `Q`.
- Train Conscript: `ekaterina.train.ekaterina_conscript`, suggested key `Q`.
- Mark Target: `ekaterina.ability.markTarget`, suggested key `D`.

Custom hotkey bindings remain per faction. Imports may preserve unavailable Ekaterina bindings with
warnings, but Kriegsia bindings must not apply to Ekaterina command cards.

## Assignment Path for Phase 10

Phase 10 may use the existing dev-scenario lifecycle path as the only way to start an Ekaterina
match before normal lobby selection exists.

Required shape:

- Add a server-owned dev scenario such as `dev:ekaterina_opening`.
- The scenario constructs explicit `PlayerInit` records with `faction_id = "ekaterina"` and
  `loadout_id = "ekaterina.standard"`.
- Update lifecycle validation so `FactionRequestContext::DevScenario` accepts Ekaterina only for
  this explicit dev scenario path.
- Normal lobby, quickstart, AI seats, self-play, replay branch launch, and match-history replay
  remain Kriegsia-only until later phases opt in.

This is the exact assignment path Phase 10 should use. Do not expose a lobby faction selector in
Phase 10.

## AI and Prediction

AI policy:

- Ekaterina AI is blocked.
- AI seats must reject Ekaterina.
- Self-play remains Kriegsia-only.
- No AI build-order, tactical, or retreat logic is approved for Ekaterina in Phase 10 or Phase 11.

Prediction policy:

- Local prediction is disabled when the local player is Ekaterina.
- A remote Ekaterina opponent must not disable prediction for a supported local Kriegsia player by
  itself, matching the existing local-player faction policy.
- WASM prediction support for Ekaterina requires a later explicit phase.

## Protocol and Fog

No new resource fields, player payload shapes, replay artifact shapes, or snapshot resource shapes
are approved.

Allowed protocol work:

- Add mirrored global entity-kind ids for the approved Ekaterina units/buildings.
- Add mirrored ability id/code for `markTarget` only if Phase 11 implements it through existing
  command/event payload families.
- Keep `factionId = "ekaterina"` in the existing lobby/start/replay/player payload fields.

Fog requirements:

- Starting entities, command cards, and snapshots obey normal owner/team visibility.
- Ability marker/impact events must not reveal hidden enemies or hidden caster state.
- Resource node visibility remains the existing Steel/Oil visibility model.

## Client Art and UI

Phase 10 client requirements:

- Add client descriptors checked against the Rust catalog for every exposed Ekaterina kind.
- Use distinct render fallback silhouettes and labels for the Command Post, Engineer, Supply
  Cache, Workshop, and Conscript.
- Keep the HUD resource bar fixed to Steel, Oil, and Supply.
- Render only Ekaterina legal command cards for Ekaterina selections.
- Use `ekaterina.*` command ids so hotkey profiles are isolated.

Phase 11 client requirements:

- Add a distinct Signal Team silhouette.
- Add Mark Target command-card affordance, targeting cursor, and fog-safe visible marker/impact
  feedback.

## Phase 10 May Implement

Phase 10 may implement:

- `ekaterina` catalog and `ekaterina.standard` loadout.
- Approved Phase 10 entity ids, stats, costs, build/train requirements, supply values, and client
  descriptors.
- Dev-only `dev:ekaterina_opening` assignment path.
- Command-card, placement, rendering fallback, replay/start metadata, and validation tests for the
  Phase 10 slice.
- Rejection tests for Kriegsia using Ekaterina commands and Ekaterina using Kriegsia commands.

Phase 10 must not implement:

- Signal Team ability effects.
- AI support.
- Prediction support.
- Normal lobby faction selection.
- New resource shapes.
- Full Ekaterina roster.

## Phase 11 May Implement

Phase 11 may implement:

- `ekaterina_signal_team`.
- `markTarget` ability through the narrow hook policy above.
- Focused combat/ability/fog/client tests for Conscript and Signal Team.

Phase 11 must not implement AI, prediction, normal lobby selection, or generic resources.
