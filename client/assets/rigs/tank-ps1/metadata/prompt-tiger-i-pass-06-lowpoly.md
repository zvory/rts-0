Use case: stylized-concept
Asset type: top-down RTS unit raster atlas candidate for Bewegungskrieg
Primary request: Generate a coherent Tiger I inspired top-down RTS tank raster sheet from the guided 2x2 semantic contact sheet. The output must keep the body/hull and turret/barrel as separate reusable runtime components.
Input image role: Use the simple 2x2 source contact sheet as the edit target and layout reference. If any previous detailed generated pass is visible, treat it only as a negative reference for too much detail.
Sheet layout: exactly four boxed cells in a 2x2 grid, same cell order as the source: assembled reference tank, one reusable track-link strip, hull/body assembly, turret/barrel assembly.
Runtime source rule: the hull/body cell and the turret/barrel cell are the important runtime art. They must assemble into the same tank shown in the reference cell. Do not redesign the reference tank independently from the component cells.
Style/medium: very simple low-poly 3D raster graphics, early RTS/PlayStation 1 era, anti-aliased raster shapes, broad flat facets, only a few shade values per part, readable at small RTS scale, not pixel art and not photorealistic.
Tiger I silhouette: long rectangular hull, wide straight parallel tracks, flat square turret, long centered gun barrel, heavy slab-sided armor, grounded World War II proportions.
Composition/framing: strict top-down orthographic view, no perspective tilt, no camera angle, no floor plane. Preserve the exact 2x2 grid, component isolation, centered pivots, orientation, relative scale, and empty padding from the source contact sheet.
Color/materials: dull gray-green armor with simple low-poly facets, dark rubber/track metal, dull steel barrels. Keep team-colorable armor regions clean and not hidden by camouflage.
Background: perfectly flat solid #ff00ff chroma-key background in every empty area. Do not use #ff00ff in the tank art.
Track cell: generate only one straight reusable strip of track links, not a left/right pair and not a closed loop. It should read as repeatable track art when reused for both tank sides.
Constraints: no drop shadow, cast shadow, contact shadow, ground shadow, ambient blob, text, labels, insignia, numbers, watermarks, arrows, fuel icon, warning icon, extra cells, merged cells, extra turrets, extra barrels, loose pieces, road-wheel diagrams, exposed sprocket clusters, or guide lines becoming armor seams.
Avoid: painterly detail, detailed concept art, tiny hatches and bolts everywhere, glossy lighting, dramatic gradients, toy proportions, oversized turret, side-view cues, perspective skew, camouflage that obscures the silhouette.
