# Training Centre MG42/Panzerfaust candidate v3

Preview-only candidate generated with built-in ImageGen. It is not referenced by the renderer or
any atlas.

## Reference images

- `../barracks-emblem-m14-team-tint.png`: approved low-detail rifle treatment
- `../engineering-complex-emblem-team-tint.png`: approved line weight and abstraction
- `../factory-emblem-team-tint.png`: approved silhouette treatment
- `../steelworks-emblem-team-tint.png`: approved silhouette treatment

## Prompt

```text
Use case: stylized-concept
Asset type: candidate silhouette-first RTS training-center emblem; generate the emblem only
Input images: Images 1–4 are approved existing building emblems and style references only. Match their extremely low detail, broad white silhouettes, heavy black outer keyline, flat treatment, and generous padding. Do not copy their subjects or combine them into the new emblem.
Primary request: create exactly two complete, separate weapons crossed in a clean X: one MG42-style machine gun without a bipod and one Panzerfaust-style launcher
Locked endpoint layout: upper-left quadrant = the Panzerfaust's very large pear-shaped/conical warhead at its free front end; upper-right quadrant = the MG42's narrow barrel jacket and small muzzle booster at its free muzzle end; lower-left quadrant = the MG42's short buttstock at its free rear end; lower-right quadrant = the Panzerfaust's plain narrow rear launch tube at its free rear end. These are four different, separated free ends. Do not swap, merge, connect, or duplicate them.
MG42 silhouette: one continuous complete weapon from lower-left buttstock through a short boxy receiver and small compact rectangular belt box at the center-left, then a very long narrow ventilated barrel jacket ending in a small cylindrical muzzle booster at upper-right. No bipod. The MG42 muzzle must be plain and narrow—absolutely no rocket, grenade, warhead, cone, bulb, or RPG shape attached to it.
Panzerfaust silhouette: one continuous complete launcher from the oversized warhead at upper-left through a narrow straight tube crossing the center to the plain capped rear tube at lower-right. The huge warhead belongs only at the upper-left free end; the rear tube at lower-right must be a narrow stick-like tube, not another warhead.
Crossing rule: the two complete weapons overlap only in a small central 20% area. The MG42 passes visually in front of the Panzerfaust at the crossing. Preserve a thick black separation seam around the MG42 through the crossing so players can trace each weapon from one free end to the other. Neither weapon may touch the other outside the center.
Scene/backdrop: perfectly flat solid #ff00ff chroma-key background for local background removal
Style/medium: extremely simplified flat game emblem matching the references; pure white filled silhouettes with one very thick pure black outer outline; no shading, texture, bevel, or small detail; at most three oversized black ventilation slots on the MG42 barrel jacket
Composition/framing: centered balanced X, large chunky shapes, equal visual weight, generous padding, legible at 24–32 pixels
Constraints: exactly one MG42 and exactly one Panzerfaust; exactly four separated free ends in the locked quadrants; no bipod; no assault-rifle magazine; no curved rifle magazine; no RPG-7; no missile or warhead on the MG42 muzzle; no plain-stick Panzerfaust front; no third object; no fused hybrid weapon; no backing plate, medallion, shield, circle, frame, bolts, banner, wreath, building, environment, text, letters, numbers, insignia, flag, watermark, cast shadow, contact shadow, gradients, highlights, hands, or person; keep all black outlines inside the canvas; background must be one uniform #ff00ff with no variation and must not appear inside the emblem
Avoid: weapon tips touching, RPG mounted on machine gun, Panzerfaust reduced to a featureless stick, generic crossed rifles, realism, perspective view, mechanical clutter, thin fragile lines
```

## Files

- `training-centre-mg42-panzerfaust-v3-magenta.png`: raw built-in ImageGen output
- `training-centre-mg42-panzerfaust-v3-alpha.png`: chroma-key extraction
- `training-centre-mg42-panzerfaust-v3-team-tint.png`: normalized 256×256 review candidate
