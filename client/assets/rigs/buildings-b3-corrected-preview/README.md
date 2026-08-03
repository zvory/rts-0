# B3 corrected building preview

Selected ImageGen source PNGs on magenta chroma backgrounds. The corrected Vehicle Works is wired
into the runtime renderer while the other accepted buildings continue to use the B2 pass.

- `factory-magenta.png`: 3×3 vehicle works with open repair bays and no exterior
  driveway/apron.

The existing finished rifleman, tank, and anti-tank-gun PNGs were supplied as
style references. Large cream/white surfaces are reserved for team tint.

The runtime Vehicle Works is enlarged to occupy about 95% of the 3×3 frame width.
Its third atlas frame is a blurred, offset alpha silhouette used as a perspective-aware
shadow; it does not use the legacy rectangular footprint shadow.
