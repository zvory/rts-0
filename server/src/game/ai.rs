//! Live gameplay AI adapter. See `DESIGN.md` section 8.
//!
//! An [`AiController`] drives one AI-owned player. It is invoked from
//! [`crate::game::Game::tick`] every tick, before queued commands are applied, and it pushes
//! ordinary [`SimCommand`]s onto the same pending queue a human client would use. That means the AI
//! has no special powers: its commands run through the identical validation/cost/supply/placement
//! path in `services/commands.rs`.
//!
//! The live controller may read authoritative own/resource state to build its observation, but
//! enemy entities are filtered through the player's fog grid. Shared strategy code attacks public
//! enemy start tiles for outbound waves and only targets visible enemy entities for local defense.

use crate::config;
use crate::game::ai_core::decision::{decide_profile, AiDecisionMemory};
use crate::game::ai_core::observation::AiObservation;
use crate::game::ai_core::profiles::{
    profile_by_id, AiProfile, RIFLE_FLOOD_FULL_SATURATION, RIFLE_FLOOD_FULL_SATURATION_ID,
};
use crate::game::ai_shared;
use crate::game::command::SimCommand;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::systems;
use crate::game::PlayerState;
use std::collections::BTreeSet;

/// Re-plan cadence in ticks. The AI "thinks" this often (about 3 times/second at 30 Hz);
/// decisions are staggered per player so several AIs do not all think on the same tick.
const DECISION_INTERVAL: u32 = 9;

/// Default live-lobby profile. This preserves the current macro-focused AI behavior better than
/// the faster pressure profile, while still selecting from the canonical shared profile ids.
pub(crate) const DEFAULT_LIVE_PROFILE_ID: &str = RIFLE_FLOOD_FULL_SATURATION_ID;

pub(crate) struct AiThinkContext<'a> {
    pub(crate) map: &'a Map,
    pub(crate) entities: &'a EntityStore,
    pub(crate) fog: &'a Fog,
    pub(crate) spatial: &'a SpatialIndex,
    pub(crate) players: &'a [PlayerState],
    pub(crate) tick: u32,
}

/// Drives a single AI-controlled player by emitting ordinary commands each think.
///
/// `AiController` owns live-only identity, profile selection, cadence, and persistent decision
/// memory. RTS knowledge and command synthesis live in `ai_core`.
pub(crate) struct AiController {
    player: u32,
    profile_id: &'static str,
    memory: AiDecisionMemory,
}

impl AiController {
    pub(crate) fn new(player: u32) -> Self {
        Self::with_profile_id(player, DEFAULT_LIVE_PROFILE_ID)
    }

    fn with_profile_id(player: u32, profile_id: &'static str) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION);
        Self {
            player,
            profile_id: profile.id,
            memory: AiDecisionMemory::for_profile(profile),
        }
    }

    pub(crate) fn player_id(&self) -> u32 {
        self.player
    }

    fn profile(&self) -> &'static AiProfile {
        profile_by_id(self.profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION)
    }

    /// Decide this player's actions for the current tick, pushing any commands onto `out`. This is
    /// a no-op on most ticks (gated by [`DECISION_INTERVAL`]) and whenever the player is dead.
    pub(crate) fn think(&mut self, context: AiThinkContext<'_>, out: &mut Vec<(u32, SimCommand)>) {
        if !context
            .tick
            .wrapping_add(self.player)
            .is_multiple_of(DECISION_INTERVAL)
        {
            return;
        }
        if !context.entities.player_alive(self.player) {
            return;
        }
        let Some(observation) = AiObservation::from_live_state(
            context.map,
            context.entities,
            context.fog,
            context.players,
            self.player,
            context.tick,
        ) else {
            return;
        };

        let building_margin = building_margin_tiles(context.map, context.entities);
        let profile = self.profile();
        let decision = decide_profile(
            &observation,
            profile,
            &mut self.memory,
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
            },
            |building, tile_x, tile_y| {
                live_building_placeable(
                    context.map,
                    context.entities,
                    context.spatial,
                    &building_margin,
                    building,
                    tile_x,
                    tile_y,
                )
            },
        );
        debug_assert_eq!(decision.profile_id, self.profile_id);

        out.extend(
            decision
                .commands
                .into_iter()
                .map(|command| (self.player, command)),
        );
    }
}

fn live_building_placeable(
    map: &Map,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    building_margin: &BTreeSet<(u32, u32)>,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    if !systems::footprint_placeable(map, entities, spatial, building, tile_x, tile_y) {
        return false;
    }
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    for dy in 0..stats.foot_h {
        for dx in 0..stats.foot_w {
            let Some(x) = tile_x.checked_add(dx) else {
                return false;
            };
            let Some(y) = tile_y.checked_add(dy) else {
                return false;
            };
            if building_margin.contains(&(x, y)) {
                return false;
            }
        }
    }
    true
}

fn building_margin_tiles(map: &Map, entities: &EntityStore) -> BTreeSet<(u32, u32)> {
    let mut margin = BTreeSet::new();
    for entity in entities.iter().filter(|entity| entity.is_building()) {
        for (tile_x, tile_y) in crate::game::services::occupancy::building_footprint(map, entity) {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = tile_x as i32 + dx;
                    let ny = tile_y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx < map.size as i32 && ny < map.size as i32 {
                        margin.insert((nx as u32, ny as u32));
                    }
                }
            }
        }
    }
    margin
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::command::SimCommand as Command;
    use crate::game::entity::{EntityStore, Order};

    fn player(
        id: u32,
        start_tile: (u32, u32),
        steel: u32,
        supply_used: u32,
        supply_cap: u32,
    ) -> PlayerState {
        PlayerState {
            id,
            name: format!("Player {id}"),
            color: "#000".into(),
            start_tile,
            steel,
            oil: 0,
            supply_used,
            supply_cap,
            is_ai: id != 1,
            score: crate::game::ScoreState::default(),
        }
    }

    #[test]
    fn live_controller_uses_default_profile_id() {
        let ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.profile_id, RIFLE_FLOOD_FULL_SATURATION_ID);
    }

    #[test]
    fn unknown_profile_id_falls_back_to_default_profile() {
        let ai = AiController::with_profile_id(2, "missing_profile");

        assert_eq!(ai.profile_id, RIFLE_FLOOD_FULL_SATURATION_ID);
    }

    #[test]
    fn pending_depot_build_blocks_repeat_supply_depot_plan() {
        let mut entities = EntityStore::default();
        let worker = entities
            .spawn_unit(2, EntityKind::Worker, 0.0, 0.0)
            .unwrap();
        if let Some(entity) = entities.get_mut(worker) {
            entity.set_order(Order::build(EntityKind::Depot, 5, 6));
        }
        let map = Map::generate(2, 1234);
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[2], &entities);
        let players = vec![player(2, (10, 10), 999, 8, 10)];
        let mut ai = AiController::new(2);
        let mut out = Vec::new();

        ai.think(
            AiThinkContext {
                map: &map,
                entities: &entities,
                fog: &fog,
                spatial: &spatial,
                players: &players,
                tick: 7,
            },
            &mut out,
        );

        assert!(
            !out.iter().any(|(_, command)| matches!(
                command,
                Command::Build { building, .. } if *building == EntityKind::Depot
            )),
            "AI should treat a worker's pending depot build intent as supply already in progress"
        );
    }

    #[test]
    fn live_controller_delegates_attacks_to_default_profile() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        for i in 0..RIFLE_FLOOD_FULL_SATURATION.attack.first_attack_size {
            entities
                .spawn_unit(2, EntityKind::Rifleman, ai_base.0 + i as f32, ai_base.1)
                .unwrap();
        }
        let players = vec![
            player(1, enemy_start, 0, 0, 0),
            player(
                2,
                ai_start,
                0,
                RIFLE_FLOOD_FULL_SATURATION.attack.first_attack_size as u32,
                20,
            ),
        ];
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities);
        let mut ai = AiController::new(2);
        let mut out = Vec::new();

        ai.think(
            AiThinkContext {
                map: &map,
                entities: &entities,
                fog: &fog,
                spatial: &spatial,
                players: &players,
                tick: 7,
            },
            &mut out,
        );

        assert!(out.iter().any(|(player_id, command)| {
            *player_id == 2
                && matches!(
                    command,
                    Command::AttackMove { units, x, y }
                        if units.len() == RIFLE_FLOOD_FULL_SATURATION.attack.first_attack_size
                            && *x == enemy_base.0
                            && *y == enemy_base.1
                )
        }));
    }
}
