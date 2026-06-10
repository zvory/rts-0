# Phase 5 - Stage Timing and Playtest Balance

## Objective

Tune costs, build times, research times, supply, and unit timings so the new stage structure creates
the intended strategic rhythm.

## Target Rhythm

- Training Centre opens shared infantry tech.
- Vehicle Works enables Scout Car play.
- Steelworks enables Mortar Team play.
- Tank unlock creates a strong Mobile Warfare stage-two surge.
- AT Gun unlock gives Superior Firepower a committed answer if scouted and timed correctly.
- Artillery should not arrive before the Tank-vs-AT timing has had room to matter.

## Work

- Tune Vehicle Works cost/build time if Scout Cars arrive too early or too late.
- Tune Steelworks cost/build time if Mortars cannot contest Scout Car pressure or arrive too safely.
- Tune AT Gun upgrade cost/research time against first Tank timing.
- Tune Tank upgrade cost/research time so it feels like a power spike with real investment.
- Review oil requirements so Mobile Warfare remains oil-hungry and punishable through economy raids.
- Review supply costs so massing the correct path units creates meaningful army-shape tradeoffs.
- Update balance docs and patch-note bullets with every player-facing stat change.

## Verification

- Run simulation/unit tests after each tuning pass.
- Run self-play or scripted timings for:
  - first Scout Car timing
  - first Mortar timing
  - first Tank timing
  - first AT Gun timing
- Manually inspect at least one SF-vs-MW timing scenario if tests produce unclear failures.

## Player-Facing Outcome

The matchup should create a clear midgame question: can Mobile Warfare convert Scout Cars and the
Tank unlock into decisive damage before Superior Firepower stabilizes and reaches heavier guns?

