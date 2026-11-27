---
constants: ARTILLERY_
---
## Firing

- Sets up in {{ARTILLERY_SETUP_TICKS|s}}, then fires at {{ARTILLERY_MIN_RANGE_TILES}}–{{ARTILLERY_MAX_RANGE_TILES}} tiles within a {{ARTILLERY_FIELD_OF_FIRE_RAD|deg}} arc.
- Reloads in {{ARTILLERY_RELOAD_TICKS|s}}. Shells land {{ARTILLERY_SHELL_DELAY_TICKS|s}} after firing.

## Shell

- {{ARTILLERY_INNER_DAMAGE}} damage within {{ARTILLERY_INNER_RADIUS_TILES|2}} tiles of impact.
- Beyond that, damage falls evenly to {{ARTILLERY_OUTER_MIN_DAMAGE}} at {{ARTILLERY_OUTER_RADIUS_TILES}} tiles; nothing further out. All before armor.
- Entrenched infantry take {{ENTRENCHMENT_AREA_DAMAGE_REDUCTION|pct}} less.
