# Phase 6 - Integration, Regression Tests, and Docs

Goal: verify the whole movement stack and document the final contract.

This phase happens after one or more behavior-changing phases are complete. It may be skipped for a
documentation-only branch.

## Required Verification

Run focused tests first:

```bash
cd server && cargo fmt && cargo test
```

Then run integration tests when movement behavior changed:

```bash
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

If a test requires a running server, follow the existing test README and `CLAUDE.md` guidance.

## Documentation Updates

Update `DESIGN.md` if any of these changed:

- Pathing no longer exposes every tile-center waypoint to movement.
- Movement uses static line-of-sight segment simplification.
- Tanks use longer segment-bounded lookahead.
- Tank pathing has turn costs.
- Client rendering smooths tank hulls in a new way.

Keep docs factual. Do not claim "tanks feel better" without describing the actual mechanics.

## Regression Coverage Checklist

Add or confirm tests for:

- Open-route smoothing.
- Obstacle-corner preservation.
- Tank radius clipping prevention.
- Final goal preservation.
- Tank facing around corners.
- Deterministic repeated path requests.
- Non-tank behavior unchanged where expected.
- Replay or integration coverage if movement changes affect live games broadly.

## Gameplay Patch Notes

For behavior-changing phases, include player-facing patch notes:

- Tanks now follow straighter legal route segments instead of steering through every tile center.
- Tanks should visibly pivot/slow for real corners rather than slide through small path corrections.
- No intended change to armor damage rules unless a later phase explicitly changes them.

If the impact is uncertain, state what changed mechanically and what should be watched in playtests.

## Acceptance Criteria

- Relevant tests pass or any failures are clearly documented with root cause.
- `DESIGN.md` is in sync with changed contracts.
- The final summary names the gameplay impact plainly.
- No unrelated files are staged or committed.

