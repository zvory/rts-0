use std::collections::HashMap;

use super::BaseResourceCounts;
use crate::protocol::MapDoodad;

/// Canonical materialization of an authored-map document before player starts are assigned.
///
/// HTTP/session boundaries use this to bind an untrusted authored document to an equivalent
/// wire-format draft without maintaining a second copy of the authored-map decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredMapData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<(u32, u32)>,
    pub base_sites: Vec<(u32, u32)>,
    pub base_resource_counts: HashMap<(u32, u32), BaseResourceCounts>,
    pub doodads: Vec<MapDoodad>,
    pub stealth_tiles: Vec<(u32, u32)>,
    pub no_vehicle_tiles: Vec<(u32, u32)>,
    pub damage_reduction_tiles: Vec<(u32, u32)>,
    pub slow_movement_tiles: Vec<(u32, u32)>,
}
