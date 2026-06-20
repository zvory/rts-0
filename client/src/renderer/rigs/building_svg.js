// Building SVG rigs — one per building kind.
//
// Authored at world-pixel scale (tileSize=32), centered at origin.
// Footprints: 3×3 = 96×96 px, 2×2 = 64×64 px, 3×2 = 96×64 px.
//
// No animation attributes — buildings are static.
// data-rts-tint="team" applies the player color at runtime.
// Unit geometry is borrowed and scaled where it reinforces the building's role.

// ---------------------------------------------------------------------------
// City Centre — 3×3 (96×96)
// Grand command seat. Trapezoidal hall widening toward the base, flanking
// wings, central tower spire, and gold command medallions (borrowed from the
// Command Car badge motif).
// ---------------------------------------------------------------------------

export const CITY_CENTRE_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="city_centre" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <polygon id="part.hall" points="-30,-40 30,-40 38,38 -38,38" fill="#6d89b8" fill-opacity="0.88" data-rts-tint="team" />
  <rect id="part.wing.left" x="-48" y="-22" width="20" height="52" fill="#6d89b8" fill-opacity="0.78" data-rts-tint="team-light-soft" />
  <rect id="part.wing.right" x="28" y="-22" width="20" height="52" fill="#6d89b8" fill-opacity="0.78" data-rts-tint="team-light-soft" />
  <rect id="part.tower" x="-9" y="-48" width="18" height="26" fill="#7b96c4" fill-opacity="0.95" data-rts-tint="team-light-strong" />
  <rect id="part.tower.cap" x="-7" y="-48" width="14" height="8" fill="#1a1712" fill-opacity="0.72" />
  <rect id="part.stripe" x="-30" y="-3" width="60" height="5" fill="#1a1712" fill-opacity="0.3" />
  <rect id="part.entrance" x="-10" y="30" width="20" height="12" fill="#1a1712" fill-opacity="0.62" />
  <line id="part.flag" x1="0" y1="-48" x2="0" y2="-38" stroke="#d8d0b0" stroke-width="2" opacity="0.85" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Zamok — 3×3 (96×96)
// Fortress. Four corner towers, thick outer walls, dark courtyard, south gate.
// ---------------------------------------------------------------------------

export const ZAMOK_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="zamok" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.wall.top" x="-34" y="-46" width="68" height="14" fill="#6d89b8" fill-opacity="0.86" data-rts-tint="team" />
  <rect id="part.wall.bottom" x="-34" y="32" width="68" height="14" fill="#6d89b8" fill-opacity="0.86" data-rts-tint="team" />
  <rect id="part.wall.left" x="-46" y="-34" width="14" height="68" fill="#6d89b8" fill-opacity="0.86" data-rts-tint="team" />
  <rect id="part.wall.right" x="32" y="-34" width="14" height="68" fill="#6d89b8" fill-opacity="0.86" data-rts-tint="team" />
  <circle id="part.tower.tl" cx="-34" cy="-34" r="13" fill="#7b96c4" fill-opacity="0.9" data-rts-tint="team-light" />
  <circle id="part.tower.tr" cx="34" cy="-34" r="13" fill="#7b96c4" fill-opacity="0.9" data-rts-tint="team-light" />
  <circle id="part.tower.bl" cx="-34" cy="34" r="13" fill="#7b96c4" fill-opacity="0.9" data-rts-tint="team-light" />
  <circle id="part.tower.br" cx="34" cy="34" r="13" fill="#7b96c4" fill-opacity="0.9" data-rts-tint="team-light" />
  <rect id="part.yard" x="-24" y="-24" width="48" height="48" fill="#1a1712" fill-opacity="0.38" />
  <circle id="part.cap.tl" cx="-34" cy="-34" r="5" fill="#1a1712" fill-opacity="0.62" />
  <circle id="part.cap.tr" cx="34" cy="-34" r="5" fill="#1a1712" fill-opacity="0.62" />
  <circle id="part.cap.bl" cx="-34" cy="34" r="5" fill="#1a1712" fill-opacity="0.62" />
  <circle id="part.cap.br" cx="34" cy="34" r="5" fill="#1a1712" fill-opacity="0.62" />
  <rect id="part.gate" x="-8" y="32" width="16" height="16" fill="#1a1712" fill-opacity="0.58" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Supply Depot — 2×2 (64×64)
// Two parallel storage bays with loading-door cutouts.
// ---------------------------------------------------------------------------

export const DEPOT_BUILDING_SVG = `<svg viewBox="-32 -32 64 64" data-rts-rig-kind="depot" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-32" y="-32" width="64" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.bay.a" x="-28" y="-22" width="56" height="18" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.bay.b" x="-28" y="4" width="56" height="18" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.door.tl" x="-28" y="-22" width="10" height="18" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.door.tr" x="18" y="-22" width="10" height="18" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.door.bl" x="-28" y="4" width="10" height="18" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.door.br" x="18" y="4" width="10" height="18" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.divider" x="-28" y="-4" width="56" height="8" fill="#1a1712" fill-opacity="0.28" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Barracks — 3×2 (96×64)
// Four rifles on a gun rack (horizontal, viewed top-down). Rifle barrel lines
// and grip ticks borrowed directly from the rifleman rig's barrel geometry.
// Left strip is the open drill yard.
// ---------------------------------------------------------------------------

export const BARRACKS_BUILDING_SVG = `<svg viewBox="-48 -32 96 64" data-rts-rig-kind="barracks" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-32" width="96" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.main" x="-40" y="-26" width="80" height="52" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.drill.yard" x="-48" y="-32" width="14" height="64" fill="#1a1712" fill-opacity="0.38" />
  <line id="part.rack.bar.top" x1="-26" y1="-20" x2="26" y2="-20" stroke="#2b2a23" stroke-width="3" opacity="0.85" />
  <line id="part.rack.bar.bot" x1="-26" y1="18" x2="26" y2="18" stroke="#2b2a23" stroke-width="3" opacity="0.85" />
  <line id="part.rifle.a" x1="-24" y1="-15" x2="24" y2="-15" stroke="#241d17" stroke-width="4" opacity="0.92" />
  <line id="part.rifle.b" x1="-24" y1="-5" x2="24" y2="-5" stroke="#241d17" stroke-width="4" opacity="0.92" />
  <line id="part.rifle.c" x1="-24" y1="5" x2="24" y2="5" stroke="#241d17" stroke-width="4" opacity="0.92" />
  <line id="part.rifle.d" x1="-24" y1="15" x2="24" y2="15" stroke="#241d17" stroke-width="4" opacity="0.92" />
  <line id="part.grip.a" x1="-4" y1="-18" x2="-4" y2="-12" stroke="#d8d0b0" stroke-width="2" opacity="0.72" />
  <line id="part.grip.b" x1="-4" y1="-8" x2="-4" y2="-2" stroke="#d8d0b0" stroke-width="2" opacity="0.72" />
  <line id="part.grip.c" x1="-4" y1="2" x2="-4" y2="8" stroke="#d8d0b0" stroke-width="2" opacity="0.72" />
  <line id="part.grip.d" x1="-4" y1="12" x2="-4" y2="18" stroke="#d8d0b0" stroke-width="2" opacity="0.72" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Training Centre — 3×2 (96×64)
// Firing range on the right (target rings + crosshair), obstacle course on
// the left (staggered blocks). Target ring scale matches the rifleman's
// selection-bound radius.
// ---------------------------------------------------------------------------

export const TRAINING_CENTRE_BUILDING_SVG = `<svg viewBox="-48 -32 96 64" data-rts-rig-kind="training_centre" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-32" width="96" height="64" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.drill" x="-46" y="-28" width="52" height="56" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.range" x="6" y="-28" width="40" height="56" fill="#6d89b8" fill-opacity="0.75" data-rts-tint="team-light" />
  <rect id="part.obstacle.a" x="-42" y="-20" width="9" height="7" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.obstacle.b" x="-30" y="-12" width="9" height="7" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.obstacle.c" x="-42" y="-4" width="9" height="7" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.obstacle.d" x="-30" y="4" width="9" height="7" fill="#1a1712" fill-opacity="0.55" />
  <rect id="part.obstacle.e" x="-42" y="12" width="9" height="7" fill="#1a1712" fill-opacity="0.55" />
  <circle id="part.target.outer" cx="24" cy="0" r="16" fill="#7b96c4" fill-opacity="0.72" data-rts-tint="team-light-soft" />
  <circle id="part.target.mid" cx="24" cy="0" r="10" fill="#1a1712" fill-opacity="0.48" />
  <circle id="part.target.inner" cx="24" cy="0" r="5" fill="#d8d0b0" fill-opacity="0.82" />
  <line id="part.crosshair.h" x1="8" y1="0" x2="40" y2="0" stroke="#1a1712" stroke-width="1" opacity="0.52" />
  <line id="part.crosshair.v" x1="24" y1="-16" x2="24" y2="16" stroke="#1a1712" stroke-width="1" opacity="0.52" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// R&D Complex — 3×3 (96×96)
// Octagonal building (angular, modern-science silhouette). Three lab-bench
// rows in the upper section, a research dome in the lower section,
// and two antennae at the roof line.
// ---------------------------------------------------------------------------

export const RESEARCH_COMPLEX_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="research_complex" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <polygon id="part.building" points="-30,-46 30,-46 46,-14 46,14 30,46 -30,46 -46,14 -46,-14" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.lab.a" x="-22" y="-34" width="44" height="6" fill="#1a1712" fill-opacity="0.48" />
  <rect id="part.lab.b" x="-22" y="-24" width="44" height="6" fill="#1a1712" fill-opacity="0.48" />
  <rect id="part.lab.c" x="-22" y="-14" width="44" height="6" fill="#1a1712" fill-opacity="0.48" />
  <circle id="part.dome" cx="0" cy="18" r="16" fill="#7b96c4" fill-opacity="0.9" data-rts-tint="team-light-strong" />
  <circle id="part.dome.cap" cx="0" cy="18" r="8" fill="#1a1712" fill-opacity="0.5" />
  <line id="part.antenna.left" x1="-20" y1="-46" x2="-20" y2="-38" stroke="#d8d0b0" stroke-width="2" opacity="0.75" />
  <line id="part.antenna.right" x1="20" y1="-46" x2="20" y2="-38" stroke="#d8d0b0" stroke-width="2" opacity="0.75" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Vehicle Works (Factory) — 3×3 (96×96)
// Upper section: workshop building with three chimney stacks.
// Lower section: assembly floor showing a tank hull mid-production —
// hull polygon and turret geometry lifted directly from tank_svg.js,
// positioned at the factory floor center.
// ---------------------------------------------------------------------------

export const FACTORY_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="factory" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.workshop" x="-44" y="-44" width="88" height="32" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.chimney.a" x="-36" y="-44" width="9" height="20" fill="#1a1712" fill-opacity="0.58" />
  <rect id="part.chimney.b" x="-20" y="-44" width="9" height="20" fill="#1a1712" fill-opacity="0.58" />
  <rect id="part.chimney.c" x="-4" y="-44" width="9" height="20" fill="#1a1712" fill-opacity="0.58" />
  <rect id="part.floor" x="-44" y="-12" width="88" height="56" fill="#1a1712" fill-opacity="0.22" />
  <polygon id="part.hull" points="-23.2,4.6 19.2,4.6 25.2,8.6 25.2,23.4 19.2,27.4 -23.2,27.4 -25.2,23.4 -25.2,8.6" fill="#5d7896" fill-opacity="0.9" data-rts-tint="team" />
  <rect id="part.track.top" x="-25.2" y="3" width="50.4" height="4" fill="#15120f" fill-opacity="0.88" />
  <rect id="part.track.bot" x="-25.2" y="24.4" width="50.4" height="4" fill="#15120f" fill-opacity="0.88" />
  <rect id="part.turret" x="-5" y="10.6" width="16" height="10" fill="#6b8cac" fill-opacity="0.92" data-rts-tint="team-light" />
  <line id="part.barrel" x1="9" y1="16" x2="42" y2="16" stroke="#241d17" stroke-width="5" opacity="0.95" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;

// ---------------------------------------------------------------------------
// Gun Works (Steelworks) — 3×3 (96×96)
// Three artillery barrel lines laid on a production floor — breech blocks on
// the left, muzzle ticks on the right. Barrel stroke weight and geometry
// taken from the artillery rig's barrel line. Upper building section above.
// ---------------------------------------------------------------------------

export const STEELWORKS_BUILDING_SVG = `<svg viewBox="-48 -48 96 96" data-rts-rig-kind="steelworks" data-rts-rig-version="1" data-rts-origin="center">
  <rect id="part.base" x="-48" y="-48" width="96" height="96" fill="#2b2a23" stroke="#1a1712" stroke-width="2" />
  <rect id="part.building.top" x="-44" y="-44" width="88" height="22" fill="#6d89b8" fill-opacity="0.85" data-rts-tint="team" />
  <rect id="part.main" x="-44" y="-22" width="88" height="36" fill="#6d89b8" fill-opacity="0.82" data-rts-tint="team" />
  <rect id="part.forge" x="-44" y="14" width="88" height="30" fill="#1a1712" fill-opacity="0.28" />
  <line id="part.barrel.a" x1="-40" y1="-10" x2="44" y2="-10" stroke="#241d17" stroke-width="7" opacity="0.95" />
  <line id="part.barrel.b" x1="-40" y1="4" x2="44" y2="4" stroke="#241d17" stroke-width="7" opacity="0.95" />
  <line id="part.barrel.c" x1="-40" y1="18" x2="44" y2="18" stroke="#241d17" stroke-width="7" opacity="0.95" />
  <rect id="part.breech.a" x="-48" y="-14" width="11" height="9" fill="#3d3528" fill-opacity="0.95" />
  <rect id="part.breech.b" x="-48" y="0" width="11" height="9" fill="#3d3528" fill-opacity="0.95" />
  <rect id="part.breech.c" x="-48" y="14" width="11" height="9" fill="#3d3528" fill-opacity="0.95" />
  <line id="part.muzzle.a" x1="40" y1="-14" x2="46" y2="-10" stroke="#d8d0b0" stroke-width="2" opacity="0.6" />
  <line id="part.muzzle.b" x1="40" y1="0" x2="46" y2="4" stroke="#d8d0b0" stroke-width="2" opacity="0.6" />
  <line id="part.muzzle.c" x1="40" y1="14" x2="46" y2="18" stroke="#d8d0b0" stroke-width="2" opacity="0.6" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#000000" />
  <circle id="anchor.hp" cx="0" cy="-48" r="1" fill="#000000" />
</svg>`;
