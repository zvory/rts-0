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
    profile_by_id, AiProfile, RIFLE_FLOOD_FAST_ID, RIFLE_FLOOD_FULL_SATURATION,
    RIFLE_FLOOD_FULL_SATURATION_ID, TECH_TO_TANKS_ID,
};
use crate::game::ai_shared;
use crate::game::command::SimCommand;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::systems;
use crate::game::PlayerState;
use rand::Rng;

/// How far an AI worker retreats from a direct hit, in tiles. Matches the previous combat-side
/// retreat distance so AI economy resilience is unchanged.
const WORKER_RETREAT_TILES: f32 = 5.0;

/// Re-plan cadence in ticks. The AI "thinks" this often (about 3 times/second at 30 Hz);
/// decisions are staggered per player so several AIs do not all think on the same tick.
const DECISION_INTERVAL: u32 = 9;

/// Default live-lobby profile. This preserves the current macro-focused AI behavior better than
/// the faster pressure profile, while still selecting from the canonical shared profile ids.
pub(crate) const DEFAULT_LIVE_PROFILE_ID: &str = RIFLE_FLOOD_FULL_SATURATION_ID;

/// Profiles available to ordinary lobby AI opponents. The names map to player-facing behaviors:
/// tank rush, proxy rush, and the previous rifle saturation strategy.
const LIVE_PROFILE_IDS: [&str; 3] = [
    TECH_TO_TANKS_ID,
    RIFLE_FLOOD_FAST_ID,
    RIFLE_FLOOD_FULL_SATURATION_ID,
];

pub(crate) fn random_live_profile_id(rng: &mut impl Rng) -> &'static str {
    LIVE_PROFILE_IDS[rng.gen_range(0..LIVE_PROFILE_IDS.len())]
}

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
    #[allow(dead_code)]
    pub(crate) fn new(player: u32) -> Self {
        Self::with_profile_id(player, DEFAULT_LIVE_PROFILE_ID)
    }

    pub(crate) fn with_profile_id(player: u32, profile_id: &'static str) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION);
        Self {
            player,
            profile_id: profile.id,
            memory: AiDecisionMemory::for_profile(profile),
        }
    }

    fn profile(&self) -> &'static AiProfile {
        profile_by_id(self.profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION)
    }

    #[cfg(test)]
    pub(crate) fn profile_id(&self) -> &'static str {
        self.profile_id
    }

    /// Decide this player's actions for the current tick, pushing any commands onto `out`.
    ///
    /// Most strategic decisions are gated by [`DECISION_INTERVAL`], but the worker-retreat reflex
    /// runs every tick so freshly damaged workers flee on the same cadence the combat system used
    /// to enforce. Routing the reflex through ordinary commands keeps replays player-agnostic.
    pub(crate) fn think(&mut self, context: AiThinkContext<'_>, out: &mut Vec<(u32, SimCommand)>) {
        if !context.entities.player_alive(self.player) {
            return;
        }
        emit_worker_retreat_commands(&context, self.player, out);
        if !context
            .tick
            .wrapping_add(self.player)
            .is_multiple_of(DECISION_INTERVAL)
        {
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

        let profile = self.profile();
        let decision = decide_profile(
            &observation,
            profile,
            &mut self.memory,
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            |building, tile_x, tile_y| {
                live_building_placeable(
                    context.map,
                    context.entities,
                    context.spatial,
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

/// Scan own workers for fresh direct hits and emit a `Move` away from the recorded attacker
/// position. The combat system stamps `last_damage_pos`/`last_damage_tick` on the victim; the AI
/// reacts on the very next tick (combat runs after `think`, so the previous tick's damage shows up
/// here with `tick - 1`). Constructing workers stay latched to their scaffold; pulling them off
/// strands the build.
fn emit_worker_retreat_commands(
    context: &AiThinkContext<'_>,
    player: u32,
    out: &mut Vec<(u32, SimCommand)>,
) {
    let last_tick = context.tick.checked_sub(1);
    let world_max = context.map.world_size_px() - 0.01;
    for entity in context.entities.iter() {
        if entity.owner != player || entity.kind != EntityKind::Worker || entity.hp == 0 {
            continue;
        }
        if matches!(entity.build_phase(), Some(BuildPhase::Constructing { .. })) {
            continue;
        }
        if entity.last_damage_tick() != last_tick {
            continue;
        }
        let Some((ax, ay)) = entity.last_damage_pos() else {
            continue;
        };
        let (vx, vy) = (entity.pos_x, entity.pos_y);
        let dx = vx - ax;
        let dy = vy - ay;
        let dist = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = if dist > f32::EPSILON && dist.is_finite() {
            (dx / dist, dy / dist)
        } else {
            (1.0, 0.0)
        };
        let retreat_px = WORKER_RETREAT_TILES * config::TILE_SIZE as f32;
        let target_x = (vx + ux * retreat_px).clamp(0.0, world_max);
        let target_y = (vy + uy * retreat_px).clamp(0.0, world_max);
        out.push((
            player,
            SimCommand::Move {
                units: vec![entity.id],
                x: target_x,
                y: target_y,
            },
        ));
    }
}

fn live_building_placeable(
    map: &Map,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    if !systems::footprint_placeable(map, entities, spatial, building, tile_x, tile_y) {
        return false;
    }
    for entity in entities.iter().filter(|entity| entity.is_building()) {
        let (cx, cy) = map.tile_of(entity.pos_x, entity.pos_y);
        let Some(stats) = config::building_stats(entity.kind) else {
            return false;
        };
        let existing_tile_x = cx.saturating_sub(stats.foot_w / 2);
        let existing_tile_y = cy.saturating_sub(stats.foot_h / 2);
        if !ai_shared::footprints_respect_clearance(
            building,
            tile_x,
            tile_y,
            entity.kind,
            existing_tile_x,
            existing_tile_y,
        ) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::command::SimCommand as Command;
    use crate::game::entity::{EntityStore, Order};
    use rand::SeedableRng;

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

        assert_eq!(ai.player, 2);
        assert_eq!(ai.profile_id, RIFLE_FLOOD_FULL_SATURATION_ID);
    }

    #[test]
    fn live_profile_pool_has_requested_strategies() {
        assert_eq!(
            LIVE_PROFILE_IDS,
            [
                TECH_TO_TANKS_ID,
                RIFLE_FLOOD_FAST_ID,
                RIFLE_FLOOD_FULL_SATURATION_ID
            ]
        );
    }

    #[test]
    fn random_live_profile_selection_uses_live_pool() {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(0xA1);
        for _ in 0..32 {
            let selected = random_live_profile_id(&mut rng);
            assert!(LIVE_PROFILE_IDS.contains(&selected));
        }
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
        fog.recompute(&[2], &entities, &map);
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
        fog.recompute(&[1, 2], &entities, &map);
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
