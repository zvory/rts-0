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
- **Tournament Grounds:** {{TOURNAMENT_CAPACITY}} Novices enter; one random Adept comes out.
- **Shop:** sells items that fly to any friendly unit, anywhere on the map.

## Insight

Novices and Adepts earn insight from the HP they take off enemies:

| Damage to | Insight per HP |
|---|---|
| Units | {{DISCIPLE_INSIGHT_PER_HP_UNITS}} |
| Buildings | {{DISCIPLE_INSIGHT_PER_HP_BUILDINGS}} (a {{unit:wind_adept}} gets {{DISCIPLE_INSIGHT_PER_HP_WIND_EXTRACTORS}} from extractors) |
| {{unit:tank|Tanks}}, by a {{unit:war_adept}} | {{DISCIPLE_INSIGHT_PER_HP_WAR_TANKS}} |

- A {{unit:magic_adept}} also gains {{MAGIC_ANCHOR_DEATH_INSIGHT}} insight for every enemy unit that dies inside its Vector Field.
- The {{ability:doubleXp}} item from the {{building:shop}} multiplies all insight by {{DOUBLE_XP_INSIGHT_MULTIPLIER}}.

## Master rank

An Adept that reaches {{DISCIPLE_EVOLVED_INSIGHT_CAP}} insight becomes a Master:

- HP ×{{DAO_MASTER_HP_MULTIPLIER}}, damage ×{{DAO_MASTER_DAMAGE_MULTIPLIER}}, speed ×{{DAO_MASTER_SPEED_MULTIPLIER}}, and a ×{{DAO_MASTER_BODY_SCALE}} larger body. War Masters deal another ×{{DAO_WAR_MASTER_DAMAGE_MULTIPLIER}}.
- Wind and Magic Masters' attacks reach {{unit:wind_adept.master.armorPenetration|pct}} armor penetration.
- Stronger abilities: {{WIND_DASH_MASTER_CHARGES}} Blink charges, a bigger and faster Vector Field, a {{MAGIC_MASTER_SHIELD_CAPACITY}}-HP Shield, and Throw Sword {{THROW_SWORD_ADEPT_COOLDOWN_MULTIPLIER|x}} as often.
- Item slots: {{CULTIVATOR_MASTER_ITEM_CAPACITY}}, up from {{CULTIVATOR_ITEM_CAPACITY}}.

Each Adept's page shows its own Master numbers.
