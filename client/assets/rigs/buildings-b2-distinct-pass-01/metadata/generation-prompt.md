# B2 distinct-building generation prompt

Generated with the built-in ImageGen workflow. Each request used the corresponding approved B1
building as the viewpoint/style reference plus the production Rifleman/Machine Gunner, Tank, and
Anti-Tank Gun raster assets as material, outline, and team-paint references.

## Shared constraints

- Production raster sprite for a low-detail World War II RTS.
- Original Game Boy-era grid-locked overworld camera, without Pokemon styling: south/front facade
  faces straight down, roof is visible, zero horizontal yaw, and all footprint edges are horizontal
  or vertical.
- Muted red brick, charcoal roofing/machinery, warm stone, and strong dark outlines.
- At least 25% large continuous neutral cream/white authored surfaces for runtime team tint.
- Perfectly flat uniform `#ff00ff` chroma background; no exterior shadow, floor, scenery, text,
  insignia, people, vehicles, watermark, or extra buildings.
- Strong functional silhouette readable at small RTS scale.

## Functional silhouette briefs

| Kind | Footprint | Dominant brief |
| --- | --- | --- |
| City Centre | 3x3 | Symmetric civic hall, broad stair, arched entrance, dominant command tower/cupola |
| Barracks | 3x2 | Long low quarters block, repeated narrow windows, centered porch, dormitory wings |
| Vehicle Works | 3x3 | Twin garage bays with two separate worn concrete lanes reaching the footprint edge |
| Training Centre | 3x2 | U-shaped low building around an open parade ground with three shooting targets |
| Research Complex | 3x3 | Compact lab dominated by a large rooftop radar dish, radio mast, and skylights |
| Steelworks | 3x3 | Oversized exposed H-frame press, ram, rollers, and flywheel protruding through the front |
| Pump Jack | 1x1 | Freestanding beam pump and concrete pad only; no enclosing building or shed |

Supply Depot and Tank Trap were intentionally excluded.
