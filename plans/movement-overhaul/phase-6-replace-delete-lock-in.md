# Phase 6: Replace, Delete, and Lock In

## Purpose

Once the new movement system passes the core scenarios, make it the real movement system and remove
legacy behavior that is no longer needed.

This phase should reduce complexity, not add more.

## Replace

Switch ordinary matches to the new movement system only after the lab scenarios are good enough to
trust.

Keep the ability to inspect movement decisions in development builds.

## Delete

Delete old movement mechanisms when the new system covers the reason they existed:

- delete waypoint skipping once route progression is explicit;
- delete wall sliding once legal swept movement handles walls;
- delete sidestep recovery once traffic and recovery choices are explicit;
- delete special unjam behavior once rotate, reverse, and wait choices cover it;
- delete old tests that only protect legacy quirks.

Keep or rewrite tests that protect real safety:

- no illegal static poses;
- no panics during ticks;
- no impossible vehicle motion;
- no serious overlap after cleanup;
- deterministic scenario results;
- broad completion-time limits.

## Lock In

After replacement, the core movement scenarios become permanent guardrails. They should stay small,
visual, and easy to run.

Add new scenarios only when they describe a real player-facing movement promise.

## Done

- The old movement system is removed or reduced to shared helpers.
- The new system is the default.
- The scenario viewer remains available for future tuning.
- The permanent tests protect desired behavior, not old accidents.
- The movement code is smaller and easier to explain than before.
