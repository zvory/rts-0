use crate::SnapshotEncodeError;

pub const MESSAGEPACK_SNAPSHOT_FRAME_MAGIC: [u8; 4] = [0x52, 0x54, 0x53, 0x4d]; // RTSM
pub(crate) const MESSAGEPACK_SNAPSHOT_HEADER_VERSION: u8 = 1;

pub(crate) fn serialize_compact_snapshot_value(
    compact: &serde_json::Value,
) -> Result<Vec<u8>, SnapshotEncodeError> {
    let mut out = Vec::new();
    out.extend_from_slice(&MESSAGEPACK_SNAPSHOT_FRAME_MAGIC);
    out.push(MESSAGEPACK_SNAPSHOT_HEADER_VERSION);
    write_messagepack_value(&mut out, compact)?;
    Ok(out)
}

fn write_messagepack_value(
    out: &mut Vec<u8>,
    value: &serde_json::Value,
) -> Result<(), SnapshotEncodeError> {
    match value {
        serde_json::Value::Null => out.push(0xc0),
        serde_json::Value::Bool(false) => out.push(0xc2),
        serde_json::Value::Bool(true) => out.push(0xc3),
        serde_json::Value::Number(number) => write_messagepack_number(out, number)?,
        serde_json::Value::String(value) => write_messagepack_string(out, value)?,
        serde_json::Value::Array(values) => {
            write_messagepack_array_len(out, values.len())?;
            for item in values {
                write_messagepack_value(out, item)?;
            }
        }
        serde_json::Value::Object(values) => {
            write_messagepack_map_len(out, values.len())?;
            for (key, item) in values {
                write_messagepack_string(out, key)?;
                write_messagepack_value(out, item)?;
            }
        }
    }
    Ok(())
}

fn write_messagepack_number(
    out: &mut Vec<u8>,
    number: &serde_json::Number,
) -> Result<(), SnapshotEncodeError> {
    if let Some(value) = number.as_u64() {
        write_messagepack_uint(out, value);
    } else if let Some(value) = number.as_i64() {
        write_messagepack_int(out, value);
    } else if let Some(value) = number.as_f64() {
        if !value.is_finite() {
            return Err(SnapshotEncodeError::UnsupportedNumber);
        }
        if value.fract() == 0.0 && value >= 0.0 && value <= u64::MAX as f64 {
            write_messagepack_uint(out, value as u64);
        } else if value.fract() == 0.0 && value >= i64::MIN as f64 && value < 0.0 {
            write_messagepack_int(out, value as i64);
        } else {
            out.push(0xcb);
            out.extend_from_slice(&value.to_be_bytes());
        }
    } else {
        return Err(SnapshotEncodeError::UnsupportedNumber);
    }
    Ok(())
}

fn write_messagepack_uint(out: &mut Vec<u8>, value: u64) {
    if value <= 0x7f {
        out.push(value as u8);
    } else if value <= u8::MAX as u64 {
        out.push(0xcc);
        out.push(value as u8);
    } else if value <= u16::MAX as u64 {
        out.push(0xcd);
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= u32::MAX as u64 {
        out.push(0xce);
        out.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        out.push(0xcf);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn write_messagepack_int(out: &mut Vec<u8>, value: i64) {
    if value >= 0 {
        write_messagepack_uint(out, value as u64);
    } else if value >= -32 {
        out.push((0xe0_i16 + (value + 32) as i16) as u8);
    } else if value >= i8::MIN as i64 {
        out.push(0xd0);
        out.push(value as i8 as u8);
    } else if value >= i16::MIN as i64 {
        out.push(0xd1);
        out.extend_from_slice(&(value as i16).to_be_bytes());
    } else if value >= i32::MIN as i64 {
        out.push(0xd2);
        out.extend_from_slice(&(value as i32).to_be_bytes());
    } else {
        out.push(0xd3);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn write_messagepack_string(out: &mut Vec<u8>, value: &str) -> Result<(), SnapshotEncodeError> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    if len < 32 {
        out.push(0xa0 | len as u8);
    } else if len <= u8::MAX as usize {
        out.push(0xd9);
        out.push(len as u8);
    } else if len <= u16::MAX as usize {
        out.push(0xda);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::MAX as usize {
        out.push(0xdb);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        return Err(SnapshotEncodeError::ContainerTooLarge("string", len));
    }
    out.extend_from_slice(bytes);
    Ok(())
}

fn write_messagepack_array_len(out: &mut Vec<u8>, len: usize) -> Result<(), SnapshotEncodeError> {
    if len < 16 {
        out.push(0x90 | len as u8);
    } else if len <= u16::MAX as usize {
        out.push(0xdc);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::MAX as usize {
        out.push(0xdd);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        return Err(SnapshotEncodeError::ContainerTooLarge("array", len));
    }
    Ok(())
}

fn write_messagepack_map_len(out: &mut Vec<u8>, len: usize) -> Result<(), SnapshotEncodeError> {
    if len < 16 {
        out.push(0x80 | len as u8);
    } else if len <= u16::MAX as usize {
        out.push(0xde);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::MAX as usize {
        out.push(0xdf);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        return Err(SnapshotEncodeError::ContainerTooLarge("map", len));
    }
    Ok(())
}
