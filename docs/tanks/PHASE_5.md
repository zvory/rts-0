# Phase 5 - Client Presentation and Player Feedback

## Goal

Make the improved server movement read clearly to players. The server owns the real simulation, but
the client should make track movement, hull facing, turret facing, and speed changes obvious.

## Steps

1. Audit tank rendering against the new server body dimensions. The visible hull should match the
   physical body closely.
2. Add or adjust track animation using actual movement distance and turn direction:
   - both tracks forward for forward drive;
   - both tracks backward for reverse;
   - opposite track motion for pivot turns;
   - slower track motion during braking or low oil.
3. Keep interpolation angle-safe for hull and turret facing.
4. Review selection hit testing and selection ring size so player clicks match the new body.
5. Add lightweight feedback for immobilized or oil-starved tanks if existing UI is not enough.
6. Capture before/after screenshots or short replay notes for playtest comparison.

## Plain-Language Explanation

The server can make tanks move correctly, but players still judge movement by what they see. This
phase makes the tank art and UI line up with the new physical body and makes track movement explain
why the tank is turning, reversing, or stopped.

## Expected Code Touches

- `client/src/renderer.js`
- `client/src/config.js`
- `client/src/input.js`
- `client/src/hud.js` if extra tank feedback is needed
- `DESIGN.md` if render/config mirrors change

## Refactor Depth

Low to medium. This should be presentation work unless server body dimensions require mirrored
client config.

## Done When

- Rendered tank hull and selection affordances match server occupancy closely.
- Track animation communicates forward, reverse, and pivot movement.
- No client module leaks listeners or resources across rematches.

