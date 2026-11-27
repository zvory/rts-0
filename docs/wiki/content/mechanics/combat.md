---
title: Combat
order: 10
---
How damage is worked out. Per-weapon numbers are on [Weapons & damage](weapons.html).

## Armor

Every unit and building is **Small** or **Armored**.

| Armor | Damage taken |
|---|---|
| Small | 100% |
| Armored | 25% from weapons with no armor penetration (AP) |

AP cancels part of the armor: damage × (1 − 0.75 × (1 − AP)).

| Shot | vs Small | vs Armored (Tank) |
|---|---|---|
| Rifle — {{unit:rifleman.attack.dmg}} damage, 0% AP | {{dmg:rifleman_rifle>rifleman}} | {{dmg:rifleman_rifle>tank}} |
| Panzerfaust — {{PANZERFAUST_DAMAGE}} damage, {{PANZERFAUST_ARMOR_PENETRATION|pct}} AP | — | {{dmg:panzerfaust_loaded_shot>tank}} |
| Tank Cannon — {{unit:tank.attack.dmg}} damage, 100% AP | {{dmg:tank_cannon>rifleman}} | {{dmg:tank_cannon>tank.front}} |

## Tank facing

The Tank Cannon and Anti-Tank Gun deal more damage to a Tank's side and rear.

| Arc | Multiplier | Tank Cannon | Anti-Tank Gun |
|---|---|---|---|
| Front | ×{{rules:facing.front}} | {{dmg:tank_cannon>tank.front}} | {{dmg:anti_tank_gun>tank.front}} |
| Side | ×{{rules:facing.side}} | {{dmg:tank_cannon>tank.side}} | {{dmg:anti_tank_gun>tank.side}} |
| Rear | ×{{rules:facing.rear}} | {{dmg:tank_cannon>tank.rear}} | {{dmg:anti_tank_gun>tank.rear}} |

- Front: the 90° cone ahead of the hull. Rear: the 90° cone behind. Side: the rest.
- The Panzerfaust ignores facing.
- A Tank hit by an Anti-Tank Gun, Panzerfaust or Tank Cannon turns its hull toward the shooter and holds it for {{TANK_ARMOR_REACTION_LOCK_TICKS|s}} (turns at most {{TANK_ARMOR_REACTION_MAX_PIVOT_RAD|deg}}).

## Machine-gun bursts

Applies to the Machine Gunner, Scout Car and the Tank's coaxial MG.

- Each attack fires {{MG_BURST_BULLETS}} bullets. The first is aimed at the target; the others spread up to ±{{MG_HALF_SPREAD_RAD|deg}} and hit the first unit in their path.
- Each bullet that hits deals {{MG_RAY_DAMAGE_MULTIPLIER|pct}} of the weapon's damage.
- Hits on the same target are added up **before** armor.
- Example: a Machine Gunner deals {{dmg:machine_gunner_mg>rifleman.burstMin}} (1 hit) to {{dmg:machine_gunner_mg>rifleman.burstMax}} (all hits) to a Rifleman.

## Area damage

Before armor.

| Source | Damage | Radius |
|---|---|---|
| Tank shell (splash) | {{TANK_HE_SPLASH_DAMAGE}} | {{TANK_HE_RADIUS_TILES}} tiles |
| Mortar shell | {{MORTAR_INNER_DAMAGE}} at impact, falling to {{MORTAR_OUTER_DAMAGE}} at the edge | {{MORTAR_OUTER_RADIUS_TILES}} tiles |
| Artillery shell | {{ARTILLERY_INNER_DAMAGE}} within {{ARTILLERY_INNER_RADIUS_TILES|2}} tiles, then falling evenly to {{ARTILLERY_OUTER_MIN_DAMAGE}} at the edge | {{ARTILLERY_OUTER_RADIUS_TILES}} tiles |
| Rocket (each of {{ROCKET_BARRAGE_ROCKETS}}) | {{ROCKET_BARRAGE_DIRECT_DAMAGE}} direct hit / {{ROCKET_BARRAGE_INNER_DAMAGE}} / {{ROCKET_BARRAGE_OUTER_DAMAGE}} | {{ROCKET_BARRAGE_AP_RADIUS_TILES}} / {{ROCKET_BARRAGE_INNER_RADIUS_TILES}} / {{ROCKET_BARRAGE_OUTER_RADIUS_TILES}} tiles |

Mortar, artillery and rocket damage counts as area damage (see entrenchment below).

## Other modifiers

| Rule | Effect |
|---|---|
| Anti-Tank Gun vs infantry | {{ANTI_TANK_GUN_INFANTRY_DAMAGE_MULTIPLIER|pct}} damage |
| Entrenched infantry ({{research:entrenchment}}) | −{{ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION|pct}} direct, −{{ENTRENCHMENT_AREA_DAMAGE_REDUCTION|pct}} area damage |
| Damage-reduction tiles (map) | ×{{DAMAGE_REDUCTION_TILE_DAMAGE_MULTIPLIER}} damage |

## Targeting

- Small-arms weapons prefer Small targets.
- Anti-tank weapons prefer, in order:
  1. units with an AP weapon ({{units:threatRole=AntiArmorThreat}})
  2. Armored targets
  3. anything else

## Barbed Wire

- **Slows and damages:** {{units:slowedByBarbedWire}}.
- **Crushed by:** {{units:crushesBarbedWire}}.
