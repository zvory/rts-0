// Building SVG rigs — one static SVG per building kind.
//
// Authored at world-pixel scale (tileSize=32), centered at origin so the rig
// container placed at (e.x, e.y) aligns with the building's world position.
// Footprint sizes: 3×3 = 96×96 px, 2×2 = 64×64 px, 3×2 = 96×64 px.
//
// Naming convention for parts:
//   part.base       — full-footprint dark foundation rect
//   part.roof       — primary player-tinted roof/slab
//   part.slab.*     — secondary tinted slabs (depot, factory)
//   part.tower      — tinted tower/annex volume (cc)
//   part.chimney.*  — dark accent volumes (cc, factory)
//   part.window.*   — dark inset window strips (generic buildings)
//
// All tinted shapes use data-rts-tint="team" so the player color is applied at
// runtime. Everything else is fixed hex. No animations — buildings are static.

// ---------------------------------------------------------------------------
// City Centre — 3×3 (96×96)
// ---------------------------------------------------------------------------

export const CITY_CENTRE_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="city_centre" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-31" width="60" height="50" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.tower" x="17" y="-38" width="15" height="31" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.chimney" x="25" y="-46" width="8" height="21" fill="#1a1712" fill-opacity="0.7" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="48" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Zamok — 3×3 (96×96)
// ---------------------------------------------------------------------------

export const ZAMOK_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="zamok" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-31" width="73" height="54" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.window.a" x="-27" y="-23" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <rect id="part.window.b" x="-27" y="0" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="48" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Supply Depot — 2×2 (64×64)
// ---------------------------------------------------------------------------

export const DEPOT_BUILDING_SVG = `<svg viewBox="-32 -32 64 64" data-rts-rig-kind="depot" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-32" y="-32" width="64" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.slab.a" x="-22" y="-18" width="44" height="13" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.slab.b" x="-22" y="1" width="44" height="13" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="32" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Barracks — 3×2 (96×64)
// ---------------------------------------------------------------------------

export const BARRACKS_BUILDING_SVG = `<svg viewBox="-48 -32 96 64" data-rts-rig-kind="barracks" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-32" width="96" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-21" width="73" height="36" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.window.a" x="-27" y="-15" width="54" height="8" fill="#1a1712" fill-opacity="0.42" />
  <rect id="part.window.b" x="-27" y="0" width="54" height="8" fill="#1a1712" fill-opacity="0.42" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="40" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Training Centre — 3×2 (96×64)
// ---------------------------------------------------------------------------

export const TRAINING_CENTRE_BUILDING_SVG = `<svg viewBox="-48 -32 96 64" data-rts-rig-kind="training_centre" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-32" width="96" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-21" width="73" height="36" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.window.a" x="-27" y="-15" width="54" height="8" fill="#1a1712" fill-opacity="0.42" />
  <rect id="part.window.b" x="-27" y="0" width="54" height="8" fill="#1a1712" fill-opacity="0.42" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="40" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// R&D Complex — 3×3 (96×96)
// ---------------------------------------------------------------------------

export const RESEARCH_COMPLEX_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="research_complex" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-31" width="73" height="54" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.window.a" x="-27" y="-23" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <rect id="part.window.b" x="-27" y="0" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="48" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Vehicle Works (Factory) — 3×3 (96×96)
// ---------------------------------------------------------------------------

export const FACTORY_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="factory" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.slab.a" x="-37" y="-31" width="73" height="25" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.slab.b" x="-31" y="4" width="61" height="25" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.chimney.a" x="-29" y="6" width="8" height="21" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.chimney.b" x="-10" y="6" width="8" height="21" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.chimney.c" x="10" y="6" width="8" height="21" fill="#1a1712" fill-opacity="0.55" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="48" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Gun Works (Steelworks) — 3×3 (96×96)
// ---------------------------------------------------------------------------

export const STEELWORKS_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="steelworks" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.roof" x="-37" y="-31" width="73" height="54" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.window.a" x="-27" y="-23" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <rect id="part.window.b" x="-27" y="0" width="54" height="12" fill="#1a1712" fill-opacity="0.42" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="48" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;
