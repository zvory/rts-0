#!/usr/bin/env node
import assert from "node:assert/strict";
import { compareRgbaBuffers } from "./visual_pixel_compare.mjs";

const thresholds = Object.freeze({
  minAlphaWeightedMatchingRatio: 0,
  maxPerPixelRgbaDistance: 64,
  maxOpaqueMismatchCount: 2,
  maxOpaqueMismatchClusterPx: 1,
  perChannelTolerance: 4,
  opaqueAlphaThreshold: 128,
});

test("low-alpha fringe outliers are governed by weighted match instead of max distance", () => {
  const legacy = image([
    [0, 0, 0, 0],
    [0, 0, 0, 0],
  ]);
  const rig = image([
    [0, 0, 0, 71],
    [0, 0, 0, 0],
  ]);
  const report = compareRgbaBuffers(legacy, rig, thresholds);
  assert.equal(report.passed, true);
  assert.equal(report.maxPerPixelRgbaDistance, 0);
  assert.equal(report.rawMaxPerPixelRgbaDistance, 71);
});

test("isolated opaque edge outliers are governed by count and cluster gates", () => {
  const legacy = image([
    [134, 129, 110, 158],
    [0, 0, 0, 0],
  ]);
  const rig = image([
    [0, 0, 0, 0],
    [0, 0, 0, 0],
  ]);
  const report = compareRgbaBuffers(legacy, rig, thresholds);
  assert.equal(report.passed, true);
  assert.equal(report.maxPerPixelRgbaDistance, 0);
  assert.equal(report.rawMaxPerPixelRgbaDistance > thresholds.maxPerPixelRgbaDistance, true);
  assert.equal(report.opaqueMismatchCount, 1);
  assert.equal(report.largestOpaqueMismatchClusterPx, 1);
});

test("clustered opaque outliers still fail through the cluster gate", () => {
  const legacy = image([
    [134, 129, 110, 158],
    [134, 129, 110, 158],
  ]);
  const rig = image([
    [0, 0, 0, 0],
    [0, 0, 0, 0],
  ]);
  const report = compareRgbaBuffers(legacy, rig, thresholds);
  assert.equal(report.passed, false);
  assert.equal(report.opaqueMismatchCount, 2);
  assert.equal(report.largestOpaqueMismatchClusterPx, 2);
});

function image(pixels) {
  return { width: pixels.length, height: 1, data: pixels.flat() };
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
