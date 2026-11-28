# Cultivator team-paint art review — DO NOT MERGE

Idle-pose review for all eight Cultivator appearances: Worker, Novice, Wind/War/Magic Adepts, and Wind/War/Magic Masters. The worker keeps its existing vector part colors; the seven raster bodies share one neutral base and one shaded paint mask per appearance. Player color is supplied at runtime, not baked per team. Both Magic hood rims are team-painted; dark robe/hood panels stay plum. Skin, hair, weapon metal, leather and gold ornaments remain fixed.

## Source and reproduction

`roster.json` defines every appearance. `source/approved-novice-adepts-marker.png` reuses the preceding approved Novice/Wind/War paint regions. `source/masters-magic-marker-v1.png` supplies three Masters and the corrected Magic Adept. Both source sheets were made with built-in imagegen, not the API fallback. Exact prompts and the Masters reference sheet are in `source/`. Original production idle cells, individual marker cells, neutral bases and grayscale masks are also retained there.

Run `node scripts/art/cultivator-team-paint-preview.mjs` with the repository tooling installed. It packs the two shared cells per raster body, retains original idle extents, and writes a labeled source review board under `output/cultivator-roster/`. The temporary frame-strip/mesh wiring in this branch uses those cells, standard unit transforms/sun tint, and seven color uniforms selected by scene column. The worker uses its existing vector renderer with the same palette. The palette is red, blue, green, yellow, purple, orange, white.

## Review scope

This is an isolated art review branch. It deliberately does not implement movement/attack masks, minimap portraits, production batching, fog-alpha parity, or a gameplay team-color setting. Spare animation cells are blank and must not be shipped as production animation sheets. Golden attack art remains unchanged in the original asset directory. Do not merge or arm auto-merge before art review and subsequent integration.

Two paused scenes show 28 units each, for 56 unit/color combinations in seven columns. First capture rows: Worker, Novice, Wind Adept, War Adept. Second capture rows: Magic Adept, Wind Master, War Master, Magic Master. All are owned by one Lab player; blue ownership bars are independent of the experimental body palette. Captures use the real game renderer, not a composited art board.

Magic paint extraction additionally rejects the less saturated plum fabric; semantic sample checks keep the outer hood and robe at zero mask while the bright magenta trim remains in the mask. Architecture and shared-atlas structure checks passed.

## Validation

The client architecture check and JavaScript syntax checks passed. All seven raster atlases contain only one base/mask pair, and each mask is grayscale. Semantic pixel checks reject plum hood/robe samples and retain magenta paint samples on both magic appearances. Two paused real-game scenes covered all eight appearances in seven colors (56 combinations); capture readiness reported no failed assets, missing subject textures or render errors. Images were visually inspected, including full weapon/tail framing and the corrected hood trim. The owned game server/browser were shut down afterward. This is not animation, full-suite, or production performance verification.
