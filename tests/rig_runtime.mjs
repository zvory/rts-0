#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KIND, SETUP, STATE } from "../client/src/protocol.js";
import {
  _drawUnit,
  _rigRenderContextFor,
} from "../client/src/renderer/units.js";
import { _sweep } from "../client/src/renderer/layers.js";
import {
  createLiveRigDefinitions,
  liveRigKeyForEntity,
  liveRigRoutesFor,
} from "../client/src/renderer/rigs/live_routing.js";
import { compileVisualUnitRigCandidates } from "../client/src/renderer/rigs/visual_override_rigs.js";
import { compileSvgRig } from "../client/src/renderer/rigs/svg_importer.js";
import {
  createRigRenderContext,
  sampleRigAnimation,
  transformedRigAnchorPoint,
} from "../client/src/renderer/rigs/animation.js";
import {
  createUnitRigInstance,
  renderLiveUnitRig,
} from "../client/src/renderer/rigs/runtime.js";
import {
  frameStripFrameIndex,
  frameStripVisualFacing,
  frameStripWorldScale,
} from "../client/src/renderer/rigs/frame_strip_runtime.js";
import {
  applyFrameStripColorAdjustmentToRgba,
  FRAME_STRIP_TARGET_COLOR_ADJUSTMENT,
  frameStripRuntimeColorAdjustment,
  isNeutralFrameStripColorAdjustment,
} from "../client/src/renderer/rigs/frame_strip_color_profile.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "../client/src/renderer/rigs/machine_gunner_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../client/src/renderer/rigs/rifleman_png_strip.js";
import {
  pngAtlasCanRenderRoute,
  pngAtlasRouteCoverage,
} from "../client/src/renderer/rigs/png_runtime.js";
import { ANTI_TANK_GUN_PNG_RIG_ATLAS } from "../client/src/renderer/rigs/anti_tank_gun_png_atlas.js";
import { MORTAR_TEAM_PNG_RIG_ATLAS } from "../client/src/renderer/rigs/mortar_team_png_atlas.js";
import { TANK_PNG_RIG_ATLAS } from "../client/src/renderer/rigs/tank_png_atlas.js";
import {
  LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG,
  MACHINE_GUNNER_RIG_SVG,
  RIFLEMAN_RIG_SVG,
} from "../client/src/renderer/rigs/infantry_svg.js";
import {
  ANTI_TANK_GUN_RIG_SVG,
  ARTILLERY_RIG_SVG,
  MORTAR_TEAM_RIG_SVG,
} from "../client/src/renderer/rigs/support_svg.js";
import { TANK_RIG_SVG } from "../client/src/renderer/rigs/tank_svg.js";
import {
  COMMAND_CAR_RIG_SVG,
  EKAT_RIG_SVG,
  SCOUT_CAR_RIG_SVG,
} from "../client/src/renderer/rigs/vehicle_svg.js";
import { GOLEM_RIG_SVG, WORKER_RIG_SVG } from "../client/src/renderer/rigs/worker_svg.js";
import {
  createInspectionPixiFactory,
  createInspectionPngPixiFactory,
  FakeContainer,
  FakeGraphics,
} from "./helpers/rig_inspection_pixi.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, "..");
const fixturesDir = path.join(__dirname, "fixtures/svg");
const machineGunnerPngManifestPath = path.join(
  repoRoot,
  "client/assets/rigs/machine-gunner-pass-01/metadata/manifest.json",
);
const fixedNow = 12_345;

function main() {
test("animation sampler applies game-state bindings without Pixi", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 7,
    kind: KIND.TANK,
    owner: 1,
    x: 80,
    y: 90,
    hp: 60,
    maxHp: 100,
    state: STATE.MOVE,
    facing: Math.PI / 2,
    weaponFacing: Math.PI,
  };
  const context = createRigRenderContext(entity, {
    now: fixedNow,
    state: { playerId: 1, resources: { oil: 0 }, weaponRecoil: () => 0.5 },
    colorByOwner: new Map([[1, 0x336699]]),
    vehicleMotion: { activity: 0.75 },
  });
  const sampled = sampleRigAnimation(definition, entity, context);
  assert.equal(sampled.context.teamColor, 0x336699);
  assert.equal(sampled.context.oilStarved, true);
  assert.equal(sampled.parts["part.hull"].transform.rotation, Math.PI / 2);
  assert.equal(sampled.parts["part.turret"].transform.rotation, Math.PI);
  assert.equal(sampled.parts["part.barrel"].transform.rotation, Math.PI);
});

test("tank rig exposes transformed main and coax muzzle anchors", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 8,
    kind: KIND.TANK,
    owner: 1,
    x: 100,
    y: 100,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
    facing: 0,
    weaponFacing: Math.PI / 2,
  };

  const mainMuzzle = transformedRigAnchorPoint(definition, entity, "muzzle", { now: fixedNow });
  const coaxMuzzle = transformedRigAnchorPoint(definition, entity, "coaxMuzzle", { now: fixedNow });

  assert.ok(mainMuzzle, "tank main muzzle anchor should resolve");
  assert.ok(coaxMuzzle, "tank coax muzzle anchor should resolve");
  assert.ok(Math.abs(mainMuzzle.x - 100) < 0.001);
  assert.ok(Math.abs(mainMuzzle.y - 133.2) < 0.001);
  assert.ok(Math.abs(coaxMuzzle.x - 105.55) < 0.001);
  assert.ok(Math.abs(coaxMuzzle.y - 116.6) < 0.001);

  const recoilingMainMuzzle = transformedRigAnchorPoint(definition, entity, "muzzle", {
    now: fixedNow,
    state: { weaponRecoil: () => 0.5 },
  });
  const recoilingCoaxMuzzle = transformedRigAnchorPoint(definition, entity, "coaxMuzzle", {
    now: fixedNow,
    state: { weaponRecoil: () => 0.5 },
  });

  assert.ok(Math.abs(recoilingMainMuzzle.x - 100) < 0.001);
  assert.ok(Math.abs(recoilingMainMuzzle.y - 124.875) < 0.001);
  assert.ok(Math.abs(recoilingCoaxMuzzle.x - 105.55) < 0.001);
  assert.ok(Math.abs(recoilingCoaxMuzzle.y - 112.775) < 0.001);
});

test("tank rig adds a half-scale artillery-style muzzle flare on cannon recoil", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 9,
    kind: KIND.TANK,
    owner: 1,
    x: 100,
    y: 100,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
    facing: 0,
    weaponFacing: 0,
  };

  const idle = sampleRigAnimation(definition, entity, createRigRenderContext(entity, {
    now: fixedNow,
    state: { weaponRecoil: () => 0 },
  }));
  assert.equal(idle.parts["part.tank.flashCone"].alpha, 0);
  assert.equal(idle.parts["part.tank.flashCore"].alpha, 0);
  assert.equal(idle.parts["part.tank.flashGlow"].alpha, 0);

  const firing = sampleRigAnimation(definition, entity, createRigRenderContext(entity, {
    now: fixedNow,
    state: { weaponRecoil: () => 1 },
  }));
  const recoiledMuzzle = transformedRigAnchorPoint(definition, entity, "muzzle", {
    now: fixedNow,
    state: { weaponRecoil: () => 1 },
  });
  const cone = firing.parts["part.tank.flashCone"];
  const core = firing.parts["part.tank.flashCore"];
  const glow = firing.parts["part.tank.flashGlow"];
  assert.ok(Math.abs(cone.transform.x - 25.35) < 0.001);
  assert.ok(Math.abs(cone.transform.y) < 0.001);
  assert.ok(recoiledMuzzle);
  const recoiledMuzzleLocalX = recoiledMuzzle.x - entity.x;
  assert.ok(Math.abs(cone.transform.x - recoiledMuzzleLocalX - 8.8) < 0.001);
  assert.ok(Math.abs(cone.geometryScale.x - 4) < 0.001);
  assert.ok(Math.abs(cone.geometryScale.y - 3.533333333329) < 0.001);
  assert.ok(Math.abs(core.geometryScale.x - 3.4) < 0.001);
  assert.ok(Math.abs(glow.geometryScale.x - 3) < 0.001);
  assert.ok(Math.abs(cone.alpha - 1) < 0.001);
  assert.ok(Math.abs(core.alpha - 1) < 0.001);
  assert.ok(Math.abs(glow.alpha - 1) < 0.001);
});

test("tank long-cannon override keeps the flare ahead of the recoiling muzzle", () => {
  const compiled = compileVisualUnitRigCandidates();
  const definition = compiled.definitions.get("tank-long-cannon")?.definition;
  assert.ok(definition, JSON.stringify([...compiled.errors.entries()]));
  assert.ok(Math.abs(definition.anchors.muzzle.x - 39.2) < 0.001);
  const entity = {
    id: 10,
    kind: KIND.TANK,
    owner: 1,
    x: 100,
    y: 100,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
    facing: 0,
    weaponFacing: 0,
  };
  const context = createRigRenderContext(entity, {
    now: fixedNow,
    state: { weaponRecoil: () => 1 },
  });
  const firing = sampleRigAnimation(definition, entity, context);
  const recoiledMuzzle = transformedRigAnchorPoint(definition, entity, "muzzle", context);
  assert.ok(recoiledMuzzle);
  const recoiledMuzzleLocalX = recoiledMuzzle.x - entity.x;
  const cone = firing.parts["part.tank.flashCone"];
  assert.ok(Math.abs(recoiledMuzzleLocalX - 22.55) < 0.001);
  assert.ok(Math.abs(cone.transform.x - 31.35) < 0.001);
  assert.ok(Math.abs(cone.transform.x - recoiledMuzzleLocalX - 8.8) < 0.001);
});

test("rig runtime creates one container child per part and updates transforms", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const instance = createUnitRigInstance(KIND.WORKER, definition, createInspectionPixiFactory());
  assert.equal(instance.container.children.length, definition.parts.length);
  assert.deepEqual(instance.container.children.map((child) => child.rtsRigPartId), definition.parts.map((part) => part.id));

  instance.update({
    id: 1,
    kind: KIND.WORKER,
    owner: 1,
    x: 24,
    y: 32,
    facing: Math.PI / 4,
    hp: 20,
    maxHp: 30,
  }, createRigRenderContext({ id: 1, kind: KIND.WORKER, owner: 1, facing: Math.PI / 4 }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x225588]]),
  }));

  const body = instance.parts.get("part.body").display;
  const facingTick = instance.parts.get("part.facingTick").display;
  const busyIndicator = instance.parts.get("part.busyIndicator").display;
  assert.equal(instance.container.x, 24);
  assert.equal(instance.container.y, 32);
  assert.equal(body.rotation, 0);
  assert.equal(facingTick.rotation, Math.PI / 4);
  assert.equal(busyIndicator.visible, false);
  assert.equal(body.commands.find((cmd) => cmd.op === "beginFill").color, 0x225588);
  assert.deepEqual(body.commands.find((cmd) => cmd.op === "lineStyle"), { op: "lineStyle", width: 2, color: 0x1a1712, alpha: 0.95 });

  instance.update({
    id: 1,
    kind: KIND.WORKER,
    owner: 1,
    x: 24,
    y: 32,
    facing: Math.PI / 4,
    hp: 20,
    maxHp: 30,
    state: STATE.BUILD,
  }, createRigRenderContext({ id: 1, kind: KIND.WORKER, owner: 1, facing: Math.PI / 4, state: STATE.BUILD }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x225588]]),
  }));
  assert.equal(instance.parts.get("part.busyIndicator").display.visible, true);

  instance.destroy();
  assert.equal(instance._destroyed, true);
  assert.equal(instance.parts.size, 0);
  assert.equal(instance.container.destroyed, true);
});

test("rig runtime reuses part graphics when only transforms change", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const instance = createUnitRigInstance(KIND.WORKER, definition, createInspectionPixiFactory());
  const entity = {
    id: 12,
    kind: KIND.WORKER,
    owner: 1,
    x: 24,
    y: 32,
    facing: 0,
    hp: 20,
    maxHp: 30,
  };

  instance.update(entity, createRigRenderContext(entity, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x225588]]),
  }));

  const body = instance.parts.get("part.body").display;
  const facingTick = instance.parts.get("part.facingTick").display;
  const bodyCommands = body.commands;
  const facingTickCommands = facingTick.commands;
  const bodyClearCount = body.clearCount;
  const facingTickClearCount = facingTick.clearCount;

  const moved = { ...entity, x: 64, y: 80, facing: Math.PI / 2 };
  instance.update(moved, createRigRenderContext(moved, {
    now: fixedNow + 16,
    colorByOwner: new Map([[1, 0x225588]]),
  }));

  assert.equal(instance.container.x, 64);
  assert.equal(instance.container.y, 80);
  assert.equal(facingTick.rotation, Math.PI / 2);
  assert.equal(body.commands, bodyCommands);
  assert.equal(facingTick.commands, facingTickCommands);
  assert.equal(body.clearCount, bodyClearCount);
  assert.equal(facingTick.clearCount, facingTickClearCount);

  instance.update(moved, createRigRenderContext(moved, {
    now: fixedNow + 32,
    colorByOwner: new Map([[1, 0x884422]]),
  }));

  assert.equal(body.clearCount, bodyClearCount + 1);
  assert.equal(body.commands.find((cmd) => cmd.op === "beginFill").color, 0x884422);
  instance.destroy();
});

test("rig runtime can update one routed part group", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const instance = createUnitRigInstance(KIND.WORKER, definition, createInspectionPixiFactory());
  instance.update({
    id: 11,
    kind: KIND.WORKER,
    owner: 1,
    x: 10,
    y: 20,
    facing: Math.PI / 2,
  }, createRigRenderContext({ id: 11, kind: KIND.WORKER, owner: 1, facing: Math.PI / 2 }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x778899]]),
  }), { includeParts: ["part.facingTick"] });

  assert.equal(instance.parts.get("part.facingTick").display.visible, true);
  assert.equal(instance.parts.get("part.body").display.visible, false);
  assert.equal(instance.parts.get("part.shadow").display.visible, false);
  assert.equal(instance.parts.get("part.facingTick").display.commands.some((cmd) => cmd.op === "lineTo"), true);
  instance.destroy();
});

test("animation sampler can skip parts outside the active render routes", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 13, kind: KIND.WORKER, owner: 1, facing: Math.PI / 2 };
  const sampled = sampleRigAnimation(definition, entity, createRigRenderContext(entity, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x778899]]),
  }), { includeParts: ["part.shadow", "part.facingTick"] });

  assert.deepEqual(Object.keys(sampled.parts).sort(), ["part.facingTick", "part.shadow"]);
  assert.equal(sampled.parts["part.facingTick"].transform.rotation, Math.PI / 2);
});

test("geometry-scale animation grows coordinates without scaling stroke width", () => {
  const svg = `<svg viewBox="-12 -12 24 24" data-rts-rig-kind="${KIND.WORKER}" data-rts-rig-version="1" data-rts-origin="center">
  <polygon id="part.flash" points="-1,0 2,-1 2,1" fill="#ffd84a" stroke="#d8d0b0" stroke-width="2.2" data-rts-animation="recoilProgress:geometry.scaleX:1:0;recoilProgress:geometry.scaleY:0.5:0" />
  <circle id="anchor.origin" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.selection" cx="0" cy="0" r="1" fill="#ffffff" />
  <circle id="anchor.hp" cx="0" cy="-10" r="1" fill="#ffffff" />
  <rect id="bounds.selection" x="-8" y="-8" width="16" height="16" fill="none" />
  <rect id="bounds.hp" x="-8" y="-12" width="16" height="4" fill="none" />
</svg>`;
  const compiled = compileSvgRig(svg, { expectedKind: KIND.WORKER });
  assert.equal(compiled.ok, true);
  const instance = createUnitRigInstance(KIND.WORKER, compiled.definition, createInspectionPixiFactory());

  instance.update({
    id: 21,
    kind: KIND.WORKER,
    owner: 1,
    x: 0,
    y: 0,
    recoilProgress: 0.4,
  }, createRigRenderContext({ id: 21, kind: KIND.WORKER, owner: 1, recoilProgress: 0.4 }, {
    now: fixedNow,
    state: { weaponRecoil: () => 0.4 },
    colorByOwner: new Map([[1, 0x778899]]),
  }));

  const commands = instance.parts.get("part.flash").display.commands;
  assert.deepEqual(commands.find((cmd) => cmd.op === "lineStyle"), { op: "lineStyle", width: 2.2, color: 0xd8d0b0, alpha: 1 });
  assert.deepEqual(commands.find((cmd) => cmd.op === "drawPolygon").points, [-1.4, 0, 2.8, -1.2, 2.8, 1.2]);
  instance.destroy();
});

test("live rig definitions compile production SVG sources", () => {
  const workerFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-worker.svg"), "utf8").trim();
  const riflemanFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-rifleman.svg"), "utf8").trim();
  const machineGunnerFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-machine-gunner.svg"), "utf8").trim();
  const antiTankGunFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-anti-tank-gun.svg"), "utf8").trim();
  const mortarTeamFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-mortar-team.svg"), "utf8").trim();
  const artilleryFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-artillery.svg"), "utf8").trim();
  const scoutCarFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-scout-car.svg"), "utf8").trim();
  const commandCarFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-command-car.svg"), "utf8").trim();
  const ekatFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-ekat.svg"), "utf8").trim();
  const tankFixtureText = fs.readFileSync(path.join(fixturesDir, "rig-vehicle.svg"), "utf8").trim();
  assert.equal(WORKER_RIG_SVG.trim(), workerFixtureText);
  assert.equal(GOLEM_RIG_SVG.includes('data-rts-rig-kind="golem"'), true);
  assert.equal(GOLEM_RIG_SVG.includes('id="golem.authored"'), true);
  assert.equal(RIFLEMAN_RIG_SVG.trim(), riflemanFixtureText);
  assert.equal(LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG.includes('data-rts-rig-kind="rifleman"'), true);
  assert.equal(LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG.includes('part.pzf.tube'), true);
  assert.equal(LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG.includes('part.rifle.barrel'), false);
  assert.equal(MACHINE_GUNNER_RIG_SVG.trim(), machineGunnerFixtureText);
  assert.equal(ANTI_TANK_GUN_RIG_SVG.trim(), antiTankGunFixtureText);
  assert.equal(MORTAR_TEAM_RIG_SVG.trim(), mortarTeamFixtureText);
  assert.equal(ARTILLERY_RIG_SVG.trim(), artilleryFixtureText);
  assert.equal(SCOUT_CAR_RIG_SVG.trim(), scoutCarFixtureText);
  assert.equal(COMMAND_CAR_RIG_SVG.trim(), commandCarFixtureText);
  assert.equal(EKAT_RIG_SVG.trim(), ekatFixtureText);
  assert.equal(TANK_RIG_SVG.trim(), tankFixtureText);
  const definitions = createLiveRigDefinitions();
  assert.equal(definitions.has(KIND.ANTI_TANK_GUN), true);
  assert.equal(definitions.get(KIND.ANTI_TANK_GUN).id, "anti-tank-gun.authored");
  assert.equal(definitions.has(KIND.ARTILLERY), true);
  assert.equal(definitions.get(KIND.ARTILLERY).id, "artillery.authored");
  assert.equal(definitions.has(KIND.WORKER), true);
  assert.equal(definitions.get(KIND.WORKER).id, "worker.authored");
  assert.equal(definitions.has(KIND.GOLEM), true);
  assert.equal(definitions.get(KIND.GOLEM).id, "golem.authored");
  assert.equal(definitions.has(KIND.RIFLEMAN), true);
  assert.equal(definitions.get(KIND.RIFLEMAN).id, "rifleman.authored");
  assert.equal(definitions.has(KIND.MACHINE_GUNNER), true);
  assert.equal(definitions.get(KIND.MACHINE_GUNNER).id, "machine-gunner.authored");
  assert.equal(definitions.has(KIND.MORTAR_TEAM), true);
  assert.equal(definitions.get(KIND.MORTAR_TEAM).id, "mortar-team.authored");
  const loadedRiflemanKey = liveRigKeyForEntity({ kind: KIND.RIFLEMAN, panzerfaustLoaded: true });
  assert.equal(definitions.has(loadedRiflemanKey), true);
  assert.equal(definitions.get(loadedRiflemanKey).id, "rifleman.panzerfaust-loaded.authored");
  assert.equal(definitions.has(KIND.SCOUT_CAR), true);
  assert.equal(definitions.get(KIND.SCOUT_CAR).id, "scout-car.authored");
  assert.equal(definitions.has(KIND.COMMAND_CAR), true);
  assert.equal(definitions.get(KIND.COMMAND_CAR).id, "command-car.authored");
  assert.equal(definitions.has(KIND.EKAT), true);
  assert.equal(definitions.get(KIND.EKAT).id, "ekat.authored");
  assert.equal(definitions.has(KIND.TANK), true);
  assert.equal(definitions.get(KIND.TANK).id, "tank.authored");
});

test("live rig routes expose kind-specific production part groups", () => {
  const antiTankGunRoutes = liveRigRoutesFor(KIND.ANTI_TANK_GUN);
  assert.deepEqual(antiTankGunRoutes[0].parts, ["part.shadow"]);
  assert.equal(antiTankGunRoutes[1].parts.includes("part.at.barrel.packed"), true);
  assert.equal(antiTankGunRoutes[1].parts.includes("part.at.trail.left.deployed"), true);

  const artilleryRoutes = liveRigRoutesFor(KIND.ARTILLERY);
  assert.deepEqual(artilleryRoutes[0].parts, ["part.shadow"]);
  assert.equal(artilleryRoutes[1].parts.includes("part.art.barrel.packed"), true);
  assert.equal(artilleryRoutes[1].parts.includes("part.art.flashCore"), true);

  const riflemanRoutes = liveRigRoutesFor(KIND.RIFLEMAN);
  assert.deepEqual(riflemanRoutes[0].parts, ["part.shadow"]);
  assert.equal(riflemanRoutes[1].parts.includes("part.body"), true);
  assert.equal(riflemanRoutes[1].parts.includes("part.rifle.barrel"), true);

  const panzerfaustRoutes = liveRigRoutesFor(
    liveRigKeyForEntity({ kind: KIND.RIFLEMAN, panzerfaustLoaded: true }),
  );
  assert.deepEqual(panzerfaustRoutes[0].parts, ["part.shadow"]);
  assert.equal(panzerfaustRoutes[1].parts.includes("part.body"), true);
  assert.equal(panzerfaustRoutes[1].parts.includes("part.pzf.tube"), true);
  assert.equal(panzerfaustRoutes[1].parts.includes("part.pzf.warhead"), true);
  assert.equal(panzerfaustRoutes[1].parts.includes("part.rifle.barrel"), false);

  const machineGunnerRoutes = liveRigRoutesFor(KIND.MACHINE_GUNNER);
  assert.deepEqual(machineGunnerRoutes[0].parts, ["part.shadow"]);
  assert.equal(machineGunnerRoutes[1].parts.includes("part.mg.receiver"), true);
  assert.equal(machineGunnerRoutes[1].parts.includes("part.mg.bipod"), true);

  const mortarTeamRoutes = liveRigRoutesFor(KIND.MORTAR_TEAM);
  assert.deepEqual(mortarTeamRoutes[0].parts, ["part.shadow"]);
  assert.equal(mortarTeamRoutes[1].parts.includes("part.mortar.tube.packed"), true);
  assert.equal(mortarTeamRoutes[1].parts.includes("part.mortar.leg.left.deployed"), true);

  const scoutCarRoutes = liveRigRoutesFor(KIND.SCOUT_CAR);
  assert.deepEqual(scoutCarRoutes[0].parts, ["part.shadow"]);
  assert.equal(scoutCarRoutes[1].parts.includes("part.gunnerBarrel"), true);
  assert.equal(scoutCarRoutes[1].parts.includes("part.noseTick"), true);

  const commandCarRoutes = liveRigRoutesFor(KIND.COMMAND_CAR);
  assert.deepEqual(commandCarRoutes[0].parts, ["part.shadow"]);
  assert.equal(commandCarRoutes[1].parts.includes("part.badge.top"), true);
  assert.equal(commandCarRoutes[1].parts.includes("part.breakthroughAura"), true);

  const ekatRoutes = liveRigRoutesFor(KIND.EKAT);
  assert.deepEqual(ekatRoutes[0].parts, ["part.shadow"]);
  assert.equal(ekatRoutes[1].parts.includes("part.dress.core"), true);
  assert.equal(ekatRoutes[1].parts.includes("part.staff"), true);
  assert.equal(ekatRoutes[1].parts.includes("part.orb"), true);

  const workerRoutes = liveRigRoutesFor(KIND.WORKER);
  assert.deepEqual(workerRoutes[0].parts, ["part.shadow"]);
  assert.deepEqual(workerRoutes[1].parts, ["part.body", "part.busyIndicator", "part.facingTick"]);

  const golemRoutes = liveRigRoutesFor(KIND.GOLEM);
  assert.deepEqual(golemRoutes[0].parts, ["part.shadow"]);
  assert.deepEqual(golemRoutes[1].parts, ["part.body", "part.busyIndicator", "part.facingTick"]);

  const tankRoutes = liveRigRoutesFor(KIND.TANK);
  assert.deepEqual(tankRoutes[0].parts, ["part.shadow"]);
  assert.equal(tankRoutes[1].parts.includes("part.track.left"), true);
  assert.equal(tankRoutes[1].parts.includes("part.turret"), true);
  assert.equal(tankRoutes[1].parts.includes("part.barrel"), true);
  assert.equal(tankRoutes[1].parts.includes("part.coaxBarrel"), true);
  assert.equal(tankRoutes[1].parts.includes("part.fuelCue.box"), true);
  assert.equal(tankRoutes.length, 3);
  assert.deepEqual(tankRoutes[2].parts, ["part.tank.flashCone", "part.tank.flashCore", "part.tank.flashGlow"]);
});

test("default Worker draw uses live SVG rig without enabling comparison", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 4, kind: KIND.WORKER, owner: 1, x: 32, y: 44, facing: Math.PI / 2, state: STATE.IDLE };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.WORKER, definition]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { weaponRecoil: () => 0 });

  assert.equal(renderer._liveRigPools.liveUnitRigShadows.size, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 1);
  assert.equal(renderer.layers.unitShadows.children.length, 1);
  assert.equal(renderer.layers.units.children.length, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.get(entity.id).parts.get("part.shadow").display.visible, true);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.get(entity.id).parts.size, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigs.get(entity.id).parts.has("part.shadow"), false);
});

test("default Tank draw uses live SVG rig with separate turret and hull parts", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 40,
    kind: KIND.TANK,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: Math.PI / 2,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.TANK, definition]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 0 });

  assert.equal(renderer._liveRigPools.liveUnitRigShadows.size, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigOverlays.size, 0);
  assert.equal(renderer._liveRigPools.liveUnitRigEffects.size, 1);
  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  const effects = renderer._liveRigPools.liveUnitRigEffects.get(entity.id);
  assert.equal(unit.parts.get("part.hull").display.rotation, 0);
  assert.equal(unit.parts.get("part.turret").display.rotation, Math.PI / 2);
  assert.equal(unit.parts.get("part.barrel").display.rotation, Math.PI / 2);
  assert.equal(effects.parts.get("part.tank.flashCore").display.visible, true);
  assert.equal(effects.parts.get("part.tank.flashCore").display.alpha, 0);
  assert.equal(unit.parts.has("part.shadow"), false);
  assert.equal(unit.parts.get("part.fuelCue.box").display.visible, false);
});

test("visual unit override draws a real Tank through candidate SVG art without changing kind", () => {
  const defaultDefinition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const compiled = compileVisualUnitRigCandidates();
  const candidate = compiled.definitions.get("tank-long-cannon");
  assert.ok(candidate, JSON.stringify([...compiled.errors.entries()]));
  const entity = {
    id: 43,
    kind: KIND.TANK,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: Math.PI / 2,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.TANK, defaultDefinition]]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.TANK, { ...TANK_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.TANK, fakeAtlasTexture()]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), {
    playerId: 1,
    selection: new Set([entity.id]),
    resources: { oil: 10 },
    weaponRecoil: () => 0,
  }, {
    visualOverride: {
      candidateId: "tank-long-cannon",
      kind: KIND.TANK,
      definition: candidate.definition,
    },
  });

  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  const effects = renderer._liveRigPools.liveUnitRigEffects.get(entity.id);
  assert.equal(entity.kind, KIND.TANK, "override rendering keeps the authoritative unit kind");
  assert.equal(unit.definition.id, "tank-long-cannon");
  assert.equal(typeof unit.matchesPngAtlasRig, "undefined", "visual overrides bypass the production Tank PNG atlas");
  assert.equal(effects.definition.id, "tank-long-cannon", "override effects use the same candidate definition");
});

test("tank PNG atlas route splits omitted shadow and fuel cue back to SVG", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const [shadowRoute, unitRoute, effectRoute] = liveRigRoutesFor(KIND.TANK);
  assert.equal(pngAtlasCanRenderRoute(definition, TANK_PNG_RIG_ATLAS, shadowRoute), false);
  assert.equal(pngAtlasCanRenderRoute(definition, TANK_PNG_RIG_ATLAS, unitRoute), false);
  assert.equal(pngAtlasCanRenderRoute(definition, TANK_PNG_RIG_ATLAS, effectRoute), false);
  const unitCoverage = pngAtlasRouteCoverage(definition, TANK_PNG_RIG_ATLAS, unitRoute);
  assert.equal(unitCoverage.coveredParts.includes("part.hull"), true);
  assert.equal(unitCoverage.coveredParts.includes("part.turret"), true);
  assert.deepEqual(unitCoverage.missingParts, ["part.fuelCue.box", "part.fuelCue.x1", "part.fuelCue.x2"]);
  assert.equal(TANK_PNG_RIG_ATLAS.grid?.normalization?.worldScale, 1.2);
  assertAtlasSpriteUsesWorldScale(definition, TANK_PNG_RIG_ATLAS, "sprite.hull");
  assertAtlasSpriteUsesWorldScale(definition, TANK_PNG_RIG_ATLAS, "sprite.turret");
  assertAtlasSpriteUsesWorldScale(definition, TANK_PNG_RIG_ATLAS, "sprite.barrel");
  assert.equal(TANK_PNG_RIG_ATLAS.grid?.semanticPaintTintSlot, undefined);
  assert.equal(TANK_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.hull")?.tintSlot, "team");
  assert.equal(TANK_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.turret")?.tintSlot, "team-light");
  assert.equal(TANK_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.barrel")?.tintSlot, "team");
  const turretSprite = TANK_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.turret");
  assert.equal(turretSprite?.originMode, "visible-center");
  assert.ok(Math.abs(turretSprite.frame.originX - turretSprite.frame.w * 0.5) < 0.001);
  assert.ok(Math.abs(turretSprite.frame.originY - turretSprite.frame.h * 0.5) < 0.001);

  const entity = {
    id: 41,
    kind: KIND.TANK,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: Math.PI / 2,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  let contextCallCount = 0;
  renderer._liveRigDefinitionsByKind = new Map([[KIND.TANK, definition]]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.TANK, { ...TANK_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.TANK, fakeAtlasTexture()]]);
  renderer._rigRenderContextFor = function(entityArg, colorByOwnerArg, stateArg) {
    contextCallCount += 1;
    return _rigRenderContextFor.call(this, entityArg, colorByOwnerArg, stateArg);
  };

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 0 });

  const shadow = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  const overlay = renderer._liveRigPools.liveUnitRigOverlays.get(entity.id);
  const effects = renderer._liveRigPools.liveUnitRigEffects.get(entity.id);
  assert.equal(contextCallCount, 1);
  assert.equal(typeof shadow.matches, "function");
  assert.deepEqual([...shadow.parts.keys()], ["part.shadow"]);
  assert.equal(shadow.parts.get("part.shadow").display.visible, true);
  assert.equal(typeof unit.matchesPngAtlasRig, "function");
  assert.equal(typeof overlay.matches, "function");
  assert.equal(typeof effects.matches, "function");
  assert.deepEqual([...overlay.parts.keys()].sort(), ["part.fuelCue.box", "part.fuelCue.x1", "part.fuelCue.x2"]);
  assert.deepEqual([...effects.parts.keys()].sort(), ["part.tank.flashCone", "part.tank.flashCore", "part.tank.flashGlow"]);
  assert.equal(unit.parts.has("sprite.hull"), true);
  assert.equal(unit.parts.has("sprite.turret"), true);
  assert.equal(unit.parts.has("sprite.barrel"), true);
  assert.equal(unit.parts.has("sprite.fuelCue"), false);
  assert.equal(overlay.parts.get("part.fuelCue.box").display.visible, false);
  assert.equal(effects.parts.get("part.tank.flashCore").display.alpha, 0);
  assert.equal(unit.parts.get("sprite.turret").display.rotation, Math.PI / 2);
  assert.equal(unit.parts.get("sprite.barrel").display.rotation, Math.PI / 2);
});

test("PNG route coverage keeps mutable and Set part selections independent", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const mutableParts = ["part.shadow"];
  const shadowCoverage = pngAtlasRouteCoverage(definition, TANK_PNG_RIG_ATLAS, {
    parts: mutableParts,
  });
  assert.deepEqual(shadowCoverage.coveredParts, []);
  assert.deepEqual(shadowCoverage.missingParts, ["part.shadow"]);

  mutableParts[0] = "part.hull";
  const hullCoverage = pngAtlasRouteCoverage(definition, TANK_PNG_RIG_ATLAS, {
    parts: mutableParts,
  });
  assert.deepEqual(hullCoverage.coveredParts, ["part.hull"]);
  assert.deepEqual(hullCoverage.missingParts, []);

  const turretCoverage = pngAtlasRouteCoverage(definition, TANK_PNG_RIG_ATLAS, {
    parts: new Set(["part.turret"]),
  });
  assert.deepEqual(turretCoverage.coveredParts, ["part.turret"]);
  assert.deepEqual(turretCoverage.missingParts, []);
});

test("tank PNG atlas keeps cannon recoil on the generated barrel sprite", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 43,
    kind: KIND.TANK,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: 0,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.TANK, definition]]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.TANK, { ...TANK_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.TANK, fakeAtlasTexture()]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 0 });
  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  const barrelScaleStill = unit.parts.get("sprite.barrel").display.scaleX;
  const turretScaleStill = unit.parts.get("sprite.turret").display.scaleX;

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 1 });
  const barrelScaleRecoiling = unit.parts.get("sprite.barrel").display.scaleX;
  const turretScaleRecoiling = unit.parts.get("sprite.turret").display.scaleX;

  assert.equal(barrelScaleRecoiling < barrelScaleStill, true);
  assert.equal(turretScaleRecoiling, turretScaleStill);
});

test("anti-tank gun recoil moves the barrel much farther than the carriage", () => {
  const result = compileSvgRig(ANTI_TANK_GUN_RIG_SVG, { expectedKind: KIND.ANTI_TANK_GUN });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  const definition = result.definition;
  const entity = {
    id: 44,
    kind: KIND.ANTI_TANK_GUN,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: 0,
    setupState: SETUP.DEPLOYED,
    state: STATE.ATTACK,
  };
  const sampled = sampleRigAnimation(definition, entity, createRigRenderContext(entity, {
    now: fixedNow,
    setupVisual: { prongFactor: 1, barrel: true },
    state: { weaponRecoil: () => 1 },
  }));

  const carriage = sampled.parts["part.at.axle.deployed"];
  const barrel = sampled.parts["part.at.barrel.deployed"];
  assert.ok(carriage, "deployed carriage part should exist");
  assert.ok(barrel, "deployed barrel part should exist");
  assert.ok(Math.abs(carriage.transform.x + 3.12) < 0.001);
  assert.ok(Math.abs(carriage.transform.y) < 0.001);
  assert.ok(Math.abs(barrel.transform.x + 26) < 0.001);
  assert.ok(Math.abs(barrel.transform.y) < 0.001);
  assert.ok(Math.abs(barrel.transform.x) > Math.abs(carriage.transform.x) * 8);
});

test("anti-tank gun PNG atlas covers the unit route and keeps barrel recoil split", () => {
  const result = compileSvgRig(ANTI_TANK_GUN_RIG_SVG, { expectedKind: KIND.ANTI_TANK_GUN });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  const definition = result.definition;
  const [, unitRoute] = liveRigRoutesFor(KIND.ANTI_TANK_GUN);
  const coverage = pngAtlasRouteCoverage(definition, ANTI_TANK_GUN_PNG_RIG_ATLAS, unitRoute);
  assert.deepEqual(coverage.missingParts, []);
  assert.equal(pngAtlasCanRenderRoute(definition, ANTI_TANK_GUN_PNG_RIG_ATLAS, unitRoute), true);
  const packedCarriageSprite = ANTI_TANK_GUN_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.at.carriage.packed");
  const deployedLeftTrailSprite = ANTI_TANK_GUN_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.at.leftTrail.deployed");
  const deployedRightTrailSprite = ANTI_TANK_GUN_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.at.rightTrail.deployed");
  assert.ok(packedCarriageSprite);
  assert.ok(deployedLeftTrailSprite);
  assert.ok(deployedRightTrailSprite);
  assert.ok(packedCarriageSprite.sourceParts.includes("part.at.trail.left.packed"));
  assert.ok(packedCarriageSprite.sourceParts.includes("part.at.trail.right.packed"));
  assert.equal(deployedLeftTrailSprite.sourceParts.includes("part.at.trail.left.packed"), false);
  assert.equal(deployedRightTrailSprite.sourceParts.includes("part.at.trail.right.packed"), false);

  const entity = {
    id: 45,
    kind: KIND.ANTI_TANK_GUN,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: 0,
    setupState: SETUP.DEPLOYED,
    state: STATE.ATTACK,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.ANTI_TANK_GUN, definition]]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.ANTI_TANK_GUN, { ...ANTI_TANK_GUN_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.ANTI_TANK_GUN, fakeAtlasTexture()]]);
  renderer._deployedWeaponSetupVisual = () => ({ prongFactor: 1, frameProgress: 1, barrel: true });

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, weaponRecoil: () => 1 });

  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.equal(typeof unit.matchesPngAtlasRig, "function");
  const carriage = unit.parts.get("sprite.at.carriage.deployed").display;
  const barrel = unit.parts.get("sprite.at.barrelAssembly.deployed").display;
  const leftTrail = unit.parts.get("sprite.at.leftTrail.deployed").display;
  const rightTrail = unit.parts.get("sprite.at.rightTrail.deployed").display;
  assert.equal(carriage.visible, true);
  assert.equal(barrel.visible, true);
  assert.equal(leftTrail.visible, true);
  assert.equal(rightTrail.visible, true);
  assert.ok(Math.abs(carriage.x + 3.12) < 0.001);
  assert.ok(Math.abs(barrel.x + 26) < 0.001);
  assert.notEqual(carriage.tint, 0xffffff);
  assert.notEqual(barrel.tint, 0xffffff);
  assert.notEqual(carriage.tint, barrel.tint);
  assert.equal(Math.sign(leftTrail.scaleX), -1);
  assert.equal(Math.sign(leftTrail.scaleY), -1);
  assert.equal(Math.sign(rightTrail.scaleX), 1);
  assert.equal(Math.sign(rightTrail.scaleY), 1);

  renderer._deployedWeaponSetupVisual = () => ({ prongFactor: 0, frameProgress: 0, barrel: false });
  renderer._drawUnit({ ...entity, setupState: SETUP.PACKED, state: STATE.IDLE }, new Map([[1, 0x336699]]), {
    playerId: 1,
    weaponRecoil: () => 0,
  });
  const packedCarriage = unit.parts.get("sprite.at.carriage.packed").display;
  assert.equal(packedCarriage.visible, true);
  assert.equal(packedCarriage.alpha, 1);
  assert.equal(leftTrail.alpha, 0);
  assert.equal(rightTrail.alpha, 0);
});

test("mortar PNG atlas covers unit route with tinted carriage, tinted tube, and fixed tires", () => {
  const result = compileSvgRig(MORTAR_TEAM_RIG_SVG, { expectedKind: KIND.MORTAR_TEAM });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  const definition = result.definition;
  const [, unitRoute] = liveRigRoutesFor(KIND.MORTAR_TEAM);
  const coverage = pngAtlasRouteCoverage(definition, MORTAR_TEAM_PNG_RIG_ATLAS, unitRoute);
  assert.deepEqual(coverage.missingParts, []);
  assert.equal(pngAtlasCanRenderRoute(definition, MORTAR_TEAM_PNG_RIG_ATLAS, unitRoute), true);

  const packedCarriageSprite = MORTAR_TEAM_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.mortar.carriage.packed");
  const packedTubeSprite = MORTAR_TEAM_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.mortar.tube.packed");
  const packedLeftTireSprite = MORTAR_TEAM_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.mortar.tire.left.packed");
  const packedRightTireSprite = MORTAR_TEAM_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.mortar.tire.right.packed");
  for (const sprite of [packedCarriageSprite, packedTubeSprite, packedLeftTireSprite, packedRightTireSprite]) assert.ok(sprite);
  assert.deepEqual(
    [packedCarriageSprite, packedTubeSprite, packedLeftTireSprite, packedRightTireSprite].map((sprite) => sprite.tintSlot),
    ["team-light", "team-light", "fixed", "fixed"],
  );
  assert.ok(packedCarriageSprite.sourceParts.includes("part.mortar.body.packed"));
  assert.ok(packedCarriageSprite.sourceParts.includes("part.mortar.wheel.left.body.packed"));
  assert.ok(packedLeftTireSprite.sourceParts.includes("part.mortar.wheel.left.body.packed"));
  assert.equal(packedLeftTireSprite.sourceParts.includes("part.mortar.wheel.right.body.packed"), false);
  assert.ok(packedRightTireSprite.sourceParts.includes("part.mortar.wheel.right.body.packed"));
  assert.equal(packedRightTireSprite.sourceParts.includes("part.mortar.wheel.left.body.packed"), false);
  assert.ok(packedTubeSprite.sourceParts.includes("part.mortar.tube.packed"));
  assert.equal(packedTubeSprite.sourceParts.includes("part.mortar.body.packed"), false);

  const entity = {
    id: 46,
    kind: KIND.MORTAR_TEAM,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: 0,
    setupState: SETUP.DEPLOYED,
    state: STATE.ATTACK,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.MORTAR_TEAM, definition]]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.MORTAR_TEAM, { ...MORTAR_TEAM_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.MORTAR_TEAM, fakeAtlasTexture()]]);
  renderer._deployedWeaponSetupVisual = () => ({ prongFactor: 1, frameProgress: 1, barrel: true });

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, selection: new Set(), weaponRecoil: () => 1 });

  const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.equal(typeof unit.matchesPngAtlasRig, "function");
  const carriage = unit.parts.get("sprite.mortar.carriage.deployed").display;
  const tube = unit.parts.get("sprite.mortar.tube.deployed").display;
  const leftTire = unit.parts.get("sprite.mortar.tire.left.deployed").display;
  const rightTire = unit.parts.get("sprite.mortar.tire.right.deployed").display;
  for (const display of [carriage, tube, leftTire, rightTire]) {
    assert.equal(display.visible, true);
    assert.equal(display.alpha, 1);
  }
  assert.notEqual(carriage.tint, 0xffffff);
  assert.notEqual(tube.tint, 0xffffff);
  assert.equal(leftTire.tint, 0xffffff);
  assert.equal(rightTire.tint, 0xffffff);
  assert.ok(Math.abs(tube.x) > Math.abs(carriage.x) * 2);

  renderer._deployedWeaponSetupVisual = () => ({ prongFactor: 0, frameProgress: 0, barrel: false });
  renderer._drawUnit({ ...entity, setupState: SETUP.PACKED, state: STATE.IDLE }, new Map([[1, 0x336699]]), {
    playerId: 1,
    selection: new Set(),
    weaponRecoil: () => 0,
  });
  const packedCarriage = unit.parts.get("sprite.mortar.carriage.packed").display;
  const packedLeftTire = unit.parts.get("sprite.mortar.tire.left.packed").display;
  const packedRightTire = unit.parts.get("sprite.mortar.tire.right.packed").display;
  for (const display of [packedCarriage, packedLeftTire, packedRightTire]) {
    assert.equal(display.visible, true);
    assert.equal(display.alpha, 1);
  }
  for (const display of [carriage, tube, leftTire, rightTire]) assert.equal(display.alpha, 0);
});

test("rifleman PNG frame strip uses idle frame and movement cycle", () => {
  const definition = compileFixture("rig-rifleman.svg", KIND.RIFLEMAN);
  const strip = { ...RIFLEMAN_PNG_FRAME_STRIP, enabled: true };
  const entity = {
    id: 40,
    kind: KIND.RIFLEMAN,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0.25,
    weaponFacing: 1.5,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  let renderNow = 0;
  renderer._liveRigDefinitionsByKind = new Map([[KIND.RIFLEMAN, definition]]);
  renderer._liveFrameStripsByKind = new Map([[KIND.RIFLEMAN, strip]]);
  renderer._liveFrameStripTextures = new Map([[KIND.RIFLEMAN, fakeFrameStripTexture()]]);
  renderer._rigRenderContextFor = function(entityArg, colorByOwnerArg, stateArg) {
    return createRigRenderContext(entityArg, {
      state: stateArg,
      colorByOwner: colorByOwnerArg,
      now: renderNow,
      setupVisual: this._deployedWeaponSetupVisual(entityArg),
      selected: stateArg.selection?.has?.(entityArg.id) ?? false,
      map: this._map,
      occupiedTrench: Number.isFinite(entityArg.occupiedTrenchId),
    });
  };

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: {}, weaponRecoil: () => 0 });

  const shadow = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
  const stripInstance = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.equal(typeof stripInstance.matchesFrameStripUnit, "function");
  assert.equal(stripInstance.container, stripInstance.sprite);
  assert.equal(renderer.layers.units.children.includes(stripInstance.sprite), true);
  assert.equal(shadow.parts.get("part.shadow").display.visible, true);
  assert.equal(stripInstance.sprite.texture.frame.x, 0);
  assert.equal(stripInstance.container.rotation, entity.weaponFacing);
  assert.equal(stripInstance.sprite.scaleX, strip.worldScale);
  assert.equal(stripInstance.sprite.tint, 0x6194c7);

  entity.occupiedTrenchId = 80;
  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: {}, weaponRecoil: () => 0 });
  assert.equal(stripInstance.sprite.scaleX, 0.85 * strip.worldScale);
  delete entity.occupiedTrenchId;

  renderNow = 1000 / strip.fps;
  entity.state = STATE.MOVE;
  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: {}, weaponRecoil: () => 0 });

  const expectedFrame = frameStripFrameIndex(strip, entity, renderNow);
  assert.equal(expectedFrame, 2);
  assert.equal(stripInstance.sprite.texture.frame.x, strip.frameWidth * expectedFrame);
  assert.equal(frameStripVisualFacing(entity), entity.facing);
  assert.equal(stripInstance.container.rotation, entity.facing);
  assert.equal(stripInstance.parts, undefined);
});

test("machine gunner PNG frame strip maps setup progress to deploy frames", () => {
  const strip = { ...MACHINE_GUNNER_PNG_FRAME_STRIP, enabled: true };
  const entity = {
    id: 48,
    kind: KIND.MACHINE_GUNNER,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0.4,
    weaponFacing: Math.PI,
    setupState: SETUP.PACKED,
    state: STATE.IDLE,
  };

  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0 } }), strip.idleFrame);
  assert.equal(frameStripVisualFacing(strip, entity), entity.facing);
  assert.equal(frameStripWorldScale(strip, entity), strip.worldScale);

  entity.state = STATE.MOVE;
  assert.equal(frameStripFrameIndex(strip, entity, 0), strip.movementFrames[0]);
  assert.ok(Math.abs(frameStripVisualFacing(strip, entity) - (entity.facing + strip.movementFacingOffset)) < 0.001);
  assert.equal(frameStripWorldScale(strip, entity), strip.movementWorldScale);

  const idleOnlyStrip = { ...strip, movementFrames: [] };
  assert.equal(frameStripFrameIndex(idleOnlyStrip, entity, 0), idleOnlyStrip.idleFrame);
  assert.equal(frameStripVisualFacing(idleOnlyStrip, entity), entity.facing);
  assert.equal(frameStripWorldScale(idleOnlyStrip, entity), idleOnlyStrip.worldScale);

  entity.state = STATE.IDLE;
  entity.setupState = SETUP.SETTING_UP;
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0 } }), 6);
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0.5 } }), 9);
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0.999 } }), 11);
  assert.ok(Math.abs(frameStripVisualFacing(strip, entity) - (Math.PI / 2)) < 0.001);

  entity.setupState = SETUP.DEPLOYED;
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 1 } }), strip.deployedFrame);

  entity.setupState = SETUP.TEARING_DOWN;
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0.25 } }), 7);

  entity.state = STATE.MOVE;
  assert.equal(frameStripFrameIndex(strip, entity, { setupVisual: { frameProgress: 0.75 } }), 10);
  assert.ok(Math.abs(frameStripVisualFacing(strip, entity) - (Math.PI / 2)) < 0.001);
  assert.equal(frameStripWorldScale(strip, entity), strip.worldScale);

  entity.state = STATE.IDLE;
  entity.setupState = SETUP.DEPLOYED;
  entity.weaponFacing = undefined;
  entity.facing = Math.PI;
  assert.ok(Math.abs(frameStripVisualFacing(strip, entity) - (Math.PI / 2)) < 0.001);
});

test("frame-strip color profile applies shared and per-strip targets only when not already baked", () => {
  assert.equal(isNeutralFrameStripColorAdjustment(frameStripRuntimeColorAdjustment(RIFLEMAN_PNG_FRAME_STRIP)), true);
  assert.deepEqual(frameStripRuntimeColorAdjustment(MACHINE_GUNNER_PNG_FRAME_STRIP), {
    brightness: 145,
    saturation: FRAME_STRIP_TARGET_COLOR_ADJUSTMENT.saturation,
    hue: FRAME_STRIP_TARGET_COLOR_ADJUSTMENT.hue,
  });
  assert.deepEqual(frameStripRuntimeColorAdjustment({}), FRAME_STRIP_TARGET_COLOR_ADJUSTMENT);
});

test("frame-strip color adjustment brightens raw pixels without changing alpha", () => {
  const pixels = new Uint8ClampedArray([
    40, 50, 60, 255,
    200, 210, 220, 0,
  ]);
  applyFrameStripColorAdjustmentToRgba(pixels, FRAME_STRIP_TARGET_COLOR_ADJUSTMENT);
  assert.equal(pixels[3], 255);
  assert.equal(pixels[7], 0);
  assert.equal(pixels[0] > 40, true);
  assert.equal(pixels[1] > 50, true);
  assert.equal(pixels[2] > 60, true);
});

test("machine gunner PNG frame strip mirrors asset manifest runtime metadata", () => {
  const manifest = readMachineGunnerPngManifest();
  const runtime = manifest.runtime;
  const strip = MACHINE_GUNNER_PNG_FRAME_STRIP;

  assert.deepEqual(Object.keys(runtime).sort(), [
    "bakedColorAdjustment",
    "deployedFrame",
    "firingFrames",
    "fps",
    "frameCount",
    "frameHeight",
    "frameWidth",
    "idleFrame",
    "imageVersion",
    "module",
    "movementFacingOffsetRadians",
    "movementFrames",
    "movementWorldScale",
    "packedFacing",
    "runtimeStrip",
    "setupForwardAngleRadians",
    "setupFrames",
    "stripImageUrl",
    "targetColorAdjustment",
    "tintSlot",
    "worldScale",
  ].sort());
  assert.equal(runtime.module, "client/src/renderer/rigs/machine_gunner_png_strip.js");
  assert.equal(strip.enabled, manifest.enabled);
  assert.equal(strip.unit, manifest.unit);
  assert.equal(strip.image, runtime.stripImageUrl);
  assert.equal(strip.imageVersion, runtime.imageVersion);
  assert.equal(strip.frameWidth, runtime.frameWidth);
  assert.equal(strip.frameHeight, runtime.frameHeight);
  assert.equal(strip.frameCount, runtime.frameCount);
  assert.equal(strip.idleFrame, runtime.idleFrame);
  assert.deepEqual(strip.movementFrames, runtime.movementFrames);
  assert.deepEqual(strip.setupFrames, runtime.setupFrames);
  assert.equal(strip.deployedFrame, runtime.deployedFrame);
  assert.deepEqual(strip.firingFrames, runtime.firingFrames);
  assert.equal(strip.fps, runtime.fps);
  assert.equal(strip.worldScale, runtime.worldScale);
  assert.equal(strip.movementWorldScale, runtime.movementWorldScale);
  assert.equal(strip.movementFacingOffset, runtime.movementFacingOffsetRadians);
  assert.equal(strip.tintSlot, runtime.tintSlot);
  assert.deepEqual(strip.bakedColorAdjustment, runtime.bakedColorAdjustment);
  assert.deepEqual(strip.targetColorAdjustment, runtime.targetColorAdjustment);
  assert.equal(strip.packedFacing, runtime.packedFacing);
  assert.equal(strip.setupForwardAngle, runtime.setupForwardAngleRadians);
  assert.deepEqual(strip.source, {
    ...manifest.sourceSheets,
    runtimeStrip: runtime.runtimeStrip,
  });

  const runtimeStripSize = readPngDimensions(runtime.runtimeStrip);
  assert.equal(runtimeStripSize.width, runtime.frameWidth * runtime.frameCount);
  assert.equal(runtimeStripSize.height, runtime.frameHeight);

  const recoilStripSize = readPngDimensions(manifest.sourceSheets.fireRecoilStrip);
  assert.equal(recoilStripSize.width, runtime.frameWidth * runtime.firingFrames.length);
  assert.equal(recoilStripSize.height, runtime.frameHeight);
  assert.deepEqual(
    runtime.firingFrames,
    Array.from(
      { length: runtime.firingFrames.length },
      (_, index) => runtime.frameCount - runtime.firingFrames.length + index,
    ),
  );
});

test("tank PNG atlas SVG fallback is destroyed when same id no longer needs it", () => {
  const tankDefinition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const workerDefinition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = {
    id: 42,
    kind: KIND.TANK,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    weaponFacing: Math.PI / 2,
    state: STATE.IDLE,
  };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([
    [KIND.TANK, tankDefinition],
    [KIND.WORKER, workerDefinition],
  ]);
  renderer._livePngRigAtlasesByKind = new Map([[KIND.TANK, { ...TANK_PNG_RIG_ATLAS, enabled: true }]]);
  renderer._livePngRigAtlasTextures = new Map([[KIND.TANK, fakeAtlasTexture()]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 0 });

  const overlay = renderer._liveRigPools.liveUnitRigOverlays.get(entity.id);
  const effects = renderer._liveRigPools.liveUnitRigEffects.get(entity.id);
  assert.equal(typeof overlay.matches, "function");
  assert.equal(typeof effects.matches, "function");

  renderer._drawUnit({
    ...entity,
    kind: KIND.WORKER,
    weaponFacing: undefined,
  }, new Map([[1, 0x336699]]), { playerId: 1, resources: { oil: 10 }, weaponRecoil: () => 0 });

  const workerRig = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.equal(overlay._destroyed, true);
  assert.equal(effects._destroyed, true);
  assert.equal(renderer._liveRigPools.liveUnitRigOverlays.has(entity.id), false);
  assert.equal(renderer._liveRigPools.liveUnitRigEffects.has(entity.id), false);
  assert.equal(workerRig.kind, KIND.WORKER);
  assert.equal(workerRig.parts.has("part.body"), true);
});

test("live rig renderer rebuilds same-id Rifleman instances when Panzerfaust loadout changes", () => {
  const panzerfaust = compileSvgRig(LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG, { expectedKind: KIND.RIFLEMAN });
  const rifleman = compileSvgRig(RIFLEMAN_RIG_SVG, { expectedKind: KIND.RIFLEMAN });
  assert.equal(panzerfaust.ok, true, JSON.stringify(panzerfaust.errors));
  assert.equal(rifleman.ok, true, JSON.stringify(rifleman.errors));

  const renderer = makeRigRenderer();
  const loadedRiflemanKey = liveRigKeyForEntity({ kind: KIND.RIFLEMAN, panzerfaustLoaded: true });
  renderer._liveRigDefinitionsByKind = new Map([
    [loadedRiflemanKey, panzerfaust.definition],
    [KIND.RIFLEMAN, rifleman.definition],
  ]);
  const colorByOwner = new Map([[1, 0x336699]]);
  const state = { playerId: 1, selection: new Set(), weaponRecoil: () => 0 };
  const id = 92;

  renderer._drawUnit({
    id,
    kind: KIND.RIFLEMAN,
    panzerfaustLoaded: true,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    state: STATE.IDLE,
  }, colorByOwner, state);
  const panzerfaustRig = renderer._liveRigPools.liveUnitRigs.get(id);
  const panzerfaustContainer = panzerfaustRig.container;
  assert.equal(panzerfaustRig.kind, KIND.RIFLEMAN);
  assert.equal(panzerfaustRig.parts.has("part.pzf.tube"), true);
  assert.equal(renderer.layers.units.children.includes(panzerfaustContainer), true);

  renderer._drawUnit({
    id,
    kind: KIND.RIFLEMAN,
    panzerfaustLoaded: false,
    owner: 1,
    x: 32,
    y: 44,
    facing: 0,
    state: STATE.IDLE,
  }, colorByOwner, state);
  const riflemanRig = renderer._liveRigPools.liveUnitRigs.get(id);
  assert.equal(panzerfaustRig._destroyed, true);
  assert.equal(panzerfaustContainer.parent, null);
  assert.equal(renderer.layers.units.children.includes(panzerfaustContainer), false);
  assert.equal(riflemanRig.kind, KIND.RIFLEMAN);
  assert.equal(riflemanRig.parts.has("part.rifle.barrel"), true);
  assert.equal(riflemanRig.parts.has("part.pzf.tube"), false);
  assert.equal(renderer.layers.units.children.includes(riflemanRig.container), true);
  riflemanRig.destroy();
});

test("live rig renderer rebuilds instances after a mutable route changes", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 93, kind: KIND.WORKER, owner: 1, x: 32, y: 44, facing: 0 };
  const renderer = makeRigRenderer();
  const parts = ["part.shadow"];
  const options = {
    routes: [{ poolName: "liveUnitRigs", layerName: "units", parts }],
  };

  renderLiveUnitRig(renderer, entity, new Map([[1, 0x336699]]), {}, definition, options);
  parts[0] = "part.body";
  renderLiveUnitRig(renderer, entity, new Map([[1, 0x336699]]), {}, definition, options);
  const bodyRig = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.deepEqual([...bodyRig.parts.keys()], ["part.body"]);

  parts[0] = "part.facingTick";
  renderLiveUnitRig(renderer, entity, new Map([[1, 0x336699]]), {}, definition, options);
  const facingRig = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.equal(bodyRig._destroyed, true);
  assert.deepEqual([...facingRig.parts.keys()], ["part.facingTick"]);
  facingRig.destroy();
});

test("missing live rig definitions fail closed instead of drawing procedural fallback", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 5, kind: KIND.EKAT, owner: 1, x: 32, y: 44, facing: 0, state: STATE.IDLE };
  const renderer = makeRigRenderer();
  renderer._liveRigDefinitionsByKind = new Map([[KIND.WORKER, definition]]);

  assert.throws(
    () => renderer._drawUnit(entity, new Map([[1, 0x336699]]), { weaponRecoil: () => 0 }),
    /missing live SVG rig definition/,
  );
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 0);
});

test("live rig instances are destroyed through renderer-style sweep", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 6, kind: KIND.WORKER, owner: 1, x: 10, y: 12, facing: 0, state: STATE.IDLE };
  const renderer = makeRigRenderer();
  renderLiveUnitRig(renderer, entity, new Map([[1, 0x112233]]), {}, definition, {
    routes: [
      { poolName: "liveUnitRigShadows", layerName: "unitShadows", parts: ["part.shadow"] },
      { poolName: "liveUnitRigs", layerName: "units", parts: ["part.body", "part.busyIndicator", "part.facingTick"] },
    ],
  });
  const shadowInstance = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
  const unitInstance = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  assert.deepEqual([...shadowInstance.parts.keys()], ["part.shadow"]);
  assert.deepEqual([...unitInstance.parts.keys()].sort(), ["part.body", "part.busyIndicator", "part.facingTick"]);
  for (const seen of Object.values(renderer._seen)) seen.clear();
  renderer._unseen = new Map([[entity.id, 999]]);
  _sweep.call(renderer);
  assert.equal(shadowInstance._destroyed, true);
  assert.equal(unitInstance._destroyed, true);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.size, 0);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 0);
});

}

function compileFixture(file, expectedKind) {
  const result = compileSvgRig(fs.readFileSync(path.join(fixturesDir, file), "utf8"), { expectedKind });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  return result.definition;
}

function readMachineGunnerPngManifest() {
  return JSON.parse(fs.readFileSync(machineGunnerPngManifestPath, "utf8"));
}

function readPngDimensions(repoRelativePath) {
  const buffer = fs.readFileSync(path.join(repoRoot, repoRelativePath));
  assert.equal(buffer.toString("hex", 0, 8), "89504e470d0a1a0a");
  assert.equal(buffer.toString("ascii", 12, 16), "IHDR");
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

function makeRigRenderer() {
  return {
    _liveRigDefinitionsByKind: new Map(),
    _liveFrameStripsByKind: new Map(),
    _liveFrameStripTextures: new Map(),
    _liveRigPools: {
      liveUnitRigShadows: new Map(),
      liveUnitRigs: new Map(),
      liveUnitRigOverlays: new Map(),
      liveUnitRigEffects: new Map(),
      liveShotRevealRigShadows: new Map(),
      liveShotRevealRigs: new Map(),
      liveShotRevealRigOverlays: new Map(),
      liveShotRevealRigEffects: new Map(),
    },
    _liveRigRoutes: {
      liveUnitRigShadows: { poolName: "liveUnitRigShadows", layerName: "unitShadows" },
      liveUnitRigs: { poolName: "liveUnitRigs", layerName: "units" },
      liveUnitRigOverlays: { poolName: "liveUnitRigOverlays", layerName: "units" },
      liveUnitRigEffects: { poolName: "liveUnitRigEffects", layerName: "units" },
      liveShotRevealRigShadows: { poolName: "liveShotRevealRigShadows", layerName: "shotRevealShadows" },
      liveShotRevealRigs: { poolName: "liveShotRevealRigs", layerName: "shotReveals" },
      liveShotRevealRigOverlays: { poolName: "liveShotRevealRigOverlays", layerName: "shotReveals" },
      liveShotRevealRigEffects: { poolName: "liveShotRevealRigEffects", layerName: "shotReveals" },
    },
    _rigPixiFactory: createInspectionPngPixiFactory(),
    _pools: { unitShadows: new Map(), units: new Map(), shotRevealShadows: new Map(), shotReveals: new Map() },
    _seen: {
      unitShadows: new Set(),
      units: new Set(),
      shotRevealShadows: new Set(),
      shotReveals: new Set(),
      liveUnitRigShadows: new Set(),
      liveUnitRigs: new Set(),
      liveUnitRigOverlays: new Set(),
      liveUnitRigEffects: new Set(),
      liveShotRevealRigShadows: new Set(),
      liveShotRevealRigs: new Set(),
      liveShotRevealRigOverlays: new Set(),
      liveShotRevealRigEffects: new Set(),
    },
    layers: {
      unitShadows: new FakeContainer(),
      units: new FakeContainer(),
      shotRevealShadows: new FakeContainer(),
      shotReveals: new FakeContainer(),
    },
    _drawUnit(_entity, _colorByOwner, _state, pools = {}) {
      return _drawUnit.call(this, _entity, _colorByOwner, _state, pools);
    },
    _slot(poolName, id) {
      const pool = this._pools[poolName];
      let graphic = pool.get(id);
      if (!graphic) {
        graphic = new FakeGraphics();
        pool.set(id, graphic);
        this.layers[poolName].addChild(graphic);
      }
      this._seen[poolName].add(id);
      graphic.visible = true;
      graphic.alpha = 1;
      graphic.clear();
      return graphic;
    },
    _shadow(g, cx, cy, radius) {
      g.beginFill(0x000000, 0.28);
      g.drawEllipse(cx, cy + radius * 0.35, radius, radius * 0.6);
      g.endFill();
    },
    _vehicleShadow() {
      throw new Error("worker comparison test should not draw vehicle shadow");
    },
    _tintFor(owner, colorByOwner) {
      return colorByOwner.get(owner) ?? 0x9aa0a8;
    },
    _rigRenderContextFor(entity, colorByOwner, state) {
      return _rigRenderContextFor.call(this, entity, colorByOwner, state);
    },
    _deployedWeaponSetupVisual: () => ({ prongFactor: 0, barrel: false }),
    _tankMotionVisual: () => ({ activity: 0 }),
    _map: { tileSize: 32 },
  };
}

function fakeAtlasTexture() {
  return { baseTexture: { id: "fake-tank-atlas" } };
}

function fakeFrameStripTexture() {
  return { baseTexture: { id: "fake-rifleman-strip" } };
}

function assertAtlasSpriteUsesWorldScale(definition, atlas, spriteId) {
  const worldScale = atlas.grid?.normalization?.worldScale;
  assert.equal(typeof worldScale, "number");
  const sprite = atlas.sprites.find((candidate) => candidate.id === spriteId);
  assert.ok(sprite, `${spriteId} should exist`);
  const visibleBounds = sprite.frame?.visibleBounds;
  assert.ok(visibleBounds, `${spriteId} should have normalized visible bounds`);
  const sourceBounds = unionPartBounds(definition, sprite.sourceParts);
  const expectedPixelsPerUnitX = (visibleBounds.w / Math.max(1, sourceBounds.maxX - sourceBounds.minX)) / worldScale;
  const expectedPixelsPerUnitY = (visibleBounds.h / Math.max(1, sourceBounds.maxY - sourceBounds.minY)) / worldScale;
  assertAlmostEqual(sprite.frame.pixelsPerUnitX, expectedPixelsPerUnitX, `${spriteId} pixelsPerUnitX`);
  assertAlmostEqual(sprite.frame.pixelsPerUnitY, expectedPixelsPerUnitY, `${spriteId} pixelsPerUnitY`);
}

function unionPartBounds(definition, partIds) {
  const bounds = partIds
    .map((partId) => definition.parts.find((part) => part.id === partId))
    .filter(Boolean)
    .map(partBounds);
  assert.ok(bounds.length > 0, "sprite should reference at least one source part");
  return {
    minX: Math.min(...bounds.map((bound) => bound.minX)),
    minY: Math.min(...bounds.map((bound) => bound.minY)),
    maxX: Math.max(...bounds.map((bound) => bound.maxX)),
    maxY: Math.max(...bounds.map((bound) => bound.maxY)),
  };
}

function partBounds(part) {
  const geometry = part?.geometry || {};
  const points = [];
  if (geometry.type === "rect") {
    points.push([geometry.x, geometry.y], [geometry.x + geometry.width, geometry.y + geometry.height]);
  } else if (geometry.type === "line") {
    points.push([geometry.from.x, geometry.from.y], [geometry.to.x, geometry.to.y]);
  } else if (geometry.type === "polygon" || geometry.type === "polyline") {
    for (const point of geometry.points || []) points.push([point.x, point.y]);
  } else if (geometry.type === "circle") {
    points.push([geometry.cx - geometry.r, geometry.cy - geometry.r], [geometry.cx + geometry.r, geometry.cy + geometry.r]);
  } else if (geometry.type === "ellipse") {
    points.push([geometry.cx - geometry.rx, geometry.cy - geometry.ry], [geometry.cx + geometry.rx, geometry.cy + geometry.ry]);
  }
  assert.ok(points.length > 0, `${part?.id || "part"} should have measurable geometry`);
  const strokePad = Math.max(part?.paint?.strokeWidth || 0, geometry.strokeWidth || 0, 1) * 0.5 + 0.5;
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  return {
    minX: Math.min(...xs) - strokePad,
    minY: Math.min(...ys) - strokePad,
    maxX: Math.max(...xs) + strokePad,
    maxY: Math.max(...ys) + strokePad,
  };
}

function assertAlmostEqual(actual, expected, label, epsilon = 0.000001) {
  assert.ok(
    Math.abs(actual - expected) <= epsilon,
    `${label}: expected ${expected}, got ${actual}`,
  );
}

function test(name, fn) {
  try {
    fn();
  } catch (err) {
    console.error(`not ok - ${name}`);
    throw err;
  }
  console.log(`ok - ${name}`);
}

main();
