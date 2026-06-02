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
- `3 * SNAPSHOT_MS` only if measurement shows repeated smaller gaps still punch through;
- keep `TICK_HZ` unchanged;
- keep command send path unchanged.

Do not guess in production. Measure snapshot receive gaps before and after the change.

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

## Measurements

Record before/after:

- snapshot receive interval p50/p90/p99;
- number of frames where interpolation alpha clamps to `1`;
- visible stutter reports under the same network-loss setup;
- user-perceived command feedback, if relevant.

Useful derived metric:

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
- Measurement shows fewer alpha-clamped frames or smoother receive jitter handling.
- Command send path is unchanged.
- Existing tests pass.
