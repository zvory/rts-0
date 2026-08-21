# Rocket Launcher Prototype

Status: Phase 0/1 approved for immediate implementation by the requester on 2026-08-21.

## Phase 0: Unit brief

The Rocket Launcher is a manually fired mobile saturation-artillery vehicle for Kriegsia. It is
trained at the Gun Works after completing Rockets research at the Engineering Complex. Its purpose
is to punish concentrations and static positions at long range while exposing a fragile,
oil-hungry vehicle that must stop and commit to a visible two-second launch sequence.

Player-facing description: "Mobile rocket artillery. Stop and manually fire a 16-rocket Barrage
over a wide area. The first barrage is free; later barrages cost 75 oil."

Prototype interactions: vehicle pathing and collision; manual ground targeting through fog;
friendly/allied splash damage; no automatic weapon or attack-move firing; no sound; placeholder
Katyusha-like truck presentation with translucent magenta launch and impact effects. AI production
and ability use are deferred. The unit is enabled in ordinary play and Lab authoring.

## Phase 1: Rules and balance

- Research: `rockets`, Engineering Complex, 75 steel / 125 oil, 750 ticks (25 seconds), no
  prerequisite beyond the completed building; unlocks Rocket Launcher training.
- Production: Gun Works (`steelworks`); provisional 225 steel / 125 oil, 6 supply, 600 ticks
  (20 seconds), build hotkey `R`.
- Body: 150 HP, small/soft armor class, vehicle body and pathing, 18 px selection radius, 20 px
  collision half-length, 11 px half-width; 2.0 px/tick speed; 8-tile sight.
- Default combat: no automatic attack. Attack-move behaves as movement and never fires or spends
  oil.
- Ability: `barrage`; Grid hotkey `X`, RTS Classic hotkey `F`; manual world-point targeting;
  10-tile minimum and 35-tile maximum range (copied values); queue policy waits until ready.
- Execution: the launcher must reach a legal firing position, stop, and then commits to the
  barrage. Sixteen rockets launch evenly across 60 ticks (2 seconds). Moving or stopping before
  launch cancels the pending order; after the first rocket launches the sequence is not refundable.
- Area: each rocket samples independently and uniformly inside a provisional 4-tile target circle.
  Each impact copies Mortar splash: 100 damage within 0.5 tiles and 40 damage out to 2 tiles, with
  existing Mortar target modifiers and friendly/allied splash behavior. Rockets use staggered
  flight delays so the barrage reads as many impacts rather than one pulse.
- Economy/cooldown: the first barrage is free per launcher lifetime. A later barrage spends 75 oil
  when authoritative execution begins. Reload is 150 ticks (5 seconds) after the last rocket
  launches. An unaffordable queued barrage waits; a non-queued attempt is rejected with normal
  resource feedback.
- Targeting/cancellation: ground positions may be chosen through fog. Invalid coordinates, stale
  units, death, or loss of ownership are safe no-ops. Destroying the launcher does not cancel
  rockets already launched.
- Presentation: translucent magenta rockets/trails, four-tile targeting preview, and magenta impact
  flashes; placeholder truck-mounted rack, team color retained on the vehicle body; no audio.
- AI: may perceive, attack, and be damaged by Rocket Launchers, but does not build or command them
  in this prototype.

Deferred tuning: unit economy/body stats, scatter radius, per-rocket damage, flight timing, reload,
AI usage, final art/animation, audio, and strategic counters.
