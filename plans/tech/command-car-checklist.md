# Command Car Unit Checklist

Working checklist for `plans/tech/phase-7-command-car.md`, following
`docs/new-unit-checklist.md`.

## Phase 0: Unit Brief

- [x] Name the unit and its intended battlefield role.
  - Command Car: late Mobile Warfare capstone and army-speed support vehicle for decisive assaults.
- [x] Define the player-facing description used by the UI.
  - Short description: "Late-game support vehicle that accelerates assaults and creates decoy
    armies."
  - Tooltip draft for this implementation pass: "Uses Breakthrough! to speed nearby forces,
    especially units fighting through smoke."
- [x] Define the strategic purpose: what problem does this unit solve, and what should counter it?
  - Solves Mobile Warfare's late-game need to force attacks through entrenched Superior Firepower
    positions after Artillery comes online.
  - It is intentionally overpowered in the tech-path matchup rather than having a clean tactical
    counter. The main answers are destroying the Mobile Warfare economy with Artillery, surviving
    the attacks with AT killing fields, and punishing failed assaults during the cooldown window.
- [x] List expected unusual interactions with existing units, buildings, terrain, fog, commands, or
      AI behavior.
  - Command Car has no weapon.
  - Breakthrough! is centered on the Command Car and applies an instant, timed speed buff in an
    area.
  - Breakthrough! can be queued, can be preempted before it resolves if queued, cannot be canceled
    after cast because it is instant, and can be cast while the Command Car is moving.
  - Breakthrough! affects owned units and should eventually affect allied units if a team/ally
    system exists. The first implementation should note that there is no team/ally system yet.
  - Breakthrough! affects fake units if Fake Army is implemented later.
  - Multiple Breakthrough! effects do not stack.
  - The Breakthrough! smoke synergy doubles the speed bonus, not the final speed multiplier.
  - Units currently in smoke or within the recent-smoke grace window receive the larger speed buff.
  - Enemies should see the Breakthrough! effect only when they have vision of affected units.
  - AI ignores Command Cars for the first implementation pass.
  - Fake Army remains a future/vestigial design concept and is explicitly deferred out of this
    implementation pass.
- [x] Decide whether this unit is allowed in the initial implementation, debug-only, or hidden until
      later polish.
  - Available in normal matches immediately after the R&D Complex Command Car unlock is
    researched.
- [x] Start patch-note bullets for player-facing changes and keep them updated through the work.
  - Mobile Warfare gains Command Car as a late Vehicle Works capstone after Tank Production.
  - R&D Complex can research Command Car for 150 steel / 150 oil over 30 seconds after Tank
    Production.
  - Command Car is planned as a 150 steel / 75 oil, 4-supply support vehicle with 225 HP, Scout
    Car speed, Scout Car sight, Scout Car movement-oil cost, and Scout Car collision/selection/
    render size.
  - Command Car has no weapon.
  - Command Car can cast Breakthrough!, a 7-tile centered area speed boost lasting 6 seconds with a
    25-second cooldown and no resource cost.
  - Breakthrough! gives affected units 1.2x speed, or 1.4x speed while in smoke or within
    2 seconds after leaving smoke.
  - Fake Army is deferred out of the first implementation pass.

Exit criteria:

- [x] The brief is written down in the task, issue, design note, or commit body.
- [x] Known unknowns are explicit instead of implied.
  - Team/ally support is not implemented yet, so Breakthrough! should be scoped to owned units while
    documenting the intended allied-unit behavior.
  - Fake Army is explicitly deferred/abandoned for this implementation pass despite remaining in
    older high-level design notes.
- [x] No implementation files have been edited.
- [x] The next step is either user review or a clearly scoped implementation phase.

## Phase 1: Rules And Balance Specification

Read: `docs/context/balance.md`, `docs/design/balance.md`.

- [x] Cost is specified.
  - 150 steel / 75 oil.
- [x] Supply impact is specified.
  - 4 supply.
- [x] Build source is specified: which building, unit, queue, or ability creates it.
  - Trained from Vehicle Works.
- [x] Build hotkey is specified.
  - Train hotkey: `E`, assuming it is the next available Vehicle Works train slot after Scout Car
    and Tank.
- [x] Build time is specified.
  - 450 ticks, approximately 15 seconds at 30 Hz.
- [x] Research prerequisite, tech prerequisite, or unlock timing is specified.
  - Requires Tank Production to be researched first.
  - Requires R&D Complex research `Command Car`: 150 steel / 150 oil, 900 ticks
    (30 seconds), UI description "Unlocks production of Command Cars".
  - Upgrade hotkey: `S`, inserted beside the Tank Production upgrade in the R&D Complex command
    card.
- [x] Hit points are specified.
  - 225 HP, 50% more than the Scout Car's current 150 HP.
- [x] Armor, armored status, tags, status immunities, or special vulnerabilities are specified.
  - Unarmored like Scout Car.
  - Should not receive Tank armored damage reduction.
- [x] Sight range is specified.
  - Same as Scout Car: 10 tiles.
- [x] Collision size, selection size, and render size are specified.
  - Same as Scout Car.
  - Visual target is a Scout Car-like vehicle with the rear weapon replaced by leather seats and
    two visible officers. From top-down view, officers should read through peaked caps, gold trim,
    gold epaulettes, and team-colored uniforms; this can be approximate in the first pass.
- [x] Movement speed is specified.
  - Same as Scout Car: 2.35 px/tick.
- [x] Movement semantics are specified: ground, blocked-by-terrain, ignores collision, setup mode,
      cannot move while attacking, transport-like, or other special behavior.
  - Ground vehicle with Scout Car movement semantics.
  - Spends oil while moving at the same rate as Scout Car.
  - Can cast Breakthrough! while moving.
- [x] Pathing semantics are specified if different from ordinary ground units.
  - Same as Scout Car.
- [x] Attack range is specified.
  - No weapon and no attack.
- [x] Damage, cooldown, windup, projectile behavior, area behavior, and target filters are
      specified.
  - No damage, no projectile, and no target attack.
- [x] Abilities are specified, including cost, cooldown, range, target rules, queued behavior, and
      cancellation behavior.
  - Breakthrough! hotkey: `D`.
  - Breakthrough! is centered on the caster.
  - Radius: 7 tiles.
  - Duration: 180 ticks, approximately 6 seconds at 30 Hz.
  - Cooldown: 750 ticks, approximately 25 seconds at 30 Hz.
  - Cost: no steel or oil cost.
  - Base speed multiplier: 1.2x.
  - Smoke/recent-smoke multiplier: 1.4x, because smoke doubles the +20% bonus rather than the final
    speed.
  - Recent-smoke grace window: 60 ticks, approximately 2 seconds after leaving smoke.
  - Target rules: affects owned units in the first implementation; intended future behavior also
    includes allied units once team/ally systems exist.
  - Also affects fake units if Fake Army is implemented later.
  - Multiple Breakthrough! buffs do not stack.
  - Can be queued.
  - Queued casts can be preempted before they resolve.
  - Cannot be canceled after cast because the cast is instant.
  - Can be cast while moving.
  - Enemy visibility: enemies see the effect only when they have vision of affected units.
  - Fake Army is deferred out of this implementation pass. Do not implement fake-unit snapshot
    representation, fake-unit fog projection, fake attacks, fake HP, fake lifetime, or fake cleanup
    in the first Command Car implementation.
- [x] Economy, repair, build, harvest, or production interactions are specified if relevant.
  - Requires Tank Production before Command Car research.
  - Requires Command Car research before production.
  - Uses the same movement oil cost as Scout Car.
- [x] AI availability and intended AI usage are specified.
  - AI ignores Command Cars for the first implementation pass.

Exit criteria:

- [x] The unit's numbers and rules can be reviewed without reading implementation code.
- [x] Any unresolved tuning item is marked as deferred and assigned to a later pass.
  - Fake Army is deferred/abandoned for this pass.
  - Allied-unit targeting is deferred until there is an ally/team system.
  - Breakthrough! audio should use the newest recent ElevenLabs download from `~/Downloads` during
    the implementation/polish pass; the exact asset is not copied or committed in this planning
    phase.
- [x] No implementation files have been edited.
- [x] The next step is either user review or a clearly scoped implementation phase.
