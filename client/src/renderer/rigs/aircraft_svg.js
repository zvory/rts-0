export const SCOUT_PLANE_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  unit: Object.freeze([
    "part.wing",
    "part.boom.left",
    "part.boom.right",
    "part.tail.left",
    "part.tail.right",
    "part.tail.bridge",
    "part.fuselage",
    "part.cockpit",
    "part.nose",
    "part.prop",
  ]),
});

export const SCOUT_PLANE_RIG_SVG = `<svg viewBox="-48 -36 96 72" data-rts-rig-kind="scout_plane" data-rts-rig-version="1" data-rts-origin="center" id="scout-plane.authored">
  <ellipse id="part.shadow" cx="-2" cy="8.5" rx="28" ry="12" fill="#000000" opacity="0.22" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wing" points="-13,-28 5,-28 12,-5 12,5 5,28 -13,28 -7,6 -7,-6" fill="#5d7896" fill-opacity="0.94" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.left" x="-25" y="-18" width="33" height="5" fill="#6f8ca9" fill-opacity="0.97" stroke="#1a1712" stroke-width="1.5" stroke-opacity="0.92" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.right" x="-25" y="13" width="33" height="5" fill="#6f8ca9" fill-opacity="0.97" stroke="#1a1712" stroke-width="1.5" stroke-opacity="0.92" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.left" x1="-27" y1="-20" x2="-34" y2="-25" stroke="#1a1712" stroke-width="3" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.right" x1="-27" y1="20" x2="-34" y2="25" stroke="#1a1712" stroke-width="3" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.bridge" x1="-34" y1="-25" x2="-34" y2="25" stroke="#d8d0b0" stroke-width="2.4" stroke-opacity="0.78" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.fuselage" points="-31,-5 -19,-8 19,-6 34,-2 37,0 34,2 19,6 -19,8 -31,5" fill="#7f9ab7" fill-opacity="0.97" stroke="#1a1712" stroke-width="2" stroke-opacity="0.95" data-rts-tint="team-light-14" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.cockpit" cx="4" cy="0" rx="7.8" ry="3.8" fill="#211b14" fill-opacity="0.76" stroke="#d8d0b0" stroke-width="1.5" stroke-opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.nose" points="26,-4.2 39,0 26,4.2" fill="#d8d0b0" fill-opacity="0.7" stroke="#1a1712" stroke-width="1.6" stroke-opacity="0.9" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop" x1="39" y1="-7.5" x2="39" y2="7.5" stroke="#211b14" stroke-width="2.3" opacity="0.9" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-32" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-34" y="-29" width="72" height="58" fill="none" />
  <rect id="bounds.hp" x="-15" y="-35" width="30" height="6" fill="none" />
</svg>`;
