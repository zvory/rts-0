# Panzerfaust Unit Requirements

Status: Phase 0 and Phase 1 complete; implementation authorized by the originating user request.

## Phase 0: Unit brief

- **Name:** Panzerfaust.
- **Role:** Optional late-game anti-vehicle Rifleman variant with one disposable launcher shot.
- **Player-facing description:** Rifle infantry carrying one disposable 5-tile anti-vehicle shot.
  After launching it, the unit keeps fighting as ordinary rifle infantry.
- **Strategic purpose:** Panzerfausts research unlocks a deliberate production choice instead of
  adding an oil surcharge and launcher to every later Rifleman. Players can continue training the
  oil-free baseline Rifleman when oil is scarce, or spend 5 oil for the launcher variant.
- **Counters:** The unit keeps Rifleman durability and remains vulnerable to normal anti-infantry
  weapons. Its launcher does not target infantry, support weapons, buildings, or obstacles.
- **Unusual interactions:** The unit inherits Rifleman movement, rifle combat, Methamphetamines,
  Entrenchment eligibility, fog rules, command behavior, and one-supply weight. The launcher targets
  visible Scout Cars, Tanks, and Command Cars under the existing Panzerfaust acquisition and attack
  rules. Firing spends the launcher permanently without replacing the entity or changing its id,
  orders, HP, control group, or trench occupancy.
- **Availability:** Normal Kriegsia unit, trained at the Barracks after Panzerfausts research.
  Existing AI profiles do not train it in the first implementation pass.
- **Patch-note draft:** Panzerfausts research now unlocks a separate Panzerfaust unit. Riflemen no
  longer receive launchers and return to 50 steel / 0 oil; Panzerfausts cost 55 steel / 5 oil.

Known unknowns: automated AI composition tuning and further cost tuning are deferred to playtests.

## Phase 1: Rules and balance specification

| Field | Specification |
| --- | --- |
| Cost | 55 steel / 5 oil |
| Supply | 1 |
| Build source | Barracks |
| Build hotkey | Barracks grid slot 3 (`E`) |
| Build time | 300 ticks (~10 seconds), matching Rifleman |
| Prerequisite | Completed Panzerfausts research at the Training Centre |
| HP / armor | 45 HP; Small armor, matching Rifleman |
| Sight / size | 11 tiles sight; 9 px radius, matching Rifleman |
| Movement | 1.6 px/tick ordinary ground infantry movement and pathing, matching Rifleman |
| Rifle | 5 damage, 5-tile range, 16-tick cooldown, matching Rifleman |
| Launcher | One lifetime 5-tile shot; 100 base damage; 50% armor penetration; 15-tick windup and travel |
| Launcher targets | Visible Scout Cars, Tanks, and Command Cars only, under existing automatic and explicit-attack rules |
| Methamphetamines | Same moving rifle fire, 2.0 px/tick speed, 12-tick rifle cooldown, and 12-tick launcher windup as an upgraded Rifleman |
| Entrenchment | Eligible under the same digging, occupancy, range, and damage-reduction rules as Rifleman |
| Economy / production | Baseline Rifleman returns to 50 steel / 0 oil and never receives a launcher; both choices remain independently repeat-trainable |
| AI | Legal to spawn and command; not added to current AI production plans in this pass |

The existing compact snapshot field and launcher events are sufficient. Restore the reserved
`panzerfaust` entity-kind tag (compact code 24); no new command, snapshot field, or event shape is
required. Existing loaded/spent launcher art and audio are reused, with the normal Rifleman visual
shown after launch.

## Deferred checklist

- Dedicated AI composition and resource-threshold logic.
- New art or audio beyond the existing loaded-launcher and Rifleman assets.
- Post-playtest cost, timing, or target-priority tuning.
