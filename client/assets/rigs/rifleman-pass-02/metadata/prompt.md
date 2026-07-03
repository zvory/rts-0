Use case: stylized-concept
Asset type: PNG sprite sheet for a top-down RTS infantry unit atlas

Primary request: Create one six-frame sprite sheet of a WWII-era German rifleman for a small RTS game. The unit will be tiny on screen and rotated in-engine, so prioritize a chunky, readable top-down silhouette over anatomy detail.

Camera: strict nadir / zenith orthographic plan view, directly overhead, like a map token. Compact top-view shapes, centered consistently, believable when rotated to any direction.

Subject: rifleman in unmarked field-grey uniform, steel helmet seen from above, broad backpack roll across the back, and a simple gas mask canister near the pack. The pack, coat, and shoulders form the main body mass and conceal most of the hips and legs.

Rifle pose: braced firing stance from directly above. The rifle sits along the lower/south shoulder edge, outside the torso centerline. The stock touches the outer lower/south shoulder corner; the barrel projects straight east. The torso sits behind and slightly above the rifle, making a compact firing wedge. The sleeves are broad simple shapes supporting the stock without fine anatomy.

Detail level: small RTS token art. Use large readable blocks: helmet disk, shoulder mass, pack block, rifle line, sleeve blocks. Keep fine gear and digit detail minimal so the pose reads at 32px.

Motion language: show running/moving by subtle changes to shoulder angle, torso lean, pack sway, and rifle bob. Lower body stays hidden by equipment except tiny boot hints where helpful.

Layout: six equal cells in one horizontal row, all centered at the same origin and same scale, all facing east. Frame 1 standing ready. Frames 2-5 four running/moving stages, not firing. Frame 6 standing just after firing with slight rearward recoil in body and rifle.

Style: clean RTS-readable raster sprite art, crisp dark outline, flat shaded field-grey and muted brown equipment, limited detail.

Background: perfectly flat solid #ff00ff chroma-key background in all empty areas. Use unmarked equipment; no text, labels, UI marks, shadows, smoke, muzzle flash, or insignia.
