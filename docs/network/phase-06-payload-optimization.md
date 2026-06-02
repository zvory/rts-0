# Phase 06: Payload Optimization

Purpose: make snapshots small and cheap enough for unreliable delivery and browser parsing. Do this
only after measurement proves payload size or parse/allocation cost is a limiting factor.

## When This Phase Is Needed

Do this phase if:

- worst-case snapshots do not fit the chosen datagram budget;
- JSON parse or object allocation shows up in Phase 00 long-task traces;
- WebTransport datagrams work but fall back to streams too often;
- late-game p90/p99 snapshot sizes are much larger than early-game baselines.

Do not do this phase only because binary protocols are theoretically nicer.

## Principles

- Keep control/lobby messages JSON unless measurement says otherwise.
- Optimize snapshots first.
- Keep snapshots independently decodable.
- Avoid delta-only state until there is a reliable keyframe/resync design.
- Preserve fog authority: never send hidden enemy data as an optimization shortcut.

## Candidate Optimizations

### Binary snapshot format

Replace snapshot JSON with a compact binary payload:

- numeric message kind;
- u32 tick;
- u32 resources/supply;
- compact entity records;
- compact event records;
- compact resource remaining records.

Keep `server/src/protocol.rs` JSON types as the semantic source of truth until the binary format is
well documented. Add encode/decode tests that compare binary-decoded snapshots with the JSON shape.

### Intern strings

Replace repeated strings with numeric ids:

- `kind`;
- `state`;
- `prodKind`;
- `setupState`;
- event kind;
- player color only if needed in start/control messages.

The client already has constants in `client/src/protocol.js`. Add a mirrored numeric table rather
than ad-hoc magic numbers.

### Quantize positions

Current positions are floats in world pixels. Options:

- u16 or u32 fixed-point world pixels;
- integer pixels;
- smaller fixed-point if subpixel precision matters visually.

Measure visual impact before committing. Pathing and simulation can stay float internally.

### Reduce snapshot rate

Consider sending 15 snapshots/s while keeping 60 fps rendering and interpolation. This is simpler
than binary encoding, but can reduce input feedback and combat visual smoothness.

Do not change `TICK_HZ`; this only changes outbound snapshot cadence.

### Interest filtering

Fog already filters enemy/neutral entities by visibility. If late-game owned entities dominate
payloads, consider additional client interest rules carefully. Owned units outside the camera may
still be needed for minimap, selection, alerts, and command feedback.

## Things To Avoid Initially

- Multi-datagram chunking.
- Delta-only snapshots without periodic absolute keyframes.
- Compression on tiny per-frame payloads before measuring CPU cost.
- Changing simulation state just to make transport encoding easier.

## Tests

- Encode/decode round-trip for every snapshot field.
- Fuzz or property tests for malformed binary payload bounds.
- Browser smoke test using binary snapshots.
- Comparison test: JSON snapshot and binary snapshot decode to equivalent client-visible state.
- Payload size regression test for representative snapshots, if fixture generation is stable.

## Done Criteria

- Worst-case snapshots fit the chosen datagram/stream budget, or fallback rate is acceptable and
  measured.
- Parse/apply cost improves relative to JSON baseline.
- Binary format is documented enough for a future agent to extend safely.
- WebSocket fallback still works, either by keeping JSON or by sharing the binary path deliberately.
