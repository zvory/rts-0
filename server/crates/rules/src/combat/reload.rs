//! Stateless, replay-stable attack timing. No command ordering or shared RNG consumption.

use crate::EntityKind;

/// OpenBW-style per-attack offset: -1, 0, +1 or +2 simulation ticks, after reload modifiers.
/// Call only when committing an attack; readiness checks must never reroll the cooldown.
/// The attack id separates independent weapons and barrage reloads on the same unit/tick.
/// Buildings, Ekat and non-repeating attacks retain their original timers.
pub fn randomized_attack_reload(
    kind: EntityKind,
    entity_id: u32,
    attack_id: &str,
    tick: u32,
    base: u32,
) -> u32 {
    if !kind.is_unit() || kind == EntityKind::Ekat || base == 0 {
        return base;
    }
    // Stable string salt plus a 32-bit avalanche: adjacent entities and firing ticks must
    // not inherit a shared phase. All inputs already belong to checkpointed simulation state.
    let salt = attack_id.bytes().fold(2_166_136_261u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    });
    let mut hash = entity_id.wrapping_mul(0x9E37_79B9) ^ tick.rotate_left(16) ^ salt;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^= hash >> 16;
    match hash & 3 {
        0 => base.saturating_sub(1).max(1),
        offset => base.saturating_add(offset - 1),
    }
}

#[cfg(test)]
mod tests;
