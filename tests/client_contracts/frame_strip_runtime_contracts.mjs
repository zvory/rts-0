import { createMinimapAnimation } from "../../client/src/renderer/rigs/minimap_animation.js";
import { assert } from "./assertions.mjs";
import { KIND, SETUP, STATE, WEAPON_KIND } from "../../client/src/protocol.js";
import {
  frameStripFrameIndex,
  frameStripSpriteOffset,
  frameStripVisualFacing,
  frameStripWorldScale,
  renderFrameStripUnit,
} from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { RIFLEMAN_DIRECTIONAL_PROTOTYPE } from "../../client/src/renderer/rigs/rifleman_directional_prototype.js";
import { createInspectionPngPixiFactory, FakeContainer } from "../helpers/rig_inspection_pixi.mjs";

// A directional body must stay upright and fixed to its ground anchor while
// heading crosses zero, wraps full turns, or differs from stale weapon aim.
{
  const strip = RIFLEMAN_DIRECTIONAL_PROTOTYPE;
  const entity = { id: 1, kind: KIND.RIFLEMAN, state: STATE.MOVE, x: 123, y: 456 };
  const layer = new FakeContainer();
  const renderer = { _liveRigPools: { units: new Map() }, _seen: { units: new Set() },
    layers: { units: layer }, _rigPixiFactory: createInspectionPngPixiFactory() };
  const texture = { source: {} };
  for (let direction = 0; direction < 16; direction++) {
    const facing = Math.PI / 2 + direction * Math.PI / 8;
    for (const turns of [-2, 0, 2]) {
      entity.facing = facing + turns * 2 * Math.PI;
      entity.weaponFacing = entity.facing + Math.PI;
      const context = { now: direction * 1234, frameStripMoving: true };
      assert(frameStripFrameIndex(strip, entity, context) === direction, `moving heading ${direction} wraps correctly`);
      assert(frameStripVisualFacing(strip, entity, context) === 0, "directional body never rotates as a flat card");
      const instance = renderFrameStripUnit(renderer, entity, strip, texture, {
        poolName: "units", layerName: "units", renderContext: context,
      });
      assert(instance.container.x === 123 && instance.container.y === 456, "turning preserves authoritative position");
      assert(instance.sprite.anchorX === 0.5 && instance.sprite.anchorY === 112 / 128,
        "every heading uses the registered foot anchor");
      assert(instance.sprite.texture.frame.x === direction * 128, "runtime selects the directional cell");
      assert(instance.sprite.x === 0 && instance.sprite.y === 0, "turning adds no translation or bob");
    }
    entity.state = STATE.IDLE;
    entity.weaponFacing = facing;
    assert(frameStripFrameIndex(strip, entity, { frameStripMoving: false }) === direction,
      "stationary directional body follows weapon heading");
    entity.state = STATE.MOVE;
  }
  renderer._liveRigPools.units.get(entity.id).destroy();
}
import { _frameStripMovementVisual } from "../../client/src/renderer/units.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/machine_gunner_png_strip.js";
import { RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_panzerfaust_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";
import { WARRIOR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/warrior_png_strip.js";
import { WAR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/disciple_png_strips.js";
import { sampleWeaponRecoilCycle } from "../../client/src/weapon_recoil_cycle.js";

// Sample the real sword clock: the Yari clip must finish in 400 ms even though
// sword feedback remains active for 720 ms. Minimap and worker use this same context.
const war = { id: 91, kind: KIND.WAR_ADEPT, dao: "war", state: STATE.ATTACK,
  x: 100, y: 100, facing: 0, weaponFacing: 0 };
const warMinimap = createMinimapAnimation();
for (const [elapsed, expectedFrame, expectedStep] of [
  [0, 3, 0], [80, 3, -2.016], [200, 5, 11.2], [280, 5, 11.2], [400, 0, 0], [600, 0, 0],
]) {
  const cycle = sampleWeaponRecoilCycle(KIND.WARRIOR, elapsed, WEAPON_KIND.WARRIOR_SWORD);
  const context = { recoilProgress: cycle.progress, recoilPhase: cycle.phase,
    recoilWeaponKind: WEAPON_KIND.WARRIOR_SWORD };
  const frame = frameStripFrameIndex(WAR_PNG_FRAME_STRIP, war, context);
  assert(frame === expectedFrame, `Yari frame at ${elapsed} ms`);
  const offset = frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, frame, context);
  assert(Math.abs(offset.y - WAR_PNG_FRAME_STRIP.originLateralPx - expectedStep) < 1e-8, `Yari step at ${elapsed} ms`);
  warMinimap.beginFrame([war], {
    weaponRecoil: () => cycle.progress, weaponRecoilPhase: () => cycle.phase,
    weaponRecoilKind: () => WEAPON_KIND.WARRIOR_SWORD,
  }, elapsed);
  const layer = warMinimap.layers(war)[0];
  const normalizedStep = expectedStep / WAR_PNG_FRAME_STRIP.worldScale / (143 * 1.05);
  assert(Math.abs(layer.x - normalizedStep) < 1e-8 && Math.abs(layer.y) < 1e-8,
    `Yari minimap step matches the worker at ${elapsed} ms`);
}
warMinimap.destroy();

assert(WARRIOR_PNG_FRAME_STRIP.frameCount === 12, "Novice preserves six original cells and adds six attack poses");
assert(
  frameStripFrameIndex(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.ATTACK },
    { recoilProgress: 1, recoilPhase: 0.2, recoilWeaponKind: WEAPON_KIND.WARRIOR_SWORD },
  ) === 7,
  "Novice sword attack selects the intermediate early-swing pose",
);
assert(
  frameStripFrameIndex(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.ATTACK },
    { recoilProgress: 1, recoilPhase: 0.2, recoilWeaponKind: WEAPON_KIND.RIFLEMAN_RIFLE },
  ) === 0,
  "Warrior swipe art fails closed for non-sword attack feedback",
);

const warriorFacing = 0.75;
assert(
  frameStripVisualFacing(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.MOVE, facing: warriorFacing },
    { now: 0, frameStripMoving: true },
  ) === warriorFacing - Math.PI / 2,
  "Warrior locomotion rotates the authored sideways pose counterclockwise into travel direction",
);
assert(
  frameStripVisualFacing(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.IDLE, facing: warriorFacing, weaponFacing: 1.25 },
    { recoilProgress: 0 },
  ) === 1.25 - Math.PI / 2,
  "Warrior idle art keeps the same corrected body orientation as locomotion",
);
assert(
  frameStripVisualFacing(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.ATTACK, facing: warriorFacing, weaponFacing: 1.25 },
    { recoilProgress: 1, recoilPhase: 0.2, recoilWeaponKind: WEAPON_KIND.WARRIOR_SWORD },
  ) === 1.25,
  "Warrior sword swipe keeps its accepted attack-facing orientation",
);
assert(
  frameStripVisualFacing(
    WARRIOR_PNG_FRAME_STRIP,
    { id: 6, kind: KIND.WARRIOR, state: STATE.ATTACK, facing: warriorFacing, weaponFacing: 1.25 },
    { frameStripMoving: true, recoilProgress: 1, recoilPhase: 0.2, recoilWeaponKind: WEAPON_KIND.WARRIOR_SWORD },
  ) === 1.25,
  "Warrior sword swipe uses attack-facing art while the authoritative position is still moving",
);
assert(
  frameStripVisualFacing(
    {
      frameCount: 2,
      idleFrame: 0,
      firingFrames: [1],
      facingOffset: 0.5,
    },
    { id: 6, kind: KIND.WARRIOR, state: STATE.ATTACK, facing: warriorFacing, weaponFacing: 1.25 },
    { recoilProgress: 1, recoilPhase: 0.2 },
  ) === 1.75,
  "Firing art inherits the strip's base facing offset when it has no firing-specific override",
);

const deployedMachineGunner = {
  id: 7,
  kind: KIND.MACHINE_GUNNER,
  state: STATE.ATTACK,
  setupState: SETUP.DEPLOYED,
};

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0,
    recoilPhase: 0,
  }) === 11,
  "deployed Machine Gunner holds the setup-complete frame when not firing",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 1,
    recoilPhase: 0,
  }) === 12,
  "Machine Gunner firing animation starts from the deployed rest frame",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0.8,
    recoilPhase: 0.34,
  }) === 13,
  "Machine Gunner firing animation advances to the recoil peak frame",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0.4,
    recoilPhase: 0.67,
  }) === 14,
  "Machine Gunner firing animation returns to the deployed rest frame",
);

assert(
  frameStripFrameIndex(
    MACHINE_GUNNER_PNG_FRAME_STRIP,
    { ...deployedMachineGunner, setupState: SETUP.SETTING_UP },
    {
      setupVisual: { frameProgress: 1 },
      recoilProgress: 1,
      recoilPhase: 0.34,
    },
  ) === 11,
  "Machine Gunner setup frames take precedence over firing frames",
);

const riflemanFrameDuringRecoil = frameStripFrameIndex(
  RIFLEMAN_PNG_FRAME_STRIP,
  { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE },
  {
    now: 0,
    recoilProgress: 1,
    recoilPhase: 0.05,
    recoilWeaponKind: WEAPON_KIND.RIFLEMAN_RIFLE,
  },
);
assert(frameStripSpriteOffset(RIFLEMAN_PNG_FRAME_STRIP, RIFLEMAN_PNG_FRAME_STRIP.idleFrame).x === 10, "Rifleman art uses its corrected forward origin");
assert(frameStripSpriteOffset(RIFLEMAN_PNG_FRAME_STRIP, riflemanFrameDuringRecoil).x === 6, "Rifleman firing art applies the authored recoil offset");
assert(
  riflemanFrameDuringRecoil === RIFLEMAN_PNG_FRAME_STRIP.firingFrames[0],
  "moving Rifleman uses the authored firing frame during the brief recoil impulse",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.ATTACK },
    { now: 0, recoilProgress: 1, recoilPhase: 0.05, recoilWeaponKind: WEAPON_KIND.RIFLEMAN_RIFLE },
  ) === RIFLEMAN_PNG_FRAME_STRIP.firingFrames[0],
  "stationary Rifleman uses the authored firing frame during the brief recoil impulse",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.ATTACK },
    { now: 0, recoilProgress: 1, recoilPhase: 0.05 },
  ) === RIFLEMAN_PNG_FRAME_STRIP.idleFrame,
  "weapon-specific Rifleman firing art fails closed when recoil kind is unavailable",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.ATTACK },
    { now: 0, recoilProgress: 1, recoilPhase: 0.05, recoilWeaponKind: WEAPON_KIND.PANZERFAUST_LOADED_SHOT },
  ) === RIFLEMAN_PNG_FRAME_STRIP.idleFrame,
  "Panzerfaust recoil does not show the rifle muzzle-flare frame",
);

const windingUpPanzerfaust = {
  id: 12,
  kind: KIND.PANZERFAUST,
  state: STATE.ATTACK,
  panzerfaustLoaded: true,
};
for (const [progress, expectedFrame] of [
  [0, 5],
  [0.34, 6],
  [0.67, 7],
  [1, 7],
]) {
  assert(
    frameStripFrameIndex(
      RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP,
      { ...windingUpPanzerfaust, panzerfaustWindupProgress: progress },
    ) === expectedFrame,
    `Panzerfaust wind-up progress ${progress} selects authored frame ${expectedFrame}`,
  );
}
assert(
  frameStripFrameIndex(RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP, windingUpPanzerfaust) ===
    RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP.idleFrame,
  "loaded Panzerfaust holds its carried idle art when no authoritative wind-up is active",
);
assert(
  frameStripSpriteOffset(RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP, 5).x === 0,
  "Panzerfaust wind-up art uses its torso-centered origin instead of the carried-rifle offset",
);

assert(
  RIFLEMAN_PNG_FRAME_STRIP.movementFrames.includes(frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE },
    { now: 0, recoilProgress: 0.8, recoilPhase: RIFLEMAN_PNG_FRAME_STRIP.firingFrameHoldPhase },
  )),
  "moving Rifleman returns to its movement cycle after the recoil impulse",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.ATTACK },
    { now: 0, recoilProgress: 0.8, recoilPhase: RIFLEMAN_PNG_FRAME_STRIP.firingFrameHoldPhase },
  ) === RIFLEMAN_PNG_FRAME_STRIP.idleFrame,
  "stationary Rifleman returns to idle after the recoil impulse",
);

const stationaryMoveContext = { now: 1000, frameStripMoving: false };

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE, weaponFacing: 1.2 },
    stationaryMoveContext,
  ) === RIFLEMAN_PNG_FRAME_STRIP.idleFrame,
  "Rifleman in move state holds idle frame when its position is not changing",
);

assert(
  frameStripVisualFacing(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE, facing: 0.4, weaponFacing: 1.2 },
    stationaryMoveContext,
  ) === 1.2,
  "stationary Rifleman in move state uses standing weapon-facing art",
);

const stationaryMachineGunner = {
  id: 12,
  kind: KIND.MACHINE_GUNNER,
  state: STATE.MOVE,
  setupState: SETUP.PACKED,
  facing: 0.3,
  weaponFacing: 1.1,
};

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    MACHINE_GUNNER_PNG_FRAME_STRIP.idleFrame,
  "Machine Gunner in move state holds idle frame when its position is not changing",
);

assert(
  frameStripVisualFacing(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    stationaryMachineGunner.facing,
  "stationary packed Machine Gunner in move state keeps body-facing idle art",
);

assert(
  frameStripWorldScale(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    MACHINE_GUNNER_PNG_FRAME_STRIP.worldScale,
  "stationary Machine Gunner in move state does not use carried-movement scale",
);

const stationaryMoveEntity = {
  id: 20,
  kind: KIND.RIFLEMAN,
  state: STATE.MOVE,
  x: 40,
  y: 50,
};

assert(
  frameStripMovementFor(stationaryMoveEntity, stationaryMoveEntity, stationaryMoveEntity).moving === false,
  "renderer marks move-state frame-strip units stationary when current and previous snapshots did not move",
);

assert(
  frameStripMovementFor(stationaryMoveEntity, { ...stationaryMoveEntity, x: 38 }, stationaryMoveEntity).moving === true,
  "renderer marks frame-strip units moving when authoritative snapshot positions changed",
);

assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    stationaryMoveEntity,
    stationaryMoveEntity,
    new Map([[stationaryMoveEntity.id, { x: 36, y: 50 }]]),
  ).moving === true,
  "renderer keeps frame-strip movement active for predicted render-position changes",
);

const heldMovementSnapshot = {
  previous: { ...stationaryMoveEntity, x: 38 },
  current: stationaryMoveEntity,
  motion: new Map(),
  tick: 120,
};
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    0,
  ).moving === true,
  "renderer admits a fresh authoritative movement sample",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    1000 / 60,
  ).moving === true,
  "renderer keeps movement frames latched across the next 60 FPS render of a held 30 Hz snapshot",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    1000 / 30,
  ).moving === true,
  "renderer keeps movement frames latched until the next authoritative snapshot is due",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    120,
  ).moving === false,
  "renderer settles a paused or stalled held movement snapshot back to idle after the movement hold",
);

const interruptedMovement = {
  motion: new Map(),
  tick: 200,
};
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    { ...stationaryMoveEntity, x: 38 },
    stationaryMoveEntity,
    interruptedMovement.motion,
    interruptedMovement.tick,
    0,
  ).moving === true,
  "renderer latches observed movement while the unit is in move state",
);
assert(
  frameStripMovementFor(
    { ...stationaryMoveEntity, state: STATE.ATTACK },
    stationaryMoveEntity,
    stationaryMoveEntity,
    interruptedMovement.motion,
    interruptedMovement.tick + 1,
    25,
  ).moving === false,
  "renderer clears latched movement when an attacking unit is stationary",
);
const pursuingAttackEntity = { ...stationaryMoveEntity, state: STATE.ATTACK };
assert(
  frameStripMovementFor(
    pursuingAttackEntity,
    { ...pursuingAttackEntity, x: 38 },
    pursuingAttackEntity,
  ).moving === true,
  "renderer animates frame-strip walking while an attack-state unit pursues its target",
);
assert(
  RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP.movementFrames.includes(
    frameStripFrameIndex(RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP, pursuingAttackEntity, {
      now: 0,
      frameStripMoving: true,
    }),
  ),
  "attacking Panzerfaust pursuit uses the authored walking frames",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    stationaryMoveEntity,
    stationaryMoveEntity,
    interruptedMovement.motion,
    interruptedMovement.tick + 2,
    50,
  ).moving === false,
  "renderer does not resurrect stale movement after the unit returns to move state",
);

function frameStripMovementFor(entity, previous, current, motion = new Map(), tick = null, now = 0) {
  return _frameStripMovementVisual.call(
    { _frameStripMotion: motion, visualNow: () => now },
    entity,
    {
      _prevById: new Map([[entity.id, previous]]),
      _curById: new Map([[entity.id, current]]),
      tick,
    },
  );
}

// Move orders can remain active while a disciple waits in a crowd. Its minimap
// pose follows displacement, including movement during attack pursuit.
{
  const animation = createMinimapAnimation();
  const disciple = { id: 6, kind: KIND.WARRIOR, x: 100, y: 100,
    state: STATE.MOVE, facing: 0.4, weaponFacing: 1.2 };
  const state = { tick: 1, _prevById: new Map([[6, { x: 100, y: 100 }]]),
    _curById: new Map([[6, { x: 100, y: 100 }]]) };
  const sample = now => {
    animation.beginFrame([disciple], state, now);
    return animation.layers(disciple)[0];
  };
  for (const now of [0, 125, 300, 1000]) {
    assert(sample(now).part === "frame:0", "stationary disciple with move order holds minimap idle pose");
  }
  state.tick++;
  state._curById.set(6, { x: 104, y: 100 }); disciple.x = 104;
  assert(sample(1100).part !== "frame:0", "moving disciple walks on minimap");
  assert(sample(2000).part === "frame:0", "stale movement sample cannot animate forever");
  disciple.state = STATE.ATTACK;
  state.tick++; state._prevById.set(6, { x: 104, y: 100 });
  state._curById.set(6, { x: 108, y: 100 }); disciple.x = 108;
  assert(sample(2100).part !== "frame:0", "disciple pursuit walks on minimap");
  state.tick++; state._prevById.set(6, { x: 108, y: 100 });
  assert(sample(3000).part === "frame:0", "stopped pursuit returns to idle on minimap");
  state.weaponRecoil = () => 1;
  state.weaponRecoilPhase = () => 0.2;
  state.weaponRecoilKind = () => WEAPON_KIND.WARRIOR_SWORD;
  assert(sample(3100).part === "frame:7", "stationary disciple still shows its sword attack");
  delete state.weaponRecoil; delete state.weaponRecoilPhase; delete state.weaponRecoilKind;
  disciple.state = STATE.MOVE;
  state.tick++; state._curById.set(6, { x: 112, y: 100 }); disciple.x = 112;
  assert(sample(3200).part !== "frame:0", "disciple is walking before removal");
  animation.beginFrame([], state, 3210);
  state.tick++; state._prevById.set(6, { x: 112, y: 100 });
  assert(sample(3220).part === "frame:0", "returning disciple does not inherit removed motion within the movement hold");
  animation.destroy();
}

// After a Move completes, automatic pursuit may continue under the idle public
// order state. Displacement must keep the main renderer and minimap walking and
// facing travel instead of snapping to an old weapon bearing.
{
  const entity = { id: 101, kind: KIND.WAR_ADEPT, dao: "war", state: STATE.MOVE,
    x: 100, y: 100, facing: Math.PI / 2, weaponFacing: 0 };
  const motion = new Map();
  const animation = createMinimapAnimation();
  const previous = { ...entity };
  frameStripMovementFor(entity, previous, previous, motion, 1, 0);
  entity.state = STATE.IDLE; entity.y = 104;
  const state = { tick: 2, _prevById: new Map([[entity.id, previous]]),
    _curById: new Map([[entity.id, { ...entity }]]) };
  for (const now of [33, 50]) {
    const movement = frameStripMovementFor(entity, previous, entity, motion, 2, now);
    assert(movement.moving, "idle-state pursuit remains animated between snapshots");
    const context = { now, frameStripMoving: movement.moving };
    assert(WAR_PNG_FRAME_STRIP.movementFrames.includes(frameStripFrameIndex(WAR_PNG_FRAME_STRIP, entity, context)),
      "idle-state pursuit selects walking art");
    assert(frameStripVisualFacing(WAR_PNG_FRAME_STRIP, entity, context) === 0,
      "idle-state pursuit faces down with travel rather than sideways with stale weapon aim");
    animation.beginFrame([entity], state, now);
    const layer = animation.layers(entity)[0];
    assert(layer.part !== "dao-war:frame:0" && layer.rotation === 0,
      "minimap shares idle-state pursuit walking and travel facing");
  }
  const stoppedContext = { now: 200, frameStripMoving:
    frameStripMovementFor(entity, entity, entity, motion, 3, 200).moving };
  assert(!stoppedContext.frameStripMoving, "idle-state pursuit stops after displacement expires");
  assert(frameStripFrameIndex(WAR_PNG_FRAME_STRIP, entity, stoppedContext) === 0,
    "stopped idle-state pursuit returns to idle art");
  assert(frameStripFrameIndex(WAR_PNG_FRAME_STRIP, entity, { ...stoppedContext,
    recoilProgress: 1, recoilPhase: 0.2, recoilWeaponKind: WEAPON_KIND.WARRIOR_SWORD }) === 4,
    "idle-state pursuit still plays its spear strike when a shot arrives");
  assert(!frameStripMovementFor(entity, entity, entity).moving,
    "a stationary idle unit never invents walking without displacement");
  animation.destroy();
}
