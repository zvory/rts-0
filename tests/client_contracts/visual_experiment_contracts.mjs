// tests/client_contracts/visual_experiment_contracts.mjs
// Contracts for local lab visual experimentation renderer-only samples.

import { assert, assertApprox } from "./assertions.mjs";
import { COLORS } from "../../client/src/config.js";
import { buildFrameEntityViews } from "../../client/src/frame_entity_views.js";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
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

    renderer.render(state, { x: 0, y: 0, zoom: 2 }, null, 1, { visualSamples: samples });

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
    assertApprox(labelDisplay.scaleX, 0.5, 0.001, "static sample labels compensate for camera zoom");
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
