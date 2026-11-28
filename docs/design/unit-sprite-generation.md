# Generated unit sprites

Workflow for new or regenerated raster unit art. Read the
[raster handoff](raster-unit-art-handoff.md) for production ownership and rendering contracts.
This workflow incorporates the west-facing Rifleman experiments; it does not mean the other
headings, production windup timing, or all presentation states have been completed.

## Establish the reference and pose plan

- Preserve one approved character reference: proportions, moderate detail, equipment, lighting,
  elevated oblique camera, and orthographic projection. Keep camera and character scale consistent
  across poses and headings. Review at actual RTS size as well as enlarged.
- Specify the loadout and anatomical handedness. A rifle stock meets the intended shoulder;
  support and operating hands keep their roles. Mirroring a sprite can incorrectly reverse these.
- Declare which states are in scope, including transitions. Track omitted death, portrait,
  minimap, deployment, or ability art explicitly rather than implying that idle/run/fire is a
  complete presentation set.
- Prototype one heading through the entire in-game sequence before multiplying it into the full
  heading set. Respect requested contact-sheet review stages and prototype-only scope.

## Author distinct states and transitions

| State | Rifleman pose and motion |
| --- | --- |
| Relaxed idle | Natural planted stance, head upright, rifle lowered. |
| Running | Both hands hold a low diagonal carry across the torso; alternating strides and modest body bob. |
| Stop and shoulder | Settle the feet, establish the staggered stance, lift the stock to the shoulder, lower the cheek to the stock. |
| Aimed combat hold | Eye aligned with sights, stock touching shoulder, front foot forward and rear foot behind, knees flexed, slight forward lean. |
| Shot and recoil | Small rifle/shoulder compression; boots stay planted and the stock stays seated. |
| Bolt cycle and re-aim | Operating hand moves to the receiver and returns; support hand stabilizes the rifle; return to the aimed hold. |
| Leave combat or resume movement | Deliberate transition to lowered idle or running carry. |

Remain raised between repeated shots. Do not replay low-ready → raise → lower on every cooldown.
Keep the planted foot locations consistent through aim, recoil and bolt motion. Running feet move
as part of the gait; do not register every run frame on whichever boot happens to be lowest.

Frame count should buy visible articulation. Start planning around six to eight distinct running
poses and six to eight weapon-handling key poses, with reusable idle/aim holds and any necessary
transition poses. The approved west sheet is a reference, not a requirement to duplicate its exact
index layout. Add intermediate recoil/bolt poses where playback needs them; near-identical images
do not solve a frozen animation. Record frame roles, durations, holds, loop seams and precedence
in metadata, and verify that the renderer actually selects every authored phase.

Separate animation timing from simulation timing. The west demo delays shot presentation by
350 ms to show a shoulder windup; that is a prototype convention, not a production attack rule.
Before shipping, define how acquisition/readying, actual shot release, muzzle effects, recoil and
damage line up. Additional headings must not silently bake in a gameplay timing change.

## Declare the heading contract

- Record the heading count, angle convention, first heading, clockwise/counterclockwise order,
  and exact source-cell mapping before prompting or packing.
- Sixteen headings means 22.5-degree steps. The experimental Rifleman convention starts at screen
  south (`firstFacing = pi/2`, positive Y down); west is index 4, north index 8 and east index 12.
  Other rigs may have different conventions; read their metadata.
- Keep the camera fixed while rotating the character. Preserve handedness and the depth ordering
  of the near/far arm, boot, weapon and equipment as the view changes.
- South is a ground-plane heading, not an eye-level portrait. Require the same elevated view:
  visible helmet crown/shoulder tops, shortened face under the brim, and foreshortened torso and
  equipment. Reject a camera change even if the compass direction is correct.
- Verify all headings, opposite views, adjacent-heading transitions and the final-to-first wrap.
  Check both body-facing locomotion and weapon-facing combat. A sixteen-heading sheet routed to
  eight reachable headings is a failed integration even if the contact sheet looks correct.

## Generate team materials deliberately

Use shaded magenta for designated team paint, cloth uniforms and helmets on a genuinely
transparent background. Preserve natural skin, hair, leather, wood, dark steel, rubber and boots.
Specify exactly which materials are team-colored; do not infer them later from white brightness
or generate separate blue/orange versions. Fixed materials must not receive magenta reflections.

In this workflow magenta marks materials, not the background. Older background-keyed assets
belong to their original pipelines; do not combine a magenta background key with magenta team
materials. Keep shadows, native muzzle flashes, tracers and other independently rendered effects
out of the body sheets when the runtime owns them.

Use the built-in image generator for art, label reference versus edit-target roles, preserve raw
outputs and exact prompts/settings/reference paths, and version outputs without overwriting the
approved source. Do not invent a model identifier when the tool does not expose it. Keep assets
consumed by the project in the project, rather than referencing only a generator cache.

## Register and pack without breaking the matte

The authoritative world position is the projected ground point at the feet. Place the standing
stance's foot-contact center at the sprite anchor, with the body extending upward from it.
Movement, orders, collision and selection refer to this ground point, not the torso center.
For running, carry that ground reference through the gait using body registration; raised boots
must not redefine the unit position. A renderer-owned contact shadow and selection ring should
make this correspondence visible during review.

Health bars belong above the visible sprite. Derive their clearance from the transformed visible
bounds (or a conservative envelope across the active pose family), including scale, anchor and
sprite offsets. Do not reuse an overhead radius or a hardcoded torso-height offset. Verify the
bar's bottom clears the sprite in idle, running and weapon-handling poses for every heading,
including when the bar becomes visible through selection or damage.

1. Keep each generated image's RGB and alpha together. Remove disconnected artifacts while
   preserving its connected soft-alpha boundary; a threshold used to locate the body must not
   become a hard final cutout.
2. Align using uniform scale and translation, stable body landmarks and ground contact. Keep a
   shared scale across a pose family. Preserve authored head/shoulder motion rather than centering
   the helmet independently in every frame. Document anchors, world scale and registration.
3. A bounding-box fit is not contour registration. Do not force an old alpha outline onto new RGB,
   stretch width and height independently, or fill uncovered pixels with old white artwork. That
   combination reintroduced pale fixed-color pixels in the Rifleman prototype. Exact silhouette
   preservation requires a separately validated contour/material transfer.
4. Extract the Cultivator magenta chroma/purity mask at source resolution. Split fixed and paint
   contributions before downsampling; classifying a filtered mix of magenta and neighboring skin
   or steel can create an incorrect fixed-material seam.
5. Filter coverage and color together in premultiplied form with area sampling. Convert the
   filtered contributions to the runtime's fixed/paint source-over format. Check the recomposed
   alpha against independently filtered source coverage, allowing only PNG quantization error.
6. Decontaminate matte colors conservatively from nearby reliable material samples. Preserve
   legitimate skin and metal highlights. Do not remove halos by globally eroding the silhouette
   or blurring the sprite. Check the texture-filtering/premultiplication path too.

Keep cell margins and sampling gutters sufficient for the configured renderer. Every browser
texture must fit the existing 2048-pixel limit in both dimensions. Use explicit atlas geometry,
frame metadata and versioned image URLs; do not infer frame layout from a contact-sheet appearance.

## Automated checks and visual review

Every new packer should have deterministic rebuild verification and focused checks for:

- Correct source grid, pose count, heading count and reachable runtime frame mappings.
- Nonempty body and paint coverage in every required cell; no clipped opaque/soft-alpha edges,
  stray components, unexpected border pixels, duplicate gait poses or unexplained scale/anchor jumps.
- Bright low-saturation fixed-color pixels near the silhouette, including helmet and uniform
  edges. Flag candidates for review: skin and metal highlights are not automatically defects.
  Calibrate thresholds per asset and retain before/after evidence. The west cleanup reduced one
  such detector from 113 candidates to 1; those numbers are not universal acceptance limits.
- Stable alpha across team palettes, unchanged fixed materials, correct neutral reconstruction,
  and agreement between composed alpha and independently filtered source coverage.
- Playback holds, transitions, loop seams, planted combat feet, muzzle origin and flash timing.

Inspect light/dark/grass backgrounds at native size and magnified, using multiple team colors.
Use the processed game-size sprites as the style/detail review target. Clearly distinguish large
raw generation sheets from these final-scale previews; raw source detail is not a different
approved rendering style.
Review contact sheets across every heading and animation phase. Use Interact for an actual game
capture: lowered idle → running → stop/shoulder → several shots → return to movement/idle when
that transition is in scope. A rotating sprite over a game screenshot does not validate gameplay
integration. Prefer fixed-step capture when real-time capture reports deficient frame coverage.
Test animation selection with rendering faster than the 30Hz server snapshots
(60/144/240Hz), including repeated positions, stops and heading changes. A 30fps
fixed capture alone can hide idle/run flicker. Reuse the renderer's snapshot-aware
movement signal rather than classifying each zero-delta rendered frame as idle.

Verify renderer-owned contact shadows, selection/hit bounds, HP bars, muzzle/tracer origins and
asset readiness at the new scale and camera angle. The unit must remain grounded and selectable.
Retain raw art, manifests, prompts, processing scripts, checks and representative in-game evidence.
Do not scale generation to the remaining headings until the reference pose/state plan, mask
pipeline and one-heading in-game sequence have passed their review stage.
