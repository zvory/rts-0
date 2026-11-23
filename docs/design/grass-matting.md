# Grass matting experiment

## Delivered scope

`grass-survey-12` is an opt-in Lab visual prototype, built on normal profile 9 turf.
Infantry leaves a soft partly flattened central path; wheeled vehicles, guns, and tanks leave
paired stronger lines. Other ground units use the central path. Aircraft never touch grass.
Pressure bends blade tips along travel and reduces wind motion; the existing tank dirt tracks
remain separate. No gameplay, network, checkpoint, or replay schema changes ship in this PR.

Run `node scripts/interact/cli.mjs lab open '{"visualProfile":"grass-survey-12"}'`,
then use catalog, spawn, and normal move orders. Use paused `capture-fixed` for dependable video.
The field persists only for the current material instance. Seeking, changing perspective, and
reconnecting are **not supported** by this prototype. Do not enable it outside Lab.
There is no regrowth; this isolates the persistent-matting look requested for review.

## Existing machinery inspected

- `server/crates/sim/src/game/ground_decal/tank_trail.rs` quantizes positions to 4 world pixels,
  samples at 64 pixels of travel or 0.25 radians, finalizes chunks of up to 8 poses / 512 pixels
  span, and caps finalized history at 4,096 chunks. Its checkpoint module packs poses and rebuilds
  the spatial index on restore. Blindly adding every infantry unit would exhaust that shared cap
  sooner and make restoration scale with cumulative travel.
- `client/src/renderer/tank_tread_layer.js` lazily allocates 512-world-pixel tiles, clips historical
  painting to currently visible map tiles, discards processed reveal buckets, and uploads only
  touched tile textures. Live contacts are swept between 10 Hz uploads.
- `client/src/match_ground_decal_sync.js` resets the discovery cursor and requests correlated
  replacement authority after seek/perspective changes. Old inline deltas stay blocked until repair.
  `renderer/decals.js` batches replacement hydration; `state_ground_decals.js` deduplicates history.
  Those semantics matter more than copying a tank's particular pigment renderer.

## Cost and limits

The prototype stores RGBA pressure plus travel direction on a grid with minimum 4-world-pixel
spacing. Either dimension is capped at 1,024 texels by increasing spacing on large maps.
The mask is opaque data with heading channels centered on byte 128, including untouched texels,
so canvas premultiplication and linear filtering do not introduce a spurious edge direction.
Zero-pressure blades retain the original curved silhouette.
The retained field is at most 4 MiB, plus another 4 MiB ImageData staging copy, canvas backing,
GPU texture, and current visible-unit poses. Canvas/driver copies are additional and not measured.
Repeated pressure uses max, so stationary units and unchanged revisits do not queue uploads.
Absent/revealed-only entities never connect motion across fog gaps; jumps over 96 pixels reset
contact. This is presentation filtering, not a substitute for durable server fog authority.

`node scripts/bench-grass-matting.mjs` measures CPU field updates only: 600 frames, first 100
excluded, 128×128 tiles, 75% infantry / 25% tanks moving 2.1 pixels per update. Initial local
results: 100 units median 0.120 ms / p95 0.443 ms; 1,000 units median 1.373 ms / p95 2.523 ms.
These are Node microbenchmarks on this machine, not browser frame-time or GPU claims.
They exclude texture uploads, canvas copies, shader cost, simulation, and replay switching.

A changed frame currently uploads the whole mask. At the cap, 60 changed frames/second imply
240 MiB/second of raw RGBA upload payload, before overhead. The shader also samples pressure
once per candidate tuft (up to nine texture reads per grass fragment). This intentionally simple
prototype is unsuitable as a production performance implementation.

## Recommended durable representation (not implemented)

Use sparse, server-owned 512-world-pixel pressure tiles with 4-pixel cells: 128×128 cells/tile.
Two bytes per cell can encode quantized pressure and heading: 32 KiB per allocated tile,
plus map/index/revision overhead. A fully traversed 4,096²-world-pixel map has 64 tiles:
2 MiB per complete pressure field before compression. Do not allocate a full copy per player
by default. Compress checkpoint tiles with zero runs or another measured codec; transfer changed
cell runs / tile revisions, not one permanent mark per footstep. Keep a bounded global state,
not an ever-growing path log. Sweep moved contacts deterministically on simulation ticks,
skipping stationary units and non-grass terrain. A monotone pressure field avoids regrowth timers.
Direction ties need a deterministic rule shared by restore and live updates.

Separate world pressure from per-player knowledge. Preserve the game's existing mark discovery
semantics: materialize only owned/actually discovered pressure, and clip reveal to authoritative
visible cells. Do not send full-world pressure to players or cache omniscient textures under a
player key. Changes behind fog must not silently update a player's remembered tile. Team/allied
unions must follow the same authority as snapshots. This requires protocol, fog, and checkpoint
contract work; a renderer-only motion history cannot reconstruct unseen travel at a replay seek.

## Replay perspective switching

Checkpoint pressure tiles and discovery/remembered revisions alongside simulation state. Restore
from the nearest replay checkpoint, then apply only the subsequent bounded tile deltas. This
bounds work by checkpoint distance and affected cells, rather than total match duration.

Cache immutable CPU tiles and GPU pressure pages under replay identity, checkpoint/tick epoch,
exact perspective/team union, tile coordinate, and discovered tile revision. Reuse unchanged tile
pages across selections only when their content and authorization match. Cap the cache by bytes
with LRU eviction. Backward seeks must reject later revisions and cancel stale hydration jobs.
A repeated warm switch should mostly exchange page references; a cold switch hydrates only visible
pages under an explicit per-frame byte/time budget, with generation-tagged completion. Immediately
hide the old perspective's surface until authorized replacement pages are ready. Never temporarily
show another perspective's flattened grass. Avoid clearing/rasterizing all historical paths on the
switch handler. Keep normal presentation cadence intact; cold loading is explicit asset hydration,
not skipping updates of already-presented state.

GPU use should follow the existing sparse tread approach: dirty page uploads and a pressure-page
atlas/page table or bounded chunk meshes sharing the grass shader. Measure the page lookup versus
nine candidate-root samples; a pressure lookup once per fragment is cheaper but changes tuft-edge
semantics, so assess it as a visual choice rather than an exact optimization.

Before promoting this: measure full browser/worker/GPU frame cost on the canonical
`fixed-roster-hellhole-stream`; cold and warm perspective switch p50/p95/max after a long replay;
backward/forward seeks; peak cache memory and upload bytes; and exact fog-boundary correctness.
The microbenchmark above does not establish a stutter-free switch or a production frame budget.

The unmodified-default canonical baseline run reported main-thread `frame.work` average 5.1 ms,
p95 8 ms, with 59.8 completed presentations/second. Its PNG and ranked JSON identify minimap,
entity snapshot cloning, and worker submission as existing main-thread costs. This is context,
not an incremental cost measurement for profile 12. The independent worker flame-graph conversion
failed (`invalid time delta at sample 9050`), so no complete worker-profile claim is made. The
prototype's full GPU cost and replay-switch timings remain unmeasured.
