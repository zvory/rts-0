export const SCOUT_PLANE_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  unit: Object.freeze([
    "part.wing",
    "part.boom.left",
    "part.boom.right",
    "part.engine.left",
    "part.engine.right",
    "part.tail.left",
    "part.tail.right",
    "part.tail.bridge",
    "part.fuselage",
    "part.cockpit",
    "part.nose",
    "part.prop.left",
    "part.prop.right",
  ]),
});

export const SCOUT_PLANE_RIG_SVG = `<svg viewBox="-48 -36 96 72" data-rts-rig-kind="scout_plane" data-rts-rig-version="1" data-rts-origin="center" id="scout-plane.authored">
  <ellipse id="part.shadow" cx="-1.5" cy="5.5" rx="24" ry="9.5" fill="#000000" opacity="0.23" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.tail.bridge" x="-30" y="-15" width="3" height="30" fill="#e1ddd0" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.tail.left" points="-31,-17 -23,-14 -23,-10 -31,-12" fill="#f2efe3" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.tail.right" points="-31,17 -23,14 -23,10 -31,12" fill="#f2efe3" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.left" x="-27" y="-15" width="36" height="4" fill="#eeeadd" stroke="#16120f" stroke-width="1.3" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.right" x="-27" y="11" width="36" height="4" fill="#eeeadd" stroke="#16120f" stroke-width="1.3" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wing" points="-9,-23 7,-21 12,-6 12,6 7,21 -9,23 -11,6 -11,-6" fill="#f5f1e6" fill-opacity="0.98" stroke="#16120f" stroke-width="1.7" stroke-opacity="0.96" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.engine.left" cx="8" cy="-13" rx="5.5" ry="4.2" fill="#d6d1c4" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.engine.right" cx="8" cy="13" rx="5.5" ry="4.2" fill="#d6d1c4" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.fuselage" points="-20,-4.5 -12,-6 13,-5.2 24,-2.5 28,0 24,2.5 13,5.2 -12,6 -20,4.5" fill="#fffdf3" fill-opacity="0.98" stroke="#16120f" stroke-width="1.7" stroke-opacity="0.96" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.cockpit" points="-4,-3.2 10,-3 18,-1.2 19,0 18,1.2 10,3 -4,3.2" fill="#252522" fill-opacity="0.78" stroke="#d8d0b0" stroke-width="1.1" stroke-opacity="0.7" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.nose" points="23,-2.4 30,0 23,2.4" fill="#ffffff" fill-opacity="0.96" stroke="#16120f" stroke-width="1.2" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop.left" x1="14" y1="-17.6" x2="14" y2="-8.4" stroke="#211b14" stroke-width="1.8" opacity="0.88" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop.right" x1="14" y1="8.4" x2="14" y2="17.6" stroke="#211b14" stroke-width="1.8" opacity="0.88" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-22" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-25" y="-18" width="52" height="36" fill="none" />
  <rect id="bounds.hp" x="-14" y="-25" width="28" height="5" fill="none" />
</svg>`;
