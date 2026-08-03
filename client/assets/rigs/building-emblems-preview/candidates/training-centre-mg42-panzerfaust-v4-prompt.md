# Training Centre MG42/Panzerfaust candidate v4

Preview-only candidate generated with a targeted built-in ImageGen edit. It is not referenced by
the renderer or any atlas.

## Inputs

- `training-centre-mg42-panzerfaust-v3-magenta.png`: edit target
- `../barracks-emblem-m14-team-tint.png`: approved low-detail style reference
- `../factory-emblem-team-tint.png`: approved low-detail style reference
- `../steelworks-emblem-team-tint.png`: approved low-detail style reference

## Prompt

```text
Use case: precise-object-edit
Asset type: candidate silhouette-first RTS training-center emblem; edit the emblem only
Input images: Image 1 is the edit target. Images 2–4 are approved low-detail style references only.
Primary request: change only the MG42 receiver area in Image 1 so the machine gun has an unmistakable pistol grip and compact round drum magazine
Required MG42 edit: remove the current hanging rectangular box. Add one clearly circular drum magazine attached tightly beneath the receiver, immediately left of the central crossing; the drum should be compact but unmistakable, about twice the barrel-jacket thickness in diameter, with a clean round outer silhouette. Add one separate thick pistol grip immediately behind the drum, projecting downward and slightly toward the lower-left; it must visibly extend beyond the receiver silhouette and end in a rounded free tip. Preserve a narrow black separation notch between the circular drum and pistol grip so both shapes read independently at small size.
Invariants: keep the MG42 buttstock at lower-left, receiver length, long ventilated barrel jacket, three large ventilation slots, narrow muzzle booster at upper-right, and absence of bipod unchanged. Keep the entire Panzerfaust completely unchanged: its huge warhead remains at upper-left and its plain rear tube remains at lower-right. Keep all four free endpoints, the central-only crossing, object scale, angles, generous padding, white fill, thick black outlines, and perfectly flat #ff00ff background unchanged.
Crossing rule: the MG42 still passes in front at the center with a black separation seam; the new drum and pistol grip sit on the lower edge of the MG42 just left of the crossing and must not touch, merge with, or be mistaken for any part of the Panzerfaust.
Style: match the reference badges' extremely low detail and chunky silhouette language; use only the large circular drum and large pistol-grip shapes, with no trigger guard or small mechanical detail
Constraints: exactly one compact circular drum magazine and exactly one pistol grip on the MG42; no rectangular box magazine; no curved rifle magazine; no oversized DP/Lewis-style disc; no bipod; no warhead on MG42 muzzle; no change to Panzerfaust; no additional weapons, backing plate, text, shading, texture, bevel, environment, person, cast shadow, or watermark; preserve uniform #ff00ff chroma background
```

## Files

- `training-centre-mg42-panzerfaust-v4-magenta.png`: raw built-in ImageGen output
- `training-centre-mg42-panzerfaust-v4-alpha.png`: chroma-key extraction
- `training-centre-mg42-panzerfaust-v4-team-tint.png`: normalized 256×256 review candidate
