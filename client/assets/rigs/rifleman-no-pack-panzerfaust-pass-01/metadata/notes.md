# Rifleman No-Pack and Panzerfaust-on-Back Generation Pass

This is an asset-only concept pass. Neither sprite is referenced by runtime code.

The normal Rifleman removes the bedroll, backpack, and gas-mask canister while retaining the
rotation-safe, no-leg top-down coat silhouette. The upgraded variant uses the same soldier and adds
an intentionally oversized Panzerfaust across the upper back with two visible retaining straps.

The two generated sources were rotated and normalized together for runtime-scale review. The
normal no-pack preview has the exact same 133x59 opaque bounds at `(19, 26)` as the production idle
frame on the existing 160x112 canvas. The Panzerfaust preview uses the same transform, so the
soldier's anchor is unchanged even though the launcher expands the total silhouette.

The comparison image is ordered: current production idle, generated no-pack Rifleman, generated
Panzerfaust-on-back Rifleman. The production strip's baked `170,118,100` ImageMagick modulation was
applied to the generated runtime previews for a useful like-for-like color comparison.
