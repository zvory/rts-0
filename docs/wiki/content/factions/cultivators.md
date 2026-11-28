---
name: Cultivators
summary: Sword Disciples and Bow Novices that grow into Wind, War and Magic Adepts.
constants: DAO_, DISCIPLE_, CULTIVATOR_, MAGIC_, MAX_DAO_
---
**Cultivators** train {{unit:warrior|Sword Disciples}} and {{unit:bow_novice|Bow Novices}} that evolve into Adepts.
Both are immediately available at a Portal. Bow Novices stop to shoot arrows; Sword Disciples fight in melee.

## Overview

- **Evolving:** a Novice picks a Dao — Wind, War or Magic — and becomes that Adept at {{DISCIPLE_INSIGHT_REQUIRED}} insight.
- **Training Adepts directly:** research a Dao at the {{building:three_dao_pavilion}}, then train its Adept at the {{building:portal}}.
- **Tournament Grounds:** four Novices enter; one random Adept comes out.
- **Shop:** sells items that fly to any friendly unit, anywhere on the map.

## Insight

Novices and Adepts earn insight from the HP they take off enemies:

| Damage to | Insight per HP |
|---|---|
| Units | 1 |
| Buildings | ¼ (a {{unit:wind_adept}} gets 1 from extractors) |
| {{unit:tank|Tanks}}, by a {{unit:war_adept}} | 2 |

- A {{unit:magic_adept}} also gains {{MAGIC_ANCHOR_DEATH_INSIGHT}} insight for every enemy unit that dies inside its Vector Field.
- The {{ability:doubleXp}} item from the {{building:shop}} doubles all insight.

## Master rank

An Adept that reaches {{DISCIPLE_EVOLVED_INSIGHT_CAP}} insight becomes a Master:

- HP ×{{DAO_MASTER_HP_MULTIPLIER}}, damage ×{{DAO_MASTER_DAMAGE_MULTIPLIER}}, speed ×{{DAO_MASTER_SPEED_MULTIPLIER}}, and a ×{{DAO_MASTER_BODY_SCALE}} larger body. War Masters deal another ×{{DAO_WAR_MASTER_DAMAGE_MULTIPLIER}}.
- Wind and Magic Masters' attacks reach {{unit:wind_adept.master.armorPenetration|pct}} armor penetration.
- Stronger abilities: a second Blink charge, a bigger and faster Vector Field, a {{MAGIC_MASTER_SHIELD_CAPACITY}}-HP Shield, and Throw Sword twice as often.
- Item slots: {{CULTIVATOR_MASTER_ITEM_CAPACITY}}, up from {{CULTIVATOR_ITEM_CAPACITY}}.

Each Adept's page shows its own Master numbers.
