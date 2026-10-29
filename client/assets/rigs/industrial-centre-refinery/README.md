# Industrial Centre — Refinery Works

Selected from a four-option contact sheet, then simplified at the user's request. Generated with
the built-in ImageGen tool using existing depot, factory and steelworks sprite references.

Final prompt: isolate option 2 Refinery Works; retain three white cylindrical tanks, stepped
processing tower, chunky charcoal pipes, orange furnace, simple brown yard and white fence;
elevated frontal orthographic view, broad shading, bold outlines, low detail, solid magenta
background, no text or gold rim.

`source-magenta.png` is the retained generated source. Run
`node scripts/generate-industrial-centre-atlas.mjs` to reproduce the 384-pixel runtime frames.
The atlas contains fixed base, neutral-white team paint, silhouette shadow and an empty emblem
frame, matching the existing Resource Depot rig. Only the completed `industrialCenter` state
selects this atlas. Ordinary depots retain their existing worksite art and wrench emblem.
