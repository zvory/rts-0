#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import { KIND } from "../client/src/protocol.js";
import { liveRigKinds, liveRigRoutesFor } from "../client/src/renderer/rigs/live_routing.js";
import {
  SVG_MIGRATION_MANIFESTS,
  SVG_MIGRATION_MANIFESTS_BY_KIND,
} from "./fixtures/svg/unit_migration_manifests.mjs";

const baseline = JSON.parse(fs.readFileSync("tests/fixtures/svg/legacy-unit-oracle.baseline.json", "utf8"));
const baselineLabels = new Set(baseline.samples.map((sample) => sample.label));
const liveKinds = liveRigKinds();
const manifestKinds = SVG_MIGRATION_MANIFESTS.map((manifest) => manifest.kind);

test("every live-routed rig kind has a migration manifest", () => {
  assert.deepEqual([...liveKinds].sort(), [...manifestKinds].sort());
});

test("manifests name valid samples, thresholds, parts, and SVG sources", () => {
  for (const manifest of SVG_MIGRATION_MANIFESTS) {
    assert.equal(typeof manifest.kind, "string");
    assert.equal(fs.existsSync(manifest.svgPath), true, `${manifest.kind} SVG source should exist`);
    assert.equal(manifest.requiredSamples.length > 0, true, `${manifest.kind} requires sample labels`);
    assert.equal(manifest.partMappings.length > 0, true, `${manifest.kind} requires part mappings`);
    assert.deepEqual(manifest.approvedIntentionalDrift, [], `${manifest.kind} should not hide drift during migration`);
    assertCompositionThresholds(manifest.compositionThresholds, manifest.kind);
    for (const sample of manifest.requiredSamples) {
      assert.equal(baselineLabels.has(sample), true, `${manifest.kind} manifest sample missing from legacy oracle: ${sample}`);
    }
    for (const mapping of manifest.partMappings) {
      assert.equal(typeof mapping.legacyPart, "string");
      assert.equal(mapping.rigParts.length > 0, true, `${manifest.kind}/${mapping.legacyPart} needs rig parts`);
      assertPartThresholds(mapping.thresholds, `${manifest.kind}/${mapping.legacyPart}`);
    }
  }
});

test("live routing part groups match migration manifests", () => {
  for (const kind of liveKinds) {
    const manifest = SVG_MIGRATION_MANIFESTS_BY_KIND[kind];
    assert.ok(manifest, `${kind} should have manifest`);
    const routes = liveRigRoutesFor(kind);
    assert.deepEqual(routes.map((route) => route.parts), [
      manifest.liveRoutes.shadow,
      manifest.liveRoutes.unit,
    ]);
  }
});

test("non-migrated unit kinds are not live-routed", () => {
  const unmigratedKinds = Object.values(KIND).filter((kind) => !manifestKinds.includes(kind));
  for (const kind of unmigratedKinds) {
    assert.deepEqual(liveRigRoutesFor(kind), [], `${kind} should stay legacy until it gets a manifest and gate`);
  }
});

function assertCompositionThresholds(thresholds, kind) {
  assert.equal(thresholds.minAlphaWeightedMatchingRatio >= 0.97, true, kind);
  assert.equal(thresholds.maxOpaqueMismatchClusterPx <= 40, true, kind);
  assert.equal(thresholds.maxPerPixelRgbaDistance, 96, kind);
  assert.equal(thresholds.maxOpaqueMismatchCount <= 128, true, kind);
  assert.equal(thresholds.perChannelTolerance, 6, kind);
  assert.equal(thresholds.opaqueAlphaThreshold, 128, kind);
}

function assertPartThresholds(thresholds, label) {
  assert.equal(thresholds.minAlphaWeightedMatchingRatio >= 0.97, true, label);
  assert.equal(thresholds.maxPerPixelRgbaDistance <= 64, true, label);
  assert.equal(thresholds.perChannelTolerance <= 4, true, label);
  assert.equal(thresholds.opaqueAlphaThreshold, 128, label);
  assert.equal(Number.isFinite(thresholds.maxOpaqueMismatchCount), true, label);
  assert.equal(Number.isFinite(thresholds.maxOpaqueMismatchClusterPx), true, label);
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
