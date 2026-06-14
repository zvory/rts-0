use serde_json::{json, Value};

use crate::game::map::Map;

use super::{MapAtlas, MovementClass, MovementLayerAtlas};

pub(in crate::game::map) fn atlas_diagnostics_json(map: &Map) -> Value {
    MapAtlas::generate(map).diagnostics_json(map)
}

impl MapAtlas {
    fn diagnostics_json(&self, map: &Map) -> Value {
        json!({
            "size": map.size,
            "movementClasses": MovementClass::ALL
                .into_iter()
                .map(MovementClass::as_str)
                .collect::<Vec<_>>(),
            "layers": self
                .movement_layers
                .iter()
                .map(MovementLayerAtlas::diagnostics_json)
                .collect::<Vec<_>>(),
            "anchors": self
                .anchors
                .iter()
                .map(|anchor| anchor.diagnostics_json())
                .collect::<Vec<_>>(),
        })
    }
}

impl MovementLayerAtlas {
    fn diagnostics_json(&self) -> Value {
        json!({
            "movementClass": self.movement_class.as_str(),
            "passableTiles": &self.passable_tiles,
            "clearanceTiles": &self.clearance_tiles,
            "componentByTile": &self.component_by_tile,
            "components": self.components
                .iter()
                .map(|component| json!({
                    "id": component.id,
                    "tileCount": component.tile_count,
                }))
                .collect::<Vec<_>>(),
            "regionByTile": &self.region_by_tile,
            "regions": self.regions
                .iter()
                .map(|region| json!({
                    "id": region.id,
                    "componentId": region.component_id,
                    "minTile": tile_json(region.min_tile),
                    "maxTile": tile_json(region.max_tile),
                    "tileCount": region.tile_count,
                }))
                .collect::<Vec<_>>(),
            "portals": self.portals
                .iter()
                .map(|portal| json!({
                    "id": portal.id,
                    "movementClass": portal.movement_class.as_str(),
                    "componentId": portal.component_id,
                    "fromRegion": portal.from_region,
                    "toRegion": portal.to_region,
                    "centerTile": tile_json(portal.center_tile),
                    "widthTiles": portal.width_tiles,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

fn tile_json(tile: (u32, u32)) -> Value {
    json!({ "x": tile.0, "y": tile.1 })
}
