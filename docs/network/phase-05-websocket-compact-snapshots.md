# Phase 05: Compact/Binary WebSocket Snapshots

Purpose: reduce WebSocket frame size and snapshot parse/apply pressure without changing transport.

This phase should be measurement-driven. Do it after Phase 00 shows that snapshot size, JSON parse,
or JSON object allocation is a material contributor, or after Phase 01/02 still leave unacceptable
stutter under packet loss.

## Why This Helps The Current WebSocket Path

Smaller snapshot frames help the existing WebSocket path:

- less time spent serializing on the server writer task;
- fewer bytes exposed to TCP head-of-line stalls;
- less browser parse/allocation work;
- lower chance that a snapshot write blocks long enough to build backlog.

This still uses one reliable ordered WebSocket. It does not remove TCP head-of-line blocking.

## Scope

Optimize snapshots first. Keep lobby/control messages as JSON unless measurement says otherwise.

Candidate approaches, in increasing complexity:

1. Compact JSON:
   - shorter field names for snapshot-only payloads;
   - numeric enum codes for `kind`, `state`, `prodKind`, `setupState`, and events;
   - omit stable fields that can be loaded from static tables if still independently decodable.
2. Array-shaped JSON:
   - encode entities as fixed-position arrays rather than objects;
   - keep a versioned schema doc in `DESIGN.md` or this phase file.
3. Binary snapshots over WebSocket:
   - send snapshot frames as `Message::Binary`;
   - keep control messages as JSON text;
   - use numeric enums and fixed-width integers.

Do not jump to binary until compact/array JSON has been considered. Binary is more code and more
test surface.

## Binary Snapshot Sketch

If binary is chosen, keep it versioned:

```text
byte 0      protocol version
byte 1      message kind, 1 = snapshot
bytes 2-5   tick, u32 little-endian
bytes 6-9   steel, u32
bytes 10-13 oil, u32
bytes 14-17 supplyUsed, u32
bytes 18-21 supplyCap, u32
bytes ...   entity count + entity records
bytes ...   event count + event records
```

Entity records should use numeric kind/state ids mirrored between Rust and JS. Positions can be
quantized if measurement shows it is worth it.

Keep `server/src/protocol.rs` as the semantic source of truth even if snapshot wire encoding gets a
binary transport-specific representation.

## Resource Delta Caveat

If snapshots are compacted, do not accidentally make `resourceDeltas` harder to reason about.
Before binary snapshots, decide whether visible resource remaining values should be absolute and
repeated while visible. That also helps Phase 02 and this phase.

## Tests

- Encode/decode round-trip for representative snapshots.
- Malformed binary/compact payload bounds tests.
- Browser smoke test against the real client.
- Comparison test: decoded compact/binary snapshot produces the same client-visible state as the
  existing JSON snapshot.
- Payload-size regression fixture for representative snapshots, if fixture generation is stable.

## Done Criteria

- Snapshot byte p50/p90/p99/max improves under representative load.
- Parse/apply cost improves if that was the measured problem.
- Control messages can remain JSON and reliable.
- WebSocket fallback and existing tests pass.
