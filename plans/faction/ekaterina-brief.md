# Ekaterina Faction Brief

Status: Approved for Phase 10 and Phase 11 implementation scope.

## Identity

Ekaterina is the first real second faction and uses canonical faction id `ekaterina`.
It is an ability-heavy, positional faction built around fragile specialists, temporary field
equipment, and deliberate timing windows rather than Kriegsia's direct combined-arms tech curve.

Theme goals:

- Fictional battlefield faction with no national, historical-regime, or real-world insignia.
- Readable silhouettes, bright team-color anchors, and clear ability telegraphs.
- A field-expedient identity: light frames, signal gear, portable emplacements, and improvised
  vehicles.

## Strategic Identity

Ekaterina should reward planning and set-piece execution. Its early army is weaker in a straight
fight than Kriegsia, but it gets earlier access to utility abilities that create short windows for
movement, denial, and focused damage.

Strengths:

- Earlier tactical tools than Kriegsia.
- Flexible support units that can reposition and enable pushes.
- Good vision and short-duration area control when abilities are managed well.

Weaknesses:

- Lower raw durability and fewer forgiving attack-move answers.
- More dependence on cooldowns, charges, and setup timing.
- Punishable if caught while equipment is deployed or abilities are unavailable.

Expected match pacing:

- Early game: workers and basic infantry contest resources cautiously; Ekaterina should avoid
  equal-number open-field rifle fights until support tools arrive.
- First production path: the faction gets its first specialist from a light production building
  before heavy armor equivalents exist.
- First combat slice: a baseline infantry unit and one signature ability unit should be enough to
  demonstrate the faction without completing the full roster.
- Later game: roster expansion can add heavier field equipment, more support choices, and tech
  progression only after the first playable slices prove the loop.

## Economy

Ekaterina uses the existing global Steel, Oil, and Supply contract. It does not introduce new
resources, faction-specific map resource objects, new HUD resource rows, or alternate snapshot
resource payloads.

Economy decisions:

- Steel is the main cost for infantry, workers, buildings, and field equipment.
- Oil is used sparingly for powered equipment, mobile specialists, and later vehicles.
- Supply is fixed-field `supplyUsed` / `supplyCap`, with Ekaterina supply provided by its own
  support building.
- Existing universal Steel and Oil map nodes are used exactly like Kriegsia resource nodes.
- Workers gather existing Steel and Oil nodes through the same authoritative gather rules.

Ekaterina may have different starting resources, costs, supply values, and production dependencies,
but those differences stay inside the faction catalog and balance definitions.

## Production Model

Phase 10 should implement a narrow, ground-only production loop:

- Start building: `ekaterina_command_post`.
- Worker: `ekaterina_engineer`.
- Supply building: `ekaterina_supply_cache`.
- First production building: `ekaterina_workshop`.
- First trainable combat unit: `ekaterina_conscript`.
- First specialist unit for Phase 11: `ekaterina_signal_team`.

The first production path is:

1. Start with Command Post plus Engineers.
2. Gather Steel/Oil from existing map nodes.
3. Build Supply Cache as the first supply-cap expansion.
4. Build Workshop.
5. Train Conscripts from Workshop.
6. In Phase 11, train Signal Teams from Workshop after the basic path is stable.

Normal lobby faction selection remains hidden. AI remains unavailable for Ekaterina. Local
prediction remains disabled for Ekaterina until an explicit later phase implements and verifies
WASM prediction support.

## Player-Facing Readability

The first implementation does not need final art, but each unit and building must be visually
distinct from Kriegsia:

- Command Post: larger tent/radio-hub structure with a clear antenna or mast.
- Engineer: compact worker silhouette with tool pack; distinct from Kriegsia Worker.
- Supply Cache: low crate-and-tarp structure; clear supply role at small scale.
- Workshop: medium field workshop with gantry/frame silhouette; visually not a Barracks clone.
- Conscript: light infantry silhouette with shorter weapon profile than Rifleman.
- Signal Team: two-person or gear-heavy support silhouette with an obvious radio/marker rig.

Team color must be visible on every Ekaterina unit/building at normal zoom. Ability target and
impact visuals must be readable through fog-safe projection only; no client visual may reveal
hidden enemies, hidden impact positions, or hidden caster state.

## Implementation Gates

Phase 10 may implement only the start, economy, and first production path named above. It may add
the minimum global ids, catalog entries, start loadout, client descriptors, rendering fallbacks,
dev assignment path, and command validation required for that slice.

Phase 11 may implement Conscripts as the baseline combat unit and Signal Teams as the first
signature ability-heavy unit. Later roster/progression work is Phase 12 or beyond.

Open product questions are intentionally deferred:

- Full late-game roster.
- Heavy vehicle identity.
- Final numeric balance beyond the ranges in the rules spec.
- AI support.
- Prediction support.
- Normal lobby faction-selection UX.
