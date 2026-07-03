Use case: stylized-concept
Asset type: PNG sprite sheet for a top-down RTS infantry unit atlas

Input image role: the visible rifleman sprite sheet in this conversation is a style reference only.
Match its production style: compact overhead RTS token, chunky readable silhouette, crisp dark
outline, flat shaded muted field-grey uniform, helmet disk from above, backpack/bedroll gear,
small skin highlights, simple weapon shapes, no painterly background. Do not edit the rifleman
into the output; create a new machine gunner unit.

Primary request: Create one single 9-frame sprite sheet of a WWII-era unmarked machine gunner
infantry unit for a small top-down RTS game. The unit will be rotated in-engine, so prioritize a
compact, rotation-safe overhead silhouette over anatomy detail.

Camera: strict nadir / zenith orthographic plan view, directly overhead, like a map token. Show top
surfaces and plan-view shapes only. No visible side-view body, no isometric angle, no 3/4 view, no
perspective tilt. The sprite must remain believable if rotated to any direction in-engine.

Subject: one right-handed standing machine gunner in unmarked muted field-grey uniform, steel
helmet seen from above, compact shoulder/coat mass, broad backpack or bedroll and small gear
canister partly hiding hips and legs. The weapon is a long MG42-like machine gun with a box/drum
detail and a small folding bipod near the front. Use no insignia, no flags, no markings.

Layout: exactly 9 equal cells in one horizontal row, no visible grid lines, no borders, no labels,
no numbers. Every frame is centered on the same origin, same scale, same silhouette size, facing
local +X/east for atlas consistency. Keep generous padding so no weapon or body is cropped. Use one
soldier only in each cell.

Frames 1-6: walking/running/moving frames. The machine gunner is standing normally and carrying the
machine gun across his chest, not ready to shoot. The long gun is held close across the torso,
roughly east-west or slightly diagonal along the chest, with barrel not fully shouldered and not
aimed aggressively. Show motion through subtle shoulder angle changes, torso lean, backpack/bedroll
sway, arm/gun bob, and tiny boot hints hidden under coat/pack. No firing, no deployed bipod.

Frames 7-9: standing setup-to-fire transition frames, increasingly ready to fire but still upright,
never prone and never kneeling. Frame 7: begins bringing the machine gun forward from carried
position. Frame 8: gun points more directly east, stance braces, bipod begins unfolding. Frame 9:
deployed ready pose, standing upright as if hip-firing or braced at waist height, machine gun
pointed forward/east, small bipod deployed near the front and visibly open, but no muzzle flash or
smoke.

Style/medium: clean RTS-readable raster sprite art matching the visible rifleman reference, flat
shaded, crisp dark outline, limited detail, readable at small in-game size. Slight frame-to-frame
jiggle is desirable, but maintain consistent character identity, scale, facing, equipment, and
palette.

Background: perfectly flat solid #ff00ff chroma-key background in every empty area. No shadows, no
floor plane, no lighting gradient. Do not use #ff00ff anywhere on the soldier or weapon.

Constraints: strict top-down plan view; standing character in all frames; no lying down, no prone
pose, no kneeling pose; no side-view anatomy; no full long legs; no duplicate soldiers; no cropped
frames; no cell borders; no text, labels, watermarks, UI marks, insignia, swastikas, eagles,
crosses, flags, armbands, medals, or unit badges; no muzzle flash, smoke, gore, or extra weapons.
