# Vector Field visual prototypes

Local review branch based on origin/main `05b58b6d8`; no production merge before selection.

Vector Field uses the existing Anchor carrier slot, hotkey C, range, and 10-second lifetime.
The first press/click fixes the center of a 6 by 3 tile rectangle; drag-release or a second click
sets direction. Escape/right-click cancels. Preview and deployed field share a large green arrow
and rectangular outline without particles.

Both teams' ground units receive additive displacement without changing facing. Terrain and hostile
body checks remain. Flat mode adds half Tank base speed to every body. Weighted mode multiplies that
by `sqrt(12 / max(footing resistance, 1))`: tanks remain at half speed, lighter infantry move harder.
`VECTOR_FIELD_WEIGHTED` selects builds. Weighted source commit is `3cd461b15`; flat is `ca2051fcd`.

The immediate `vectorField` command validates finite center/direction, rejects zero direction,
normalizes direction, caps/deduplicates unit ids, and uses ordinary owner/carrier/faction/readiness/
cost/range guards. Public direction is projected with the same fog gate as the object in compact
snapshot v73. Existing Anchor stable ids and unrelated legacy metadata remain for this prototype;
production selection must replace superseded radial-pull assertions and retire old semantics.
Overlapping fields currently select the first active field. Queued placement is not implemented.

Validation: two focused simulation tests pass in each mode (rotated bounds/lifetime; tank half-speed
and allied/enemy displacement), client staged-target/cancel/drawing contracts pass, protocol and
faction parity pass, client architecture and docs health pass. Real UI recording covers both placement
gestures; the paused static UI naturally reuses video frames.

Media source IDs:
- Weighted: `media_89d1ac1c645c560e90f8d9871314275c`.
- Flat: `media_661a4b997c2e18e5d6aee9caecf70060`.
- UI: `media_b85e4415c14cfb2aaed1fb9a059bf015`.

Matched 12-second gameplay runs use 36 Riflemen moving right, two tanks five tiles behind,
a northwest-directed field, and identical camera/seed. Capture servers are stopped afterward.
