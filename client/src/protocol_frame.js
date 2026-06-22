const MESSAGEPACK_SNAPSHOT_FRAME_MAGIC = Object.freeze([0x52, 0x54, 0x53, 0x4d]); // RTSM
const MESSAGEPACK_TEXT_DECODER = new TextDecoder("utf-8", { fatal: true });

export function parseProtocolFrame(frame, { snapshotTag, snapshotCodecVersion }) {
  if (typeof frame === "string") return JSON.parse(frame);
  if (frame instanceof ArrayBuffer || ArrayBuffer.isView(frame)) {
    return decodeMessagePackSnapshotFrame(frame, { snapshotTag, snapshotCodecVersion });
  }
  throw new Error("unsupported server frame type");
}

function decodeMessagePackSnapshotFrame(frame, { snapshotTag, snapshotCodecVersion }) {
  const reader = new MessagePackReader(frame);
  reader.magic(MESSAGEPACK_SNAPSHOT_FRAME_MAGIC, "MessagePack snapshot frame");
  const version = reader.u8();
  if (version !== snapshotCodecVersion) {
    throw new Error(`unsupported MessagePack snapshot codec version: ${version}`);
  }
  const raw = readMessagePackValue(reader);
  reader.done();
  if (!raw || raw.t !== snapshotTag) {
    throw new Error("MessagePack snapshot frame must contain a snapshot payload");
  }
  return raw;
}

function readMessagePackValue(reader) {
  const tag = reader.u8();
  if (tag <= 0x7f) return tag;
  if ((tag & 0xe0) === 0xa0) return reader.stringFixed(tag & 0x1f);
  if ((tag & 0xf0) === 0x90) return readMessagePackArray(reader, tag & 0x0f);
  if ((tag & 0xf0) === 0x80) return readMessagePackMap(reader, tag & 0x0f);
  if (tag >= 0xe0) return tag - 0x100;
  if (tag === 0xc0) return null;
  if (tag === 0xc2) return false;
  if (tag === 0xc3) return true;
  if (tag === 0xcc) return reader.u8();
  if (tag === 0xcd) return reader.u16();
  if (tag === 0xce) return reader.u32();
  if (tag === 0xcf) return reader.u64Number();
  if (tag === 0xd0) return reader.i8();
  if (tag === 0xd1) return reader.i16();
  if (tag === 0xd2) return reader.i32();
  if (tag === 0xd3) return reader.i64Number();
  if (tag === 0xcb) return reader.f64();
  if (tag === 0xd9) return reader.stringFixed(reader.u8());
  if (tag === 0xda) return reader.stringFixed(reader.u16());
  if (tag === 0xdb) return reader.stringFixed(reader.u32());
  if (tag === 0xdc) return readMessagePackArray(reader, reader.u16());
  if (tag === 0xdd) return readMessagePackArray(reader, reader.u32());
  if (tag === 0xde) return readMessagePackMap(reader, reader.u16());
  if (tag === 0xdf) return readMessagePackMap(reader, reader.u32());
  throw new Error(`unsupported MessagePack tag 0x${tag.toString(16)}`);
}

function readMessagePackArray(reader, len) {
  const out = [];
  for (let i = 0; i < len; i += 1) out.push(readMessagePackValue(reader));
  return out;
}

function readMessagePackMap(reader, len) {
  const out = {};
  for (let i = 0; i < len; i += 1) {
    const key = readMessagePackValue(reader);
    if (typeof key !== "string") throw new Error("MessagePack map key must be a string");
    out[key] = readMessagePackValue(reader);
  }
  return out;
}

class MessagePackReader {
  constructor(bytes) {
    this.bytes = toUint8Array(bytes);
    this.pos = 0;
    this.view = new DataView(this.bytes.buffer, this.bytes.byteOffset, this.bytes.byteLength);
  }

  ensure(len) {
    if (this.pos + len > this.bytes.length) throw new Error("truncated MessagePack frame");
  }

  done() {
    if (this.pos !== this.bytes.length) throw new Error("trailing MessagePack bytes");
  }

  magic(expected, label) {
    this.ensure(expected.length);
    for (let i = 0; i < expected.length; i += 1) {
      if (this.bytes[this.pos + i] !== expected[i]) {
        throw new Error(`invalid ${label} header`);
      }
    }
    this.pos += expected.length;
  }

  u8() {
    this.ensure(1);
    return this.bytes[this.pos++];
  }

  i8() {
    this.ensure(1);
    return this.view.getInt8(this.pos++);
  }

  u16() {
    this.ensure(2);
    const value = this.view.getUint16(this.pos, false);
    this.pos += 2;
    return value;
  }

  i16() {
    this.ensure(2);
    const value = this.view.getInt16(this.pos, false);
    this.pos += 2;
    return value;
  }

  u32() {
    this.ensure(4);
    const value = this.view.getUint32(this.pos, false);
    this.pos += 4;
    return value;
  }

  i32() {
    this.ensure(4);
    const value = this.view.getInt32(this.pos, false);
    this.pos += 4;
    return value;
  }

  u64Number() {
    this.ensure(8);
    const value = this.view.getBigUint64(this.pos, false);
    this.pos += 8;
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error("MessagePack uint64 exceeds safe integer range");
    }
    return Number(value);
  }

  i64Number() {
    this.ensure(8);
    const value = this.view.getBigInt64(this.pos, false);
    this.pos += 8;
    if (value < BigInt(Number.MIN_SAFE_INTEGER) || value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error("MessagePack int64 exceeds safe integer range");
    }
    return Number(value);
  }

  f64() {
    this.ensure(8);
    const value = this.view.getFloat64(this.pos, false);
    this.pos += 8;
    return value;
  }

  stringFixed(len) {
    this.ensure(len);
    const slice = this.bytes.subarray(this.pos, this.pos + len);
    this.pos += len;
    return MESSAGEPACK_TEXT_DECODER.decode(slice);
  }
}

function toUint8Array(value) {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  throw new Error("expected binary server frame");
}
