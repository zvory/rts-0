# Artillery Follow-Up Plan

## Requested fixes

- Add a dramatic artillery recoil animation when a shell fires.
  - The gun should visibly kick back farther than tanks, with the barrel/carriage snapping rearward and recovering over a longer heavy-weapon timing.
  - The shot should kick up a large dust cloud around the gun position at fire time.
  - Prefer driving this from the owner-visible artillery fire/target event so the specific firing gun recoils reliably.
- Require manual artillery setup with `Z`, matching AT gun setup flow.
  - Artillery should not automatically set up from a Point Fire order.
  - Point Fire should require an already deployed artillery piece in arc and in range.
  - Movement or teardown should continue to reset artillery accuracy.
- [x] Increase artillery shell travel time from 4 seconds to 5 seconds.
  - Update server timing, client mirrored timing, target-marker lifetime, docs, and tests together.
- [x] Improve invalid minimum-range targeting feedback.
  - While Point Fire targeting is armed, hovering inside artillery minimum range should draw an `X` at the target cursor instead of a valid crosshair.
  - The max-range affordance should remain readable while the target is invalid.
- [x] Make the minimum-range dotted line a deeper hue.
  - Use a darker, less bright red for the inner minimum-range ring so it reads as a danger/invalid boundary without competing with the target marker.
- Verify and, if needed, fix artillery damage against buildings.
  - If the center of the shell lands anywhere inside a building footprint, the building should take full inner-radius armor-piercing damage.
  - Building splash should be based on distance to the footprint, not only distance to the building center.
  - Confirm the inner artillery damage path actually uses armor-piercing damage against armored and hard targets.

## Implementation notes

- Server setup behavior likely lives in the support-weapon setup and artillery point-fire command paths.
  - Remove artillery from any automatic packed-to-setup transition triggered by Point Fire.
  - Keep explicit `SetupAtGuns` support for both AT guns and artillery.
  - Client setup selection should include artillery as well as AT guns when `setupAtGuns` targeting is active.
- Artillery fire visuals need a firing-unit id on the client.
  - Current owner-only artillery target feedback may need to carry the firing unit id.
  - If the event shape changes, update both protocol mirrors and `docs/design/protocol.md`.
- Building damage should use existing footprint helpers where possible.
  - For buildings, compute squared distance from impact point to the nearest point on the building footprint rect.
  - Treat points inside the rect as distance zero so they receive full inner AP damage.
  - Units can keep current center-distance splash behavior unless a broader AOE model is deliberately chosen.

## Tests to add or update

- Artillery Point Fire while packed does not start setup and does not fire.
- Artillery manually set up with `Z` can Point Fire once deployed, in arc, and outside minimum range.
- Artillery Point Fire inside minimum range is rejected and does not spend steel.
- [x] Artillery shell delay is 5 seconds in server config, client config, and target-marker rendering.
- Artillery shell landing inside a building footprint deals full inner armor-piercing damage.
- Artillery shell landing outside the footprint but within splash range uses footprint-distance falloff.
- Protocol/client contract tests cover any added artillery firing-unit id.

## Patch-note bullets

- Artillery will require manual `Z` setup before firing, matching AT gun deployment expectations.
- Artillery shells will take 5 seconds to land, giving players more time to react to marked impacts.
- Artillery firing feedback will read heavier, with stronger recoil and a large dust cloud at the gun.
- Artillery minimum-range targeting will be clearer: invalid close targets show an `X`, and the minimum-range ring uses a deeper hue.
- Direct shell impacts on building footprints will reliably apply full armor-piercing inner damage.
