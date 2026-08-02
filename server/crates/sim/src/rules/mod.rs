//! Compatibility surface for extracted rules plus sim-owned projection.

pub mod projection;
mod projection_abilities;
mod projection_panzerfaust;
mod projection_visibility;

#[allow(unused_imports)]
pub use rts_rules::{
    artillery_ground_decal_source_kind, combat, death_ground_decal_class, defs, economy, faction,
    is_anti_tank_gun, is_rifle_infantry, mortar_ground_decal_source_kind, target, terrain,
};
