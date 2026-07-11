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
  createLiveRigDefinitions,
  liveRigDefinitionFor,
  liveRigRoutesFor,
} from "../../client/src/renderer/rigs/live_routing.js";
import {
  VISUAL_UNIT_RIG_CANDIDATE_SOURCES,
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

const PANZERFAUST_MANIFEST_URL = new URL(
  "../../client/assets/rigs/panzerfaust-pass-01/metadata/manifest.json",
  import.meta.url,
);

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
  const profile = getVisualProfile("unit-rig-overrides-1");
  assert(profile, "real-unit visual override profile is registered");
  assert(profile.unitOverrides.length >= 3, "unit override profile compares multiple same-kind units");
  assert(
    profile.unitOverrides.some((rule) => rule.selector?.entityId === 126) &&
      profile.unitOverrides.some((rule) => rule.selector?.kind === KIND.TANK && rule.selector?.ordinal === 2) &&
      profile.unitOverrides.some((rule) => rule.selector?.kind === KIND.TANK && rule.selector?.nearest),
    "unit override profile covers entity id, kind ordinal, and kind nearest selector forms",
  );
  for (const rule of profile.unitOverrides) {
    assert(
      visualUnitRigCandidateIds().includes(rule.candidateId),
      `unit override candidate ${rule.candidateId} is registered through the checked-in rig registry`,
    );
  }
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
  const manifest = JSON.parse(fs.readFileSync(PANZERFAUST_MANIFEST_URL, "utf8"));
  const runtime = manifest.runtime;
  const profile = getVisualProfile("panzerfaust-long-preview-1");
  assert(profile, "Panzerfaust long frame-strip visual profile is registered");
  assert(profile.frameStripOverrides.length === 1, "Panzerfaust long profile has one frame-strip override");
  const override = profile.frameStripOverrides[0];
  const strip = override.strip;
  assert(override.kind === KIND.PANZERFAUST, "Panzerfaust long profile targets Panzerfaust units");
  assert(strip.image === runtime.stripImageUrl, "Panzerfaust profile uses the manifest runtime asset URL");
  assert(strip.imageVersion === runtime.imageVersion, "Panzerfaust profile mirrors the manifest image version");
  assert(strip.frameWidth === runtime.frameWidth, "Panzerfaust profile mirrors the manifest frame width");
  assert(strip.frameHeight === runtime.frameHeight, "Panzerfaust profile mirrors the manifest frame height");
  assert(strip.frameCount === runtime.frameCount, "Panzerfaust profile mirrors the manifest frame count");
  assert(strip.worldScale === runtime.worldScale, "Panzerfaust profile mirrors the manifest world scale");
  assert(strip.tintSlot === runtime.tintSlot, "Panzerfaust profile mirrors the manifest tint slot");
  assert(
    JSON.stringify(strip.targetColorAdjustment) === JSON.stringify(runtime.targetColorAdjustment),
    "Panzerfaust profile mirrors the manifest runtime color target",
  );
  const imageSize = readPngDimensions(strip.image);
  assert(imageSize.width === runtime.frameWidth * runtime.frameCount,
    "Panzerfaust strip PNG width matches runtime atlas geometry");
  assert(imageSize.height === runtime.frameHeight, "Panzerfaust strip PNG height matches runtime atlas geometry");
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
    atlas.image.includes("/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas.png"),
    "scout car PNG atlas uses the checked-in pass-02 team-color asset",
  );
  assert(
    atlas.runtimeColorAdjustment?.brightness === 90 &&
      atlas.runtimeColorAdjustment?.saturation === 90 &&
      atlas.runtimeColorAdjustment?.hue === 100,
    "scout car PNG atlas applies a subtle global dampening pass over pre-colored team frames",
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
    atlas.image.includes("/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-alpha.png"),
    "mortar PNG atlas uses the checked-in generated alpha asset",
  );
  const imageSize = readPngDimensions(atlas.image);
  assert(
    imageSize.width >= atlas.grid?.components?.tube?.x + atlas.grid?.components?.tube?.w &&
      imageSize.height >= atlas.grid?.components?.carriage?.y + atlas.grid?.components?.carriage?.h,
    "mortar PNG atlas frame coordinates fit inside the generated three-cell sheet",
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
  const definitions = createLiveRigDefinitions();
  const definition = liveRigDefinitionFor(definitions, KIND.MORTAR_TEAM);
  const routes = liveRigRoutesFor(KIND.MORTAR_TEAM);
  const unitRoute = routes.find((route) => route.layerName === "units");
  const shadowRoute = routes.find((route) => route.layerName === "unitShadows");
  const unitCoverage = pngAtlasRouteCoverage(definition, atlas, unitRoute);
  const shadowCoverage = pngAtlasRouteCoverage(definition, atlas, shadowRoute);
  assert(unitCoverage.missingParts.length === 0, "mortar PNG atlas covers every unit-route part");
  assert(
    unitCoverage.coveredParts.includes("part.mortar.body.packed") &&
      unitCoverage.coveredParts.includes("part.mortar.tube.packed"),
    "mortar PNG atlas replaces both carriage/body and tube SVG parts",
  );
  assert(
    shadowCoverage.coveredParts.length === 0 &&
      shadowCoverage.missingParts.includes("part.shadow"),
    "mortar PNG atlas leaves the existing SVG shadow route in place",
  );
}

{
  const compiled = compileVisualUnitRigCandidates();
  const ids = visualUnitRigCandidateIds();
  assert(ids.length >= 3, "checked-in visual rig registry exposes multiple tank candidates");
  for (const id of ids) {
    const candidate = compiled.definitions.get(id);
    assert(candidate?.kind === KIND.TANK, `${id} compiles as a Tank candidate`);
    assert(candidate.definition?.id === id, `${id} keeps its registered candidate id after import`);
  }
  assert(compiled.errors.size === 0, "checked-in visual rig candidates compile without importer errors");
}

{
  const profile = getVisualProfile("unit-rig-overrides-1");
  const compiled = compileVisualUnitRigCandidates();
  const entities = [
    { id: 126, owner: 1, kind: KIND.TANK, x: 1887.97, y: 1860.91, facing: 0, weaponFacing: 0 },
    { id: 127, owner: 1, kind: KIND.TANK, x: 1883.97, y: 1944.91, facing: 0, weaponFacing: 0 },
    { id: 128, owner: 1, kind: KIND.TANK, x: 1883.97, y: 2031.91, facing: 0, weaponFacing: 0 },
    { id: 140, owner: 1, kind: KIND.RIFLEMAN, x: 2000, y: 1900, facing: 0, weaponFacing: 0 },
  ];

  const resolved = resolveVisualUnitOverrides(profile.unitOverrides, entities, compiled.definitions);
  assert(resolved.errors.length === 0, "unit override selectors resolve cleanly for render-preview tanks");
  assert(resolved.overrides.size === 3, "unit override profile assigns three real units");
  assert(resolved.overrides.get(126)?.candidateId === "tank-low-profile", "entity-id selector targets tank 126");
  assert(resolved.overrides.get(127)?.candidateId === "tank-wide-turret", "kind ordinal selector targets the second tank");
  assert(resolved.overrides.get(128)?.candidateId === "tank-long-cannon", "nearest selector targets the intended tank");
  assert(entities.every((entity) => entity.kind === KIND.TANK || entity.kind === KIND.RIFLEMAN),
    "visual override resolution does not mutate real entity kinds");
}

{
  const restorePixi = installFakePixi();
  try {
    const renderer = new Renderer(fakeParent());
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
  const brokenSvg = `<svg viewBox="-10 -10 20 20" data-rts-rig-kind="${KIND.TANK}" data-rts-rig-version="1" data-rts-origin="center">
    <script id="part.bad"></script>
    <circle id="anchor.origin" cx="0" cy="0" r="1" />
    <circle id="anchor.selection" cx="0" cy="0" r="1" />
    <circle id="anchor.hp" cx="0" cy="-8" r="1" />
  </svg>`;
  const compiled = compileVisualUnitRigCandidates([
    ...VISUAL_UNIT_RIG_CANDIDATE_SOURCES,
    { id: "broken-tank-candidate", label: "Broken", kind: KIND.TANK, svgText: brokenSvg },
  ]);
  const entities = [
    { id: 1, owner: 1, kind: KIND.TANK, x: 0, y: 0 },
    { id: 2, owner: 1, kind: KIND.TANK, x: 20, y: 0 },
  ];
  const resolved = resolveVisualUnitOverrides([
    { id: "missing-unit", candidateId: "tank-low-profile", selector: { entityId: 999 } },
    { id: "ambiguous-tank", candidateId: "tank-low-profile", selector: { kind: KIND.TANK, owner: 1 } },
    { id: "invalid-candidate", candidateId: "broken-tank-candidate", selector: { entityId: 1 } },
  ], entities, compiled.definitions, { candidateErrors: compiled.errors });

  assert(compiled.errors.has("broken-tank-candidate"), "invalid checked-in SVG candidates fail importer validation");
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
    const renderer = new Renderer(fakeParent());
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
    const renderer = new Renderer(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    const profile = getVisualProfile("unit-rig-overrides-1");
    const tankA = { id: 126, owner: 1, kind: KIND.TANK, x: 1887.97, y: 1860.91, hp: 180, maxHp: 180, facing: 0, weaponFacing: 0 };
    const tankB = { id: 127, owner: 1, kind: KIND.TANK, x: 1883.97, y: 1944.91, hp: 180, maxHp: 180, facing: 0.2, weaponFacing: 0.5 };
    const tankC = { id: 128, owner: 1, kind: KIND.TANK, x: 1883.97, y: 2031.91, hp: 180, maxHp: 180, facing: 0.4, weaponFacing: 0.8 };
    const state = {
      playerId: 1,
      players: [{ id: 1, color: "#4878c8" }],
      resources: { oil: 10 },
      selection: new Set([tankB.id]),
      rememberedBuildings: [],
      map: { tileSize: 32 },
      trenches: [],
      entitiesInterpolated() {
        return [tankA, tankB, tankC];
      },
      selectedEntities() {
        return [tankB];
      },
      weaponRecoil(entityId) {
        return entityId === tankC.id ? 0.5 : 0;
      },
    };
    const beforeKeys = Object.keys(state).sort().join(",");

    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1, {
      visualUnitOverrides: profile.unitOverrides,
    });

    const diagnostics = renderer.visualUnitOverrideDiagnostics();
    assert(diagnostics.activeOverrides === 3, "renderer resolves three real-unit visual overrides");
    assert(diagnostics.errors === 0, "valid unit override profile has no selector diagnostics");
    assert(renderer._liveRigPools.liveUnitRigs.get(tankA.id)?.definition.id === "tank-low-profile",
      "entity-id override routes tank A through the candidate SVG rig");
    assert(renderer._liveRigPools.liveUnitRigs.get(tankB.id)?.definition.id === "tank-wide-turret",
      "kind ordinal override routes tank B through the candidate SVG rig");
    assert(renderer._liveRigPools.liveUnitRigs.get(tankC.id)?.definition.id === "tank-long-cannon",
      "nearest override routes tank C through the candidate SVG rig");
    assert(renderer._pools.selectionRings.has(tankB.id), "selection rings still use real selected entity ids");
    assert(renderer._pools.hpBars.size === 1, "HP overlays still come from real entity state");
    assert(Object.keys(state).sort().join(",") === beforeKeys, "unit override rendering does not add GameState fields");
    assert(state.selection.has(tankB.id), "unit override rendering does not mutate selection");
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
    const renderer = new Renderer(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    renderer._visualUnitRigCandidateRegistry = () => {
      throw new Error("candidate registry failed");
    };
    const tank = {
      id: 126,
      owner: 1,
      kind: KIND.TANK,
      x: 1887.97,
      y: 1860.91,
      hp: 180,
      maxHp: 180,
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
        return [tank];
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
        { id: "registry-fails", candidateId: "tank-low-profile", selector: { entityId: tank.id } },
      ],
    });

    const diagnostics = renderer.visualUnitOverrideDiagnostics();
    assert(diagnostics.activeOverrides === 0, "failed override resolution falls back to zero active overrides");
    assert(diagnostics.errors === 1, "failed override resolution records a diagnostic error");
    assert(globalThis.__rtsVisualUnitOverrideErrors?.latest?.reason === "resolver-error",
      "unexpected override resolution failures publish local diagnostics");
    assert(globalThis.__rtsRenderErrors?.latest?.label === "visualUnitOverrides",
      "unexpected override resolution failures use renderer error diagnostics");
    assert(renderer._liveRigPools.liveUnitRigs.get(tank.id)?.definition.id === "tank.authored",
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
    const renderer = new Renderer(fakeParent());
    for (const name of NOOP_RENDERER_OVERLAYS) renderer[name] = () => {};
    renderer._drawGroundDecals = () => 0;
    renderer._drawTrenches = () => 0;
    const profile = getVisualProfile("unit-rig-overrides-1");
    const now = performance.now();
    const reveal = {
      id: 126,
      owner: 1,
      kind: KIND.TANK,
      x: 1887.97,
      y: 1860.91,
      hp: 180,
      maxHp: 180,
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
      visualUnitOverrides: [profile.unitOverrides[0]],
    });

    assert(renderer.visualUnitOverrideDiagnostics().activeOverrides === 1,
      "shot-reveal-only frame can still resolve an entity-id unit override");
    assert(renderer._liveRigPools.liveShotRevealRigs.get(reveal.id)?.definition.id === "tank-low-profile",
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
