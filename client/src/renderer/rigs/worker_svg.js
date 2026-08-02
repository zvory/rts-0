const GOLEM_BASE_RIG_SVG = `<svg viewBox="-24 -24 48 48" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center" id="worker.authored">
  <ellipse id="part.shadow" cx="0" cy="3.15" rx="9" ry="5.4" fill="#000000" opacity="0.28" />
  <polygon id="part.body" points="0,-9 7.65,-2.25 4.95,8.1 -4.95,8.1 -7.65,-2.25" fill="#6d89b8" stroke="#1a1712" stroke-width="2" fill-opacity="1" stroke-opacity="0.95" data-rts-tint="team" />
  <polyline id="part.busyIndicator" points="-4.95,-10.35 -1.8,-13.05 1.8,-13.05 4.95,-10.35" fill="none" stroke="#f2d16b" stroke-width="2" opacity="0.95" data-rts-animation="busy:visible:1:0" />
  <line id="part.facingTick" x1="0" y1="0" x2="12" y2="0" stroke="#d8d0b0" stroke-width="2" opacity="0.85" data-rts-animation="facing:transform.rotation:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-17" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-13" y="-13" width="26" height="26" fill="none" />
  <rect id="bounds.hp" x="-11" y="-18" width="22" height="6" fill="none" />
</svg>`;

export const WORKER_RIG_SVG = `<svg viewBox="-34 -28 68 56" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center" id="worker.authored">
  <ellipse id="part.shadow" cx="0.6" cy="4" rx="11.5" ry="6.2" fill="#000000" opacity="0.3" />
  <polygon id="part.body" points="7.4,0 3.4,-6.7 -4.6,-6.7 -8.6,0 -4.6,6.7 3.4,6.7" fill="#6d89b8" stroke="#1a1712" stroke-width="2.2" stroke-opacity="0.96" data-rts-tint="team" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.head" points="9.8,-6.5 12.1,-3.4 10.7,-0.3 6.8,0.5 4.2,-2.2 4.8,-5.4" fill="#7b96c4" stroke="#1a1712" stroke-width="2.1" stroke-opacity="0.96" data-rts-tint="team-light-strong" data-rts-animation="facing:transform.rotation:1:0" />
  <path id="part.shoulders" d="M -6.6 0 L 1 -4.9 M -6.6 0 L 1 4.9" fill="none" stroke="#1a1712" stroke-width="2.1" opacity="0.52" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.wrench.shaft" x1="-8.6" y1="10.4" x2="5.2" y2="3.8" stroke="#241d17" stroke-width="4.7" stroke-opacity="0.98" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.wrench.shaftHighlight" x1="-5.9" y1="9" x2="3.2" y2="4.6" stroke="#d8d0b0" stroke-width="1.35" stroke-opacity="0.66" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wrench.collar" points="3.9,1.8 10.2,-0.6 12.1,3.9 5.8,6.5" fill="#3d3528" fill-opacity="0.98" stroke="#d8d0b0" stroke-width="1.7" stroke-opacity="0.72" data-rts-animation="facing:transform.rotation:1:0" />
  <polygon id="part.wrench.headBack" points="6.1,-4.1 12.1,-6.4 17.3,-3.5 15.1,0.1 10.9,-0.2 9.3,3 12.3,5.8 16.5,4.7 19.4,8.2 15,11.7 8.4,9.8 5.4,4.9" fill="#5a574d" fill-opacity="0.98" stroke="#1a1712" stroke-width="2.2" stroke-opacity="0.96" data-rts-animation="facing:transform.rotation:1:0" />
  <polyline id="part.wrench.jawCut" points="13,-1.7 10.3,2.7 12.8,4.8 16,3" fill="none" stroke="#1a1712" stroke-width="2.1" opacity="0.92" data-rts-animation="facing:transform.rotation:1:0" />
  <line id="part.wrench.headHighlight" x1="8.2" y1="-3.2" x2="13.2" y2="-4.8" stroke="#d8d0b0" stroke-width="1.5" stroke-opacity="0.78" data-rts-animation="facing:transform.rotation:1:0" />
  <polyline id="part.busyIndicator" points="-5.2,-12.5 -1.9,-15.2 2.1,-15.2 5.4,-12.5" fill="none" stroke="#f2d16b" stroke-width="2" opacity="0.95" data-rts-animation="busy:visible:1:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-20" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-17" y="-17" width="34" height="34" fill="none" />
  <rect id="bounds.hp" x="-12" y="-22" width="24" height="6" fill="none" />
</svg>`;

export const GOLEM_RIG_SVG = GOLEM_BASE_RIG_SVG
  .replace('data-rts-rig-kind="worker"', 'data-rts-rig-kind="golem"')
  .replace('id="worker.authored"', 'id="golem.authored"');
