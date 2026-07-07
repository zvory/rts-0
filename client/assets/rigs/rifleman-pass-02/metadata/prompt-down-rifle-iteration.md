Use case: stylized-concept
Asset type: PNG sprite sheet for a top-down RTS infantry unit atlas

Input images: Image 1 is the accepted long-rifle idle pose rotated so the rifleman points down/south; use it as the rifle length, soldier identity, pose language, and style reference. Image 2 is the old six-frame rifleman source sheet; use it only for frame count, animation sequence, top-down token style, and consistent same-origin sprite layout.

Primary request: Regenerate the complete Rifleman sprite sheet with the rifleman pointing straight down/south in every frame. The rifle must use the long realistic service-rifle silhouette from Image 1: long wooden stock and fore-end, slim dark metal barrel, front sight, small muzzle. Do not shorten it. The whole reason for this pass is to use vertical cell space so the rifle can remain visibly long and real.

Layout: exactly six equal portrait-oriented cells in one horizontal row, all facing down/south, all centered on the same origin and same scale. No visible grid, no dividers, no labels. Frame 1 standing/idle ready. Frames 2-5 four running/moving stages with subtle shoulder/torso/pack/rifle bob but legs mostly hidden. Frame 6 just after firing with slight rearward recoil in body and rifle. Keep consistent body size and anchor across frames.

Camera: strict nadir/zenith orthographic plan view directly overhead, like a map token. No side view, no perspective tilt.

Subject: one WWII-era rifleman in unmarked field-grey uniform, steel helmet seen from above, broad backpack roll across the back/upper side, gas mask canister near the pack, muted brown equipment, hands supporting the rifle. The body should stay compact, but the rifle should extend down from the shoulder line into the available vertical space.

Rifle requirements: in every frame the rifle points straight down/south, is long enough to read as a real bolt-action/service rifle at RTS scale, and remains fully inside its cell. It should not collapse into a short carbine or toy gun. Keep hand contact believable.

Style/medium: clean hand-painted raster RTS sprite art matching Image 1, readable at small scale, flat shaded, crisp dark outline, not photoreal, not pixel art.

Background: perfectly flat solid #ff00ff chroma-key background in all empty areas. The background must be one uniform color with no shadow, no texture, no gradient, no vignette, no floor, and no lighting variation.

Constraints: preserve the compact top-down rifleman silhouette, helmet/backpack/canister/uniform design, muted field-grey palette, brown wood rifle, dark metal barrel, and black outline. Do not add insignia, text, UI marks, muzzle flash, smoke, bullets, shadows, extra characters, extra weapons, bayonet, scope, guide marks, or crop any muzzle.
