// tests/client_contracts/protocol_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
  assertThrows,
} from "./assertions.mjs";
import {
  assertMalformedBinaryRejected,
  fixtureSnapshotFrames,
  runSnapshotCodecBakeoff,
} from "../../scripts/snapshot-codec-bakeoff.mjs";
import {
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_SHELL_DELAY_TICKS,
} from "../../client/src/config.js";
import {
  COMPACT_SNAPSHOT_VERSION,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  PREDICTION_PROTOCOL_VERSION,
  ABILITY,
  ABILITY_CODE,
  ABILITY_OBJECT_KIND,
  ABILITY_OBJECT_KIND_CODE,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  NOTICE_SEVERITY,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  SETUP,
  SETUP_CODE,
  STATE,
  STATE_CODE,
  UPGRADE,
  UPGRADE_CODE,
  WEAPON_KIND,
  WEAPON_KIND_CODE,
  cmd,
  decodeServerMessage,
  parseServerFrame,
  msg,
} from "../../client/src/protocol.js";

import { messagePackSnapshotFrame } from "./snapshot_frame_helpers.mjs";

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------
{
  const decoded = decodeServerMessage({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [42, 100, 25, 3, 10],
    n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 7, 42],
    e: [
      [
        1,
        1,
        KIND_CODE[KIND.WORKER],
        10,
        20,
        40,
        40,
        STATE_CODE[STATE.GATHER],
        1.5,
        1.75,
        null,
        null,
        null,
        null,
        200,
        9,
        null,
        null,
        null,
        null,
        null,
        [
          [ORDER_STAGE_CODE[ORDER_STAGE.MOVE], 96, 112],
          [ORDER_STAGE_CODE[ORDER_STAGE.SETUP_ANTI_TANK_GUNS], 128, 160],
          [ORDER_STAGE_CODE[ORDER_STAGE.CHARGE], 176, 208],
          [ORDER_STAGE_CODE[ORDER_STAGE.SMOKE], 192, 224],
          [ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE], 320, 352],
        ],
        87,
        [[ABILITY_CODE[ABILITY.CHARGE], 87, 2, null, 77, 45, null, 90]],
        66,
        true,
        [[[112, 128], [144, 160]], [192, 224], 12, 2, 1, 2],
      ],
      [
        2,
        1,
        KIND_CODE[KIND.MACHINE_GUNNER],
        30,
        40,
        55,
        55,
        STATE_CODE[STATE.ATTACK],
        null,
        0.3,
        null,
        null,
        null,
        null,
        null,
        7,
        SETUP_CODE[SETUP.DEPLOYED],
      ],
      [
        3,
        1,
        KIND_CODE[KIND.CITY_CENTRE],
        100,
        120,
        450,
        500,
        STATE_CODE[STATE.TRAIN],
        null,
        null,
        KIND_CODE[KIND.WORKER],
        0.25,
        2,
        0.75,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        true,
        null,
        null,
        null,
        null,
        true,
        null,
        KIND_CODE[KIND.WORKER],
      ],
      [
        4,
        2,
        KIND_CODE[KIND.TANK_TRAP],
        140,
        150,
        150,
        150,
        STATE_CODE[STATE.IDLE],
        ...Array(21).fill(null),
        false,
        0.4,
        9.5,
        80,
      ],
      [
        5,
        1,
        KIND_CODE[KIND.SCOUT_PLANE],
        160,
        170,
        40,
        40,
        STATE_CODE[STATE.IDLE],
        ...Array(25).fill(null),
        [[512, 544]],
      ],
      [
        6,
        1,
        KIND_CODE[KIND.PANZERFAUST],
        180,
        190,
        45,
        45,
        STATE_CODE[STATE.IDLE],
        ...Array(27).fill(null),
        false,
      ],
    ],
    r: [[200, 1498]],
    sm: [[50, 320, 352, 2, 120]],
    ao: [
      [
        60,
        1,
        ABILITY_CODE[ABILITY.EKAT_TELEPORT],
        ABILITY_OBJECT_KIND_CODE[ABILITY_OBJECT_KIND.RETURN_MARKER],
        384,
        416,
        90,
        7,
        [45, null, 14, null, null, null],
      ],
    ],
    tr: [[80, 448, 480, 0.375]],
    u: [1, UPGRADE_CODE[UPGRADE.ARTILLERY_UNLOCK]],
    fg: [1, 2, 3, 1],
    ev: [
      [EVENT_CODE[EVENT.ATTACK], 1, 7],
      [EVENT_CODE[EVENT.OVERPENETRATION], 22],
      [EVENT_CODE[EVENT.MISS], 7],
      [EVENT_CODE[EVENT.DEATH], 200, 64, 96, KIND_CODE[KIND.STEEL]],
      [EVENT_CODE[EVENT.BUILD], 3, KIND_CODE[KIND.CITY_CENTRE]],
      [EVENT_CODE[EVENT.NOTICE], "Not enough steel"],
      [EVENT_CODE[EVENT.NOTICE], "alert:under_attack", 3, 512, 768],
      [EVENT_CODE[EVENT.MORTAR_LAUNCH], 9, [256, 272], [320, 352], 1.5, 68],
      [EVENT_CODE[EVENT.ARTILLERY_TARGET], 10, [320, 352], 3, ARTILLERY_SHELL_DELAY_TICKS],
      [EVENT_CODE[EVENT.ARTILLERY_FIRING], 1, 288, 304, 0.25],
      [EVENT_CODE[EVENT.ARTILLERY_IMPACT], 336, 368, 3],
      [EVENT_CODE[EVENT.PANZERFAUST_LAUNCH], 11, [360, 384], [416, 384], 15],
      [EVENT_CODE[EVENT.PANZERFAUST_IMPACT], 416, 384],
      [EVENT_CODE[EVENT.PANZERFAUST_CONVERSION], 11, KIND_CODE[KIND.RIFLEMAN]],
    ],
  });

  assert(decoded.t === "snapshot", "compact snapshot keeps the semantic tag");
  assert(decoded.upgrades[0] === UPGRADE.METHAMPHETAMINES, "compact upgrades decode");
  assert(decoded.upgrades[1] === UPGRADE.ARTILLERY_UNLOCK, "compact artillery upgrade decodes");
  assert(decoded.tick === 42 && decoded.steel === 100 && decoded.supplyCap === 10, "compact scalars decode");
  assert(decoded.netStatus.predictionVersion === PREDICTION_PROTOCOL_VERSION, "compact prediction version decodes");
  assert(decoded.netStatus.lastSimConsumedClientSeq === 7, "compact consumed client sequence decodes");
  assert(decoded.netStatus.lastSimConsumedClientTick === 42, "compact consumed client tick decodes");
  assert(decoded.entities.length === 6, "compact entities decode");
  assert(decoded.entities[0].kind === KIND.WORKER, "entity kind code decodes");
  assert(decoded.entities[0].state === STATE.GATHER, "entity state code decodes");
  assert(decoded.entities[0].weaponFacing === 1.75, "entity optional weaponFacing decodes");
  assert(decoded.entities[0].latchedNode === 200, "entity optional latchedNode decodes");
  assert(decoded.entities[0].orderPlan.length === 5, "entity order plan decodes");
  assert(decoded.entities[0].chargeCooldownLeft === 87, "legacy charge cooldown decodes");
  assert(
    decoded.entities[0].abilities[0].ability === ABILITY.CHARGE &&
      decoded.entities[0].abilities[0].cooldownLeft === 87 &&
      decoded.entities[0].abilities[0].remainingUses === 2 &&
      decoded.entities[0].abilities[0].activeObjectId === 77 &&
      decoded.entities[0].abilities[0].availableTick === 45 &&
      decoded.entities[0].abilities[0].expiresIn === 90,
    "entity ability cooldowns decode",
  );
  assert(
    decoded.entities[0].orderPlan[0].kind === ORDER_STAGE.MOVE &&
      decoded.entities[0].orderPlan[0].x === 96 &&
      decoded.entities[0].orderPlan[0].y === 112,
    "entity active order stage decodes",
  );
  assert(decoded.entities[0].breakthroughTicks === 66, "entity breakthrough status decodes");
  assert(decoded.entities[0].visionOnly === true, "entity visionOnly flag decodes");
  assert(
    decoded.entities[0].debugPath.waypoints[0].x === 112 &&
      decoded.entities[0].debugPath.waypoints[1].y === 160 &&
      decoded.entities[0].debugPath.goal.x === 192 &&
      decoded.entities[0].debugPath.lastRepathTick === 12 &&
      decoded.entities[0].debugPath.stuckTicks === 2 &&
      decoded.entities[0].debugPath.staticBlockedTicks === 1 &&
      decoded.entities[0].debugPath.totalWaypoints === 2,
    "entity debug path decodes",
  );
  assert(
      decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      decoded.entities[0].orderPlan[2].kind === ORDER_STAGE.CHARGE &&
      decoded.entities[0].orderPlan[3].kind === ORDER_STAGE.SMOKE &&
      decoded.entities[0].orderPlan[4].kind === ORDER_STAGE.POINT_FIRE,
    "order plan stage flavor decodes",
  );
  assert(
    decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      decoded.entities[0].orderPlan[1].x === 128 &&
      decoded.entities[0].orderPlan[1].y === 160,
    "queued anti-tank gun setup order stage decodes",
  );
  assert(
    decoded.entities[0].orderPlan[2].kind === ORDER_STAGE.CHARGE &&
      decoded.entities[0].orderPlan[2].x === 176 &&
      decoded.entities[0].orderPlan[2].y === 208,
    "queued Charge order stage decodes",
  );
  assert(decoded.entities[1].setupState === SETUP.DEPLOYED, "entity setupState code decodes");
  assert(decoded.entities[2].prodKind === KIND.WORKER, "entity prodKind code decodes");
  assert(decoded.entities[2].prodProgress === 0.25, "entity prodProgress decodes");
  assert(decoded.entities[2].prodScoutPlaneQueued === true, "entity Scout Plane queue flag decodes");
  assert(decoded.entities[2].prodRepeatKind === KIND.WORKER, "entity repeat production kind decodes");
  assert(decoded.entities[2].buildActive === true, "entity construction activity flag decodes");
  assert(
    decoded.entities[2].orderPlan === undefined,
    "compact snapshot tolerates missing order plan fields",
  );
  assert(decoded.entities[3].deconstructProgress === 0.4, "entity deconstructProgress decodes");
  assert(decoded.entities[3].weaponRangeTiles === 9.5, "entity weaponRangeTiles decodes");
  assert(decoded.entities[3].occupiedTrenchId === 80, "entity occupiedTrenchId decodes");
  assert(decoded.entities[4].kind === KIND.SCOUT_PLANE, "Scout Plane kind code decodes");
  assert(
    decoded.entities[4].scoutPlane.orbitCenter[0] === 512 &&
      decoded.entities[4].scoutPlane.orbitCenter[1] === 544,
    "Scout Plane compact owner state decodes",
  );
  assert(decoded.entities[5].panzerfaustLoaded === false, "entity Panzerfaust loaded flag decodes");
  assert(decoded.resourceDeltas[0].remaining === 1498, "resource deltas decode");
  assert(
    decoded.smokes[0].id === 50 &&
      decoded.smokes[0].radiusTiles === 2 &&
      decoded.smokes[0].expiresIn === 120,
    "smoke clouds decode",
  );
  assert(
    decoded.abilityObjects[0].id === 60 &&
      decoded.abilityObjects[0].kind === ABILITY_OBJECT_KIND.RETURN_MARKER &&
      decoded.abilityObjects[0].ownerState.earliestReturnTick === 45 &&
      decoded.abilityObjects[0].ownerState.radius === 14,
    "ability objects decode",
  );
  assert(
      decoded.trenches[0].id === 80 &&
      decoded.trenches[0].x === 448 &&
      decoded.trenches[0].y === 480 &&
      decoded.trenches[0].radiusTiles === 0.375,
    "trenches decode",
  );
  assert(
    decoded.visibleTiles.join(",") === "1,1,0,0,0,1",
    "compact snapshot decodes server visibility grid",
  );
  assert(decoded.events[0].e === EVENT.ATTACK && decoded.events[0].to === 7, "attack event decodes");
  const weaponEventDecoded = decodeServerMessage({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [43, 0, 0, 0, 0],
    e: [],
    ev: [
      [EVENT_CODE[EVENT.ATTACK], 1, 7, null, null, WEAPON_KIND_CODE[WEAPON_KIND.TANK_CANNON]],
      [EVENT_CODE[EVENT.ATTACK], 1, 7, null, [48, 96], WEAPON_KIND_CODE[WEAPON_KIND.ANTI_TANK_GUN]],
      [EVENT_CODE[EVENT.ATTACK], 1, 7, null, null, 255],
    ],
    n: [0, 0, 0, 0, 0],
  });
  assert(
    weaponEventDecoded.events[0].weaponKind === WEAPON_KIND.TANK_CANNON,
    "six-slot compact attack event decodes weaponKind",
  );
  assert(
    weaponEventDecoded.events[1].toPos[1] === 96 &&
      weaponEventDecoded.events[1].weaponKind === WEAPON_KIND.ANTI_TANK_GUN,
    "six-slot compact attack event preserves explicit null placeholders before weaponKind",
  );
  assert(
    !("weaponKind" in weaponEventDecoded.events[2]),
    "unknown compact weaponKind falls back to a missing weapon hint",
  );
  assert(
    decoded.events[1].e === EVENT.OVERPENETRATION && decoded.events[1].to === 22,
    "overpenetration event decodes",
  );
  assert(decoded.events[2].e === EVENT.MISS && decoded.events[2].to === 7, "miss event decodes");
  assert(decoded.events[3].kind === KIND.STEEL, "death event kind decodes");
  assert(decoded.events[5].msg === "Not enough steel", "notice event decodes");
  assert(decoded.events[5].severity === NOTICE_SEVERITY.INFO, "legacy notice defaults to info");
  assert(decoded.events[6].severity === NOTICE_SEVERITY.ALERT, "notice severity decodes");
  assert(decoded.events[6].x === 512 && decoded.events[6].y === 768, "notice position decodes");
  assert(
    decoded.events[7].e === EVENT.MORTAR_LAUNCH &&
      decoded.events[7].from === 9 &&
      decoded.events[7].fromX === 256 &&
      decoded.events[7].toY === 352 &&
      decoded.events[7].delayTicks === 68,
    "mortar launch event decodes",
  );
  assert(
    decoded.events[8].e === EVENT.ARTILLERY_TARGET &&
      decoded.events[8].from === 10 &&
      decoded.events[8].delayTicks === ARTILLERY_SHELL_DELAY_TICKS &&
      decoded.events[8].radiusTiles === 3,
    "artillery target event decodes",
  );
  assert(
    decoded.events[9].e === EVENT.ARTILLERY_FIRING &&
      decoded.events[9].owner === 1 &&
      decoded.events[9].x === 288 &&
      decoded.events[9].facing === 0.25,
    "artillery firing minimap event decodes",
  );
  assert(
    decoded.events[10].e === EVENT.ARTILLERY_IMPACT &&
      decoded.events[10].x === 336 &&
      decoded.events[10].y === 368,
    "artillery impact event decodes",
  );
  assert(
    decoded.events[11].e === EVENT.PANZERFAUST_LAUNCH &&
      decoded.events[11].from === 11 &&
      decoded.events[11].fromX === 360 &&
      decoded.events[11].toX === 416 &&
      decoded.events[11].delayTicks === 15,
    "panzerfaust launch event decodes without target id",
  );
  assert(
    decoded.events[12].e === EVENT.PANZERFAUST_IMPACT &&
      decoded.events[12].x === 416 &&
      decoded.events[12].y === 384,
    "panzerfaust impact event decodes",
  );
  assert(
    decoded.events[13].e === EVENT.PANZERFAUST_CONVERSION &&
      decoded.events[13].id === 11 &&
      decoded.events[13].toKind === KIND.RIFLEMAN,
    "legacy panzerfaust conversion event decodes",
  );

  const abilityCommand = cmd.useAbility(ABILITY.SMOKE, [7, 8], 320, 384, true);
  assert(
    abilityCommand.c === "useAbility" &&
      abilityCommand.ability === ABILITY.SMOKE &&
      abilityCommand.units.length === 2 &&
      abilityCommand.x === 320 &&
      abilityCommand.y === 384 &&
      abilityCommand.queued === true,
    "useAbility command builder emits targeted ability wire shape",
  );
  const recastCommand = cmd.recastAbility(ABILITY.EKAT_TELEPORT, [9], 77, true);
  assert(
    recastCommand.c === "recastAbility" &&
      recastCommand.ability === ABILITY.EKAT_TELEPORT &&
      recastCommand.units.length === 1 &&
      recastCommand.targetObjectId === 77 &&
      recastCommand.queued === true,
    "recastAbility command builder emits explicit recast wire shape",
  );
  const buildCommand = cmd.build([7, 8], KIND.DEPOT, 12, 14, true);
  assert(
    buildCommand.c === "build" &&
      buildCommand.units.join(",") === "7,8" &&
      buildCommand.building === KIND.DEPOT &&
      buildCommand.tileX === 12 &&
      buildCommand.tileY === 14 &&
      buildCommand.queued === true,
    "build command builder emits selected-worker wire shape",
  );
  const deconstructCommand = cmd.deconstruct([7, 8], 55, true);
  assert(
    deconstructCommand.c === "deconstruct" &&
      deconstructCommand.units.join(",") === "7,8" &&
      deconstructCommand.target === 55 &&
      deconstructCommand.queued === true,
    "deconstruct command builder emits selected-worker target wire shape",
  );
  assert(
    JSON.stringify(msg.command(cmd.stop([7]), 3)) ===
      JSON.stringify({ t: "command", clientSeq: 3, cmd: { c: "stop", units: [7] } }),
    "command message builder wraps gameplay commands with clientSeq",
  );
  assert(
    JSON.stringify(msg.command(cmd.holdPosition([7]), 4)) ===
      JSON.stringify({ t: "command", clientSeq: 4, cmd: { c: "holdPosition", units: [7] } }),
    "holdPosition command builder emits the hold-position wire shape",
  );
  const pointFireCommand = cmd.pointFire([11, 12], 512, 640, true);
  assert(
    pointFireCommand.c === "useAbility" &&
      pointFireCommand.ability === ABILITY.POINT_FIRE &&
      pointFireCommand.units.join(",") === "11,12" &&
      pointFireCommand.x === 512 &&
      pointFireCommand.y === 640 &&
      pointFireCommand.queued === true,
    "pointFire command builder emits targeted ability wire shape",
  );
  const blanketFireCommand = cmd.blanketFire([11, 12], 704, 768, true);
  assert(
    blanketFireCommand.c === "useAbility" &&
      blanketFireCommand.ability === ABILITY.BLANKET_FIRE &&
      blanketFireCommand.units.join(",") === "11,12" &&
      blanketFireCommand.x === 704 &&
      blanketFireCommand.y === 768 &&
      blanketFireCommand.queued === true,
    "blanketFire command builder emits targeted ability wire shape",
  );
  assertThrows(
    () => decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION, s: [1], e: [] }),
    "compact snapshot rejects malformed scalar count",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [[1, 1, 255, 0, 0, 1, 1, STATE_CODE[STATE.IDLE]]],
      }),
    "compact snapshot rejects unknown enum codes",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: new Array(20001),
      }),
    "compact snapshot enforces entity count bounds",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [[
          1,
          1,
          KIND_CODE[KIND.WORKER],
          0,
          0,
          1,
          1,
          STATE_CODE[STATE.IDLE],
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          new Array(10),
        ]],
      }),
    "compact snapshot enforces order plan bounds",
  );
  assert(SNAPSHOT_CODEC.COMPACT_JSON === "compact-json", "client keeps compact JSON baseline codec name");
  assert(
    SNAPSHOT_CODEC.MESSAGEPACK_COMPACT === "messagepack-compact",
    "client mirrors MessagePack snapshot codec name",
  );
  assert(SNAPSHOT_CODEC_VERSION === 1, "client mirrors snapshot codec version");
  assert(SNAPSHOT_FRAME_KIND.BINARY === "binary", "client mirrors binary snapshot frame kind");
  assert(
    parseServerFrame(JSON.stringify({ t: "snapshot", tick: 1 })).t === "snapshot",
    "protocol frame parser accepts JSON text frames",
  );
  const binarySnapshot = messagePackSnapshotFrame({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [77, 10, 20, 1, 5],
    e: [],
    n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
  });
  assert(
    decodeServerMessage(parseServerFrame(binarySnapshot)).tick === 77,
    "protocol frame parser decodes MessagePack snapshot frames",
  );
  assertThrows(
    () => parseServerFrame(new Uint8Array([1, 2, 3])),
    "protocol frame parser rejects malformed binary frames",
  );
  assertThrows(
    () => parseServerFrame(new Uint8Array([0x52, 0x54, 0x53, 0x4d, 0xff])),
    "protocol frame parser rejects unsupported MessagePack frame versions",
  );
  const bakeoff = runSnapshotCodecBakeoff({ frames: fixtureSnapshotFrames(), iterations: 1 });
  assert(
    bakeoff.candidates.some((candidate) => candidate.id === "compact-json") &&
      bakeoff.candidates.some((candidate) => candidate.id === "custom-positional-binary"),
    "snapshot codec bake-off compares baseline and custom binary candidates",
  );
  assertMalformedBinaryRejected();
}

{
  assert(
    JSON.stringify(cmd.setupAntiTankGuns([1, 2], 100, 200)) ===
      JSON.stringify({ c: "setupAntiTankGuns", units: [1, 2], x: 100, y: 200 }),
    "setupAntiTankGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.tearDownAntiTankGuns([3, 4])) ===
      JSON.stringify({ c: "tearDownAntiTankGuns", units: [3, 4] }),
    "tearDownAntiTankGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.move([1], 100, 200, true)) ===
      JSON.stringify({ c: "move", units: [1], x: 100, y: 200, queued: true }),
    "queued move command builder emits the queued flag only when requested",
  );
  assert(ANTI_TANK_GUN_DEPLOYED_RANGE_TILES === 20, "client mirrors deployed anti-tank gun range");
  assertApprox(
    ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
    30 * Math.PI / 180,
    0.000001,
    "client mirrors anti-tank gun field of fire",
  );
}

// ---------------------------------------------------------------------------
