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
  <rect id="part.tail.bridge" x="-23" y="-10.5" width="2.4" height="21" fill="#e1ddd0" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.tail.left" points="-23.8,-11.9 -17.4,-9.8 -17.4,-7 -23.8,-8.4" fill="#f2efe3" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-tint="team-light-24" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.tail.right" points="-23.8,11.9 -17.4,9.8 -17.4,7 -23.8,8.4" fill="#f2efe3" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-tint="team-light-24" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.left" x="-20.6" y="-10.5" width="28.8" height="2.8" fill="#eeeadd" stroke="#16120f" stroke-width="1.3" stroke-opacity="0.95" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.right" x="-20.6" y="7.7" width="28.8" height="2.8" fill="#eeeadd" stroke="#16120f" stroke-width="1.3" stroke-opacity="0.95" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wing" points="-6.2,-16.1 6.6,-14.7 10.6,-4.2 10.6,4.2 6.6,14.7 -6.2,16.1 -7.8,4.2 -7.8,-4.2" fill="#f5f1e6" fill-opacity="0.98" stroke="#16120f" stroke-width="1.7" stroke-opacity="0.96" data-rts-tint="team-light-24" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.engine.left" cx="7.4" cy="-9.1" rx="4.4" ry="2.9" fill="#d6d1c4" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-tint="team-light-08" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.engine.right" cx="7.4" cy="9.1" rx="4.4" ry="2.9" fill="#d6d1c4" stroke="#16120f" stroke-width="1.4" stroke-opacity="0.95" data-rts-tint="team-light-08" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.fuselage" points="-15,-3.1 -8.6,-4.2 11.4,-3.6 20.2,-1.7 23.4,0 20.2,1.8 11.4,3.6 -8.6,4.2 -15,3.2" fill="#fffdf3" fill-opacity="0.98" stroke="#16120f" stroke-width="1.7" stroke-opacity="0.96" data-rts-tint="team-light-14" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.cockpit" points="-2.2,-2.2 9,-2.1 15.4,-0.8 16.2,0 15.4,0.8 9,2.1 -2.2,2.2" fill="#252522" fill-opacity="0.78" stroke="#d8d0b0" stroke-width="1.1" stroke-opacity="0.7" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.nose" points="19.4,-1.7 25,0 19.4,1.7" fill="#ffffff" fill-opacity="0.96" stroke="#16120f" stroke-width="1.2" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop.left" x1="12.2" y1="-12.3" x2="12.2" y2="-5.9" stroke="#211b14" stroke-width="1.8" opacity="0.88" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop.right" x1="12.2" y1="5.9" x2="12.2" y2="12.3" stroke="#211b14" stroke-width="1.8" opacity="0.88" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-22" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-25" y="-18" width="52" height="36" fill="none" />
  <rect id="bounds.hp" x="-14" y="-25" width="28" height="5" fill="none" />
</svg>`;
