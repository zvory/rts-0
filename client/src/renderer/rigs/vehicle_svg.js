export const COMMAND_CAR_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  unit: Object.freeze([
    "part.hull",
    "part.sideGear.top.fill",
    "part.sideGear.bottom.fill",
    "part.cabin",
    "part.darkNose",
    "part.darkSlot.top",
    "part.darkSlot.bottom",
    "part.windshield",
    "part.noseTick",
    "part.badge.top",
    "part.badge.bottom",
    "part.breakthroughAura",
  ]),
});

export const ROCKET_LAUNCHER_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  unit: Object.freeze([
    "part.hull",
    "part.wheel.top.front",
    "part.wheel.top.rear",
    "part.wheel.bottom.front",
    "part.wheel.bottom.rear",
    "part.cabin",
    "part.windshield",
    "part.rack",
    "part.rocket.1",
    "part.rocket.2",
    "part.rocket.3",
    "part.rocket.4",
    "part.rocket.5",
    "part.rocket.6",
    "part.rocket.7",
    "part.rocket.8",
  ]),
});

export const EKAT_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  unit: Object.freeze([
    "part.dress.trail",
    "part.dress.core",
    "part.shoulders",
    "part.staff.shadow",
    "part.staff",
    "part.arm.left",
    "part.arm.right",
    "part.head",
    "part.hair.back",
    "part.hair.bun",
    "part.hair.flow.crown",
    "part.hair.flow.upper",
    "part.hair.flow.side",
    "part.hair.flow.lower",
    "part.hair.flow.tip",
    "part.hair.bun.flow.upper",
    "part.hair.bun.flow.lower",
    "part.orb",
  ]),
});

export const COMMAND_CAR_RIG_SVG = `<svg viewBox="-36 -28 72 56" data-rts-rig-kind="command_car" data-rts-rig-version="1" data-rts-origin="center" id="command-car.authored">
  <polygon id="part.shadow" transform="translate(0 4.62)" points="21.400,0.000 20.671,3.416 18.533,6.600 15.132,9.334 10.700,11.432 5.539,12.750 0.000,13.200 -5.539,12.750 -10.700,11.432 -15.132,9.334 -18.533,6.600 -20.671,3.416 -21.400,0.000 -20.671,-3.416 -18.533,-6.600 -15.132,-9.334 -10.700,-11.432 -5.539,-12.750 -0.000,-13.200 5.539,-12.750 10.700,-11.432 15.132,-9.334 18.533,-6.600 20.671,-3.416" fill="#000000" opacity="0.28" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.hull" points="-17.400,-7.544 3.480,-7.544 17.400,-5.336 17.400,5.336 3.480,7.544 -17.400,7.544" fill="#5d7896" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.sideGear.top.fill" x="-15.486" y="-8.006" width="27.492" height="1.656" fill="#15120f" fill-opacity="0.58" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.sideGear.top.outline" x="-15.486" y="-8.006" width="27.492" height="1.656" fill="none" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.sideGear.bottom.fill" x="-15.486" y="6.350" width="27.492" height="1.656" fill="#15120f" fill-opacity="0.58" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.sideGear.bottom.outline" x="-15.486" y="6.350" width="27.492" height="1.656" fill="none" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.cabin" x="-10.092" y="-5.428" width="12.528" height="10.856" fill="#6d8cab" fill-opacity="0.98" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.darkNose" x="7.134" y="-3.588" width="4.176" height="7.176" fill="#211b14" fill-opacity="0.78" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.darkSlot.top" x="-7.830" y="-3.910" width="3.480" height="2.392" fill="#211b14" fill-opacity="0.78" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.darkSlot.bottom" x="-7.830" y="1.518" width="3.480" height="2.392" fill="#211b14" fill-opacity="0.78" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.windshield" x1="2.784" y1="-4.416" x2="2.784" y2="4.416" stroke="#d8d0b0" stroke-width="2" opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.noseTick" x1="13.9" y1="0" x2="17.4" y2="0" stroke="#d8d0b0" stroke-width="2" opacity="0.74" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.badge.top" cx="-1.74" cy="-2.944" r="2.6" fill="#d8c267" fill-opacity="0.95" stroke="#d8d0b0" stroke-width="2" stroke-opacity="0.74" />
  <circle id="part.badge.bottom" cx="-1.74" cy="2.944" r="2.6" fill="#d8c267" fill-opacity="0.95" stroke="#d8d0b0" stroke-width="2" stroke-opacity="0.74" />
  <circle id="part.breakthroughAura" cx="0" cy="0" r="15.147" fill="none" stroke="#f2d16b" stroke-width="2" stroke-opacity="0.82" data-rts-animation="breakthroughTicks:visible:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-22" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-21" y="-14" width="42" height="28" fill="none" />
  <rect id="bounds.hp" x="-13" y="-25" width="26" height="6" fill="none" />
</svg>`;

export const ROCKET_LAUNCHER_RIG_SVG = `<svg viewBox="-36 -28 72 56" data-rts-rig-kind="rocket_launcher" data-rts-rig-version="1" data-rts-origin="center" id="rocket-launcher.prototype">
  <ellipse id="part.shadow" cx="-1" cy="5" rx="24" ry="12" fill="#000000" opacity="0.28" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.hull" x="-21" y="-8" width="42" height="16" rx="3" fill="#ffffff" stroke="#17131a" stroke-width="2" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.wheel.top.front" cx="13" cy="-9" r="4" fill="#18151b" stroke="#655a68" stroke-width="1.2" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.wheel.top.rear" cx="-13" cy="-9" r="4" fill="#18151b" stroke="#655a68" stroke-width="1.2" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.wheel.bottom.front" cx="13" cy="9" r="4" fill="#18151b" stroke="#655a68" stroke-width="1.2" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.wheel.bottom.rear" cx="-13" cy="9" r="4" fill="#18151b" stroke="#655a68" stroke-width="1.2" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.cabin" x="8" y="-6" width="11" height="12" rx="2" fill="#ffffff" stroke="#17131a" stroke-width="1.5" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.windshield" x1="9.5" y1="-4.5" x2="9.5" y2="4.5" stroke="#d8d0b0" stroke-width="1.5" opacity="0.72" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.rack" x="-18" y="-10" width="25" height="20" rx="2" fill="#ffffff" fill-opacity="0.9" stroke="#17131a" stroke-width="1.2" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.1" x1="-17" y1="-8" x2="5" y2="-8" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.2" x1="-17" y1="-5.8" x2="5" y2="-5.8" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.3" x1="-17" y1="-3.5" x2="5" y2="-3.5" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.4" x1="-17" y1="-1.2" x2="5" y2="-1.2" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.5" x1="-17" y1="1.2" x2="5" y2="1.2" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.6" x1="-17" y1="3.5" x2="5" y2="3.5" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.7" x1="-17" y1="5.8" x2="5" y2="5.8" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.rocket.8" x1="-17" y1="8" x2="5" y2="8" stroke="#ffffff" stroke-width="2" opacity="0.9" data-rts-tint="team-light-10" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-24" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-25" y="-15" width="50" height="30" fill="none" />
  <rect id="bounds.hp" x="-15" y="-27" width="30" height="6" fill="none" />
</svg>`;

export const EKAT_RIG_SVG = `<svg viewBox="-24 -24 48 48" data-rts-rig-kind="ekat" data-rts-rig-version="1" data-rts-origin="center" id="ekat.authored">
  <ellipse id="part.shadow" cx="-2" cy="4.2" rx="14" ry="7" fill="#000000" opacity="0.28" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.dress.trail" d="M 3.5 -5.2 C -4.5 -11.5 -15.5 -10.5 -20 -4.2 L -13.6 0 L -20 4.2 C -15.5 10.5 -4.5 11.5 3.5 5.2 L 1 0 Z" fill="#5d7896" fill-opacity="0.92" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.dress.core" points="5,-3.8 0,-7.2 -9.5,-6 -14,0 -9.5,6 0,7.2 5,3.8 3,0" fill="#6d89b8" fill-opacity="0.98" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.shoulders" d="M 2.4 -6 C 4.7 -6 5.8 -4.2 5.8 -2 L 5.8 2 C 5.8 4.2 4.7 6 2.4 6 C 0.1 6 -1 4.2 -1 2 L -1 -2 C -1 -4.2 0.1 -6 2.4 -6 Z" fill="#d8ad8b" stroke="#1a1712" stroke-width="1.25" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.staff.shadow" x1="-7.5" y1="6.25" x2="19.2" y2="8.8" stroke="#1a1712" stroke-width="4" opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.staff" x1="-7.5" y1="6.25" x2="19.2" y2="8.8" stroke="#6b3f22" stroke-width="2.25" opacity="0.98" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.arm.left" points="-0.2,-6.7 4.8,-7.4 5.6,-4.2 0.2,-3.1" fill="#d8ad8b" fill-opacity="0.95" stroke="#1a1712" stroke-width="0.8" stroke-opacity="0.85" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.arm.right" d="M -0.7 2.8 L 3 3 L 5.6 5.4 L 8.5 5.7 L 7.8 9.3 L 4.1 8.8 L 1.2 6.1 L -1 5.4 Z" fill="#d8ad8b" fill-opacity="0.95" stroke="#1a1712" stroke-width="0.8" stroke-opacity="0.85" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.head" cx="3.2" cy="0" r="3.15" fill="#d8ad8b" stroke="#1a1712" stroke-width="1.25" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.back" d="M 3.54 -3.65 C 0.95 -5.23 -2.74 -3.47 -2.93 0.18 C -3.11 3.58 0.59 4.44 3.51 2.67 L 3.14 -0.13 Z" fill="#e1bf4f" stroke="#1a1712" stroke-width="1.13" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.hair.bun" cx="-3.95" cy="0.2" rx="1.05" ry="1.18" fill="#e1bf4f" stroke="#1a1712" stroke-width="0.8" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.flow.crown" d="M 2.77 -2.61 Q 0.82 -3.89 -1.54 -2.25" fill="none" stroke="#8f6b1f" stroke-width="0.56" opacity="0.72" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.flow.upper" d="M 2.4 -3.2 Q 0.17 -3.2 -2.05 -1.46" fill="none" stroke="#8f6b1f" stroke-width="0.5" opacity="0.58" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.flow.side" d="M 2.52 -0.97 Q 0.34 -0.31 -2.09 0.97" fill="none" stroke="#8f6b1f" stroke-width="0.56" opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.flow.lower" d="M 2.54 0.81 Q 0.26 1.96 -2.05 1.58" fill="none" stroke="#8f6b1f" stroke-width="0.5" opacity="0.58" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.flow.tip" d="M 2.83 1.83 Q 0.82 3.04 -1.54 2.19" fill="none" stroke="#8f6b1f" stroke-width="0.56" opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.bun.flow.upper" d="M -4.5 -0.25 Q -3.9 -0.82 -3.27 -0.12" fill="none" stroke="#8f6b1f" stroke-width="0.42" opacity="0.68" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.hair.bun.flow.lower" d="M -4.49 0.55 Q -3.83 0.94 -3.23 0.34" fill="none" stroke="#8f6b1f" stroke-width="0.42" opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="part.orb" cx="19.2" cy="8.8" r="3.1" fill="#7fa8c9" stroke="#d8d0b0" stroke-width="1.35" stroke-opacity="0.9" data-rts-tint="team-light-strong" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-21" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-22" y="-16" width="45" height="33" fill="none" />
  <rect id="bounds.hp" x="-13" y="-24" width="26" height="6" fill="none" />
</svg>`;
