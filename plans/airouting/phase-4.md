# Phase 4 - Route Memory and Validation

Status: Planned.

## Objective

Teach the harassment manager to stop reissuing a route that has become bad in live play, and harden
the atlas-backed routing behavior with focused validation. Static atlas facts should choose a
credible route; route memory and visible-threat influence should keep the AI from looping forever
when that route becomes occupied or unproductive.

## Scope

- Extend `AiDecisionMemory` with harassment route state:
  - selected route id or stable route key
  - last route issue tick
  - last meaningful progress marker
  - recent evasion count
  - route cooldowns or hot-route expiry ticks
- Detect repeated evasion or low-progress loops using public own-unit state and visible enemy
  observations only.
- Fold visible enemy combat units into route scoring as a fog-respecting influence penalty. Threats
  should penalize nearby route segments without treating unseen areas as known safe or known
  occupied.
- When a route is marked hot, choose the next acceptable route option or skip harassment for the
  current think if every route is bad.
- Keep local defense and defensive panic higher priority than harassment. Route memory must not make
  Scout Cars ignore an active base-defense need.
- Add deterministic tests for:
  - route cooldown after repeated evasion
  - switching to an alternate route when one exists
  - no-command behavior when every candidate is blocked, too narrow, or hot
  - route facts remaining deterministic for bundled authored maps
- Add compact trace details so a developer can see why a route was selected, rejected, or put on
  cooldown.
- Update `docs/design/ai.md` with route memory, visible-threat influence, diagnostics, and known
  limits.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/harassment.rs`
- Route query API from Phase 2
- `server/crates/ai/src/ai_core/decision/trace.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/ai_core/profiles.rs` only if thresholds need profile fields
- `server/crates/ai/src/selfplay/` if focused scenario validation is added
- `docs/design/ai.md`

## Verification

Run focused AI tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai harassment
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
```

If memory shape changes affect profile resets or live controllers, also run the smallest controller
or decision-memory test filter that covers reset and rematch behavior.

## Manual Testing Focus

Spectate a game long enough for Scout Cars to encounter visible enemy combat units near a route.
Confirm they do not repeatedly return to the same occupied route after several evasion orders, and
confirm they still resume harassment later if a route clears or another route is available.

## Handoff Expectations

The handoff must document route-hot thresholds, cooldown duration, trace labels, and the fastest
regression command for route behavior. It must include factual patch-note bullets for player-facing
Scout Car harassment changes and list any remaining limitations future AI movement work should
respect.

## Player-Facing Outcome

AI Scout Cars should stop visibly looping at an occupied or unproductive route. They should either
choose another harassment route or back off until a route becomes viable again.
