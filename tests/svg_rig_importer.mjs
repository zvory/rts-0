#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { compileSvgRig } from "../client/src/renderer/rigs/svg_importer.js";
import { KIND } from "../client/src/protocol.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, "fixtures/svg");

const fixtures = [
  ["rig-worker.svg", KIND.WORKER, []],
  ["rig-infantry-weapon.svg", KIND.RIFLEMAN, ["muzzle"]],
  ["rig-crew-weapon.svg", KIND.MACHINE_GUNNER, ["muzzle", "bipod"]],
  ["rig-vehicle.svg", KIND.TANK, ["muzzle", "turret"]],
];

test("authored SVG fixtures compile to normalized rig definitions", () => {
  for (const [file, kind, extraAnchors] of fixtures) {
    const result = compileSvgRig(readFixture(file), { expectedKind: kind });
    assert.equal(result.ok, true, `${file}: ${JSON.stringify(result.errors)}`);
    assert.equal(result.definition.kind, kind);
    assert.ok(result.definition.parts.length >= 3, `${file} should compile representative parts`);
    assert.deepEqual(Object.keys(result.definition.anchors).slice(0, 3), ["origin", "selection", "hp"]);
    for (const anchor of extraAnchors) {
      assert.ok(result.definition.anchors[anchor], `${file} should include ${anchor} anchor`);
    }
    assert.ok(result.definition.bounds.selection, `${file} should include selection bounds`);
    assert.ok(result.definition.bounds.hp, `${file} should include hp bounds`);
    assert.ok(result.definition.parts.every((part) => part.paint && typeof part.paint.opacity === "number"));
  }
});

test("metadata, anchors, paint, draw order, and animation bindings are extracted", () => {
  const result = compileSvgRig(readFixture("rig-vehicle.svg"), { id: "tank.test", kind: KIND.TANK });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  assert.equal(result.definition.id, "tank.test");
  assert.equal(result.definition.kind, KIND.TANK);
  assert.deepEqual(result.definition.parts.map((part) => part.id).slice(0, 3), ["part.shadow", "part.track.left", "part.track.right"]);
  assert.deepEqual(result.definition.anchors.muzzle, { x: 30, y: 0 });
  assert.deepEqual(result.definition.parts.find((part) => part.id === "part.hull").paint.fill, "#5d7896");
  assert.deepEqual(result.definition.parts.find((part) => part.id === "part.turret").tintSlot, "team-light");
  assert.ok(result.definition.animations.some((binding) => binding.partId === "part.barrel" && binding.input === "weaponFacing"));
  assert.ok(result.definition.requiredRuntimeInputs.includes("weaponFacing"));
});

test("group and child transforms flatten into part transforms while anchors use world-local points", () => {
  const result = compileSvgRig(`
    <svg viewBox="-20 -20 40 40" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center">
      <g id="part.body" transform="translate(4,5) rotate(90)" data-rts-pivot="1,2" data-rts-tint="team">
        <rect x="-2" y="-3" width="4" height="6" fill="#112233" />
      </g>
      <circle id="anchor.origin" cx="0" cy="0" r="1" />
      <circle id="anchor.selection" cx="0" cy="0" r="1" transform="translate(2,3)" />
      <circle id="anchor.hp" cx="0" cy="-8" r="1" />
      <rect id="bounds.selection" x="-4" y="-4" width="8" height="8" />
    </svg>
  `);
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  const part = result.definition.parts[0];
  assert.deepEqual(part.transform, { x: 4, y: 5, rotation: 1.570796, scaleX: 1, scaleY: 1 });
  assert.deepEqual(part.pivot, { x: 1, y: 2 });
  assert.deepEqual(result.definition.anchors.selection, { x: 2, y: 3 });
});

test("unsupported SVG features fail closed with useful errors", () => {
  const result = compileSvgRig(`
    <svg viewBox="-10 -10 20 20" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center">
      <filter id="blur"><feGaussianBlur stdDeviation="2" /></filter>
      <image id="part.body" href="http://example.invalid/body.png" width="10" height="10" />
      <circle id="anchor.origin" cx="0" cy="0" r="1" />
      <circle id="anchor.selection" cx="0" cy="0" r="1" />
      <circle id="anchor.hp" cx="0" cy="-8" r="1" />
    </svg>
  `);
  assertError(result, "svg.unsupportedElement", "blur");
  assertError(result, "svg.unsupportedElement", "part.body");
  assertError(result, "svg.externalReference", "part.body");
});

test("CSS dependencies, duplicate ids, and lowercase paths fail closed", () => {
  const result = compileSvgRig(`
    <svg viewBox="-10 -10 20 20" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center">
      <path id="part.body" d="m 0 0 l 5 5" class="team-fill" fill="#112233" />
      <circle id="anchor.origin" cx="0" cy="0" r="1" />
      <circle id="anchor.origin" cx="1" cy="1" r="1" />
      <circle id="anchor.hp" cx="0" cy="-8" r="1" />
    </svg>
  `);
  assertError(result, "svg.unsupportedCss", "part.body");
  assertError(result, "svg.duplicateId", "anchor.origin");
  assertError(result, "svg.unsupportedPathCommand", "part.body.d");
});

test("missing required anchors fail closed through schema validation", () => {
  const result = compileSvgRig(`
    <svg viewBox="-10 -10 20 20" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center">
      <rect id="part.body" x="-2" y="-2" width="4" height="4" fill="#112233" />
      <circle id="anchor.origin" cx="0" cy="0" r="1" />
      <circle id="anchor.hp" cx="0" cy="-8" r="1" />
    </svg>
  `);
  assertError(result, "rig.missingRequiredAnchor", "anchors.selection");
});

test("kind mismatches and non-decomposable transforms are rejected", () => {
  assertError(
    compileSvgRig(readFixture("rig-worker.svg"), { expectedKind: KIND.TANK }),
    "rig.unitKindMismatch",
    "kind",
  );
  assertError(
    compileSvgRig(`
      <svg viewBox="-10 -10 20 20" data-rts-rig-kind="worker" data-rts-rig-version="1" data-rts-origin="center">
        <rect id="part.body" x="-2" y="-2" width="4" height="4" fill="#112233" transform="matrix(1 0 1 1 0 0)" />
        <circle id="anchor.origin" cx="0" cy="0" r="1" />
        <circle id="anchor.selection" cx="0" cy="0" r="1" />
        <circle id="anchor.hp" cx="0" cy="-8" r="1" />
      </svg>
    `),
    "svg.unsupportedTransform",
    "part.body",
  );
});

function readFixture(file) {
  return fs.readFileSync(path.join(fixturesDir, file), "utf8");
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

function assertError(result, code, path) {
  assert.equal(result.ok, false, "expected importer to fail");
  assert.ok(
    result.errors.some((err) => err.code === code && err.path === path),
    `expected ${code} at ${path}, got ${JSON.stringify(result.errors)}`,
  );
}
