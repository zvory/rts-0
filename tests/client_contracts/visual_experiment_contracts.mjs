// tests/client_contracts/visual_experiment_contracts.mjs
// Contracts for local lab visual experimentation renderer-only samples.

import fs from "node:fs";
import { assert, assertApprox } from "./assertions.mjs";
import { COLORS, PLAYER_PALETTE } from "../../client/src/config.js";
import { buildFrameEntityViews } from "../../client/src/frame_entity_views.js";
import { KIND, STATE } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { createLivePngRigAtlases } from "../../client/src/renderer/rigs/png_routing.js";
import { pngAtlasRouteCoverage } from "../../client/src/renderer/rigs/png_runtime.js";
import {
  LOADED_RIFLEMAN_RIG_KEY,
  createLiveRigDefinitions,
  liveRigKeyForEntity,
  liveRigDefinitionFor,
  liveRigRoutesFor,
} from "../../client/src/renderer/rigs/live_routing.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";
import {
  compileVisualUnitRigCandidates,
  visualUnitRigCandidateIds,
} from "../../client/src/renderer/rigs/visual_override_rigs.js";
import { normalizeStaticVisualSamples } from "../../client/src/renderer/visual_samples.js";
import { resolveVisualUnitOverrides } from "../../client/src/renderer/visual_unit_overrides.js";
import { getVisualProfile } from "../../client/src/visual_profiles.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const NOOP_RENDERER_OVERLAYS = [
  "_drawAbilityObjects",
  "_drawSmokes",
  "_drawFog",
  "_drawSmokeCanisters",
  "_drawCommandFeedback",
  "_drawAttackTargetPreview",
  "_drawMortarTargets",
  "_drawMortarLaunches",
  "_drawMortarShells",
  "_drawMortarImpacts",
  "_drawArtilleryLaunches",
  "_drawArtilleryTargets",
  "_drawArtilleryImpacts",
  "_drawPanzerfaustShots",
  "_drawPanzerfaustImpacts",
  "_drawSelectedUnitRanges",
  "_drawSelectedMortarRanges",
  "_drawBreakthroughAuras",
  "_drawAbilityTargetPreview",
  "_drawAntiTankGunSetupPreview",
  "_drawOrderPlan",
  "_drawDebugPathOverlay",
  "_drawRallyPoints",
  "_drawResourceMiningPreview",
  "_drawMuzzleFlashes",
  "_drawPlacement",
];

const TEST_WORKER_SVG = fs.readFileSync(
  new URL("../fixtures/svg/rig-worker.svg", import.meta.url),
  "utf8",
);
const TEST_WORKER_CANDIDATES = Object.freeze([
  Object.freeze({ id: "worker-candidate-a", label: "Worker A", kind: KIND.WORKER, svgText: TEST_WORKER_SVG }),
  Object.freeze({ id: "worker-candidate-b", label: "Worker B", kind: KIND.WORKER, svgText: TEST_WORKER_SVG }),
  Object.freeze({ id: "worker-candidate-c", label: "Worker C", kind: KIND.WORKER, svgText: TEST_WORKER_SVG }),
]);
const TEST_WORKER_OVERRIDES = Object.freeze([
  Object.freeze({ id: "worker-by-entity", candidateId: "worker-candidate-a", selector: Object.freeze({ entityId: 126 }) }),
  Object.freeze({ id: "worker-by-ordinal", candidateId: "worker-candidate-b", selector: Object.freeze({ kind: KIND.WORKER, owner: 1, ordinal: 2 }) }),
  Object.freeze({ id: "worker-by-nearest", candidateId: "worker-candidate-c", selector: Object.freeze({ kind: KIND.WORKER, owner: 1, nearest: Object.freeze({ x: 1884, y: 2032 }), maxDistance: 64 }) }),
]);

{
  const profile = getVisualProfile("trench-variants-1");
  assert(profile, "trench visual profile is registered");
  assert(
    profile.staticSamples.length >= 5,
    "trench visual profile compares several checked-in static samples",
  );
  assert(
    profile.staticSamples.every((sample) =>
      sample.kind === "trench" &&
      typeof sample.id === "string" &&
      typeof sample.variant === "string" &&
      Number.isFinite(sample.x) &&
      Number.isFinite(sample.y)),
    "trench visual samples are renderer-only checked-in descriptors",
  );
  assert(
    profile.staticSamples.some((sample) => sample.occupied === true),
    "trench visual profile includes occupied-lip samples for comparison",
  );
}

{
  const profile = getVisualProfile("terrain-blend-showcase");
  assert(profile?.terrainPreviewReveal === true, "terrain showcase requests a fog-free Lab-only review");
  assert(
    profile.initialCamera?.focus?.x === 2016 &&
      profile.initialCamera?.focus?.y === 2016 &&
      profile.initialCamera?.framingScale === 0.58,
    "terrain showcase uses the 1v1 terrain-matrix framing",
  );
}

{
  assert(getVisualProfile("unit-rig-overrides-1") === null, "retired Tank SVG override profile is no longer registered");
  assert(visualUnitRigCandidateIds().length === 0, "raster units do not retain checked-in SVG candidates");
  const unitOverrides = TEST_WORKER_OVERRIDES;
  assert(unitOverrides.length >= 3, "test-only unit overrides compare multiple same-kind units");
  assert(
    unitOverrides.some((rule) => rule.selector?.entityId === 126) &&
      unitOverrides.some((rule) => rule.selector?.kind === KIND.WORKER && rule.selector?.ordinal === 2) &&
      unitOverrides.some((rule) => rule.selector?.kind === KIND.WORKER && rule.selector?.nearest),
    "unit override profile covers entity id, kind ordinal, and kind nearest selector forms",
  );
}

{
  const profile = getVisualProfile("rifleman-recoil-strip-1");
  assert(profile, "rifleman recoil frame-strip visual profile is registered");
  assert(profile.frameStripOverrides.length === 1, "rifleman recoil profile has one frame-strip override");
  const override = profile.frameStripOverrides[0];
  assert(override.kind === KIND.RIFLEMAN, "rifleman recoil profile targets Rifleman units");
  assert(
    override.strip.image.includes("/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-recoil-strip.png"),
    "rifleman recoil profile uses the generated recoil strip asset",
  );
  assert(
    override.strip.frameWidth === 96 &&
      override.strip.frameHeight === 96 &&
      override.strip.frameCount === 7,
    "rifleman recoil strip exposes the seven-cell Rifleman atlas geometry",
  );
}

{
  const profile = getVisualProfile("rifleman-panzerfaust-composite-1");
  assert(profile, "composited Panzerfaust Rifleman visual profile is registered");
  assert(profile.frameStripOverrides.length === 1, "composited Panzerfaust Rifleman profile has one frame-strip override");
  const override = profile.frameStripOverrides[0];
  const strip = override.strip;
  assert(override.kind === KIND.PANZERFAUST, "composited Panzerfaust profile targets Panzerfaust units");
  assert(
    override.rigKey === LOADED_RIFLEMAN_RIG_KEY,
    "composited Panzerfaust profile targets only units whose disposable launcher is still loaded",
  );
  assert(
    strip.image.includes("/rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-panzerfaust-windup-runtime-strip.png"),
    "composited Panzerfaust profile uses the deterministic wind-up runtime strip",
  );
  assert(
    strip.frameWidth === 160 && strip.frameHeight === 112 && strip.frameCount === 8,
    "composited Panzerfaust strip exposes idle, movement, firing, and wind-up cells",
  );
  assert(
    strip.idleFrame === 0 &&
      strip.movementFrames.join(",") === "1,2,3" &&
      strip.firingFrames.join(",") === "4" &&
      strip.windupFrames.join(",") === "5,6,7",
    "composited Panzerfaust strip routes the approved idle, movement, firing, and wind-up frames",
  );
}

{
  const profile = getVisualProfile("rifleman-panzerfaust-composite-1");
  const override = profile.frameStripOverrides[0];
  const loaded = { kind: KIND.PANZERFAUST, panzerfaustLoaded: true };
  const spent = { kind: KIND.PANZERFAUST, panzerfaustLoaded: false };
  assert(
    liveRigKeyForEntity(loaded) === override.rigKey,
    "loaded Panzerfaust resolves to the composited frame-strip route",
  );
  assert(
    liveRigKeyForEntity(spent) !== override.rigKey,
    "spent Panzerfaust resolves away from the composited frame-strip route",
  );
}

{
  const profile = getVisualProfile("scout-plane-fw189-pass-01");
  assert(profile, "Scout Plane Fw 189 frame-strip visual profile is registered");
  assert(profile.frameStripOverrides.length === 1, "Scout Plane Fw 189 profile has one frame-strip override");
  const override = profile.frameStripOverrides[0];
  const strip = override.strip;
  assert(override.kind === KIND.SCOUT_PLANE, "Scout Plane Fw 189 profile targets Scout Plane units");
  assert(
    strip.image.includes("/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png"),
    "Scout Plane Fw 189 profile uses the generated alpha strip asset",
  );
  assert(
    strip.frameWidth === 942 &&
      strip.frameHeight === 1163 &&
      strip.frameCount === 1,
    "Scout Plane Fw 189 strip exposes the generated one-frame atlas geometry",
  );
  const imageSize = readPngDimensions(strip.image);
  assert(
    imageSize.width === strip.frameWidth * strip.frameCount,
    "Scout Plane Fw 189 strip PNG width matches runtime atlas geometry",
  );
  assert(imageSize.height === strip.frameHeight, "Scout Plane Fw 189 strip PNG height matches runtime atlas geometry");
}

{
  const profile = getVisualProfile("scout-car-png-1");
  assert(profile, "scout car PNG visual profile is registered");
  assert(profile.initialCamera?.framingScale > 2, "scout car PNG profile opens zoomed in on the render-preview scout cars");
  assert(
    !profile.unitOverrides && !profile.frameStripOverrides && !profile.staticSamples,
    "scout car PNG profile is camera-only because the atlas is the normal Scout Car art path",
  );
}

{
  const atlas = createLivePngRigAtlases().get(KIND.SCOUT_CAR);
  assert(atlas?.enabled, "scout car PNG atlas is registered for live rendering");
  assert(
    atlas.image.includes("/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas-adjusted.png"),
    "scout car PNG atlas uses the checked-in worker-ready adjusted team-color asset",
  );
  assert(
    atlas.bakedColorAdjustment?.brightness === 90 &&
      atlas.bakedColorAdjustment?.saturation === 90 &&
      atlas.bakedColorAdjustment?.hue === 100 &&
      atlas.runtimeColorAdjustment?.brightness === 100 &&
      atlas.runtimeColorAdjustment?.saturation === 100 &&
      atlas.runtimeColorAdjustment?.hue === 100,
    "scout car PNG atlas records its baked dampening and requires no runtime color pass",
  );
  assert(
    JSON.stringify(atlas.grid?.palette) === JSON.stringify(PLAYER_PALETTE),
    "scout car PNG atlas maps its palette frames to the normal player palette",
  );
  const bodySprite = atlas.sprites.find((sprite) => sprite.id === "sprite.body");
  const gunSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.rearMachineGun");
  assert(
    bodySprite?.tintSlot === "fixed" &&
      JSON.stringify(Object.keys(bodySprite.paletteFrames || {})) === JSON.stringify(PLAYER_PALETTE),
    "scout car PNG body keeps fixed pre-colored frames for every player color",
  );
  assert(
    gunSprite?.tintSlot === "fixed" &&
      !gunSprite.paletteFrames &&
      gunSprite.frame,
    "scout car PNG rear machine gun uses one neutral fixed-tint frame",
  );
  const definitions = createLiveRigDefinitions();
  const definition = liveRigDefinitionFor(definitions, KIND.SCOUT_CAR);
  const routes = liveRigRoutesFor(KIND.SCOUT_CAR);
  const unitRoute = routes.find((route) => route.layerName === "units");
  const shadowRoute = routes.find((route) => route.layerName === "unitShadows");
  const unitCoverage = pngAtlasRouteCoverage(definition, atlas, unitRoute);
  const shadowCoverage = pngAtlasRouteCoverage(definition, atlas, shadowRoute);
  assert(unitCoverage.missingParts.length === 0, "scout car PNG atlas covers every unit-route part");
  assert(
    unitCoverage.coveredParts.includes("part.gunnerHead") &&
      unitCoverage.coveredParts.includes("part.gunnerBarrel"),
    "scout car PNG gun sprite replaces the old crew/gun SVG parts",
  );
  assert(
    shadowCoverage.coveredParts.length === 0 &&
      shadowCoverage.missingParts.includes("part.shadow"),
    "scout car PNG atlas leaves the existing SVG shadow route in place",
  );
}

{
  const profile = getVisualProfile("mortar-png-1");
  assert(profile, "mortar PNG visual profile is registered");
  assert(profile.initialCamera?.framingScale > 2, "mortar PNG profile opens zoomed in on the render-preview mortars");
  assert(
    !profile.unitOverrides && !profile.frameStripOverrides && !profile.staticSamples,
    "mortar PNG profile is camera-only because the atlas is the normal Mortar Team art path",
  );
}

{
  const atlas = createLivePngRigAtlases().get(KIND.MORTAR_TEAM);
  assert(atlas?.enabled, "mortar PNG atlas is registered for live rendering");
  assert(
    atlas.image.includes("/assets/rigs/mortar-png-pass-04/generated/mortar-m2-wheeled-baseplate-pass-04-alpha.png"),
    "mortar PNG atlas uses the checked-in generated alpha asset",
  );
  const imageSize = readPngDimensions(atlas.image);
  assert(
    imageSize.width >= atlas.grid?.components?.tube?.x + atlas.grid?.components?.tube?.w &&
      imageSize.height >= atlas.grid?.components?.carriage?.y + atlas.grid?.components?.carriage?.h,
    "mortar PNG atlas frame coordinates fit inside the generated component sheet",
  );
  for (const sprite of atlas.sprites) {
    assert(
      imageSize.width >= sprite.frame?.x + sprite.frame?.w &&
        imageSize.height >= sprite.frame?.y + sprite.frame?.h,
      `mortar PNG atlas sprite frame ${sprite.id} fits inside the generated sheet`,
    );
  }
  const carriageSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.mortar.carriage.packed");
  const tubeSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.mortar.tube.packed");
  const leftTireSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.mortar.tire.left.packed");
  const rightTireSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.mortar.tire.right.packed");
  const basePlateSprite = atlas.sprites.find((sprite) => sprite.id === "sprite.mortar.basePlate.deployed");
  assert(
    carriageSprite?.tintSlot === "team-light" &&
      carriageSprite.tintAdjustment?.brightness === 78 &&
      carriageSprite.tintAdjustment?.saturation === 92,
    "mortar PNG carriage keeps the off-white frame team-tinted in lab render preview",
  );
  assert(
    tubeSprite?.tintSlot === "team-light" &&
      tubeSprite.tintAdjustment?.brightness === 78 &&
      tubeSprite.tintAdjustment?.saturation === 92,
    "mortar PNG tube and barrel assembly are team-tinted in lab render preview",
  );
  assert(
    leftTireSprite?.tintSlot === "fixed" &&
      leftTireSprite.drawOrder > carriageSprite?.drawOrder &&
      leftTireSprite.drawOrder < tubeSprite?.drawOrder &&
      rightTireSprite?.tintSlot === "fixed" &&
      rightTireSprite.drawOrder > carriageSprite?.drawOrder &&
      rightTireSprite.drawOrder < tubeSprite?.drawOrder,
    "mortar PNG tire overlays remain fixed-color above the team-tinted carriage",
  );
  assert(
    basePlateSprite?.tintSlot === "team" &&
      basePlateSprite.drawOrder < carriageSprite?.drawOrder &&
      basePlateSprite.positionOffsetX === -20 &&
      basePlateSprite.frame?.w / basePlateSprite.frame?.pixelsPerUnitX === 16 &&
      basePlateSprite.frame?.h / basePlateSprite.frame?.pixelsPerUnitY === 16,
    "mortar PNG base plate is team-tinted, half a tile wide, rearward, and draws below the carriage",
  );
  const definitions = createLiveRigDefinitions();
  const definition = liveRigDefinitionFor(definitions, KIND.MORTAR_TEAM);
  const routes = liveRigRoutesFor(KIND.MORTAR_TEAM);
  const unitRoute = routes.find((route) => route.layerName === "units");
  const shadowRoute = routes.find((route) => route.layerName === "unitShadows");
  const unitCoverage = pngAtlasRouteCoverage(definition, atlas, unitRoute);
  const shadowCoverage = pngAtlasRouteCoverage(definition, atlas, shadowRoute);
  assert(unitCoverage.missingParts.length === 0, "mortar PNG atlas covers every unit-route part");
  assert(
    unitCoverage.coveredParts.includes("part.mortar.basePlate.deployed") &&
      unitCoverage.coveredParts.includes("part.mortar.body.packed") &&
      unitCoverage.coveredParts.includes("part.mortar.tube.packed"),
    "mortar PNG atlas replaces the base plate, carriage/body, and tube SVG parts",
  );
  assert(
    shadowCoverage.coveredParts.length === 0 &&
      shadowCoverage.missingParts.length === 1 &&
      shadowCoverage.missingParts.includes("part.shadow"),
    "mortar PNG atlas leaves only the ordinary unit shadow on the SVG under-unit route",
  );
}

{
  const compiled = compileVisualUnitRigCandidates();
  const ids = visualUnitRigCandidateIds();
  assert(ids.length === 0, "checked-in visual rig registry has no raster-unit SVG candidates");
  assert(compiled.definitions.size === 0, "empty checked-in registry compiles to no overrides");
  assert(compiled.errors.size === 0, "empty checked-in registry has no importer errors");

  const testCompiled = compileVisualUnitRigCandidates(TEST_WORKER_CANDIDATES);
  for (const id of TEST_WORKER_CANDIDATES.map((entry) => entry.id)) {
    const candidate = testCompiled.definitions.get(id);
    assert(candidate?.kind === KIND.WORKER, `${id} compiles as a Worker candidate`);
    assert(candidate.definition?.id === id, `${id} keeps its registered candidate id after import`);
  }
  assert(testCompiled.errors.size === 0, "test-only SVG rig candidates compile without importer errors");
}

{
  const compiled = compileVisualUnitRigCandidates(TEST_WORKER_CANDIDATES);
  const entities = [
    { id: 126, owner: 1, kind: KIND.WORKER, x: 1887.97, y: 1860.91, facing: 0, weaponFacing: 0 },
    { id: 127, owner: 1, kind: KIND.WORKER, x: 1883.97, y: 1944.91, facing: 0, weaponFacing: 0 },
    { id: 128, owner: 1, kind: KIND.WORKER, x: 1883.97, y: 2031.91, facing: 0, weaponFacing: 0 },
    { id: 140, owner: 1, kind: KIND.RIFLEMAN, x: 2000, y: 1900, facing: 0, weaponFacing: 0 },
  ];

  const resolved = resolveVisualUnitOverrides(TEST_WORKER_OVERRIDES, entities, compiled.definitions);
  assert(resolved.errors.length === 0, "unit override selectors resolve cleanly for test Workers");
  assert(resolved.overrides.size === 3, "unit override rules assign three real units");
  assert(resolved.overrides.get(126)?.candidateId === "worker-candidate-a", "entity-id selector targets Worker 126");
  assert(resolved.overrides.get(127)?.candidateId === "worker-candidate-b", "kind ordinal selector targets the second Worker");
  assert(resolved.overrides.get(128)?.candidateId === "worker-candidate-c", "nearest selector targets the intended Worker");
  assert(entities.every((entity) => entity.kind === KIND.WORKER || entity.kind === KIND.RIFLEMAN),
    "visual override resolution does not mutate real entity kinds");
}

{
  const restorePixi = installFakePixi();
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    const profile = getVisualProfile("rifleman-recoil-strip-1");
    const override = profile.frameStripOverrides[0];
    renderer._visualFrameStripTextures.set(
      `${KIND.RIFLEMAN}:${override.strip.imageVersion}`,
      PIXI.Texture.from("rifleman-recoil-test-texture"),
    );
    const rifleman = {
      id: 115,
      owner: 1,
      kind: KIND.RIFLEMAN,
      x: 2003.97,
      y: 1837.91,
      hp: 45,
      maxHp: 45,
      state: STATE.IDLE,
      facing: -1.7406152,
      weaponFacing: -1.7406152,
    };
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set([rifleman.id]),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [rifleman];
      },
      selectedEntities() {
        return [rifleman];
      },
      weaponRecoil() {
        return 0;
      },
    };
    const beforeKeys = Object.keys(state).sort().join(",");

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualFrameStripOverrides: profile.frameStripOverrides,
    });

    const instance = renderer._liveRigPools.liveUnitRigs.get(rifleman.id);
    assert(instance?.strip?.imageVersion === override.strip.imageVersion,
      "visual frame-strip profile routes Rifleman rendering through the recoil strip");
    assert(renderer._liveRigPools.liveUnitRigShadows.has(rifleman.id),
      "frame-strip overrides keep the normal SVG shadow route");
    assert(renderer._pools.selectionRings.has(rifleman.id), "selection rings still use the real Rifleman entity id");
    assert(renderer._pools.hpBars.has(rifleman.id), "selected-unit HP overlays still use the real Rifleman entity id");
    assert(Object.keys(state).sort().join(",") === beforeKeys, "frame-strip override rendering does not add GameState fields");
    assert(state.selection.has(rifleman.id), "frame-strip override rendering does not mutate selection");

    renderer.destroy();
  } finally {
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    const profile = getVisualProfile("rifleman-panzerfaust-composite-1");
    const override = profile.frameStripOverrides[0];
    assert(
      renderer._assetReadiness.get(`live-frame-strip:${LOADED_RIFLEMAN_RIG_KEY}`)?.kind === KIND.RIFLEMAN,
      "loaded Panzerfaust strip readiness is reported for Rifleman capture subjects",
    );
    renderer._visualFrameStripTextures.set(
      `${KIND.PANZERFAUST}:${override.strip.imageVersion}`,
      PIXI.Texture.from("panzerfaust-composite-test-texture"),
    );
    renderer._liveFrameStripTextures.set(
      KIND.RIFLEMAN,
      PIXI.Texture.from("normal-rifleman-test-texture"),
    );
    const rifleman = {
      id: 116,
      owner: 1,
      kind: KIND.PANZERFAUST,
      x: 2003.97,
      y: 1837.91,
      hp: 45,
      maxHp: 45,
      state: STATE.ATTACK,
      facing: 0,
      weaponFacing: 0,
      panzerfaustLoaded: true,
    };
    let recoil = 0;
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set(),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [rifleman];
      },
      selectedEntities() {
        return [];
      },
      weaponRecoil() {
        return recoil;
      },
    };

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualFrameStripOverrides: profile.frameStripOverrides,
    });
    const loadedInstance = renderer._liveRigPools.liveUnitRigs.get(rifleman.id);
    assert(
      loadedInstance?.strip?.imageVersion === override.strip.imageVersion,
      "loaded Panzerfaust renders through the composited strip",
    );

    rifleman.panzerfaustLoaded = false;
    recoil = 1;
    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualFrameStripOverrides: profile.frameStripOverrides,
    });
    const spentInstance = renderer._liveRigPools.liveUnitRigs.get(rifleman.id);
    assert(loadedInstance._destroyed === true, "launch-time loadout change destroys the loaded carrier instance");
    assert(
      spentInstance?.strip?.imageVersion === RIFLEMAN_PNG_FRAME_STRIP.imageVersion,
      "spent Panzerfaust immediately renders through the normal firing-capable Rifleman strip",
    );

    renderer.destroy();
  } finally {
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

{
  const brokenSvg = `<svg viewBox="-10 -10 20 20" data-rts-rig-kind="${KIND.WORKER}" data-rts-rig-version="1" data-rts-origin="center">
    <script id="part.bad"></script>
    <circle id="anchor.origin" cx="0" cy="0" r="1" />
    <circle id="anchor.selection" cx="0" cy="0" r="1" />
    <circle id="anchor.hp" cx="0" cy="-8" r="1" />
  </svg>`;
  const compiled = compileVisualUnitRigCandidates([
    ...TEST_WORKER_CANDIDATES,
    { id: "broken-worker-candidate", label: "Broken", kind: KIND.WORKER, svgText: brokenSvg },
  ]);
  const entities = [
    { id: 1, owner: 1, kind: KIND.WORKER, x: 0, y: 0 },
    { id: 2, owner: 1, kind: KIND.WORKER, x: 20, y: 0 },
  ];
  const resolved = resolveVisualUnitOverrides([
    { id: "missing-unit", candidateId: "worker-candidate-a", selector: { entityId: 999 } },
    { id: "ambiguous-worker", candidateId: "worker-candidate-a", selector: { kind: KIND.WORKER, owner: 1 } },
    { id: "invalid-candidate", candidateId: "broken-worker-candidate", selector: { entityId: 1 } },
  ], entities, compiled.definitions, { candidateErrors: compiled.errors });

  assert(compiled.errors.has("broken-worker-candidate"), "invalid test-only SVG candidates fail importer validation");
  assert(resolved.overrides.size === 0, "broken visual override rules do not produce renderer overrides");
  assert(
    resolved.errors.some((error) => error.reason === "selector-no-match") &&
      resolved.errors.some((error) => error.reason === "selector-ambiguous") &&
      resolved.errors.some((error) => error.reason === "invalid-candidate"),
    "visual override diagnostics distinguish no-match, ambiguous selector, and invalid candidate failures",
  );
}

{
  const normalized = normalizeStaticVisualSamples({
    staticSamples: [
      {
        id: "valid-trench",
        kind: "trench",
        label: "Valid",
        x: 100,
        y: 120,
        radiusTiles: 0.375,
        variant: "basin",
        occupied: true,
      },
      {
        id: "bad-variant",
        kind: "trench",
        label: "Bad",
        x: 160,
        y: 120,
        radiusTiles: 0.375,
        variant: "missing_variant",
      },
      {
        id: "../path",
        kind: "trench",
        x: 200,
        y: 120,
        variant: "basin",
      },
    ],
  }, { tileSize: 32 });

  assert(normalized.samples.length === 1, "visual sample normalizer keeps valid candidates");
  assert(normalized.samples[0].id === "valid-trench", "normalizer preserves the valid candidate id");
  assert(normalized.samples[0].occupied === true, "normalizer preserves occupied trench sample state");
  assert(normalized.errors.length === 2, "visual sample normalizer reports invalid candidates");
  assert(
    normalized.errors.some((error) => error.reason === "unknown-variant") &&
      normalized.errors.some((error) => error.reason === "invalid-id"),
    "visual sample normalizer classifies invalid candidates without throwing",
  );
}

{
  const frameState = {
    playerId: 1,
    spectator: false,
    entitiesInterpolated() {
      return [
        { id: 10, owner: 1, kind: KIND.RIFLEMAN, x: 64, y: 64 },
        { id: 11, owner: 2, kind: KIND.RIFLEMAN, x: 96, y: 64, visionOnly: true },
        { id: 12, owner: 1, kind: KIND.RIFLEMAN, x: 128, y: 64, shotReveal: true },
      ];
    },
    selectedEntities() {
      return [];
    },
  };
  const frameViews = buildFrameEntityViews(frameState, { alpha: 1 });
  assert(frameViews.fogSourceEntities.length === 1, "fog sources come only from authoritative entities");
  assert(frameViews.fogSourceEntities[0].id === 10, "visual-only samples never enter fog-source entity views");
}

{
  const restorePixi = installFakePixi();
  const priorConsoleError = console.error;
  const consoleErrors = [];
  console.error = (...args) => consoleErrors.push(args);
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;

    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      selection: new Set(),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [];
      },
      selectedEntities() {
        return [];
      },
    };
    const beforeKeys = Object.keys(state).sort().join(",");
    const beforeSelection = state.selection;
    const samples = {
      staticSamples: [
        {
          id: "valid-trench",
          kind: "trench",
          label: "Valid",
          x: 100,
          y: 120,
          radiusTiles: 0.375,
          variant: "basin",
          occupied: true,
        },
        {
          id: "bad-variant",
          kind: "trench",
          label: "Bad",
          x: 160,
          y: 120,
          radiusTiles: 0.375,
          variant: "missing_variant",
        },
      ],
    };

    renderer.render(state, {
      x: 0,
      y: 0,
      zoom: 2,
      projectedExtent: (point) => ({
        width: 2,
        height: 2,
        scaleX: point.y < 100 ? 2 : 0.8,
        scaleY: point.y < 100 ? 2 : 0.8,
        visible: true,
      }),
    }, null, 1, { visualSamples: samples });

    const diagnostics = renderer.visualSampleDiagnostics();
    const sampleDisplay = renderer.layers.visualSamples.children[0];
    const labelDisplay = renderer.layers.visualSampleLabels.children[0];

    assert(diagnostics.visibleSamples === 1, "renderer draws valid static visual samples");
    assert(diagnostics.invalidSamples === 1, "renderer skips invalid static visual samples");
    assert(renderer.layers.visualSamples.children.length === 1, "static samples use renderer-owned Pixi objects");
    assert(renderer.layers.visualSampleLabels.children.length === 1, "static labels use renderer-owned Pixi text");
    assert(sampleDisplay.calls.some((call) => call[0] === "drawPolygon"),
      "trench static samples draw procedural candidate geometry");
    assert(
      sampleDisplay.calls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchRim && call[2] === 0.94),
      "occupied static samples draw the checked-in foreground trench lip",
    );
    assert(labelDisplay.text === "Valid", "static sample labels identify each candidate");
    assertApprox(labelDisplay.x, 100, 0.001, "static sample labels stay anchored to world x");
    assertApprox(labelDisplay.y, 96, 0.001, "static sample labels stay anchored above the sample");
    assertApprox(labelDisplay.scaleX, 0.5, 0.001, "static sample labels compensate at their projected anchor");
    assert(Object.keys(state).sort().join(",") === beforeKeys, "static sample rendering does not add GameState fields");
    assert(state.selection === beforeSelection && state.selection.size === 0,
      "static sample rendering does not touch selection");
    assert(globalThis.__rtsVisualSampleErrors?.latest?.reason === "unknown-variant",
      "invalid visual candidates are surfaced as local diagnostics");
    assert(
      consoleErrors.some((args) => String(args[0]).includes("[RTS_RENDER] skipped visualSample:bad-variant")),
      "invalid visual candidates are reported through renderer error diagnostics",
    );

    renderer.destroy();
    assert(renderer.layers.visualSamples.children.length === 0, "renderer teardown removes static sample objects");
    assert(renderer.layers.visualSampleLabels.children.length === 0, "renderer teardown removes static label objects");
  } finally {
    console.error = priorConsoleError;
    delete globalThis.__rtsVisualSampleErrors;
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    renderer._visualUnitRigCandidates = compileVisualUnitRigCandidates(TEST_WORKER_CANDIDATES);
    const workerA = { id: 126, owner: 1, kind: KIND.WORKER, x: 1887.97, y: 1860.91, hp: 100, maxHp: 100, facing: 0, weaponFacing: 0 };
    const workerB = { id: 127, owner: 1, kind: KIND.WORKER, x: 1883.97, y: 1944.91, hp: 100, maxHp: 100, facing: 0.2, weaponFacing: 0.5 };
    const workerC = { id: 128, owner: 1, kind: KIND.WORKER, x: 1883.97, y: 2031.91, hp: 100, maxHp: 100, facing: 0.4, weaponFacing: 0.8 };
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set([workerB.id]),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [workerA, workerB, workerC];
      },
      selectedEntities() {
        return [workerB];
      },
      weaponRecoil(entityId) {
        return entityId === workerC.id ? 0.5 : 0;
      },
    };
    const beforeKeys = Object.keys(state).sort().join(",");

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualUnitOverrides: TEST_WORKER_OVERRIDES,
    });

    const diagnostics = renderer.visualUnitOverrideDiagnostics();
    assert(diagnostics.activeOverrides === 3, "renderer resolves three real-unit visual overrides");
    assert(diagnostics.errors === 0, "valid unit override profile has no selector diagnostics");
    assert(renderer._liveRigPools.liveUnitRigs.get(workerA.id)?.definition.id === "worker-candidate-a",
      "entity-id override routes Worker A through the candidate SVG rig");
    assert(renderer._liveRigPools.liveUnitRigs.get(workerB.id)?.definition.id === "worker-candidate-b",
      "kind ordinal override routes Worker B through the candidate SVG rig");
    assert(renderer._liveRigPools.liveUnitRigs.get(workerC.id)?.definition.id === "worker-candidate-c",
      "nearest override routes Worker C through the candidate SVG rig");
    assert(renderer._pools.selectionRings.has(workerB.id), "selection rings still use real selected entity ids");
    assert(renderer._pools.hpBars.size === 1, "HP overlays still come from real entity state");
    assert(Object.keys(state).sort().join(",") === beforeKeys, "unit override rendering does not add GameState fields");
    assert(state.selection.has(workerB.id), "unit override rendering does not mutate selection");
    assert(!globalThis.__rtsVisualUnitOverrideErrors, "valid unit override rendering does not publish errors");

    renderer.destroy();
  } finally {
    delete globalThis.__rtsVisualUnitOverrideErrors;
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  const priorConsoleError = console.error;
  console.error = () => {};
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    renderer._visualUnitRigCandidateRegistry = () => {
      throw new Error("candidate registry failed");
    };
    const worker = {
      id: 126,
      owner: 1,
      kind: KIND.WORKER,
      x: 1887.97,
      y: 1860.91,
      hp: 100,
      maxHp: 100,
      facing: 0,
      weaponFacing: 0,
    };
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set(),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [worker];
      },
      selectedEntities() {
        return [];
      },
      weaponRecoil() {
        return 0;
      },
    };

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualUnitOverrides: [
        { id: "registry-fails", candidateId: "worker-candidate-a", selector: { entityId: worker.id } },
      ],
    });

    const diagnostics = renderer.visualUnitOverrideDiagnostics();
    assert(diagnostics.activeOverrides === 0, "failed override resolution falls back to zero active overrides");
    assert(diagnostics.errors === 1, "failed override resolution records a diagnostic error");
    assert(globalThis.__rtsVisualUnitOverrideErrors?.latest?.reason === "resolver-error",
      "unexpected override resolution failures publish local diagnostics");
    assert(globalThis.__rtsRenderErrors?.latest?.label === "visualUnitOverrides",
      "unexpected override resolution failures use renderer error diagnostics");
    assert(renderer._liveRigPools.liveUnitRigs.get(worker.id)?.definition.id === "worker.authored",
      "unit rendering falls back to the normal live rig when override resolution fails");

    renderer.destroy();
  } finally {
    console.error = priorConsoleError;
    delete globalThis.__rtsVisualUnitOverrideErrors;
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const renderer = await Renderer.create(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    renderer._visualUnitRigCandidates = compileVisualUnitRigCandidates(TEST_WORKER_CANDIDATES);
    const now = performance.now();
    const reveal = {
      id: 126,
      owner: 1,
      kind: KIND.WORKER,
      x: 1887.97,
      y: 1860.91,
      hp: 100,
      maxHp: 100,
      facing: 0,
      weaponFacing: 0,
      shotReveal: true,
      shotRevealCreatedAt: now - 100,
      shotRevealExpiresAt: now + 900,
    };
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set(),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [reveal];
      },
      selectedEntities() {
        return [];
      },
      weaponRecoil() {
        return 0;
      },
    };

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualUnitOverrides: [TEST_WORKER_OVERRIDES[0]],
    });

    assert(renderer.visualUnitOverrideDiagnostics().activeOverrides === 1,
      "shot-reveal-only frame can still resolve an entity-id unit override");
    assert(renderer._liveRigPools.liveShotRevealRigs.get(reveal.id)?.definition.id === "worker-candidate-a",
      "shot reveal rendering uses the same visual override candidate when the reveal id matches");
    renderer.destroy();
  } finally {
    delete globalThis.__rtsVisualUnitOverrideErrors;
    delete globalThis.__rtsRenderErrors;
    restorePixi();
  }
}

function fakeParent() {
  return {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) {
      view.parentNode = this;
    },
    removeChild(view) {
      view.parentNode = null;
    },
  };
}

function readPngDimensions(assetUrl) {
  const assetPath = assetUrl.split("?")[0];
  const buffer = fs.readFileSync(new URL(`../../client${assetPath}`, import.meta.url));
  assert(buffer.toString("hex", 0, 8) === "89504e470d0a1a0a", "visual profile asset is a PNG");
  assert(buffer.toString("ascii", 12, 16) === "IHDR", "visual profile PNG has an IHDR chunk");
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}
