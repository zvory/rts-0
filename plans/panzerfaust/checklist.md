# Panzerfaust Unit Checklist

Status: Phase 0 brief and Phase 1 rules/balance spec drafted with user-reviewed close-range tuning;
Phases 2-8 have implemented hidden vocabulary, runtime/client inspection, normal Barracks
production exposure, first-pass launch/impact audio feedback, final focused regression/dev
inspection coverage, and the final review package.
This file follows [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md).

Read: [docs/context/balance.md](../../docs/context/balance.md),
[docs/design/balance.md](../../docs/design/balance.md).

## Review Questions

- Build source assumption: Panzerfaust is trained from Barracks after a completed Training Centre,
  matching Machine Gunner's existing unlock pattern. If "through the Training Centre" meant direct
  Training Centre training, revise Phase 1 before implementation.
- Cost decision: 60 steel / 15 oil.
- Base loaded speed decision: 1.44 px/tick, 10% slower than Rifleman's current 1.6 px/tick.
- Build time decision: 400 ticks, matching Machine Gunner's current build time.
- Range decision: 3-tile loaded range, extended to 4 tiles only by the normal Entrenchment +1 range
  rule while actively occupying a trench.
- Target filter draft: the one-shot weapon targets Tanks only in the first implementation. Future
  armored or hard targets are deferred until explicitly designed.
- Hull-facing draft: the Panzerfaust shot deals flat 60 armor-piercing damage to Tanks, without
  front/side/rear tank-facing multipliers in the first implementation.
- Order behavior draft: plain Move orders do not auto-interrupt into Panzerfaust firing; Attack,
  Attack Move, Idle, and Hold Position can fire at legal visible Tanks.

## Phase 0: Unit Brief

- [x] Name the unit and its intended battlefield role.
  - Panzerfaust: Training Centre-unlocked infantry anti-tank ambusher. It threatens Tanks with one
    short setup, short travel-time armor-piercing shot, then becomes an ordinary Rifleman.
- [x] Define the player-facing description used by the UI.
  - Short description: "Infantry with a one-shot anti-tank weapon. Fires once at Tanks, then becomes
    a Rifleman."
  - Tooltip draft: "Slower than Riflemen. Carries one short-range Panzerfaust shot;
    Methamphetamines speeds movement and firing."
- [x] Define the strategic purpose: what problem does this unit solve, and what should counter it?
  - Solves the gap between early infantry control and heavier anti-tank tech by giving infantry
    armies a cheap close-range one-shot punishment against unsupported Tanks.
  - Counters should include screening with infantry, Machine Gunners, Scout Cars, Mortar pressure,
    smoke-enabled dives, killing or forcing the Panzerfaust to spend its shot early, and exploiting
    the Rifleman-only body after the shot is spent.
- [x] List expected unusual interactions with existing units, buildings, terrain, fog, commands, or
      AI behavior.
  - The loaded Panzerfaust has no normal rifle attack; after firing and completing recovery it
    converts into a normal Rifleman.
  - Conversion should preserve entity id, owner, position, current HP, selection/control-group
    continuity, and trench occupation where possible.
  - The shot is target-filtered to visible enemy Tanks only in the first implementation.
  - The loaded Panzerfaust stops for a firing windup before launching and cannot move during windup
    or post-fire recovery.
  - If the target becomes illegal during windup, the shot is not consumed and the unit resumes legal
    orders. If the target dies after launch, the projectile should expire without damage at the
    target's last valid position unless Phase 2 chooses a different protocol-safe visual rule.
  - Projectile launch, travel, impact, and conversion will likely need protocol/event review in a
    later phase so hidden target ids, impact positions, and death events do not leak through fog.
  - The loaded Panzerfaust is eligible for Methamphetamines and Entrenchment.
  - The loaded Panzerfaust can occupy or dig trenches if the owner has Entrenchment; active
    trench occupation gives the normal +1 range, direct miss chance, and area damage reduction.
  - After conversion, the Rifleman inherits normal Rifleman Methamphetamines behavior, including
    moving fire if researched.
  - The first AI pass should not train Panzerfaust units, but spawned Panzerfaust units should still
    obey normal attack acquisition rules for their legal targets.
  - Visual identity needs a visible carried Panzerfaust, but no historical insignia or factional
    national marking.
- [x] Decide whether this unit is allowed in the initial implementation, debug-only, or hidden until
      later polish.
  - Intended for normal Kriegsia matches once implemented, not debug-only.
- [x] Start patch-note bullets for player-facing changes and keep them updated through the work.
  - Barracks gains Panzerfaust infantry after a completed Training Centre.
  - Panzerfaust costs 60 steel / 15 oil, uses 1 supply, has Rifleman HP and sight, and moves slower
    than a Rifleman.
  - Panzerfaust trains in 400 ticks, matching Machine Gunner build time.
  - Panzerfaust carries one 60-damage armor-piercing anti-tank shot at 3-tile range, or 4-tile range
    while actively entrenched, then converts into a Rifleman after a short recovery.
  - Methamphetamines speeds Panzerfaust movement and firing setup/recovery before conversion, but
    the loaded Panzerfaust still does not fire while moving.
  - Entrenchment applies to Panzerfaust infantry, including the normal trench range and defense
    benefits.
  - Panzerfaust launch and impact now use distinct first-pass spatial combat cues; projectile
    travel and same-id conversion remain silent to avoid clustered-fire spam.
  - Dedicated dev scenarios cover one-shot firing, windup cancellation, target death during travel,
    entrenched range, and Methamphetamines timing for final playtest inspection.

Exit criteria:

- [x] The brief is written down in the task, issue, design note, or commit body.
- [x] Known unknowns are explicit instead of implied.
- [x] No implementation files have been edited.
- [x] The next step is user review of this brief/spec.

## Phase 1: Rules And Balance Specification

- [x] Cost is specified.
  - 60 steel / 15 oil.
- [x] Supply impact is specified.
  - 1 supply.
- [x] Build source is specified: which building, unit, queue, or ability creates it.
  - Trained from Barracks after the player has a completed Training Centre.
- [x] Build hotkey is specified.
  - Train hotkey: `E`, assuming Barracks train slots remain Rifleman `Q`, Machine Gunner `W`,
    Panzerfaust `E`.
- [x] Build time is specified.
  - 400 ticks, approximately 13.3 seconds at 30 Hz, matching Machine Gunner build time.
- [x] Research prerequisite, tech prerequisite, or unlock timing is specified.
  - Requires a completed Training Centre. No new research upgrade is required.
- [x] Hit points are specified.
  - Same as Rifleman: 45 HP.
- [x] Armor, armored status, tags, status immunities, or special vulnerabilities are specified.
  - Small armor class, infantry, not armored, not hard, no special status immunity.
  - Loaded weapon is armor-piercing for damage purposes, but target-filtered to Tanks only.
  - Once spent, the unit becomes a normal Rifleman and loses all loaded Panzerfaust behavior.
- [x] Sight range is specified.
  - Same as Rifleman: 8 tiles.
- [x] Collision size, selection size, and render size are specified.
  - Same collision/selection/render radius as Rifleman: 9 px.
  - Visual silhouette should remain infantry-sized but show a carried Panzerfaust while loaded.
- [x] Movement speed is specified.
  - Base loaded speed: 1.44 px/tick.
  - With Methamphetamines: 1.8 px/tick using the existing 1.25x multiplier.
  - After conversion, Rifleman movement rules apply: 1.6 px/tick base, 2.0 px/tick with
    Methamphetamines.
- [x] Movement semantics are specified: ground, blocked-by-terrain, ignores collision, setup mode,
      cannot move while attacking, transport-like, or other special behavior.
  - Ordinary infantry ground movement, including forest passability and ordinary unit collision.
  - Cannot move during Panzerfaust windup or post-fire recovery.
  - Does not fire while moving while loaded, even with Methamphetamines; it must stop before launch.
  - Methamphetamines still applies to loaded Panzerfaust movement speed and firing windup/recovery.
  - After conversion, normal Rifleman movement and Methamphetamines moving-fire rules apply.
- [x] Pathing semantics are specified if different from ordinary ground units.
  - Same pathing semantics as Rifleman.
- [x] Attack range is specified.
  - Base loaded Panzerfaust shot range: 3 tiles.
  - While actively occupying a trench: 4 tiles through the existing Entrenchment +1 range rule.
  - After conversion: normal Rifleman range applies.
- [x] Damage, cooldown, windup, projectile behavior, area behavior, and target filters are specified.
  - One shot only; no reload and no second Panzerfaust attack.
  - Target filter: visible enemy Tanks only for the first implementation.
  - Direct Attack on non-Tank targets is invalid while loaded; after conversion, Rifleman attack
    rules apply.
  - Damage: 60 armor-piercing direct damage.
  - No area damage and no friendly fire.
  - Hull-facing multipliers do not apply in the first implementation; damage is flat 60 on hit.
  - Firing windup: 15 ticks, half a second at 30 Hz.
  - Projectile travel: 15 ticks, half a second at 30 Hz.
  - Post-fire recovery: 15 ticks, half a second at 30 Hz, then immediate conversion to Rifleman.
  - Methamphetamines reduces windup and post-fire recovery using the existing 3/4 attack-cooldown
    ratio, rounded to 12 ticks each. Projectile travel remains 15 ticks.
  - If a direct Attack target leaves range or visibility during windup, do not spend the shot.
  - If a launched projectile's target dies before impact, do no damage; resolve only a fog-safe
    visual impact if Phase 2 defines one.
  - Attack Move, Idle, and Hold Position can acquire legal Tanks. Hold Position does not chase
    outside current range.
- [x] Abilities are specified, including cost, cooldown, range, target rules, queued behavior, and
      cancellation behavior.
  - No player-activated ability in the first implementation; the Panzerfaust is a one-shot weapon
    used by normal attack acquisition and direct Attack commands.
  - No resource cost to fire after the unit has been trained.
  - Queued orders after a direct Attack or Attack Move should continue after conversion when still
    valid for the Rifleman. Queued Panzerfaust-specific attacks become invalid once the shot is
    spent.
  - Canceling or replacing the order during windup cancels the shot without spending it.
  - Canceling after launch cannot recover the shot; recovery and conversion still complete.
- [x] Economy, repair, build, harvest, or production interactions are specified if relevant.
  - Uses normal Barracks training, affordability, supply reservation, queue cancellation, and refund
    behavior.
  - No repair, build, harvest, or production role.
  - Conversion preserves the paid cost and does not refund the 10 steel / 15 oil premium over a
    Rifleman.
- [x] AI availability and intended AI usage are specified.
  - AI does not train Panzerfaust units in the first implementation pass.
  - If spawned by a scenario or lab, AI-owned Panzerfaust units may use normal target acquisition
    against legal visible Tanks.

Exit criteria:

- [x] The unit's numbers and rules can be reviewed without reading implementation code.
- [x] Any unresolved tuning item is marked as deferred and assigned to a later pass.
  - Deferred: direct Training Centre production alternative, broader armored/hard target filters,
    hull-facing multipliers, final art/audio polish beyond the first pass, optional
    travel/conversion audio after playtests, and AI build strategy.
- [x] No implementation files have been edited.
- [x] The next step is user review, not implementation.

## Phase 8: Human Review Package

- [x] Keep the implementation split into reviewable phases.
  - Phase 2 through Phase 8 are separate phase files, and implemented phases 2-7 are already marked
    done.
- [x] Summarize player-facing behavior and known deferred work.
  - Player-facing summary: Barracks can train Panzerfaust infantry after a completed Training
    Centre. The unit is a close-range anti-tank ambusher that fires one Tank-only armor-piercing
    shot, then converts into a normal Rifleman with the same entity id.
  - Deferred work remains limited to the direct Training Centre production alternative, broader
    armored/hard target filters, hull-facing multipliers, final art/audio polish beyond the first
    pass, optional travel/conversion audio after playtests, and AI training strategy.
- [x] Include patch-note bullets for stats, economy, combat behavior, UI affordances, and expected
      strategic impact.
  - Barracks gains Panzerfaust infantry after a completed Training Centre.
  - Panzerfaust costs 60 steel / 15 oil, uses 1 supply, has 45 HP, 8-tile sight, 9 px radius,
    1.44 px/tick loaded speed, and trains in 400 ticks.
  - Panzerfaust carries one 60-damage armor-piercing shot against visible enemy Tanks at 3-tile
    range, or 4-tile range while actively occupying a trench.
  - The loaded shot has 15-tick windup, 15-tick travel, and 15-tick recovery. Methamphetamines
    reduces windup and recovery to 12 ticks each and increases loaded movement speed, but the
    loaded unit still stops to fire.
  - After recovery, the unit converts into a normal Rifleman with the same id, HP, position,
    selection/control-group continuity, and valid queued follow-up orders.
  - The Barracks command card exposes Panzerfaust with hotkey `E`; visual launch/impact feedback
    and low-gain spatial launch/impact audio are first-pass dedicated Panzerfaust cues.
  - AI production profiles intentionally do not train Panzerfaust units in this pass, but spawned
    AI-owned Panzerfausts use normal target acquisition against legal Tanks.
  - Strategic impact to watch in playtests: whether the current cost, short range, one-shot damage,
    and conversion timing punish unsupported Tanks without making normal armor pushes overly brittle.
- [x] Link or name the debug scenario used for human inspection.
  - Primary smoke path: in a normal Kriegsia match, build Barracks and Training Centre, train a
    Panzerfaust, fire once at a Tank, observe damage and same-id Rifleman conversion, then inspect a
    replay or fog/spectator view.
  - Dev scenarios for targeted inspection: `/dev/scenarios?id=panzerfaust_duel&unit=panzerfaust&count=1`,
    `/dev/scenarios?id=panzerfaust_windup_cancel&unit=panzerfaust&count=1`,
    `/dev/scenarios?id=panzerfaust_target_death&unit=panzerfaust&count=1`,
    `/dev/scenarios?id=panzerfaust_entrenched_range&unit=panzerfaust&count=1`, and
    `/dev/scenarios?id=panzerfaust_methamphetamines&unit=panzerfaust&count=1`.
- [x] Call out cross-file contracts touched.
  - Balance mirror: Rust rules definitions, faction catalog exports, client config mirrors, and
    `docs/design/balance.md` agree on stats, cost, supply, train source, Training Centre
    prerequisite, one-shot timing, Methamphetamines timing, and Entrenchment range.
  - Protocol mirror: Rust/JS kind vocabulary includes `panzerfaust`, weapon vocabulary includes
    `panzerfaust_loaded_shot`, and fog-safe Panzerfaust launch, impact, and conversion events are
    documented in `docs/design/protocol.md`.
  - Server sim: the dedicated one-shot state machine lives inside the sim combat service; no new
    public `Game` API was required.
  - Fog/projection: launch endpoints, impacts, conversions, target ids, and hidden positions remain
    recipient-gated by server projection; client visual events do not stamp local fog.
  - Client UI: command-card descriptors, hotkeys, config mirrors, renderer state, and audio mapping
    all route through existing dependency-injected client modules.
- [x] Confirm there are no stray debug logs, debug events, hidden console output, temporary UI labels,
      placeholder names, or accidental lobby/match messages.
  - Final sweep found Panzerfaust references in expected docs, tests, protocol/config mirrors,
    runtime code, dev scenarios, renderer/audio assets, and wiki/catalog surfaces. No
    Panzerfaust-specific temporary debug label or stray logging path remains in the implementation.
- [x] Confirm every deferred item has an owner, follow-up task, or explicit decision to leave it out.
  - Product/balance follow-up: direct Training Centre production, broader target filters, and
    hull-facing multipliers.
  - Client polish follow-up: final art/audio treatment and optional travel or conversion sounds
    after clustered-fire playtests.
  - AI follow-up: whether and when live AI profiles should train Panzerfaust units.

Exit criteria:

- [x] A reviewer can answer what changed, how to try it, what was tested, and what remains unfinished
      without reconstructing the whole implementation from code.
