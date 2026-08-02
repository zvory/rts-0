# Command car concepts — exact generation prompts

All four prompts used the same three source-reference images:

1. `client/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-preview.png`
2. `client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-carry-alpha.png`
3. `client/assets/rigs/panzerfaust-pass-01/generated/panzerfaust-pass-01-base-carry-long-alpha-trimmed.png`

The generator was the Codex built-in `image_gen` tool. Its model name and generation settings were
not exposed.

## Concept A — open staff car

```text
Use case: stylized-concept
Asset type: top-down RTS unit concept PNG, single isolated vehicle
Primary request: Design a very small unarmed command car for a WWII-inspired RTS. It is the game's non-shooting spellcaster/support unit, carrying two senior field generals who direct breakthroughs and scout-plane sorties.
Input images: Image 1 is the current scout-car scale, top-down construction, and outline reference; Images 2 and 3 are the white-painted infantry house-style references.
Subject: Concept A — a tiny open-top two-axle field staff car, inspired by a compact 1930s/1940s military liaison car but fully original. One driver/general sits in front; a second charismatic senior general stands or half-stands in the short rear compartment, leaning over a folded map and pointing forward. Add one slim whip antenna, a compact radio box, leather seats, and a rolled map case. No weapons.
Scale: the entire car must read as only about 60–65% of the current scout car's length and clearly narrower; compact wheelbase; human figures make the small size obvious.
Style/medium: exact orthographic bird's-eye top-down view; hand-painted RTS sprite concept; chunky clean black ink outlines; subtle painterly off-white/ivory brush texture matching the references; simplified bold readable shapes; slightly exaggerated proportions; dark charcoal tires and chassis; tan leather and tiny brass accents.
Composition/framing: one vehicle only, facing right like the scout-car reference, centered with generous padding; complete silhouette visible; no perspective tilt.
Scene/backdrop: perfectly flat medium neutral gray background, no environment.
Lighting/mood: diffuse neutral sprite lighting, restrained highlights, crisp silhouette.
Color palette: vehicle body and both officers' uniforms painted warm white/ivory; charcoal mechanical parts; muted tan leather; tiny brass details. White paint is dominant.
Constraints: unmistakably unarmed command/support vehicle; two fancy senior officers clearly visible from above; no gun, turret, weapon mount, armor plating, text, national insignia, swastikas, logos, watermark, cast shadow, terrain, dust, smoke, or extra objects. Do not make it as large or bulky as the scout car. Keep the same house-style line weight and painterly finish as the provided unit references.
```

## Concept B — radio roadster

```text
Use case: stylized-concept
Asset type: top-down RTS unit concept PNG, single isolated vehicle
Primary request: Explore a second, clearly different very small unarmed command car for a WWII-inspired RTS. This is the game's non-shooting spellcaster/support unit, carrying two senior generals who order breakthroughs and scout-plane sorties.
Input images: Image 1 is the current scout-car scale, top-down construction, and outline reference; Images 2 and 3 establish the hand-painted white-unit house style.
Subject: Concept B — an extremely short, narrow open command roadster with a distinctive rounded nose and tightly clipped tail, almost a field runabout. Two senior officers sit staggered rather than side by side: the driver forward, the commanding general in a tiny raised rear seat turned partly sideways over a compact map board. Include a looped field-radio handset cord, one folded canvas roof strapped behind the seat, a map case, and two slender crossed whip antennas at the rear. No weapons.
Scale: whole vehicle about 55–60% of the scout car's length and distinctly narrower; toy-like compact wheelbase; visible officers establish how tiny it is.
Style/medium: exact orthographic bird's-eye top-down view; hand-painted RTS sprite concept; bold clean black ink outlines; off-white/ivory painted surfaces with subtle brush texture like the reference units; chunky readable geometry; slight proportions exaggeration; dark charcoal running gear; tan leather; small brass hardware.
Composition/framing: one vehicle only, facing right like the scout-car reference, centered with generous padding; full silhouette visible; no perspective tilt.
Scene/backdrop: perfectly flat muted blue-gray background, no environment.
Lighting/mood: diffuse neutral sprite lighting, minimal restrained highlights, crisp silhouette.
Color palette: dominant warm white/ivory paint on vehicle and officer uniforms; charcoal tires/chassis; muted tan leather; tiny brass details.
Constraints: support/command read, two senior officers, map and radio cues, absolutely no gun, turret, weapon mount, armor plating, text, national insignia, swastikas, logos, watermark, cast shadow, terrain, dust, smoke, or extra props. Do not create a jeep clone and do not make it as large or bulky as the scout car. Preserve the reference house-style line weight and painterly finish.
```

## Concept C — officer sidecar

```text
Use case: stylized-concept
Asset type: top-down RTS unit concept PNG, single isolated unit
Primary request: Design an unarmed motorcycle-and-sidecar command unit for a WWII-inspired RTS. It is the game's tiny non-shooting spellcaster/support unit, carrying two senior officers who order breakthroughs and call scout planes.
Input images: Image 1 is the current scout-car top-down scale and construction reference; Images 2 and 3 establish the white-painted hand-rendered unit house style.
Subject: Concept C — a compact 1940s German-style military motorcycle with sidecar, historically inspired but fully original and without insignia. A uniformed officer drives the motorcycle. A more senior, Rommel-like field general sits upright in the sidecar with a folded map across his knees, one hand pointing decisively forward. Add a small radio box behind the sidecar seat, one short whip antenna, compact saddlebags, a rolled map tube, and a spare tire. No weapons.
Scale: extremely small footprint, roughly 45–50% of the scout car's length and dramatically narrower; motorcycle wheels and the two human figures make the scale unmistakable.
Style/medium: exact orthographic bird's-eye top-down view; hand-painted RTS sprite concept; bold clean black ink outlines; warm off-white/ivory painted vehicle panels and uniforms with subtle painterly brush texture; chunky readable shapes; dark charcoal tires and engine; tan leather and small brass fittings.
Composition/framing: exactly one motorcycle-plus-attached-sidecar unit, facing right like the scout-car reference, centered with generous padding; complete silhouette visible; no perspective tilt.
Scene/backdrop: perfectly flat muted olive-gray background, no environment.
Lighting/mood: diffuse neutral sprite lighting, restrained highlights, crisp silhouette.
Color palette: dominant warm white/ivory paint and uniforms; charcoal rubber/mechanics; muted tan leather; tiny brass accents.
Constraints: two fancy senior officers clearly readable from overhead; general must be seated in sidecar with map/pointing gesture; no gun, machine gun, rifle, holstered visible weapon, turret, weapon mount, armor, text, national insignia, swastikas, logos, watermark, cast shadow, terrain, dust, smoke, or extra objects. Do not make it bulky or car-sized. Match the reference house-style line weight, painterly finish, and top-down readability.
```

## Concept D — signals sidecar

```text
Use case: stylized-concept
Asset type: top-down RTS unit concept PNG, single isolated unit
Primary request: Explore a second, visually distinct motorcycle-and-sidecar command unit for a WWII-inspired RTS. It is a tiny unarmed spellcaster/support carrier for two high-ranking officers who direct breakthroughs and scout-plane calls.
Input images: Image 1 is the existing scout-car scale, orthographic layout, and line-work reference; Images 2 and 3 establish the painterly white-unit house style.
Subject: Concept D — a lean, long-fork 1940s-style command motorcycle with a compact teardrop sidecar, historically German-inspired but original and insignia-free. One officer drives. A senior general sits deep in the sidecar, seen clearly from above, holding binoculars in one hand and a map board in the other. Turn the sidecar's clipped rear deck into a tiny mobile signals station: visible radio dials, coiled handset cord, one tall whip antenna, one short pennant-free antenna, and a strapped document satchel. No weapons. Keep equipment restrained so the silhouette remains tiny and readable.
Scale: very small, around 45% of the current scout car's length and less than half its width; visibly the smallest vehicle class in the roster.
Style/medium: exact orthographic bird's-eye top-down view; hand-painted RTS sprite concept; strong clean black ink contours; warm off-white/ivory paint and uniforms with subtle brush texture; simplified bold shapes; dark charcoal tires and engine; muted tan leather; tiny brass fittings.
Composition/framing: exactly one attached motorcycle-and-sidecar unit, oriented horizontally with its front wheel pointing right, centered with generous padding; full silhouette; zero perspective tilt.
Scene/backdrop: perfectly flat muted brown-gray background, no environment.
Lighting/mood: diffuse neutral sprite lighting, minimal highlights, crisp high-contrast silhouette.
Color palette: dominant warm white/ivory paint; charcoal mechanics; muted tan leather; tiny brass accents.
Constraints: two fancy officers clearly readable, senior officer seated in sidecar, binocular/map/radio command cues; absolutely no gun, machine gun, rifle, holster, weapon mount, armor, text, national insignia, swastikas, logos, watermark, cast shadow, terrain, dust, smoke, or extra objects. Do not make it bulky, armored, or car-sized. Match the reference house-style line weight and painterly finish.
```
