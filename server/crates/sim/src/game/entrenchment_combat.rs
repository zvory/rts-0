//! Combat-facing entrenchment helpers.
//!
//! Trench lifecycle and slotting stay in `services::entrenchment`; combat systems consume only the
//! active occupation predicate so digging progress or nearby terrain never grants benefits.

use crate::game::entity::{active_trench_occupation, Entity};
use crate::rules::combat;

pub(crate) fn is_actively_entrenched(entity: &Entity) -> bool {
    active_trench_occupation(entity).is_some()
}

pub(crate) fn attack_range_tiles(entity: &Entity, base_range_tiles: f32) -> f32 {
    if !base_range_tiles.is_finite() {
        return base_range_tiles;
    }
    if is_actively_entrenched(entity) {
        base_range_tiles + crate::config::ENTRENCHMENT_RANGE_BONUS_TILES as f32
    } else {
        base_range_tiles
    }
}

pub(crate) fn reduce_direct_damage(victim: &Entity, damage: u32) -> u32 {
    combat::direct_damage_after_entrenchment(victim.kind, damage, is_actively_entrenched(victim))
}

pub(crate) fn reduce_area_damage(victim: &Entity, damage: u32) -> u32 {
    combat::area_damage_after_entrenchment(victim.kind, damage, is_actively_entrenched(victim))
}
