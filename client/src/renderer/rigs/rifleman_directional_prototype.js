// Opt-in Lab art study: sixteen headings, no locomotion or firing animation.
export const RIFLEMAN_DIRECTIONAL_PROTOTYPE = Object.freeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-directional-prototype/directional-strip.png?v=registered-2",
  imageVersion: "registered-2",
  frameWidth: 128,
  frameHeight: 128,
  frameCount: 16,
  idleFrame: 0,
  directional: Object.freeze({ firstFacing: Math.PI / 2 }),
  anchorX: 0.5,
  anchorY: 112 / 128,
  worldScale: 0.45,
  tintSlot: "team-light",
});
