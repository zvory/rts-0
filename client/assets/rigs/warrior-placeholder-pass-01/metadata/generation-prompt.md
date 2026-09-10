# Warrior Placeholder Generation

Generated with the built-in image generation tool using the accepted production Rifleman idle
source and Machine Gunner carry strip as camera, layout, scale, outline, and readability references.

The final pass preserves the strict nadir/zenith orthographic map-token camera and six equal
horizontal frames. It depicts a basic unarmored Chinese swordsman in a plain wrapped robe, broad
cloth sleeves, narrow sash, minimal topknot, and one dao. Frames 2-5 are compact movement poses and
frame 6 is the end pose of a horizontal sword swipe. Matte white and warm off-white cloth provide
the runtime-tintable surface.

Negative constraints: no face, chest/front torso, legs, side planes, perspective tilt, armor,
helmet, jewelry, ornate trim, magic effects, shield, projectile, tracer, slash trail, glow, shadow,
text, watermark, duplicate character, or cropped blade. The runtime strip removes the generated
magenta background to alpha and downsamples each source cell to a 160 px runtime cell.

The placeholder-2 runtime derivative detects the six complete figures before slicing, groups nearby
detached details with their figure, repairs interior transparency from the nearest opaque artwork,
removes magenta spill, and independently packs each figure into a five-pixel-or-larger transparent
gutter. This prevents adjacent-frame sampling and keeps the body opaque while retaining the original
antialiased exterior. The engine applies a 90-degree counterclockwise facing correction to idle and
movement art only; the accepted sword-swipe orientation remains unchanged.
