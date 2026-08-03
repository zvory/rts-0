# Generation prompt

Built-in ImageGen was called once per emblem. Each call used the matching existing building PNG as
Image 1, a visual and color reference only.

## Shared prompt

```text
Use case: stylized-concept
Asset type: bare silhouette-first RTS building emblem, floated over the front center of the referenced building
Input images: Image 1 is the matching in-game building and color/style reference only; do not include or recreate the building
Primary request: create one extremely simplified, instantly recognizable emblem of <SUBJECT>
Scene/backdrop: perfectly flat solid #ff00ff chroma-key background for local background removal
Style/medium: minimal die-cut metal game emblem; one connected warm ivory/brass shape with a dark charcoal outer keyline and only a tiny restrained bevel highlight
Composition/framing: one centered emblem, front-facing orthographic view, generous padding; wide chunky forms; silhouette must remain unmistakable if filled completely solid black and reduced to 24–32 pixels
Detail budget: use only the few major forms named in the subject; omit surface detail, mechanical detail, texture, engraving, rivets, panel lines, seams, shading, ornament, and small holes
Constraints: the emblem itself is the symbol; absolutely no backing plate, medallion, shield, circle, square, rectangle, frame, border, rim, bolts, tabs, banner, wreath, enclosure, or background field; no building; no environment; no text; no letters; no numbers; no national symbols; no flags; no insignia; no watermark; no cast shadow; no contact shadow; crisp outer edge; keep every element inside the canvas; background must be one uniform #ff00ff with no gradients, texture, reflections, floor plane, or lighting variation; do not use #ff00ff anywhere in the emblem
Avoid: realism, illustration detail, embossed plaque, coin, badge backing, pictorial scene, thin fragile lines, separate decorative pieces, perspective tilt
```

## Subjects and references

| Emblem | Subject | Existing-building reference |
| --- | --- | --- |
| Vehicle Works | a generic 1940s medium tank in a very simple horizontal side silhouette: one continuous track block, one hull block, one turret block, and one short cannon barrel | `buildings-b3-corrected-preview/generated/runtime/factory-base.png` |
| Gun Works | a generic field-artillery cannon in a very simple side silhouette: two large wheels, one compact carriage, and one long elevated barrel | `buildings-b4-selected-pass-01/generated/runtime/steelworks-base.png` |
| R&D Complex | a classic atomic symbol reduced to thick simple geometry: one central nucleus, three broad intersecting orbital loops, and exactly eight large round electron dots | `buildings-b4-selected-pass-01/generated/runtime/research_complex-base.png` |
| Barracks | two generic bolt-action rifles reduced to unmistakable chunky silhouettes, crossed diagonally in a balanced X | `buildings-b7-team-paint-refined-preview/generated/runtime/barracks-base.png` |
| Training Centre | one generic light machine gun with a short box magazine and one Panzerfaust-style launcher with a large conical warhead, both reduced to chunky silhouettes and crossed diagonally in a balanced X | `buildings-b7-team-paint-refined-preview/generated/runtime/training_centre-base.png` |

## Barracks single-rifle revision

The crossed-rifle source remains as review history. The selected barracks emblem was regenerated
with built-in ImageGen using the barracks as Image 1 (scale/context reference only) and the previous
white-and-black emblem as Image 2 (paint/keyline reference only):

```text
Use case: stylized-concept
Asset type: silhouette-first RTS building emblem floated over the front center of a barracks
Input images: Image 1 is the in-game barracks and scale/context reference only; do not include or recreate the building. Image 2 is the existing emblem's flat white paint and heavy black keyline reference only; replace its crossed-rifle composition completely.
Primary request: create exactly one extremely simplified M14-style battle rifle silhouette, recognizable from its outer contour alone
Subject: one rifle in clean side profile, angled gently from lower left to upper right; unmistakable solid shoulder stock, receiver, one prominent curved box magazine, long fore-end, and one straight barrel; keep only these major forms
Scene/backdrop: perfectly flat solid #ff00ff chroma-key background for local background removal
Style/medium: minimal flat game emblem; pure white filled silhouette with a very thick pure black outer outline; zero internal detail
Composition/framing: one centered rifle only, large and balanced with generous padding; chunky proportions; silhouette must remain unmistakable when filled solid black and reduced to 24–32 pixels
Constraints: exactly one rifle; absolutely no second rifle, crossed weapons, backing plate, medallion, shield, circle, square, rectangle, frame, border, rim, bolts, banner, wreath, enclosure, background field, building, environment, text, letters, numbers, insignia, flag, watermark, cast shadow, contact shadow, bevel, gradient, texture, engraving, highlights, interior lines, trigger detail, sights, sling, ammunition, hands, or person; crisp outer edge; keep the entire rifle and black outline inside the canvas; background must be one uniform #ff00ff with no variation; do not use #ff00ff inside the emblem
Avoid: realism, mechanical detail, thin fragile parts, perspective view, two weapons, crossed composition, plaque or badge backing
```

## Training Centre MG42 revision

The selected training-center emblem is the fresh v5 generation documented in
`../candidates/training-centre-mg42-panzerfaust-v5-prompt.md`. Built-in ImageGen used an actual
MG42 side-profile photo only for weapon anatomy and the accepted atom and cannon emblems only for
the established low-detail white-fill/black-outline style. Earlier crossed-weapons candidates were
not image inputs. The final MG42 has a distinct stock, pistol grip, long three-slot barrel shroud,
and muzzle booster, with no bipod or visible magazine. It crosses a separately readable
large-warhead Panzerfaust on a flat `#ff00ff` chroma-key background.

## Selected Gun Works color iteration

The first cannon silhouette was correct but too green-gray against the Gun Works machinery. A
second built-in ImageGen call used the Gun Works building as Image 1 and the selected Vehicle Works
tank emblem as Image 2, with the same shared constraints plus:

```text
Match Image 2 exactly: minimal die-cut warm golden brass, saturated warm gold rather than green or gray, thick dark charcoal outer keyline, one tiny restrained ivory bevel highlight. Keep only two wheels, one carriage, and one long elevated barrel. Use broad solid forms, no spokes or at most one extremely simple wheel cutout, and no small mechanical detail.
```

## Team-tint paint pass

No additional generative call was used for the faction-color revision, so the approved silhouettes
remain pixel-for-pixel unchanged. A deterministic raster pass converts each selected silhouette to
flat white, expands its alpha mask into a thick black outline, and keeps the surrounding canvas
transparent. The emblem sprite uses multiplicative team tint at runtime, which maps white to the
owning player's color and preserves black.
