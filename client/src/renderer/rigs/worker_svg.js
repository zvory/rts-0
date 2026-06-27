export const WORKER_RIG_SVG = `<svg viewBox="-24 -24 48 48" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center" id="worker.authored">
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

export const GOLEM_RIG_SVG = WORKER_RIG_SVG
  .replace('data-rts-rig-kind="worker"', 'data-rts-rig-kind="golem"')
  .replace('id="worker.authored"', 'id="golem.authored"');
