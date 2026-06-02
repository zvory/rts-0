# Phase 05: Unreliable Snapshot Delivery

Purpose: move snapshots away from ordered reliable delivery so obsolete snapshots do not block
newer state. This is the phase that can actually use WebTransport to reduce network head-of-line
stutter.

## Preconditions

- Phase 00 has evidence that network delivery or stale ordered snapshots are a real problem.
- Phase 01 coalescing has been tried or deliberately skipped.
- Phase 04 WebTransport reliable control works locally.
- WebSocket fallback remains available.

## Transport Choices

Preferred path:

- reliable control stream for commands and session messages;
- WebTransport datagrams for snapshots when the payload fits.

Fallback path:

- reliable control stream for commands and session messages;
- one unidirectional stream per snapshot for oversized snapshots.

Avoid chunking datagrams in the first version. Chunking creates a reliability protocol:

- reassembly buffers;
- duplicate handling;
- missing chunk expiry;
- memory limits;
- partial snapshot discard;
- denial-of-service limits.

If one datagram cannot contain a snapshot, use stream fallback or reduce payload size in Phase 06.

## Datagram Header

Use a tiny binary prefix even if the payload remains JSON:

```text
byte 0      protocol version, start at 1
byte 1      message kind, 1 = snapshot
bytes 2-5   tick, u32 little-endian
bytes 6-9   match/session id, u32 little-endian
bytes 10..  payload bytes
```

Client behavior:

- drop datagrams with an unknown version;
- drop datagrams for a previous match/session id;
- drop snapshots with `tick <= current.tick`;
- keep at most the two newest accepted snapshots for interpolation;
- if a gap occurs, interpolate between the newest two accepted snapshots when possible;
- do not extrapolate hidden state.

Server behavior:

- send snapshots as best effort;
- never retransmit old datagram snapshots;
- log oversize snapshots;
- use stream fallback or skip the datagram if a snapshot is too large;
- never silently truncate.

## Snapshot Loss Semantics

Unreliable snapshots require every accepted snapshot to be independently useful.

Already safe:

- `tick`: absolute ordering.
- `steel`, `oil`, `supplyUsed`, `supplyCap`: current absolute values.
- `entities`: current visible entity views.
- `events`: transient visual flavor, mostly safe to lose.

Unsafe or ambiguous:

- `resourceDeltas`: currently incremental in the client model.
- `notice` events: some notices may be actual user feedback rather than flavor.

### Resource remaining values

Today `start.map.resources` carries static resource positions, and snapshots carry visible
remaining updates. If an unreliable snapshot with the latest remaining amount is lost, the client
may render stale resource amounts until another visible delta arrives.

Possible fixes:

1. Include visible resource remaining values in every snapshot, even if unchanged.
2. Move resource remaining updates to the reliable control stream.
3. Add a periodic reliable resource keyframe.
4. Reintroduce visible resource entities for WebTransport snapshots and keep WebSocket compaction
   as a legacy-transport optimization.

Recommended first choice: option 1. It makes current render-critical state absolute, repeated, and
safe to lose once.

### Notice events

Decide whether `notice` is best-effort or reliable.

If "Not enough steel" must be seen, move notices onto the reliable control stream. If notices are
visual flavor, they can remain in snapshots and be lost.

## Payload Size Rules

Datagrams need conservative sizing. Do not assume that a 4 KB JSON snapshot will fit. The browser
API exposes datagram limits; the server stack will have its own limit; the network path has an MTU.

Initial rules:

- log the actual WebTransport datagram max size at connection start;
- keep snapshot datagrams below the lower of client/server max size;
- keep a conservative one-UDP-packet target until measurement proves otherwise;
- log p50/p90/p99/max payload sizes in dev;
- use fallback or skip when oversized;
- never chunk in this phase.

The earlier baseline was about 1.2 KB for early normal snapshots and about 3.8 KB for early
full-world snapshots. Late-game numbers are likely larger.

## Client Apply Logic

Add or verify stale rejection:

```js
if (this._cur && msg.tick <= this._cur.tick) return;
```

Verify interpolation behavior with dropped snapshots:

- one missed snapshot should produce normal interpolation over a larger tick gap;
- several missed snapshots should freeze on newest available state until the next snapshot;
- an older datagram arriving late must be ignored;
- duplicate datagrams must be ignored.

## Tests

Add tests that simulate:

- dropping every Nth snapshot;
- reordering snapshots;
- duplicating snapshots;
- sending old snapshots after newer snapshots;
- sending an oversize snapshot;
- losing a resource update;
- switching matches and receiving an old match/session datagram.

Use a real browser for WebTransport smoke coverage.

## Done Criteria

- Client tolerates lost, duplicated, and reordered snapshots.
- Client never applies older ticks.
- Resource remaining semantics are safe under loss.
- Oversized snapshots have explicit behavior.
- Stutter trace improves under packet loss compared with WebSocket or Phase 01.
- WebSocket fallback still works.
