# Phase 2 — Spatialization & distance

Adds the "far-off battle gets louder as you approach" behavior.

## Listener

The listener is the **camera center in world pixels**, updated each frame from `camera.js`.
Expose `audio.setListener(x, y, zoom)`. Pass `zoom` because reference distance should scale with
zoom — at max zoom-out, the whole map should still feel audible at reduced volume rather than
silent.

## Per-emitter graph

```
AudioBufferSourceNode → StereoPannerNode → BiquadFilterNode (lowpass) → category GainNode
```

Reasons for `StereoPannerNode` over `PannerNode`:

- Top-down 2D; we don't need HRTF or 3D azimuth.
- Cheaper.
- Predictable. `PannerNode`'s default model surprises people.

## Attenuation curve

Custom (not `PannerNode.distanceModel`):

```
dx = emitter.x - listener.x
dy = emitter.y - listener.y
d  = sqrt(dx*dx + dy*dy)
refDist  = 1 screen-width worth of world pixels at current zoom
maxDist  = 3 * refDist
gain     = clamp(refDist / max(d, refDist), 0, 1)   // inverse-ish, flat inside refDist
pan      = clamp(dx / refDist, -1, 1)
lpHz     = lerp(20000, 1200, clamp(d / maxDist, 0, 1))
```

Beyond `maxDist`, drop the sound entirely (do not even allocate a voice). This is the single
biggest perf win in a 200-unit fight.

## Off-screen cue

Sounds whose emitter is outside the viewport but within `maxDist` still play — that *is* the "far
battle" feature. The lowpass + reduced gain provide the muffled cue. Do not gate on viewport;
gate on `maxDist` only.

## Fog gate (critical)

The audio module trusts the snapshot: it only ever sees events the server already filtered. The
audit task here is on the **server side**:

- Confirm `Event::Attack`, `Event::Death`, `Event::Build` are only emitted to players who can see
  the relevant entities (check `game/services/` snapshot assembly).
- Add a regression test in `tests/regression.mjs`: spawn two players, hide a battle in fog for
  player A, assert player A's snapshot stream contains no events referencing the hidden entities.

If any leak is found, fix the server before this phase ships. Audio will otherwise surface fog
leaks as audible cheats.

## Resolving non-positional events

`Event::Attack { from, to }` has no `(x, y)`. The client resolves position from the entity table at
the snapshot tick:

- Prefer the attacker's position (`from`). It's the player's own unit more often than not, and
  matches the "muzzle" sound model.
- If `from` is unknown (stale id, evicted from view), fall back to `to`. If both unknown, drop the
  sound — do not play at (0, 0).

`Event::Build` resolves via `id`; same fallback rule.

## Tests

- Extend the audio stub to capture `(pan, gain, lpHz)` per play; assert distance/pan math.
- A live test in `tests/client_smoke.mjs`: load a 2-player replay, scroll the camera, assert gain
  changes monotonically with distance.

Deliverable: distance attenuation, stereo pan, off-screen lowpass. Fog regression test green.
