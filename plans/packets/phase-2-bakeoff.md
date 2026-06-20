# Snapshot Codec Bake-off

Generated: 2026-06-19T15:00:09.062Z
Source: deterministic-fixture
Samples: 3
Payload budget: 1280 bytes

| candidate | p50 bytes | p95 bytes | max bytes | over budget | encode p95 ms | decode p95 ms | dependency risk | browser risk | maintenance |
|---|---:|---:|---:|---:|---:|---:|---|---|---|
| Compact JSON | 4114 | 17533 | 17533 | 66.67% | 0.0819 | 0.1067 | none | none | low |
| Compact JSON + deflate | 1206 | 4466 | 4466 | 33.33% | 0.6294 | 0.1269 | low | medium | medium |
| Proto-style schema TLV | 2812 | 11976 | 11976 | 66.67% | 0.6382 | 0.5148 | medium | medium | high |
| MessagePack compact object | 2013 | 8826 | 8826 | 66.67% | 0.3471 | 0.2864 | medium | medium | medium |
| CBOR compact object | 2247 | 9598 | 9598 | 66.67% | 0.4507 | 0.4207 | medium | medium | medium |
| Custom positional binary | 2799 | 11963 | 11963 | 66.67% | 0.2301 | 0.1901 | none | high | high |

## Notes

- Compact JSON: Current live text-frame baseline.
- Compact JSON + deflate: Offline deflateRaw proxy for permessage-deflate; not measured as actual browser wire bytes.
- Proto-style schema TLV: Manual schema-TLV stand-in for generated protobuf; enough to compare key-table binary pressure.
- MessagePack compact object: Schema-less binary encoding of the current compact positional object.
- CBOR compact object: Schema-less binary encoding of the current compact positional object.
- Custom positional binary: Versioned custom binary for the compact snapshot top-level shape with generic nested values.

## Recommendation

Historical result, superseded by Phase 2.5 and Phase 2.6: use MessagePack compact binary frames as
the active full-snapshot baseline. Keep this bake-off as local comparison evidence for compact JSON,
offline deflate, schema-style binary, MessagePack, CBOR, and custom positional binary candidates.

Deflate had the smallest measured p95 (4466 bytes vs compact JSON 17533), but the current
measurement is an offline proxy and the server/browser extension path was deliberately not promoted
into a compression rollout. Larger packet reductions should come from the later fog-safe delta
phases after explicit user approval.

## Limits

- Deflate numbers are compressed payload bytes from Node zlib, not verified browser post-extension wire bytes.
- Browser apply cost is unchanged unless a live client decoder replaces the current JSON path; this bake-off measures candidate encode/decode CPU only.
- Raw snapshot payloads are not uploaded by normal clients; only local harness captures should be used as inputs.

## Executor Coverage

- The committed numbers above are from deterministic compact snapshot fixtures and are reproducible with `node scripts/snapshot-codec-bakeoff.mjs --fixture --iterations 3`.
- The browser harness now supports Matt/Alex replay and vehicle-wall stress capture with `--snapshot-codec-bakeoff`, but this executor sandbox rejected local server port binding with `listen EPERM: operation not permitted 127.0.0.1`.
- Four-AI server-side payload and serialization numbers remain available through `scripts/ai-perf-harness.sh`; this phase did not make any codec the live default from fixture-only evidence.
