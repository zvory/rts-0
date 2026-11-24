# Cultivator golden attacks

Generated with built-in imagegen from the live Novice, Wind/War/Magic Adept, and Master attack poses. `source/prompts.json` contains the exact prompts; `*-reference.png` files are the alignment references and `*-effects.png` files are the generated effect-only artwork on black.

Run `node scripts/art/cultivator-golden-pipeline.mjs` to rebuild, or add `--check` to verify. `source/specs.json` identifies each original character atlas, cell dimensions, and reference-frame indices. The packer converts the black effect matte to straight alpha and area-samples into four attack-effect cells in the atlas's second row. The original character row is copied byte for byte; no generated character art replaces it.

The runtime keeps the body's normal team tint in every pose and draws the gold cells in a separate, untinted sprite. Four effect frames follow the existing shot envelope, facing, and lunge. Novice retains its existing body pose with four additional effect frames; Adepts and Masters use their existing four attack body poses. Timing, damage, range, and gameplay rules are unchanged. Worker has no combat attack and receives no trail. Portraits and minimap body frames crop the first row using the full two-row sheet height.
