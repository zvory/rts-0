---
title: Editing this wiki
order: 10
---
- **Generated:** infoboxes, tables and damage numbers. The game server builds every page from its own compiled rules when it starts, so they always match the build you are playing.
- **Hand-written:** Markdown notes in `docs/wiki/content/` of the game repository. They never hold numbers; they reference them (see *Live numbers*).
- **Recorded:** the version history (`docs/wiki/history.json`) and the unit art (`docs/wiki/icons.json`).

## Updating

- **Numbers:** nothing to do. A rules change shows up on the next deploy.
- **Notes:** edit `docs/wiki/content/` and ship it like any other change.
- **Changelog and unit art:** refresh them from the local wiki tool, which compares the rules at every version and draws the art with the game client's own code:

```
.\update-wiki.cmd -ExportTo <game checkout>
```

This writes `docs/wiki/history.json` and `docs/wiki/icons.json` in that checkout. The [Changelog](changes.html) lists the versions recorded there; a build newer than the last export shows its numbers everywhere but has no changelog entry yet.

The result of the tool's last accuracy check is on [Maintenance](maintenance.html).

## The changelog

- One section per game version, with the PRs merged into it (rules and simulation).
- Every changed value, grouped under the page it belongs to. A **#1234** tag names the PR that changed it, when the change can be pinned to exactly one commit.

## Where notes go

| File | Shows on |
|---|---|
| `content/units/<id>.md` | the unit's page |
| `content/buildings/<id>.md` | the building's page |
| `content/research/<id>.md` | the research page |
| `content/abilities/<id>.md` | the ability card |
| `content/factions/<id>.md` | the faction page (`name:` and `summary:` also feed the main page) |
| `content/<section>/<name>.md` | its own page, listed in the sidebar under *Section* |
| `content/main.md` | the main page intro |

`<id>` is in the page address: `unit-tank.html` → `content/units/tank.md`. [Maintenance](maintenance.html) lists pages without notes.

Optional front matter at the top of a file:

```
---
title: Page title
order: 20
lead: One-sentence summary that replaces the generated one.
constants: MORTAR_, MG_
---
```

- `order` sorts sidebar pages.
- `lead` replaces the generated first sentence.
- `constants` adds name prefixes to the page's *Related constants*.

## Live numbers

Write `{{` … `}}` instead of a number and it stays correct after every update:

| You write | You get |
|---|---|
| `{{TANK_HE_SPLASH_DAMAGE}}` | any constant: {{TANK_HE_SPLASH_DAMAGE}} |
| `{{METHAMPHETAMINES_RESEARCH_TICKS\|s}}` | ticks as seconds: {{METHAMPHETAMINES_RESEARCH_TICKS|s}} |
| `{{ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION\|pct}}` | percent: {{ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION|pct}} |
| `{{METHAMPHETAMINES_SPEED_MULTIPLIER\|change}}` | change: {{METHAMPHETAMINES_SPEED_MULTIPLIER|change}} |
| `{{MG_HALF_SPREAD_RAD\|deg}}` | radians as degrees: {{MG_HALF_SPREAD_RAD|deg}} |
| `{{unit:tank}}` / `{{unit:tank\|the Panzer}}` | link: {{unit:tank}} / {{unit:tank|the Panzer}} |
| `{{unit:tank.hp}}`, `{{unit:tank.attack.dmg}}` | a unit's field: {{unit:tank.hp}}, {{unit:tank.attack.dmg}} |
| `{{building:barracks.steel}}`, `{{research:tank_plus.oil}}` | same for buildings, research, abilities, factions |
| `{{dmg:tank_cannon>tank.rear}}` | weapon damage to a target: {{dmg:tank_cannon>tank.rear}} |
| `{{calc: STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}}` | arithmetic: {{calc: STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} |
| `{{units:entrenchmentEligible}}`, `{{units:armor=Armored}}` | every unit with that trait or value |
| `{{page:mechanics/combat}}` | link to a page: {{page:mechanics/combat}} |

- Field names are the ones in each page's *Properties* table (`server/crates/wiki/src/snapshot.rs` builds them). Constant names are on [Balance constants](constants.html).
- More filters: `|tiles` (px → tiles), `|speed` (px/tick → tiles/s), `|x` (×1.5), `|2` (2 decimals), `|round`.
- A reference that no longer matches anything turns red and is listed on [Maintenance](maintenance.html). The server's tests fail on red references, so a rename in the rules can't ship with a broken note.

## How it works

`server/crates/wiki` reads the compiled `rts-rules` definitions and the simulation's research table, and indexes where each constant is set and used while the server compiles. New units, abilities and constants appear automatically. Names and tooltips come from the game client (`client/src/config/rules_mirror.js`).

## Current roster

The player wiki filters archived and playtest-hidden content in
`server/crates/wiki/src/visibility.rs` before building pages, tables, navigation and search.
Update that presentation policy when content returns to play. Raw rules snapshots, historical
changelog entries and the full constants reference remain available; hidden entries have no
current entity page. Notes for hidden entries can stay in the repository for reuse.

The Rocket Truck icon can also be refreshed from the current client portrait with
`node scripts/update-wiki-rocket-icon.mjs`; it replaces only that entry in `icons.json`.
