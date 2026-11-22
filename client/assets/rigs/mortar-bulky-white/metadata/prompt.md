# Bulky white mortar — accepted candidate 3

Generated with the built-in image-generation edit tool from the existing
mortar-png-pass-01 alpha component sheet. Approved in game at 1.2x rig scale.

## Prompt

Edit this existing top-down strategy-game sprite sheet. Candidate 3: the boldest silhouette.
Thicken the long cylinder by about 85%, with a bold simple white end rim; make the support arms,
axle and central cradle visibly stout, simplify the tiny fasteners and emphasize large clean
shapes and open triangular gaps. Keep the overall length and recognizable design.
Keep the overhead view, eastward orientation, two black rubber wheels and three-panel component
layout: complete object, matching frame without cylinder, matching cylinder alone. Repaint ALL
exterior surfaces neutral matte white, including cylinder, rim, axle and supports; ONLY rubber
wheels stay black. Tiny dark openings and fine outlines allowed, otherwise broad white with
restrained light gray shading for team-color multiplication. Match existing crisp raster game
art. Transparent background with clean edges and clear holes, no white speckles outside the
shapes, no shadows, no text or extra objects.

## Runtime preparation

The source is 2172×724 RGBA. Downsample the first 2172×724 source pixels by 2 using
alpha-weighted 2×2 averaging into the first 1086×362 pixels of a 1150×362 transparent atlas.
Preserve the existing pass-04 deployed baseplate in the remaining 64×64 top-right region.
Independent carriage, fixed-color wheels and team-tinted tube use authored atlas crops and
rotation anchors. The renderer scales the entire rig (including anchors and shadow) by 1.2;
the server owns the matching 20% increase in collision and placement dimensions.
