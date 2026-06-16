#!/usr/bin/env node
import assert from "node:assert/strict";
import {
  ANIMATION_INPUTS,
  ANIMATION_PROPERTIES,
  GEOMETRY_TYPES,
  REQUIRED_ANCHORS,
  RIG_SCHEMA_VERSION,
  TINT_SLOTS,
  validateRigDefinition,
} from "../client/src/renderer/rigs/schema.js";
import { KIND } from "../client/src/protocol.js";

const validRig = {
  id: "worker.phase2.fixture",
  kind: KIND.WORKER,
  schemaVersion: RIG_SCHEMA_VERSION,
  requiredRuntimeInputs: ["teamColor", "facing", "selected", "teamColor"],
  anchors: {
    origin: { x: 0, y: 0 },
    selection: { x: 0, y: 0 },
    hp: { x: 0, y: -20 },
  },
  bounds: {
    selection: { x: -12, y: -12, width: 24, height: 24 },
    hp: { x: -10, y: -24, width: 20, height: 4 },
  },
  parts: [
    {
      id: "part.body",
      drawOrder: 10,
      tintSlot: "team",
      pivot: { x: 0, y: 0 },
      transform: { x: 1, y: 2, rotation: 0.25, scaleX: 1, scaleY: 1 },
      geometry: {
        type: "polygon",
        points: [
          [12, 0],
          { x: -8, y: -8 },
          { x: -8, y: 8 },
        ],
      },
      paint: { fill: "#6d89b8", stroke: null, strokeWidth: null, opacity: 1 },
    },
    {
      id: "part.shadow",
      drawOrder: 0,
      tintSlot: "fixed",
      geometry: { type: "ellipse", cx: 0, cy: 2, rx: 11, ry: 7 },
      paint: { fill: "#000000", stroke: null, strokeWidth: null, opacity: 0.32 },
    },
  ],
  animations: [
    {
      partId: "part.body",
      input: "facing",
      property: "transform.rotation",
      factor: 1,
      offset: 0,
    },
  ],
};

test("valid rigs normalize to stable plain data", () => {
  const result = validateRigDefinition(validRig, { expectedKind: KIND.WORKER });
  assert.equal(result.ok, true);
  assert.deepEqual(result.errors, []);
  assert.equal(result.definition.id, validRig.id);
  assert.equal(result.definition.schemaVersion, RIG_SCHEMA_VERSION);
  assert.deepEqual(result.definition.requiredRuntimeInputs, ["teamColor", "facing", "selected"]);
  assert.deepEqual(result.definition.parts.map((part) => part.id), ["part.shadow", "part.body"]);
  assert.deepEqual(result.definition.parts[1].transform, { x: 1, y: 2, rotation: 0.25, scaleX: 1, scaleY: 1 });
  assert.deepEqual(result.definition.parts[1].pivot, { x: 0, y: 0 });
  assert.deepEqual(result.definition.parts[1].paint, { fill: "#6d89b8", stroke: null, strokeWidth: null, opacity: 1 });
});

test("public enum lists document the phase-2 contract", () => {
  assert.deepEqual(REQUIRED_ANCHORS, ["origin", "selection", "hp"]);
  assert.ok(TINT_SLOTS.includes("team"));
  assert.ok(GEOMETRY_TYPES.includes("path"));
  assert.ok(ANIMATION_INPUTS.includes("setupVisual"));
  assert.ok(ANIMATION_PROPERTIES.includes("transform.rotation"));
});

test("missing required anchors fail closed", () => {
  const result = validateRigDefinition(withPatch({ anchors: { origin: { x: 0, y: 0 }, hp: { x: 0, y: -20 } } }));
  assertError(result, "rig.missingRequiredAnchor", "anchors.selection");
});

test("duplicate part ids fail closed", () => {
  const result = validateRigDefinition(withPatch({
    parts: [
      validRig.parts[0],
      { ...validRig.parts[1], id: "part.body" },
    ],
  }));
  assertError(result, "rig.duplicatePartId", "parts.1.id");
});

test("unsupported transform components fail closed", () => {
  const rig = clone(validRig);
  rig.parts[0].transform = { ...rig.parts[0].transform, skewX: 0.5 };
  const result = validateRigDefinition(rig);
  assertError(result, "rig.unsupportedTransform", "parts.0.transform.skewX");
});

test("non-finite geometry fails closed", () => {
  const rig = clone(validRig);
  rig.parts[0].geometry.points[1] = { x: Infinity, y: 1 };
  const result = validateRigDefinition(rig);
  assertError(result, "rig.nonFiniteNumber", "parts.0.geometry.points.1.x");
});

test("invalid tint slots fail closed", () => {
  const rig = clone(validRig);
  rig.parts[0].tintSlot = "player-color";
  const result = validateRigDefinition(rig);
  assertError(result, "rig.invalidTintSlot", "parts.0.tintSlot");
});

test("invalid paint values fail closed", () => {
  const rig = clone(validRig);
  rig.parts[0].paint = { fill: "red", stroke: "#12345", strokeWidth: 0, opacity: 2 };
  const result = validateRigDefinition(rig);
  assertError(result, "rig.invalidPaintColor", "parts.0.paint.fill");
  assertError(result, "rig.invalidPaintColor", "parts.0.paint.stroke");
  assertError(result, "rig.nonPositiveNumber", "parts.0.paint.strokeWidth");
  assertError(result, "rig.outOfRangeNumber", "parts.0.paint.opacity");
});

test("invalid animation references fail closed", () => {
  const rig = clone(validRig);
  rig.animations[0].partId = "part.missing";
  const result = validateRigDefinition(rig);
  assertError(result, "rig.invalidAnimationReference", "animations.0.partId");
});

test("invalid animation inputs fail closed", () => {
  const rig = clone(validRig);
  rig.animations[0].input = "globalRenderer";
  const result = validateRigDefinition(rig);
  assertError(result, "rig.invalidAnimationInput", "animations.0.input");
});

test("unit-kind mismatches fail closed", () => {
  assertError(validateRigDefinition(validRig, { expectedKind: KIND.RIFLEMAN }), "rig.unitKindMismatch", "kind");
  assertError(validateRigDefinition(withPatch({ authoredKind: KIND.RIFLEMAN })), "rig.unitKindMismatch", "authoredKind");
});

test("unsupported schema, geometry, runtime inputs, and path commands fail closed", () => {
  const rig = clone(validRig);
  rig.schemaVersion = 99;
  rig.requiredRuntimeInputs = ["teamColor", "camera"];
  rig.parts[0].geometry = { type: "filter", radius: 10 };
  rig.parts[1].geometry = { type: "path", commands: [{ command: "A", values: [1, 2, 3] }] };
  const result = validateRigDefinition(rig);
  assertError(result, "rig.unsupportedSchemaVersion", "schemaVersion");
  assertError(result, "rig.invalidRuntimeInput", "requiredRuntimeInputs.1");
  assertError(result, "rig.unsupportedGeometry", "parts.0.geometry.type");
  assertError(result, "rig.invalidPathCommand", "parts.1.geometry.commands.0");
});

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
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((err) => err.code === code && err.path === path),
    `expected ${code} at ${path}, got ${JSON.stringify(result.errors)}`,
  );
}

function withPatch(patch) {
  return { ...clone(validRig), ...patch };
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
