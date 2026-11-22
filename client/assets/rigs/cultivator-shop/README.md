# Cultivator Shop

Approved tall gourd concept, generated with built-in imagegen using the Cultivator Nexus and
Pavilion as style references. Only the upper portion of a giant gourd projects from a fresh
fractured-rock impact rim. Covered open service window, no door, and 愛 (love) on the side.
Cork, neck rope and canopy are white team-paint surfaces; ochre shell, wood and rocks stay fixed.

`shop-source.png` preserves the approved source on solid magenta. `shop-atlas.png` has two
384×384 frames: fixed base then team paint. The portrait source is sampled into the square atlas;
runtime 3×4 footprint scaling restores its original 3:4 proportions. This follows the existing
Cultivator 1.1 visual scale and disables the fallback footprint shadow.

Regenerate: `node scripts/art/shop-pipeline.mjs`; verify with `--check`. The pipeline removes
magenta spill before premultiplied supersampling and excludes lower rocks from team paint.

Prompt summary: edit the open-counter gourd concept to four units tall by three wide; remove
the side door; add correctly formed 愛 on the exposed side; paint cork and all neck rope white;
increase angular freshly fractured rocks around the half-buried base. Preserve the elevated
45-degree orthographic camera, hand-painted RTS outlines, ochre gourd, timber counter, ivory
awning and solid RGB 255,0,255 background. No people, merchandise, scenery, UI or effects.
