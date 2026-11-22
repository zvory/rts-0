# Grass material studies

Five Lab-only visual studies compare directions for replacing the coarse static grass surface.
Normal gameplay keeps its current terrain. The prototypes share a world-anchored procedural mesh
approach inspired by the animated water layer; they are five visual treatments, not five
production-ready renderer architectures.

| Profile | Technique | Intended effect | Tradeoff |
| --- | --- | --- | --- |
| `grass-survey-1` | Fine pixel carpet with moving wind-light bands | Quiet ground texture; coherent wind across a large field | Reads more as surface lighting than individual bending grass |
| `grass-survey-2` | Sparse tapered blade clusters with anchored roots | Clearly visible bending tips and open ground between tufts | Repeated cluster shape can look planted |
| `grass-survey-3` | Noise-distributed dense clumps | Broken, fuller grass islands with coordinated sway | More visual detail around small infantry |
| `grass-survey-4` | Dry meadow with taller blades and seed heads | Warm natural variation and slower-looking long stalks | Changes the biome/palette more substantially |
| `grass-survey-5` | Mixed tuft carpet, coherent gusts, broad moving shade | Layered local motion and large-scale field movement | Most complex treatment; shade is decorative, not weather simulation |

The selected direction is **5**, with a requested refinement: continuous grass coverage rather than
isolated tufts over exposed ground. The follow-up profiles below retain its gust/shade direction
while changing how dense blade detail is represented. These intentional visual changes are not
pixel-parity optimizations of profile 5.

## Reproduction

Use Interact from a clean task worktree. Open Classic with seed 1 and one of the profile IDs,
then run `catalog`, pause room time, and set camera focus `(2700, 1740)`, framing scale `1.5`,
with `boundsPolicy: "mapOverscroll"`. The comparison viewport is 1100×720 at DPR 1.
Spawn player 1 Riflemen at `(2510,1700)`, `(2540,1740)`, `(2490,1770)` and a Scout Car at
`(2430,1830)`. Use `capture-fixed` at 30 FPS for 180 frames; convert the returned H.264 MP4
with `ffmpeg -i input.mp4 -c:v libvpx-vp9 -crf 24 -b:v 0 -an output.webm`.
The five clips share map, camera, scene layout and six-second duration; each session's visual-clock
phase starts independently. They are visual comparisons rather than an exact-pixel parity test.

These sketches use a partly opaque grass-only surface below objects, shadows, decals and fog.
They retain only a small contribution from baked ground shading; full elevation/terrain-shadow
integration, shoreline transitions, camera-scale antialiasing and GPU cost need a production pass.
No physical blade/unit interaction or grass gameplay mechanics are implemented. The fragment-loop
prototype is intended for rapid visual evaluation, not a performance recommendation.

## Survey references

- [NVIDIA GPU Gems: Rendering Countless Blades of Waving Grass](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-7-rendering-countless-blades-waving-grass)
  motivates grouped blades and rooted wind motion. Crossed 3D cards are unnecessary for this view.
- [GDQuest wind grass shader source](https://github.com/gdquest-demos/godot-shaders/blob/main/godot/Shaders/wind_grass.gdshader)
  demonstrates coherent world-space wind and root-to-tip displacement.
- [Billboard grass with wind, original shader](https://godotshaders.com/shader/billboard-grass-with-wind/)
  demonstrates clump variation, palette gradients and tip-weighted sway.
- [Sucker Punch procedural grass talk](https://www.youtube.com/watch?v=Ibe1JBF5i5Y)
  provides field-composition and supplemental-plant inspiration. Its engine and performance claims
  do not transfer to this Pixi renderer.

The prototypes use original procedural shader code; the references inform the techniques rather
than being drop-in engine ports.


## Continuous coverage follow-up

| Profile | Technique | Runtime texture reads | RGBA8 base-level texel storage |
| --- | --- | --- | --- |
| `grass-survey-6` | Dense baked short-blade weave, warped and relit by a wind/shade field | 2 | 320 KiB |
| `grass-survey-7` | Two differently oriented samples of a taller weave, with different wind distortion | 3 | 320 KiB |
| `grass-survey-8` | Eight cached dense blade-tip poses, smoothly blended under the wind/shade field | 3 | 2.06 MiB |

Memory figures exclude shared mesh geometry, driver overhead and upload staging.
Overlapping source strokes fill every texture cell. Density is paid during one-time source-art
baking, rather than by adding runtime blade objects or repeated neighbor searches per fragment.
All three update the material every presented frame; the atlas interpolates between authored
poses instead of lowering presentation cadence. Profiles 6/7 move the sampled weave as a surface;
profile 8 keeps source roots anchored while its baked tips bend. The latter is the preferred next
candidate for natural-looking movement; the simple weave is the lower-memory alternative.
These remain Lab studies with the original grass lighting/transition integration limitations.

Reproduce using the same scene above and profiles 6, 7 or 8. Visual-clock phase is independent per
session. The new cloud field is a baked, repeatable low-frequency field with the same broad moving
shade direction, rather than an exact copy of profile 5's procedural cloud noise.

### Measured material GPU cost

Run `node scripts/grass-material-benchmark.mjs <output.json>` with the pinned repository Node
dependencies and Chrome. This is an isolated material microbenchmark using the real material class
in the pinned Pixi WebGL worker: a 64×64 all-grass map fills a 1100×720 viewport, with the same
geometry and 100% grass coverage for every profile. It does not replace the canonical fixed-roster
renderer benchmark and does not measure simulation, units, water, complete game frames, or FPS.

The sample below was taken on 2026-11-22 with an Apple M5 Pro via ANGLE Metal. There are three
rotated rounds per technique/resolution, 16 warmups and 96 accepted asynchronous GPU timer queries
per round. No GPU disjoint or incomplete sample sets were accepted. The table reports the median
of each round's **mean** GPU time; p95 columns are the median round p95. The raw result retains all
rounds and construction times in [the evidence JSON](evidence/grass-carpet-gpu-2026-11-22.json).

| Profile | 1100×720 mean / p95 (ms) | 2200×1440 mean / p95 (ms) | Median one-time construction (ms) |
| --- | --- | --- | --- |
| 5 | 2.592 / 5.460 | 5.012 / 6.903 | 0.2 |
| 6 | 0.723 / 3.227 | 1.263 / 4.538 | 5.0 |
| 7 | 0.803 / 3.073 | 1.229 / 4.269 | 5.0 |
| 8 | 1.018 / 3.756 | 1.181 / 4.522 | 36.8 |

The recorded GPU has scheduling noise and long tails; these are local measurements, not a universal
speedup or minimum-hardware guarantee. The texture approaches reduce shader work in this test,
while memory, a one-time texture bake/upload, and hardware-dependent texture bandwidth are the
tradeoffs. Construction time excludes first GPU upload/compilation; those occur during warmup.
The atlas's extra construction cost can be moved into an offline art pipeline if it is adopted.
