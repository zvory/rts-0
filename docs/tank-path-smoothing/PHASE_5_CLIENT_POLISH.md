# Phase 5 - Client Visual Polish Only

Goal: add client-side polish after authoritative movement is improved, without lying about tank
armor facing.

This phase should be small. The main fix belongs on the server.

## Allowed Client Polish

Acceptable changes:

- Smooth visual interpolation between authoritative snapshots.
- Add very small hull render interpolation if it uses the same server `facing` samples.
- Avoid jitter from angle wraparound by using existing `lerpAngle` behavior.
- Add optional debug rendering for paths or headings if it is hidden from normal play.

## Not Allowed

Do not:

- Render tank hull facing differently from authoritative `facing` in a way that changes perceived
  armor angle.
- Invent client-side positions that diverge materially from server snapshots.
- Add client-only obstacle avoidance.
- Hide server movement bugs with large easing delays.

## Files to Inspect

- `client/src/state.js`
- `client/src/renderer.js`
- `client/src/protocol.js`

If no client changes are needed after server fixes, skip this phase and say so in the PR summary.

## Tests

Use existing client smoke coverage if client code changes:

```bash
cd tests && npm install && node client_smoke.mjs
```

If adding debug path rendering, ensure it is disabled by default and does not affect normal player
UI.

## Acceptance Criteria

- Authoritative server `facing` remains the rendered tank hull direction.
- Owned, allied, and visible enemy tanks still render consistently.
- No protocol shape changes unless `server/src/protocol.rs`, `client/src/protocol.js`, and
  `DESIGN.md` are updated together.

