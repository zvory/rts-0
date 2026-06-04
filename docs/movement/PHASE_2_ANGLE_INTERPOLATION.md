# Phase 2 - Client Angle Interpolation

Goal: make rendered unit angles rotate smoothly between snapshots instead of snapping. This prepares
the client for tank body facing and weapon facing without changing server simulation.

This is a client-only presentation phase.

## Scope

In scope:

- Interpolate `facing` in `client/src/state.js`.
- Use shortest-arc angle interpolation.
- Add helper tests or contract coverage for wraparound behavior.

Out of scope:

- No server changes.
- No protocol changes.
- No tank body turn-rate changes.
- No `weaponFacing` field yet.
- No renderer redesign.

## Files To Touch

- `client/src/state.js`
- `tests/client_contracts.mjs` or a focused client test file if one already exists for state
  helpers.

## Implementation Steps

1. Add pure helpers in `state.js`:

   ```js
   function normalizeAngle(a) { ... }
   function shortestAngleDelta(from, to) { ... }
   function lerpAngle(from, to, t) { ... }
   ```

2. In `entitiesInterpolated(alpha)`, continue interpolating `x` and `y` as today.

3. If both prior and current entities have numeric `facing`, set the output `facing` to
   `lerpAngle(prior.facing, e.facing, t)`.

4. If only the current entity has numeric `facing`, keep current `facing`.

5. If current `facing` is absent, leave it absent. Do not invent facing for buildings or resources.

6. Keep all other fields carried through from the current snapshot.

7. Do not change renderer logic in this phase unless a small fallback fix is required.

## Tests

Add client contract coverage for:

- Interpolating from `0` to `Math.PI / 2` at `alpha = 0.5`.
- Interpolating across the `-PI`/`PI` wrap takes the short path.
- Missing prior entity keeps current `facing`.
- Missing current `facing` does not add a field.
- `x` and `y` interpolation still works.

Run:

```bash
node tests/client_contracts.mjs
cd tests && npm install && node client_smoke.mjs
```

## Acceptance Criteria

- Moving and attacking units no longer snap their rendered angle between snapshots.
- Angles near wraparound do not spin the long way.
- Snapshot data shape is unchanged.
- No server, protocol, or design docs change in this phase.
