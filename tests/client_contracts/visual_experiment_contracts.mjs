// tests/client_contracts/visual_experiment_contracts.mjs
// Contracts for local lab visual experimentation renderer-only samples.

import { assert, assertApprox } from "./assertions.mjs";
import { buildFrameEntityViews } from "../../client/src/frame_entity_views.js";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { normalizeStaticVisualSamples } from "../../client/src/renderer/visual_samples.js";
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
