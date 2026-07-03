Use case: stylized-concept
Asset type: PNG sprite sheet for a top-down RTS infantry unit atlas

Primary request: Create a single six-frame sprite sheet of a right-handed WWII-era German rifleman,
viewed in strict nadir / zenith orthographic plan view, for a small RTS unit that will be rotated
in-engine to face any direction.

Critical camera rule: rotation-safe top-down orthographic plan-view sprite. The camera is directly
above the soldier looking straight down, like a map token. Show only top surfaces and plan-view
silhouettes. No visible side faces, no side-view anatomy, no isometric view, no 3/4 view, no oblique
camera, no perspective tilt. The art must still look believable if the whole sprite is rotated left,
right, up, or down.

Key design change: the rifleman wears a bulky rectangular field backpack / bedroll pack across the
back in addition to a small German gas mask canister. The backpack and coat obscure the hips and
lower body from above. Do not emphasize running anatomy. Use shoulder movement, torso twist, pack
sway, and compact silhouette shifts to communicate motion. If any boot or lower-body hint is
included, it must be only a tiny flattened top-view hint partly hidden under the backpack/coat,
never full visible thighs or long side-view boots.

Layout: exactly 6 equal square cells in one horizontal row, no visible grid lines, no cell borders,
no labels, no numbers. Every frame centered on the same origin, same scale, same silhouette size,
facing local +X/east for atlas consistency, rifle pointed local +X/east.

Frame 1: standing rifleman upper body only, lower body concealed by pack/coat, rifle shouldered and
pointed forward/east.

Frames 2-5: four different running/moving stages. The rifle remains pointed forward/east and the
soldier is not firing. Show motion through alternating shoulder positions, upper-body lean,
backpack/bedroll sway, and slight arm/rifle bob. Lower body remains mostly hidden; no long legs.

Frame 6: standing upper body only, lower body concealed by pack/coat, just fired, slight backward
recoil in body and rifle, rifle still pointed forward/east, no muzzle flash or smoke.

Subject details: right-handed rifleman, steel helmet top/crown, field-grey uniform top surfaces,
compact readable upper body, rifle held forward, bulky field backpack/bedroll on the rear/back,
small German gas mask canister visible beside or below the pack as a top-view cylinder. Historical
equipment is acceptable, but use no insignia.

Style/medium: clean RTS-readable raster sprite art, flat shaded, crisp dark outline, limited detail,
not photorealistic, not painterly, not cartoonish. Designed to be readable at small in-game size.

Background: perfectly flat solid #ff00ff chroma-key background in every empty area.

Constraints: strict zenith/nadir plan view; top surfaces only; no side planes; no full legs; no
visible thighs; no long side-view boots; no face/chest/vertical torso sides; no cast shadow; no
floor plane; no lighting gradient; no text; no labels; no watermarks; no UI marks. Do not use
#ff00ff anywhere on the soldier. Avoid swastikas, eagles, unit badges, armbands, flags, crosses,
medals, or any political/military insignia. Avoid extra weapons, bayonets, muzzle flash, smoke,
gore, duplicate characters, cropped frames, inconsistent scale, inconsistent facing, and visible
guide boxes.
