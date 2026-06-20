# rts-0 TODO

## Visual / Renderer

- [ ] Local rendering preview workflow — set up a way to preview SVG rigs and building draw changes without running a full match
- [ ] Palette pass — audit and update base colors in config.js and baked SVG strings for a more cohesive look
- [ ] Building draw enrichment — replace thin per-kind rect stubs in _drawBuilding with richer geometry per building (CC, Zamok, Depot, Barracks, Training Centre, Research, Factory, Steelworks)
- [ ] Unit SVG enrichment — improve unit SVGs one at a time starting with tank and rifleman; more shapes, curves, layered opacity shading
- [ ] Automated image→SVG rig pipeline research — investigate vtracer/potrace for vectorising flat-palette Nolan outputs into rig-compatible SVG paths; buildings are fully automatable (static, single part), units need per-part generation or segmentation before vectorisation
