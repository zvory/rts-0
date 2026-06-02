# Phase 03: Interpolation Buffer Tuning

Purpose: reduce visible jitter from small snapshot receive gaps by rendering slightly farther in the
past.

This is a WebSocket-compatible change. It does not improve command registration latency; it trades a
small amount of visual delay for smoother snapshot playback. That tradeoff is acceptable only
because the reported issue is stutter, not input commands failing to register.

## Current State

`client/src/config.js` currently has:

```js
export const TICK_HZ = 30;
export const SNAPSHOT_MS = 1000 / TICK_HZ;
export const INTERP_DELAY_MS = SNAPSHOT_MS;
```

That means the renderer targets about one snapshot interval of delay, roughly 33 ms. A small receive
jitter burst can exhaust that buffer and make interpolation clamp to the newest snapshot until the
next frame arrives.

## Target Behavior

Try a slightly larger delay:

- `2 * SNAPSHOT_MS` as the first candidate;
- `3 * SNAPSHOT_MS` only if repeated smaller gaps still punch through in manual reproduction;
- keep `TICK_HZ` unchanged;
- keep command send path unchanged.

Keep the default conservative. Compare the same manual reproduction before and after the change.

## Suggested Implementation

First pass:

```js
export const INTERP_DELAY_MS = SNAPSHOT_MS * 2;
```

Better pass:

- add a named constant such as `SNAPSHOT_INTERP_DELAY_TICKS = 2`;
- compute `INTERP_DELAY_MS = SNAPSHOT_MS * SNAPSHOT_INTERP_DELAY_TICKS`;
- optionally support a dev query flag for comparison, such as `?interpDelayTicks=1|2|3`.

If a dev query flag is added, clamp it tightly and keep the default explicit in `config.js`.

## Validation

Check before/after:

- visible stutter under the same reproduction setup;
- whether unit movement looks less stop-start during the freeze pattern;
- whether command feedback still feels acceptable.

If debug counters already exist, this ratio is useful:

```text
alpha_clamped_frames / total_frames
```

If this ratio drops when delay increases, the buffer is doing useful work.

## Risks

- More visual delay. A 2-snapshot buffer at 30 Hz is about 66 ms.
- If the game later sends snapshots at 15 Hz, the same tick multiplier becomes about 133 ms.
- Too much delay can make unit responses feel sluggish even if commands are registered quickly.

## Done Criteria

- The chosen interpolation delay is documented in `client/src/config.js`.
- Validation shows fewer alpha-clamped frames or smoother receive jitter handling.
- Command send path is unchanged.
- Existing tests pass.
