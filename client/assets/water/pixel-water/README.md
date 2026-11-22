# Pond water

Direct port of Jess Hammer's top-down pixel-art water shader:
https://github.com/jess-hammer/2d-pixel-water-shader-godot

Pinned upstream commit: `91f3f4a7ddc38a75d279db461af58a8c829e3e66`.
MIT license is included in `LICENSE-jess-hammer.txt`.

- `upstream.gdshader`: unchanged generated `code` from `Assets/Water/WaterShader.tres`.
- `caustics.png`, `highlights.png`: unchanged upstream `CausticTexture.png` and
  `CausticTextureHighlights.png`.
- `water.frag`: upstream fragment calculations, ported from Godot to GLSL ES 3/Pixi.
  Godot globals become explicit uniforms/varyings, uniforms become the source material's
  constants, and GLSL `fma` becomes multiply/add; reserved double-underscore locals are renamed.
  A two-world-pixel grid and four stepped shore-distance palette bands supply this game’s input
  mask. The caustic texture scale is reduced to 0.0009 for this map’s world units. Foam and
  highlight colors use the selected darker green-brown pond palette, with a subtle sediment
  tint at the banks; the source layer blending, thresholds, fade, and quantization remain.
- `movement.png`, `fade.png`, `specular-a.png`, `specular-b.png`, `foam.png`: baked FastNoiseLite
  OpenSimplex2S/FBm inputs using the frequencies, seeds and fractal settings in upstream
  `WaterMaterial.tres`. Seamless wrapping is baked by blending offset samples; this is an
  approximation of Godot's seamless skirt, not a byte-identical Godot noise export.

Noise baking uses the MIT-licensed official JavaScript FastNoiseLite, pinned to
`785f37a9ad76e283586a379675085f2063ae03f7`; license included in `LICENSE-FastNoiseLite.txt`.
Download `JavaScript/dist/FastNoiseLite.js` from that revision as an `.mjs` file, then from
repository root run `node scripts/art/pixel-water-noise.mjs /path/to/FastNoiseLite.mjs`.
No source images are AI-generated.
