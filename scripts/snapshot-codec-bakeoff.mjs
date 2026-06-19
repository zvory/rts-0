#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const DEFAULT_PACKET_BUDGET_BYTES = 1280;
const DEFAULT_ITERATIONS = 5;
const DEFAULT_FIXTURE_VERSION = 22;
const UTF8 = new TextEncoder();
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });

const VALUE_NULL = 0;
const VALUE_FALSE = 1;
const VALUE_TRUE = 2;
const VALUE_UINT = 3;
const VALUE_SINT = 4;
const VALUE_F64 = 5;
const VALUE_STRING = 6;
const VALUE_ARRAY = 7;
const VALUE_OBJECT = 8;

const SNAPSHOT_MAGIC = [0x52, 0x53, 0x4e, 0x50]; // RSNP
const PROTO_MAGIC = [0x52, 0x53, 0x50, 0x42]; // RSPB
const CUSTOM_BINARY_VERSION = 1;

const TOP_LEVEL_SNAPSHOT_KEYS = Object.freeze([
  "s",
  "e",
  "r",
  "sm",
  "ao",
  "fg",
  "mb",
  "ev",
  "pr",
  "u",
  "n",
]);

const KNOWN_OBJECT_KEYS = Object.freeze([
  "t",
  "v",
  ...TOP_LEVEL_SNAPSHOT_KEYS,
  "tick",
  "steel",
  "oil",
  "supplyUsed",
  "supplyCap",
  "entities",
  "resourceDeltas",
  "smokes",
  "abilityObjects",
  "visibleTiles",
  "rememberedBuildings",
  "events",
  "playerResources",
  "upgrades",
  "netStatus",
]);

const KNOWN_KEY_TO_CODE = new Map(KNOWN_OBJECT_KEYS.map((key, index) => [key, index + 1]));
const KNOWN_CODE_TO_KEY = new Map(KNOWN_OBJECT_KEYS.map((key, index) => [index + 1, key]));

const CANDIDATES = Object.freeze([
  {
    id: "compact-json",
    label: "Compact JSON",
    dependencyRisk: "none",
    browserSupportRisk: "none",
    maintenanceCost: "low",
    note: "Current live text-frame baseline.",
    encode: encodeCompactJson,
    decode: decodeCompactJson,
  },
  {
    id: "compact-json-deflate",
    label: "Compact JSON + deflate",
    dependencyRisk: "low",
    browserSupportRisk: "medium",
    maintenanceCost: "medium",
    note: "Offline deflateRaw proxy for permessage-deflate; not measured as actual browser wire bytes.",
    encode: encodeCompactJsonDeflate,
    decode: decodeCompactJsonDeflate,
  },
  {
    id: "proto-style-tlv",
    label: "Proto-style schema TLV",
    dependencyRisk: "medium",
    browserSupportRisk: "medium",
    maintenanceCost: "high",
    note: "Manual schema-TLV stand-in for generated protobuf; enough to compare key-table binary pressure.",
    encode: encodeProtoStyle,
    decode: decodeProtoStyle,
  },
  {
    id: "messagepack-compact",
    label: "MessagePack compact object",
    dependencyRisk: "medium",
    browserSupportRisk: "medium",
    maintenanceCost: "medium",
    note: "Schema-less binary encoding of the current compact positional object.",
    encode: encodeMessagePack,
    decode: decodeMessagePack,
  },
  {
    id: "cbor-compact",
    label: "CBOR compact object",
    dependencyRisk: "medium",
    browserSupportRisk: "medium",
    maintenanceCost: "medium",
    note: "Schema-less binary encoding of the current compact positional object.",
    encode: encodeCbor,
    decode: decodeCbor,
  },
  {
    id: "custom-positional-binary",
    label: "Custom positional binary",
    dependencyRisk: "none",
    browserSupportRisk: "high",
    maintenanceCost: "high",
    note: "Versioned custom binary for the compact snapshot top-level shape with generic nested values.",
    encode: encodeCustomSnapshot,
    decode: decodeCustomSnapshot,
  },
]);

export function runSnapshotCodecBakeoff({
  frames,
  label = "snapshot-codec-bakeoff",
  budgetBytes = DEFAULT_PACKET_BUDGET_BYTES,
  iterations = DEFAULT_ITERATIONS,
  generatedAt = new Date().toISOString(),
} = {}) {
  const normalized = normalizeFrames(frames);
  if (normalized.length === 0) {
    throw new Error("snapshot codec bakeoff requires at least one snapshot frame");
  }

  const samples = normalized.map((frame, index) => ({
    index,
    text: frame.text,
    object: frame.object,
  }));

  const candidates = CANDIDATES.map((candidate) =>
    measureCandidate(candidate, samples, budgetBytes, iterations),
  );
  const baseline = candidates.find((candidate) => candidate.id === "compact-json");
  const smallest = [...candidates].sort((a, b) => a.bytes.p95 - b.bytes.p95)[0];

  return {
    schemaVersion: 1,
    generatedAt,
    label,
    sampleCount: samples.length,
    budgetBytes,
    iterations,
    candidates,
    recommendation: recommendationFor({ baseline, smallest, candidates }),
  };
}

export function formatBakeoffMarkdown(result) {
  const lines = [];
  lines.push("# Snapshot Codec Bake-off");
  lines.push("");
  lines.push(`Generated: ${result.generatedAt}`);
  lines.push(`Source: ${result.label}`);
  lines.push(`Samples: ${result.sampleCount}`);
  lines.push(`Payload budget: ${result.budgetBytes} bytes`);
  lines.push("");
  lines.push("| candidate | p50 bytes | p95 bytes | max bytes | over budget | encode p95 ms | decode p95 ms | dependency risk | browser risk | maintenance |");
  lines.push("|---|---:|---:|---:|---:|---:|---:|---|---|---|");
  for (const row of result.candidates) {
    lines.push(
      `| ${row.label} | ${row.bytes.p50} | ${row.bytes.p95} | ${row.bytes.max} | ${formatPctX100(row.bytes.overBudgetPctX100)} | ${formatMs(row.encodeMs.p95)} | ${formatMs(row.decodeMs.p95)} | ${row.dependencyRisk} | ${row.browserSupportRisk} | ${row.maintenanceCost} |`,
    );
  }
  lines.push("");
  lines.push("## Notes");
  lines.push("");
  for (const row of result.candidates) {
    lines.push(`- ${row.label}: ${row.note}`);
  }
  lines.push("");
  lines.push("## Recommendation");
  lines.push("");
  lines.push(result.recommendation.summary);
  lines.push("");
  lines.push(result.recommendation.reason);
  lines.push("");
  lines.push("## Limits");
  lines.push("");
  lines.push("- Deflate numbers are compressed payload bytes from Node zlib, not verified browser post-extension wire bytes.");
  lines.push("- Browser apply cost is unchanged unless a live client decoder replaces the current JSON path; this bake-off measures candidate encode/decode CPU only.");
  lines.push("- Raw snapshot payloads are not uploaded by normal clients; only local harness captures should be used as inputs.");
  return `${lines.join("\n")}\n`;
}

export function loadSnapshotFramesFromFile(inputPath) {
  const text = fs.readFileSync(inputPath, "utf8");
  if (inputPath.endsWith(".jsonl")) {
    return text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map(parseFrameLine);
  }
  const parsed = JSON.parse(text);
  if (Array.isArray(parsed)) return parsed;
  if (Array.isArray(parsed.snapshotFrames)) return parsed.snapshotFrames;
  if (Array.isArray(parsed.snapshots)) return parsed.snapshots;
  throw new Error(`${inputPath} does not contain a snapshot frame array`);
}

export function fixtureSnapshotFrames() {
  return [
    compactSnapshotFixture({ tick: 1, entityCount: 8, visibleRuns: [1, 40, 0, 80, 1, 40] }),
    compactSnapshotFixture({
      tick: 300,
      entityCount: 96,
      resourceCount: 12,
      eventCount: 6,
      visibleRuns: [1, 160, 0, 380, 1, 220, 0, 180, 1, 84],
    }),
    compactSnapshotFixture({
      tick: 1200,
      entityCount: 420,
      resourceCount: 24,
      eventCount: 18,
      visibleRuns: [1, 300, 0, 620, 1, 512, 0, 760, 1, 480, 0, 100],
      spectatorResources: true,
    }),
  ].map((snapshot) => JSON.stringify(snapshot));
}

export function assertMalformedBinaryRejected() {
  for (const [label, decode] of [
    ["proto-style-tlv", decodeProtoStyle],
    ["custom-positional-binary", decodeCustomSnapshot],
    ["messagepack-compact", decodeMessagePack],
    ["cbor-compact", decodeCbor],
  ]) {
    let rejected = false;
    try {
      decode(new Uint8Array([0xff, 0x00, 0x01]));
    } catch {
      rejected = true;
    }
    if (!rejected) throw new Error(`${label} decoder accepted malformed binary`);
  }
}

function parseFrameLine(line) {
  const parsed = JSON.parse(line);
  return typeof parsed === "string" ? parsed : JSON.stringify(parsed);
}

function normalizeFrames(frames) {
  if (!Array.isArray(frames)) throw new Error("frames must be an array");
  return frames.map((frame, index) => {
    const text = typeof frame === "string" ? frame : JSON.stringify(frame);
    const object = JSON.parse(text);
    if (!object || object.t !== "snapshot") {
      throw new Error(`frame ${index} is not a snapshot message`);
    }
    return { text, object };
  });
}

function measureCandidate(candidate, samples, budgetBytes, iterations) {
  const encoded = [];
  const encodeMs = [];
  const decodeMs = [];

  for (const sample of samples) {
    const first = candidate.encode(sample.object);
    assertRoundTrip(candidate, sample.object, first);
    encoded.push(first);
    encodeMs.push(timedAverage(() => candidate.encode(sample.object), iterations));
  }

  for (const payload of encoded) {
    decodeMs.push(timedAverage(() => candidate.decode(payload), iterations));
  }

  const sizes = encoded.map((payload) => payload.byteLength);
  const overBudget = sizes.filter((size) => size > budgetBytes).length;
  return {
    id: candidate.id,
    label: candidate.label,
    bytes: summarizeNumbers(sizes, {
      overBudgetCount: overBudget,
      overBudgetPctX100: pctX100(overBudget, sizes.length),
    }),
    encodeMs: summarizeNumbers(encodeMs),
    decodeMs: summarizeNumbers(decodeMs),
    dependencyRisk: candidate.dependencyRisk,
    browserSupportRisk: candidate.browserSupportRisk,
    maintenanceCost: candidate.maintenanceCost,
    note: candidate.note,
  };
}

function timedAverage(fn, iterations) {
  const count = Math.max(1, iterations | 0);
  const start = performance.now();
  for (let i = 0; i < count; i += 1) fn();
  return (performance.now() - start) / count;
}

function assertRoundTrip(candidate, expected, encoded) {
  const decoded = candidate.decode(encoded);
  const left = canonicalJson(expected);
  const right = canonicalJson(decoded);
  if (left !== right) {
    throw new Error(`${candidate.id} round-trip mismatch\nexpected=${left}\nactual=${right}`);
  }
}

function summarizeNumbers(values, extra = {}) {
  const sorted = [...values].sort((a, b) => a - b);
  const total = sorted.reduce((sum, value) => sum + value, 0);
  return {
    count: sorted.length,
    total: Math.round(total),
    avg: sorted.length > 0 ? Math.round(total / sorted.length) : 0,
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    p99: percentile(sorted, 99),
    max: sorted.at(-1) ?? 0,
    ...extra,
  };
}

function percentile(sorted, pct) {
  if (sorted.length === 0) return 0;
  const index = Math.min(sorted.length - 1, Math.ceil((sorted.length * pct) / 100) - 1);
  const value = sorted[index];
  return Number.isInteger(value) ? value : Number(value.toFixed(4));
}

function pctX100(part, whole) {
  return whole > 0 ? Math.round((part * 10000) / whole) : 0;
}

function recommendationFor({ baseline, smallest, candidates }) {
  const deflate = candidates.find((candidate) => candidate.id === "compact-json-deflate");
  const custom = candidates.find((candidate) => candidate.id === "custom-positional-binary");
  const deflateWins =
    deflate && baseline && deflate.bytes.p95 < Math.round(baseline.bytes.p95 * 0.75);
  const customWins =
    custom && baseline && custom.bytes.p95 < Math.round(baseline.bytes.p95 * 0.75);
  if (deflateWins) {
    return {
      summary: "Keep compact JSON as the default and run a focused WebSocket compression follow-up before any binary codec rollout.",
      reason:
        `Deflate had the smallest measured p95 (${deflate.bytes.p95} bytes vs compact JSON ${baseline.bytes.p95}), ` +
        "but the current measurement is an offline proxy and the server/browser extension path still needs live verification.",
    };
  }
  if (customWins) {
    return {
      summary: "Keep compact JSON as the default and defer custom binary unless delta work still leaves packet pressure unresolved.",
      reason:
        `The custom positional binary had the smallest measured p95 (${custom.bytes.p95} bytes vs compact JSON ${baseline.bytes.p95}), ` +
        "but it carries high browser and maintenance risk compared with the upcoming fog-safe delta phases.",
    };
  }
  return {
    summary: "Keep compact JSON as the default and prioritize fog-safe delta design over a format-only binary rollout.",
    reason:
      `The smallest measured candidate was ${smallest.label} at p95 ${smallest.bytes.p95} bytes, while compact JSON was p95 ${baseline.bytes.p95}. ` +
      "The measured savings do not justify changing the live codec before delta/keyframe work proves a larger win.",
  };
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function encodeCompactJson(value) {
  return UTF8.encode(JSON.stringify(value));
}

function decodeCompactJson(bytes) {
  return JSON.parse(UTF8_DECODER.decode(toUint8Array(bytes)));
}

function encodeCompactJsonDeflate(value) {
  return zlib.deflateRawSync(Buffer.from(JSON.stringify(value), "utf8"));
}

function decodeCompactJsonDeflate(bytes) {
  return JSON.parse(zlib.inflateRawSync(Buffer.from(toUint8Array(bytes))).toString("utf8"));
}

function encodeProtoStyle(value) {
  const writer = new ByteWriter();
  writer.bytes(PROTO_MAGIC);
  writeTypedValue(writer, value);
  return writer.finish();
}

function decodeProtoStyle(bytes) {
  const reader = new ByteReader(bytes);
  reader.magic(PROTO_MAGIC, "proto-style snapshot codec");
  const value = readTypedValue(reader);
  reader.done();
  return value;
}

function encodeCustomSnapshot(snapshot) {
  const writer = new ByteWriter();
  writer.bytes(SNAPSHOT_MAGIC);
  writer.byte(CUSTOM_BINARY_VERSION);
  writer.varUint(snapshot.v ?? DEFAULT_FIXTURE_VERSION);
  for (const key of TOP_LEVEL_SNAPSHOT_KEYS) {
    if (!Object.prototype.hasOwnProperty.call(snapshot, key)) continue;
    writer.varUint(TOP_LEVEL_SNAPSHOT_KEYS.indexOf(key) + 1);
    writeTypedValue(writer, snapshot[key]);
  }
  writer.varUint(0);
  return writer.finish();
}

function decodeCustomSnapshot(bytes) {
  const reader = new ByteReader(bytes);
  reader.magic(SNAPSHOT_MAGIC, "custom positional snapshot codec");
  const codecVersion = reader.u8();
  if (codecVersion !== CUSTOM_BINARY_VERSION) {
    throw new Error(`unsupported custom snapshot binary version ${codecVersion}`);
  }
  const snapshot = {
    t: "snapshot",
    v: Number(reader.varUint()),
  };
  for (;;) {
    const code = Number(reader.varUint());
    if (code === 0) break;
    const key = TOP_LEVEL_SNAPSHOT_KEYS[code - 1];
    if (!key) throw new Error(`unknown custom snapshot top-level field ${code}`);
    snapshot[key] = readTypedValue(reader);
  }
  reader.done();
  return snapshot;
}

function writeTypedValue(writer, value) {
  if (value === null || value === undefined) {
    writer.byte(VALUE_NULL);
  } else if (value === false) {
    writer.byte(VALUE_FALSE);
  } else if (value === true) {
    writer.byte(VALUE_TRUE);
  } else if (typeof value === "number") {
    if (Number.isInteger(value) && value >= 0 && value <= Number.MAX_SAFE_INTEGER) {
      writer.byte(VALUE_UINT);
      writer.varUint(value);
    } else if (Number.isInteger(value) && value >= Number.MIN_SAFE_INTEGER) {
      writer.byte(VALUE_SINT);
      writer.varSint(value);
    } else if (Number.isFinite(value)) {
      writer.byte(VALUE_F64);
      writer.f64(value);
    } else {
      throw new Error(`cannot encode non-finite number ${value}`);
    }
  } else if (typeof value === "string") {
    writer.byte(VALUE_STRING);
    writer.string(value);
  } else if (Array.isArray(value)) {
    writer.byte(VALUE_ARRAY);
    writer.varUint(value.length);
    for (const item of value) writeTypedValue(writer, item);
  } else if (typeof value === "object") {
    const entries = Object.entries(value).filter(([, item]) => item !== undefined);
    writer.byte(VALUE_OBJECT);
    writer.varUint(entries.length);
    for (const [key, item] of entries) {
      const code = KNOWN_KEY_TO_CODE.get(key) || 0;
      writer.varUint(code);
      if (code === 0) writer.string(key);
      writeTypedValue(writer, item);
    }
  } else {
    throw new Error(`unsupported value type ${typeof value}`);
  }
}

function readTypedValue(reader) {
  const tag = reader.u8();
  if (tag === VALUE_NULL) return null;
  if (tag === VALUE_FALSE) return false;
  if (tag === VALUE_TRUE) return true;
  if (tag === VALUE_UINT) return Number(reader.varUint());
  if (tag === VALUE_SINT) return Number(reader.varSint());
  if (tag === VALUE_F64) return reader.f64();
  if (tag === VALUE_STRING) return reader.string();
  if (tag === VALUE_ARRAY) {
    const len = Number(reader.varUint());
    const out = [];
    for (let i = 0; i < len; i += 1) out.push(readTypedValue(reader));
    return out;
  }
  if (tag === VALUE_OBJECT) {
    const len = Number(reader.varUint());
    const out = {};
    for (let i = 0; i < len; i += 1) {
      const code = Number(reader.varUint());
      const key = code === 0 ? reader.string() : KNOWN_CODE_TO_KEY.get(code);
      if (!key) throw new Error(`unknown proto-style object key code ${code}`);
      out[key] = readTypedValue(reader);
    }
    return out;
  }
  throw new Error(`unknown typed value tag ${tag}`);
}

function encodeMessagePack(value) {
  const writer = new ByteWriter();
  writeMessagePackValue(writer, value);
  return writer.finish();
}

function decodeMessagePack(bytes) {
  const reader = new ByteReader(bytes);
  const value = readMessagePackValue(reader);
  reader.done();
  return value;
}

function writeMessagePackValue(writer, value) {
  if (value === null || value === undefined) {
    writer.byte(0xc0);
  } else if (value === false) {
    writer.byte(0xc2);
  } else if (value === true) {
    writer.byte(0xc3);
  } else if (typeof value === "number") {
    writeMessagePackNumber(writer, value);
  } else if (typeof value === "string") {
    const bytes = UTF8.encode(value);
    if (bytes.length < 32) writer.byte(0xa0 | bytes.length);
    else if (bytes.length <= 0xff) {
      writer.byte(0xd9);
      writer.byte(bytes.length);
    } else if (bytes.length <= 0xffff) {
      writer.byte(0xda);
      writer.u16(bytes.length);
    } else {
      writer.byte(0xdb);
      writer.u32(bytes.length);
    }
    writer.bytes(bytes);
  } else if (Array.isArray(value)) {
    if (value.length < 16) writer.byte(0x90 | value.length);
    else if (value.length <= 0xffff) {
      writer.byte(0xdc);
      writer.u16(value.length);
    } else {
      writer.byte(0xdd);
      writer.u32(value.length);
    }
    for (const item of value) writeMessagePackValue(writer, item);
  } else if (typeof value === "object") {
    const entries = Object.entries(value).filter(([, item]) => item !== undefined);
    if (entries.length < 16) writer.byte(0x80 | entries.length);
    else if (entries.length <= 0xffff) {
      writer.byte(0xde);
      writer.u16(entries.length);
    } else {
      writer.byte(0xdf);
      writer.u32(entries.length);
    }
    for (const [key, item] of entries) {
      writeMessagePackValue(writer, key);
      writeMessagePackValue(writer, item);
    }
  } else {
    throw new Error(`unsupported MessagePack value type ${typeof value}`);
  }
}

function writeMessagePackNumber(writer, value) {
  if (!Number.isFinite(value)) throw new Error(`cannot encode non-finite number ${value}`);
  if (Number.isInteger(value) && value >= 0 && value <= 0x7f) writer.byte(value);
  else if (Number.isInteger(value) && value >= 0 && value <= 0xff) {
    writer.byte(0xcc);
    writer.byte(value);
  } else if (Number.isInteger(value) && value >= 0 && value <= 0xffff) {
    writer.byte(0xcd);
    writer.u16(value);
  } else if (Number.isInteger(value) && value >= 0 && value <= 0xffffffff) {
    writer.byte(0xce);
    writer.u32(value);
  } else if (Number.isInteger(value) && value >= -32 && value < 0) {
    writer.byte(0xe0 | (value + 32));
  } else if (Number.isInteger(value) && value >= -128 && value < 0) {
    writer.byte(0xd0);
    writer.i8(value);
  } else if (Number.isInteger(value) && value >= -32768 && value < 0) {
    writer.byte(0xd1);
    writer.i16(value);
  } else if (Number.isInteger(value) && value >= -2147483648 && value < 0) {
    writer.byte(0xd2);
    writer.i32(value);
  } else {
    writer.byte(0xcb);
    writer.f64(value);
  }
}

function readMessagePackValue(reader) {
  const tag = reader.u8();
  if (tag <= 0x7f) return tag;
  if ((tag & 0xe0) === 0xa0) return reader.stringFixed(tag & 0x1f);
  if ((tag & 0xf0) === 0x90) return readArrayItems(reader, tag & 0x0f, readMessagePackValue);
  if ((tag & 0xf0) === 0x80) return readMapItems(reader, tag & 0x0f, readMessagePackValue);
  if (tag >= 0xe0) return tag - 0x100;
  if (tag === 0xc0) return null;
  if (tag === 0xc2) return false;
  if (tag === 0xc3) return true;
  if (tag === 0xcc) return reader.u8();
  if (tag === 0xcd) return reader.u16();
  if (tag === 0xce) return reader.u32();
  if (tag === 0xd0) return reader.i8();
  if (tag === 0xd1) return reader.i16();
  if (tag === 0xd2) return reader.i32();
  if (tag === 0xcb) return reader.f64();
  if (tag === 0xd9) return reader.stringFixed(reader.u8());
  if (tag === 0xda) return reader.stringFixed(reader.u16());
  if (tag === 0xdb) return reader.stringFixed(reader.u32());
  if (tag === 0xdc) return readArrayItems(reader, reader.u16(), readMessagePackValue);
  if (tag === 0xdd) return readArrayItems(reader, reader.u32(), readMessagePackValue);
  if (tag === 0xde) return readMapItems(reader, reader.u16(), readMessagePackValue);
  if (tag === 0xdf) return readMapItems(reader, reader.u32(), readMessagePackValue);
  throw new Error(`unsupported MessagePack tag 0x${tag.toString(16)}`);
}

function encodeCbor(value) {
  const writer = new ByteWriter();
  writeCborValue(writer, value);
  return writer.finish();
}

function decodeCbor(bytes) {
  const reader = new ByteReader(bytes);
  const value = readCborValue(reader);
  reader.done();
  return value;
}

function writeCborValue(writer, value) {
  if (value === null || value === undefined) {
    writer.byte(0xf6);
  } else if (value === false) {
    writer.byte(0xf4);
  } else if (value === true) {
    writer.byte(0xf5);
  } else if (typeof value === "number") {
    if (Number.isInteger(value) && value >= 0 && value <= Number.MAX_SAFE_INTEGER) {
      writeCborHead(writer, 0, value);
    } else if (Number.isInteger(value) && value < 0 && value >= Number.MIN_SAFE_INTEGER) {
      writeCborHead(writer, 1, -1 - value);
    } else if (Number.isFinite(value)) {
      writer.byte(0xfb);
      writer.f64(value);
    } else {
      throw new Error(`cannot encode non-finite number ${value}`);
    }
  } else if (typeof value === "string") {
    const bytes = UTF8.encode(value);
    writeCborHead(writer, 3, bytes.length);
    writer.bytes(bytes);
  } else if (Array.isArray(value)) {
    writeCborHead(writer, 4, value.length);
    for (const item of value) writeCborValue(writer, item);
  } else if (typeof value === "object") {
    const entries = Object.entries(value).filter(([, item]) => item !== undefined);
    writeCborHead(writer, 5, entries.length);
    for (const [key, item] of entries) {
      writeCborValue(writer, key);
      writeCborValue(writer, item);
    }
  } else {
    throw new Error(`unsupported CBOR value type ${typeof value}`);
  }
}

function writeCborHead(writer, major, value) {
  if (value < 24) writer.byte((major << 5) | value);
  else if (value <= 0xff) {
    writer.byte((major << 5) | 24);
    writer.byte(value);
  } else if (value <= 0xffff) {
    writer.byte((major << 5) | 25);
    writer.u16(value);
  } else if (value <= 0xffffffff) {
    writer.byte((major << 5) | 26);
    writer.u32(value);
  } else {
    writer.byte((major << 5) | 27);
    writer.u64(value);
  }
}

function readCborValue(reader) {
  const first = reader.u8();
  if (first === 0xf4) return false;
  if (first === 0xf5) return true;
  if (first === 0xf6) return null;
  if (first === 0xfb) return reader.f64();
  const major = first >> 5;
  const value = readCborArgument(reader, first & 0x1f);
  if (major === 0) return Number(value);
  if (major === 1) return Number(-1n - value);
  if (major === 3) return reader.stringFixed(Number(value));
  if (major === 4) return readArrayItems(reader, Number(value), readCborValue);
  if (major === 5) return readMapItems(reader, Number(value), readCborValue);
  throw new Error(`unsupported CBOR major type ${major}`);
}

function readCborArgument(reader, addl) {
  if (addl < 24) return BigInt(addl);
  if (addl === 24) return BigInt(reader.u8());
  if (addl === 25) return BigInt(reader.u16());
  if (addl === 26) return BigInt(reader.u32());
  if (addl === 27) return reader.u64();
  throw new Error(`unsupported CBOR additional value ${addl}`);
}

function readArrayItems(reader, len, readValue) {
  const out = [];
  for (let i = 0; i < len; i += 1) out.push(readValue(reader));
  return out;
}

function readMapItems(reader, len, readValue) {
  const out = {};
  for (let i = 0; i < len; i += 1) {
    const key = readValue(reader);
    if (typeof key !== "string") throw new Error("map key must be a string");
    out[key] = readValue(reader);
  }
  return out;
}

class ByteWriter {
  constructor() {
    this.out = [];
  }

  byte(value) {
    this.out.push(value & 0xff);
  }

  bytes(values) {
    for (const value of values) this.byte(value);
  }

  u16(value) {
    this.byte(value >> 8);
    this.byte(value);
  }

  u32(value) {
    this.byte(value >>> 24);
    this.byte(value >>> 16);
    this.byte(value >>> 8);
    this.byte(value);
  }

  u64(value) {
    let n = BigInt(value);
    const bytes = new Array(8);
    for (let i = 7; i >= 0; i -= 1) {
      bytes[i] = Number(n & 0xffn);
      n >>= 8n;
    }
    this.bytes(bytes);
  }

  i8(value) {
    this.byte(value);
  }

  i16(value) {
    this.u16(value & 0xffff);
  }

  i32(value) {
    this.u32(value >>> 0);
  }

  f64(value) {
    const buffer = new ArrayBuffer(8);
    new DataView(buffer).setFloat64(0, value, false);
    this.bytes(new Uint8Array(buffer));
  }

  string(value) {
    const bytes = UTF8.encode(value);
    this.varUint(bytes.length);
    this.bytes(bytes);
  }

  varUint(value) {
    let n = BigInt(value);
    if (n < 0n) throw new Error("varUint cannot encode negative values");
    while (n >= 0x80n) {
      this.byte(Number((n & 0x7fn) | 0x80n));
      n >>= 7n;
    }
    this.byte(Number(n));
  }

  varSint(value) {
    const n = BigInt(value);
    this.varUint(n >= 0n ? n * 2n : -n * 2n - 1n);
  }

  finish() {
    return Uint8Array.from(this.out);
  }
}

class ByteReader {
  constructor(bytes) {
    this.bytes = toUint8Array(bytes);
    this.pos = 0;
  }

  ensure(count) {
    if (this.pos + count > this.bytes.length) throw new Error("truncated binary payload");
  }

  done() {
    if (this.pos !== this.bytes.length) {
      throw new Error(`binary payload has ${this.bytes.length - this.pos} trailing bytes`);
    }
  }

  magic(expected, label) {
    this.ensure(expected.length);
    for (const byte of expected) {
      if (this.u8() !== byte) throw new Error(`${label} magic mismatch`);
    }
  }

  u8() {
    this.ensure(1);
    return this.bytes[this.pos++];
  }

  u16() {
    return (this.u8() << 8) | this.u8();
  }

  u32() {
    return ((this.u8() * 0x1000000) + ((this.u8() << 16) | (this.u8() << 8) | this.u8())) >>> 0;
  }

  u64() {
    let value = 0n;
    for (let i = 0; i < 8; i += 1) value = (value << 8n) | BigInt(this.u8());
    return value;
  }

  i8() {
    const value = this.u8();
    return value & 0x80 ? value - 0x100 : value;
  }

  i16() {
    const value = this.u16();
    return value & 0x8000 ? value - 0x10000 : value;
  }

  i32() {
    const value = this.u32();
    return value > 0x7fffffff ? value - 0x100000000 : value;
  }

  f64() {
    this.ensure(8);
    const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.pos, 8).getFloat64(0, false);
    this.pos += 8;
    return value;
  }

  string() {
    return this.stringFixed(Number(this.varUint()));
  }

  stringFixed(len) {
    this.ensure(len);
    const slice = this.bytes.subarray(this.pos, this.pos + len);
    this.pos += len;
    return UTF8_DECODER.decode(slice);
  }

  varUint() {
    let shift = 0n;
    let value = 0n;
    for (let i = 0; i < 10; i += 1) {
      const byte = this.u8();
      value |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return value;
      shift += 7n;
    }
    throw new Error("varUint is too long");
  }

  varSint() {
    const raw = this.varUint();
    return (raw & 1n) === 0n ? raw / 2n : -(raw / 2n) - 1n;
  }
}

function toUint8Array(value) {
  if (value instanceof Uint8Array) return value;
  if (Buffer.isBuffer(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  throw new Error("expected binary data");
}

function compactSnapshotFixture({
  tick,
  entityCount,
  resourceCount = 4,
  eventCount = 0,
  visibleRuns,
  spectatorResources = false,
}) {
  const entities = [];
  for (let i = 0; i < entityCount; i += 1) {
    const kind = i % 17 === 0 ? 5 : i % 11 === 0 ? 14 : i % 7 === 0 ? 2 : 1;
    const state = i % 9 === 0 ? 3 : i % 5 === 0 ? 2 : 1;
    const record = [
      i + 1,
      (i % 4) + 1,
      kind,
      128 + (i % 32) * 16,
      256 + Math.floor(i / 32) * 18,
      35 + (i % 90),
      120,
      state,
    ];
    if (i % 6 === 0) {
      record[8] = Number(((i % 8) * 0.25).toFixed(2));
      record[9] = Number(((i % 12) * 0.2).toFixed(2));
      record[15] = Math.max(1, i - 1);
    }
    if (i % 13 === 0) {
      record[18] = [320 + i, 480 + i];
      record[21] = [[1, 320 + i, 480 + i], [2, 360 + i, 512 + i]];
    }
    while (record.length > 8 && record.at(-1) == null) record.pop();
    entities.push(record);
  }
  const snapshot = {
    t: "snapshot",
    v: DEFAULT_FIXTURE_VERSION,
    s: [tick, 900, 450, 28, 80],
    e: entities,
    r: Array.from({ length: resourceCount }, (_, i) => [200 + i, 1800 - i * 17]),
    fg: visibleRuns,
    n: [2, 6, 0, 0, 0, 1, tick % 100, tick],
  };
  if (eventCount > 0) {
    snapshot.ev = Array.from({ length: eventCount }, (_, i) =>
      i % 3 === 0
        ? [1, i + 1, i + 2]
        : i % 3 === 1
          ? [2, i + 3, 512 + i * 8, 640 + i * 4, 1]
          : [4, "under attack", 3, 384 + i * 3, 448 + i * 2],
    );
  }
  if (spectatorResources) {
    snapshot.pr = [1, 800, 300, 22, 80, 2, 760, 280, 20, 80, 3, 700, 260, 24, 80, 4, 660, 220, 18, 80]
      .reduce((rows, _value, index, arr) => {
        if (index % 5 === 0) rows.push(arr.slice(index, index + 5));
        return rows;
      }, []);
  }
  return snapshot;
}

function formatPctX100(value) {
  return `${(value / 100).toFixed(2)}%`;
}

function formatMs(value) {
  return Number(value).toFixed(4);
}

function parseArgs(argv) {
  const args = {
    input: "",
    output: "",
    markdown: "",
    fixture: false,
    label: "",
    budgetBytes: DEFAULT_PACKET_BUDGET_BYTES,
    iterations: DEFAULT_ITERATIONS,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const value = () => {
      i += 1;
      if (i >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[i];
    };
    if (arg === "--fixture") args.fixture = true;
    else if (arg === "--input") args.input = value();
    else if (arg.startsWith("--input=")) args.input = arg.slice("--input=".length);
    else if (arg === "--output") args.output = value();
    else if (arg.startsWith("--output=")) args.output = arg.slice("--output=".length);
    else if (arg === "--markdown") args.markdown = value();
    else if (arg.startsWith("--markdown=")) args.markdown = arg.slice("--markdown=".length);
    else if (arg === "--label") args.label = value();
    else if (arg.startsWith("--label=")) args.label = arg.slice("--label=".length);
    else if (arg === "--budget-bytes") args.budgetBytes = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--budget-bytes=")) args.budgetBytes = parsePositiveInt(arg.slice("--budget-bytes=".length), "--budget-bytes");
    else if (arg === "--iterations") args.iterations = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--iterations=")) args.iterations = parsePositiveInt(arg.slice("--iterations=".length), "--iterations");
    else if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown arg: ${arg}`);
    }
  }
  return args;
}

function parsePositiveInt(raw, label) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function printHelp() {
  console.log(`Usage: node scripts/snapshot-codec-bakeoff.mjs [options]

Options:
  --fixture                    Use deterministic compact snapshot fixtures.
  --input <path>               Read snapshot frames from JSONL or JSON array.
  --output <path>              Write JSON summary.
  --markdown <path>            Write markdown summary.
  --label <name>               Label for the source workload.
  --budget-bytes <n>           Packet budget. Default: ${DEFAULT_PACKET_BUDGET_BYTES}.
  --iterations <n>             Encode/decode timing iterations per sample. Default: ${DEFAULT_ITERATIONS}.
`);
}

async function cli() {
  const args = parseArgs(process.argv.slice(2));
  const frames = args.fixture ? fixtureSnapshotFrames() : loadSnapshotFramesFromFile(args.input);
  const label = args.label || (args.fixture ? "deterministic-fixture" : path.basename(args.input));
  assertMalformedBinaryRejected();
  const result = runSnapshotCodecBakeoff({
    frames,
    label,
    budgetBytes: args.budgetBytes,
    iterations: args.iterations,
  });
  if (args.output) {
    fs.mkdirSync(path.dirname(path.resolve(args.output)), { recursive: true });
    fs.writeFileSync(args.output, `${JSON.stringify(result, null, 2)}\n`);
  }
  const markdown = formatBakeoffMarkdown(result);
  if (args.markdown) {
    fs.mkdirSync(path.dirname(path.resolve(args.markdown)), { recursive: true });
    fs.writeFileSync(args.markdown, markdown);
  }
  if (!args.output && !args.markdown) {
    process.stdout.write(markdown);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  cli().catch((err) => {
    console.error(err.stack || err.message);
    process.exit(1);
  });
}
