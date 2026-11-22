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

The first direction to refine is **3**, using some of **5**'s broad gust variation with restrained
contrast. Keep the shorter carpet in roadsides and busy base areas; use taller clumps selectively.
Seed heads suit occasional dry patches rather than a uniform map-wide meadow.

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
