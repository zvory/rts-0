# Phase 5 - Losses, Resources Lost, and Hardening

Status: Not implemented.

## Objective

Complete the first replay analysis suite by rendering losses and resources-lost tabs, then harden
the whole overlay system around replay seeking, vision changes, layout, lifecycle, and performance.

## Scope

- Units lost tab:
  - show per-player per-kind lost counts
  - show steel and oil value lost
  - show totals
- Resources lost tab:
  - render the Phase 3 resource-loss definition exactly
  - include enough labeling that users understand whether the number means killed army value,
    spent resources destroyed, or broader economy loss
- Add final tab switching polish and keyboard/focus behavior if the overlay is interactive.
- Confirm overlay state survives:
  - timeline seeks
  - rewind buttons
  - reset-to-start
  - replay conclusion
  - replay branch creation
  - returning to lobby
- Confirm layout does not overlap the top replay resources HUD, minimap, command card area,
  replay speed/timeline controls, game-over score screen, or settings menu on common desktop and
  mobile viewport sizes.
- Add or update docs for replay analysis UX and protocol behavior.

## Expected Touch Points

- replay analysis overlay module
- `client/styles.css`
- client tests/smoke harness
- `docs/design/client-ui.md`
- `docs/design/protocol.md` only if Phase 3 contract needed correction
- `docs/context/client-ui.md` or `docs/context/protocol.md` only if section lists shifted

## Verification

- DOM/unit tests for losses and resource-loss rendering.
- Replay/session regression tests if Phase 3 server counters need adjustments.
- Browser smoke test that opens a replay, toggles every tab, seeks, and asserts tab state and
  representative numbers remain present after the rebuild.
- Client architecture check:

```bash
node scripts/check-client-architecture.mjs
```

- Run protocol tests if any protocol/doc correction is made.

## Manual Testing Focus

Watch a replay with at least one major fight. Before the fight, use Army Value and Units tabs to
compare visible and global state; after the fight, use Units Lost and Resources Lost tabs to confirm
losses increased for the correct players. Seek before and after the fight several times and confirm
numbers do not double-count or lag behind the replay tick.

Also test a post-match replay, a match-history replay, and replay branch creation from a tick where
the overlay is open.

## Handoff Expectations

The handoff must include final manual testing notes, any residual automated coverage gaps, and a
short player-facing patch-note summary of the new replay analysis affordances. If further overlays
are desired, the handoff should describe how to add a new tab descriptor and whether it needs
client-only viewport data or server-backed replay analysis data.

## Player-Facing Outcome

Replay viewers have a coherent analysis suite for current fight value, production, unit
composition, units lost, and resource value lost, with reliable behavior across replay seeking.

