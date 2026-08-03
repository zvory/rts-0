# Training centre MG42 + Panzerfaust candidate v5

This is a standalone review candidate only. It is not wired into the renderer or any building atlas, and no in-game scene was generated for it.

## Generation mode

Built-in ImageGen, new composition from scratch. The earlier crossed-weapons candidates were not used as image inputs.

## References

- `/tmp/mg42-side-reference.jpg`: subject/anatomy reference only. Source: [MG42 machine gun at the Batey ha-Osef museum](https://commons.wikimedia.org/wiki/File:MG42-machine-gun-batey-haosef-1.jpg), Bukvoed, CC BY 3.0.
- `../engineering-complex-emblem-team-tint.png`: approved abstraction, white-fill, and black-outline style only.
- `../steelworks-emblem-team-tint.png`: approved abstraction, white-fill, and black-outline style only.

## Prompt

> Create a BRAND-NEW standalone raster emblem candidate on a flat solid #ff00ff chroma-key background. Do not edit or trace an earlier crossed-weapons candidate.
>
> REFERENCE ROLES:
> - Image 1 is an anatomical reference for the MG 42 only. Preserve the distinctive overall MG 42 side-profile silhouette, but omit its bipod and all fine mechanical detail.
> - Images 2 and 3 are the approved game-building emblem style references only: extremely low-detail, bold white silhouette masses, very thick smooth black outline, readable when tiny, no shading and no backing plate.
>
> COMPOSITION:
> Exactly two complete, separate weapons crossed diagonally in a clean X.
> 1) An unmistakably MG 42 general-purpose machine gun runs from lower-left (buttstock) to upper-right (muzzle).
> 2) A Panzerfaust runs from upper-left (large warhead) to lower-right (plain rear tube).
> All four endpoints must remain visible and separate. The weapons overlap only around the central 20% of the emblem. Keep a clear thick black separating seam at the crossing so they never fuse into one hybrid object.
>
> MG 42 SILHOUETTE — MUST BE DISTINCTIVE:
> - angular shoulder stock with the characteristic narrow neck/wrist joining the receiver
> - long, low rectangular receiver body
> - one clearly protruding pistol grip, angled downward and slightly rearward
> - extremely long straight perforated barrel shroud extending toward upper-right, roughly as long as or longer than the receiver
> - distinct widened muzzle booster at the very tip
> - show only 3 oversized simple ventilation cutouts along the barrel shroud
> - no bipod
> - NO drum magazine, NO box magazine, NO curved magazine, and NO visible magazine of any kind; it is a belt-fed MG 42 with nothing hanging from the receiver besides the pistol grip
> - it must not resemble an assault rifle, M14, AK, or generic rifle
>
> PANZERFAUST SILHOUETTE:
> - unmistakably huge bulbous pear/conical warhead at the upper-left endpoint
> - narrow straight launch tube continuing through the crossing to a visibly separate lower-right rear end
> - add one simple blocky sight/trigger bump near the middle so it is not merely a featureless stick
> - the Panzerfaust warhead must never touch or sit on the MG 42 muzzle
>
> STYLE:
> White painted interior shapes intended for later team tinting, surrounded by a very thick black outline. Almost pictogram-level abstraction. Use broad shapes, smooth contours, and minimal interior marks. No textures, screws, gradients, shadows, lighting, labels, words, circle, shield, plaque, metal backing, frame, badge plate, scenery, building, or extra objects. Center the emblem with generous margin. Square canvas. Flat #ff00ff background only.

## Outputs

- `training-centre-mg42-panzerfaust-v5-magenta.png`: untouched ImageGen output.
- `training-centre-mg42-panzerfaust-v5-alpha.png`: chroma-keyed transparent source.
- `training-centre-mg42-panzerfaust-v5-team-tint.png`: normalized 256×256 white-and-black game-style candidate.
