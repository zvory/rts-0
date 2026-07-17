# Mortar base plate imagegen prompt — pass 04

Mode: `stylized-concept`

The generated source uses a flat magenta chroma key. Postprocessing removes that key, tightly crops
the visible plate, resamples it to 128x128 pixels, and maps the result to a 16x16-world-pixel atlas
frame (half of one tile). Runtime metadata offsets the plate 20 world pixels rearward and multiplies
the neutral white paint by the owning player's team color.

```text
Use case: precise-object-edit
Asset type: transparent top-down RTS game sprite
Input image: edit target. Preserve the exact plate geometry, square silhouette, shallow stamped X reinforcement ribs, small central socket, flush fasteners, orthographic top-down angle, centered framing, surface wear pattern, PS1-era strategy-game rendering, and flat magenta chroma-key backdrop.
Primary request: Repaint only the recoil plate in neutral matte white military enamel so it can receive a runtime team-color multiply tint.
Color palette: neutral white and very light cool grey paint; dark neutral steel only inside the deepest socket and tiny chipped/worn areas. No olive, green, beige, cream, or colored paint.
Materials/textures: thin stamped steel under worn white paint; subtle grey shading may preserve the shallow relief, but the broad plate surface must read clearly as white-painted.
Composition/framing: unchanged—exactly centered, axis-aligned, symmetric, generous uniform padding, whole plate visible.
Scene/backdrop: preserve a perfectly flat solid #ff00ff chroma-key background with no variation.
Constraints: change only the paint color; preserve the exact form and perceived thinness; keep neutral values suitable for multiplicative team tint; crisp silhouette; no cast or contact shadow; no text or watermark; do not use #ff00ff in the plate.
Avoid: redesigning the ribs or socket, changing proportions, block-like thickness, green/olive paint, warm ivory, saturated color, perspective, diamond rotation, mortar, wheels, tripod, ground, scenery.
```
