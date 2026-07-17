use std::collections::BTreeMap;

use super::*;
use crate::protocol::{
    ObserverAnalysisKindCount, ObserverAnalysisPayload, ObserverAnalysisPlayer,
    ObserverAnalysisProduction, ObserverAnalysisResourcesLost,
};

impl Game {
    /// Authoritative analysis state for observer overlays.
    ///
    /// This is rebuilt from the current simulation state instead of stored in replay keyframes, so
    /// seek restore + fast-forward produces the same payload as normal playback at the target tick.
    pub fn observer_analysis(&self) -> ObserverAnalysisPayload {
        ObserverAnalysisPayload {
            tick: self.tick_count(),
            map_analysis: None,
            players: self
                .state
                .players
                .iter()
                .map(|player| ObserverAnalysisPlayer {
                    id: player.id,
                    units: self.current_unit_inventory(player.id),
                    production: self.current_production(player.id),
                    upgrades: player
                        .upgrades
                        .iter()
                        .map(|upgrade| upgrade.to_protocol_str().to_string())
                        .collect(),
                    units_lost: unit_loss_rows(&player.score.units_lost_by_kind),
                    resources_lost: resources_lost(&player.score.units_lost_by_kind),
                    resources: player.observer_analysis_resources(self.tick_count()),
                    ai_diagnostics: None,
                })
                .collect(),
        }
    }

    fn current_unit_inventory(&self, player_id: u32) -> Vec<ObserverAnalysisKindCount> {
        let mut counts = BTreeMap::new();
        for entity in self.state.entities.iter() {
            if entity.owner == player_id && entity.kind.is_unit() && entity.hp > 0 {
                *counts.entry(entity.kind).or_insert(0) += 1;
            }
        }
        kind_count_rows(&counts)
    }

    fn current_production(&self, player_id: u32) -> Vec<ObserverAnalysisProduction> {
        let mut rows = Vec::new();
        for entity in self.state.entities.iter() {
            if entity.owner != player_id || !entity.kind.is_building() || entity.hp == 0 {
                continue;
            }
            if let Some(item) = entity.prod_queue().first() {
                rows.push(ObserverAnalysisProduction {
                    building_id: entity.id,
                    building_kind: crate::protocol::kind_to_wire(entity.kind).to_string(),
                    item_kind: crate::protocol::kind_to_wire(item.unit).to_string(),
                    item_type: "unit".to_string(),
                    progress: progress_fraction(item.progress, item.total),
                    queue_depth: entity.prod_queue().len() as u32,
                });
                continue;
            }
            if let Some(item) = entity.research_queue().first() {
                rows.push(ObserverAnalysisProduction {
                    building_id: entity.id,
                    building_kind: crate::protocol::kind_to_wire(entity.kind).to_string(),
                    item_kind: item.upgrade.to_protocol_str().to_string(),
                    item_type: "upgrade".to_string(),
                    progress: progress_fraction(item.progress, item.total),
                    queue_depth: entity.research_queue().len() as u32,
                });
            }
        }
        rows.sort_by_key(|row| row.building_id);
        rows
    }
}

fn unit_loss_rows(counts: &BTreeMap<EntityKind, u32>) -> Vec<ObserverAnalysisKindCount> {
    kind_count_rows(counts)
}

fn kind_count_rows(counts: &BTreeMap<EntityKind, u32>) -> Vec<ObserverAnalysisKindCount> {
    counts
        .iter()
        .filter_map(|(&kind, &count)| {
            if count == 0 {
                return None;
            }
            let (steel, oil) = economy_rules::cost(kind);
            Some(ObserverAnalysisKindCount {
                kind: crate::protocol::kind_to_wire(kind).to_string(),
                count,
                steel_value: steel.saturating_mul(count),
                oil_value: oil.saturating_mul(count),
            })
        })
        .collect()
}

fn resources_lost(counts: &BTreeMap<EntityKind, u32>) -> ObserverAnalysisResourcesLost {
    let mut total_steel = 0u32;
    let mut total_oil = 0u32;
    for (&kind, &count) in counts {
        let (steel, oil) = economy_rules::cost(kind);
        total_steel = total_steel.saturating_add(steel.saturating_mul(count));
        total_oil = total_oil.saturating_add(oil.saturating_mul(count));
    }
    ObserverAnalysisResourcesLost {
        steel: total_steel,
        oil: total_oil,
    }
}

fn progress_fraction(progress: u32, total: u32) -> f32 {
    if total == 0 {
        1.0
    } else {
        (progress as f32 / total as f32).clamp(0.0, 1.0)
    }
}
