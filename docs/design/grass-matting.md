# Grass matting experiment

## Delivered scope

`grass-survey-12` is an opt-in Lab visual prototype, built on normal profile 9 turf.
Repeated foot-unit traffic gradually turns turf into softly feathered gray-brown dirt and thins
its grass tufts and their root shadows. Individual passages stay below visible wear; about
20 center-line passages over the same patch produce
clear wear. This means repeated traffic over a patch, not merely having 20 units somewhere nearby.
The field saturates and does not regrow. All vehicles and wheeled support weapons are excluded;
the existing tank dirt-track renderer and authoritative tread history are unchanged.
No gameplay, network, checkpoint, or replay schema changes ship with this visual experiment.

Run `node scripts/interact/cli.mjs lab open '{"visualProfile":"grass-survey-12"}'`,
then use catalog, spawn, and normal move orders. Use paused `capture-fixed` for dependable video.
The field persists only for the current material instance. Seeking, changing perspective, and
reconnecting are **not supported** by this prototype. Do not enable it outside Lab.
There is no regrowth; this isolates the persistent traffic-wear look requested for review.

The visual review scene uses only 20 Riflemen in a 5×4 clump, initially spaced 20 world pixels
apart from `(1660,1760)`. They make five round trips between group goals `(2040,1800)` and
`(1700,1800)`. Fixed captures at 10 FPS retain every output frame when encoded at 30 FPS for
3× playback. The camera focuses `(1870,1800)` at framing scale 2, viewport 1100×760, DPR 1.

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

The prototype keeps a bounded RGBA data texture on a grid with minimum 4-world-pixel spacing.
Either dimension is capped at 1,024 texels by increasing spacing on large maps. R contains the
shader-visible wear; GB pack a saturating unsigned 16-bit traffic counter. Alpha is always opaque
so canvas premultiplication cannot alter the data. The retained field is at most 4 MiB, plus another
4 MiB ImageData staging copy, canvas backing, GPU texture, and currently visible contact poses.
Canvas/driver copies are additional and not measured.

Each foot unit deposits a soft 28-world-pixel-radius blob every four world pixels
travelled. The contact pose carries the remaining distance to the next sample, so different render
cadences trace the same spatial samples along the same path. The squared radial kernel is normalized
by its center-line integral (16r/15), giving approximately 1/20 of full wear per passage. Integer
credit is retained in GB before quantizing visible wear to R. Credit saturates at 65,535; stationary
units cannot accumulate wear and a saturated/unchanged visible field queues no upload. Overlapping
blobs and ground variation produce a broad, irregular worn lane rather than authored footsteps.

Absent/revealed-only entities never connect motion across fog gaps; jumps over 96 pixels reset
contact, including its sampling phase. This is presentation filtering, not durable server fog
authority. The existing CPU microbenchmark is a field-only diagnostic; it excludes texture uploads,
canvas copies, shader cost, simulation, and replay switching. Its 75% infantry / 25% tank mix now
also exercises vehicle exclusion and must not be interpreted as 100% active foot-unit contacts.

A changed frame currently uploads the whole mask. At the cap, 60 changed frames/second imply
240 MiB/second of raw RGBA upload payload, before overhead. The shader also samples pressure
once per fragment for soil color plus once per candidate tuft (up to ten pressure reads per
grass fragment). This simple prototype is unsuitable as a production performance implementation.

## Measured cumulative-wear client cost

A delegated check ran 23 valid serialized samples of the canonical
`fixed-roster-hellhole-stream` at 1440×900, DPR 1, CPU throttle 1, on Apple M5 Pro. The earlier
17 samples compared plain grass, previous matting, and cumulative wear before the final root-shadow
fade. Six further samples validated the final shader: four 15-second uncapped samples in
plain/new/new/plain order, then one normal-rAF pair. Final-source means were:

| Metric | Plain grass | New infantry wear |
| --- | ---: | ---: |
| Uncapped worker-acknowledged frames/sec | 118.73 | 121.06 |
| Worker update + present elapsed ms/frame | 6.415 | 6.310 |
| Normal-rAF completed frames/sec | 59.60 | 59.53 |

These values do not establish a causal improvement or precise regression. Same-mode capacity
varied 113.67–123.80 for plain grass and 117.80–124.33 for new wear, exceeding the apparent mean
difference. The earlier block also showed substantial session drift. All original samples were
retained; no apparent gain is claimed by selecting one block. The defensible conclusion is no
clear measurable steady-state slowdown in this local session.

Worker-acknowledged capacity is a pipelined benchmark, not screen refresh rate or GPU-complete
FPS. GPU timer queries were supported and valid, but their draw-cost contrasts varied across
blocks and do not isolate an intrinsic shader penalty. Both variants used the same recorded
battle, player fog, camera, terrain shadows, and presentation cadence. The sole final off/on
switch was the default grass technique (9 versus 12). All 23 samples reported zero failed or
superseded frames, GPU disjoint intervals, and dropped queries.

These measurements cover steady-state short-stream rendering, not startup, fully worn long
matches, weaker devices, or replay switching. Replay wear persistence and perspective caches
are still unimplemented.

## Recommended durable representation (not implemented)

Use sparse, server-owned 512-world-pixel pressure tiles with 4-pixel cells: 128×128 cells/tile.
Two bytes per cell can encode a saturating traffic count: 32 KiB per allocated tile,
plus map/index/revision overhead. A fully traversed 4,096²-world-pixel map has 64 tiles:
2 MiB per complete pressure field before compression. Do not allocate a full copy per player
by default. Compress checkpoint tiles with zero runs or another measured codec; transfer changed
cell runs / tile revisions, not one permanent mark per footstep. Keep a bounded global state,
not an ever-growing path log. Sweep moved contacts deterministically on simulation ticks,
skipping stationary units and non-grass terrain. A monotone traffic field avoids regrowth timers.
Spatial sample carry and integer credit rules must be shared by live updates and checkpoint restore.

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
