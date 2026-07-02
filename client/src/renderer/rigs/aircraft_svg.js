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
  <ellipse id="part.shadow" cx="-1.5" cy="5.5" rx="20" ry="8" fill="#000000" opacity="0.22" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wing" points="-8,-17 4,-17 8,-4 8,4 4,17 -8,17 -5,4 -5,-4" fill="#5d7896" fill-opacity="0.94" stroke="#1a1712" stroke-width="1.7" stroke-opacity="0.95" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.left" x="-17" y="-11" width="23" height="3.5" fill="#6f8ca9" fill-opacity="0.97" stroke="#1a1712" stroke-width="1.2" stroke-opacity="0.92" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <rect id="part.boom.right" x="-17" y="7.5" width="23" height="3.5" fill="#6f8ca9" fill-opacity="0.97" stroke="#1a1712" stroke-width="1.2" stroke-opacity="0.92" data-rts-tint="team-light-soft" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.left" x1="-19" y1="-13" x2="-24" y2="-16" stroke="#1a1712" stroke-width="2.3" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.right" x1="-19" y1="13" x2="-24" y2="16" stroke="#1a1712" stroke-width="2.3" stroke-opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.tail.bridge" x1="-24" y1="-16" x2="-24" y2="16" stroke="#d8d0b0" stroke-width="1.9" stroke-opacity="0.78" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.fuselage" points="-22,-4 -14,-5.5 13,-4.5 23,-1.4 25,0 23,1.4 13,4.5 -14,5.5 -22,4" fill="#7f9ab7" fill-opacity="0.97" stroke="#1a1712" stroke-width="1.7" stroke-opacity="0.95" data-rts-tint="team-light-14" data-rts-animation="facing:transform.rotation:1:0" />
  <ellipse id="part.cockpit" cx="3" cy="0" rx="5" ry="2.6" fill="#211b14" fill-opacity="0.76" stroke="#d8d0b0" stroke-width="1.1" stroke-opacity="0.62" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.nose" points="18,-3 26,0 18,3" fill="#d8d0b0" fill-opacity="0.7" stroke="#1a1712" stroke-width="1.2" stroke-opacity="0.9" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.prop" x1="26" y1="-5.2" x2="26" y2="5.2" stroke="#211b14" stroke-width="1.8" opacity="0.9" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-22" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-25" y="-18" width="52" height="36" fill="none" />
  <rect id="bounds.hp" x="-14" y="-25" width="28" height="5" fill="none" />
</svg>`;
