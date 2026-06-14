# Phase 4 - Route Memory and Threat Influence

Status: Planned.

## Objective

Teach the harassment manager to stop reissuing a route that has become occupied or has repeatedly
failed. This phase adds lightweight route memory and visible-threat influence so Scout Cars can
switch away from a hot corridor instead of bouncing forever between evasion and the same route
command.

## Scope

- Extend `AiDecisionMemory` with harassment route state: selected corridor id, last route issue tick,
  last meaningful progress, recent evasion count, and route cooldowns or hot-route expiry ticks.
- Detect repeated evasion loops using existing visible-threat evasion behavior and route progress.
  Keep thresholds deterministic and profile-configurable only if a concrete tuning need appears.
- Fold visible enemy combat units into route scoring as a public, fog-respecting influence penalty.
  Threats should penalize nearby route segments without treating unseen areas as known safe or
  known occupied.
- When a route is marked hot, choose the next acceptable route candidate or skip harassment for the
  current think if every route is bad.
- Keep local defense and defensive panic higher priority than harassment. Route memory must not make
  Scout Cars ignore an active base defense need.
- Add tests for route cooldown after repeated evasion, route switching when an alternate candidate
  exists, and no-command behavior when every candidate is blocked or hot.
- Add or update trace details so a developer can see why a route was selected, avoided, or put on
  cooldown.
- Update `docs/design/ai.md` to describe route memory and visible-threat influence at a high level.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/harassment.rs`
- Route evaluator module from Phase 2
- `server/crates/ai/src/ai_core/decision/trace.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/ai_core/profiles.rs` only if policy thresholds need profile fields
- `docs/design/ai.md`

## Verification

Run focused AI tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai harassment
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
```

If memory shape changes affect profile resets or live controllers, also run:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai AiDecisionMemory
```

## Manual Testing Focus

Spectate the known right-side two-AI case long enough for Scout Cars to encounter enemies near a
flank. Confirm they do not repeatedly return to the same occupied choke after several evasion
orders, and confirm they still resume harassment later if the route clears or another route is
available.

## Handoff Expectations

The handoff must document the route-hot thresholds, cooldown duration, and trace labels added for
route selection or rejection. It should tell Phase 5 which logs, scorecard fields, or self-play
checks would best catch regressions.

## Player-Facing Outcome

AI Scout Cars should stop visibly looping at an occupied flank. They should either choose another
harassment path or back off until a route becomes viable again.
