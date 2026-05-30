//! Tick invariant checks. See `PLAN.md` §1.1.
//!
//! These assertions run in debug builds and tests after every tick. They are intentionally
//! panic-on-failure so broken assumptions surface immediately during development.

use crate::config;
use crate::game::entity::{EntityKind, GatherPhase, Order, NEUTRAL};
use crate::game::services::movement::is_collision_anchored;
use crate::game::services::occupancy::building_footprint;
use crate::game::Game;

/// Maximum residual overlap (world px) tolerated between two non-anchored mobile units after
/// a tick. The iterative resolver converges to within numerical noise on flat ground; this
/// slack also absorbs the rare case of a unit cornered against impassable terrain where the
/// resolver's push lands on a blocked tile and has to be skipped.
const OVERLAP_TOLERANCE_PX: f32 = 6.0;

impl Game {
    /// Assert that the current world state satisfies all simulation invariants.
    ///
    /// Called automatically at the end of [`Game::tick`] in debug builds. Tests may also call it
    /// explicitly after manual state mutations.
    pub fn assert_invariants(&self) {
        let world_max = self.map.world_size_px();
        let player_ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();

        // ------------------------------------------------------------------
        // 1. Entity id / store-key consistency
        // ------------------------------------------------------------------
        for e in self.entities.iter() {
            assert!(
                self.entities.contains(e.id),
                "invariant: entity {} kind {:?} has id that does not exist in store",
                e.id,
                e.kind
            );
            // Also verify the entity we get back by id is the same record.
            if let Some(by_key) = self.entities.get(e.id) {
                assert_eq!(
                    by_key.id, e.id,
                    "invariant: store key {} does not match entity id {}",
                    by_key.id, e.id
                );
            }
        }

        // ------------------------------------------------------------------
        // 2. No NaN or out-of-world coordinates
        // ------------------------------------------------------------------
        for e in self.entities.iter() {
            assert!(
                e.pos_x.is_finite() && e.pos_y.is_finite(),
                "invariant: entity {} has non-finite position ({}, {})",
                e.id,
                e.pos_x,
                e.pos_y
            );
            assert!(
                e.pos_x >= 0.0 && e.pos_x < world_max && e.pos_y >= 0.0 && e.pos_y < world_max,
                "invariant: entity {} position ({}, {}) out of world bounds [0, {})",
                e.id,
                e.pos_x,
                e.pos_y,
                world_max
            );
        }

        // ------------------------------------------------------------------
        // 3. Supply equals living plus queued units
        // ------------------------------------------------------------------
        for ps in &self.players {
            let mut expected_cap = 0u32;
            let mut expected_used = 0u32;
            for e in self.entities.iter() {
                if e.owner != ps.id {
                    continue;
                }
                if e.is_building() && !e.under_construction() {
                    if let Some(s) = config::building_stats(e.kind) {
                        expected_cap += s.provides_supply;
                    }
                    for item in e.prod_queue() {
                        if let Some(us) = config::unit_stats(item.unit) {
                            expected_used += us.supply;
                        }
                    }
                } else if e.is_unit() {
                    if let Some(us) = config::unit_stats(e.kind) {
                        expected_used += us.supply;
                    }
                }
            }
            expected_cap = expected_cap.min(config::SUPPLY_CAP_MAX);
            assert_eq!(
                ps.supply_cap, expected_cap,
                "invariant: player {} supply_cap {} != expected {}",
                ps.id, ps.supply_cap, expected_cap
            );
            assert_eq!(
                ps.supply_used, expected_used,
                "invariant: player {} supply_used {} != expected {}",
                ps.id, ps.supply_used, expected_used
            );
        }

        // ------------------------------------------------------------------
        // 4. Buildings never overlap
        // ------------------------------------------------------------------
        let mut occupied: Vec<(u32, u32)> = Vec::new();
        for e in self.entities.iter() {
            if !e.is_building() {
                continue;
            }
            let footprint = building_footprint(&self.map, e);
            for tile in &footprint {
                assert!(
                    !occupied.contains(tile),
                    "invariant: building {} footprint overlaps another building at tile {:?}",
                    e.id,
                    tile
                );
                occupied.push(*tile);
            }
        }

        // ------------------------------------------------------------------
        // 5. Resource-node miner reservations are valid or ignored
        // ------------------------------------------------------------------
        for e in self.entities.iter() {
            if !e.is_node() {
                continue;
            }
            if let Some(miner_id) = e.miner() {
                let miner = match self.entities.get(miner_id) {
                    Some(m) => m,
                    None => {
                        // Advisory field pointing at a dead entity is allowed as a
                        // transient condition; the gather system self-heals it next tick.
                        continue;
                    }
                };
                assert!(
                    miner.kind == EntityKind::Worker,
                    "invariant: node {} miner {} is not a worker (kind {:?})",
                    e.id,
                    miner_id,
                    miner.kind
                );
                assert!(
                    miner.hp > 0,
                    "invariant: node {} miner {} has hp == 0",
                    e.id,
                    miner_id
                );
                let on_this_node = miner.order().gather_node() == Some(e.id);
                assert!(
                    on_this_node,
                    "invariant: node {} miner {} does not have Gather order for this node (order {:?})",
                    e.id, miner_id, miner.order()
                );
                assert!(
                    miner.gather_phase() == Some(GatherPhase::Harvesting),
                    "invariant: node {} miner {} gather_phase is not Harvesting ({:?})",
                    e.id,
                    miner_id,
                    miner.gather_phase()
                );
            }
        }

        // ------------------------------------------------------------------
        // 6. Orders do not point at invalid required targets
        //    (transition windows where a target just died are allowed because
        //     death_system cleans them up on the same tick).
        // ------------------------------------------------------------------
        for e in self.entities.iter() {
            if !e.is_unit() {
                continue;
            }
            match e.order() {
                Order::Attack(_) => {
                    let Some(target) = e.order().attack_target() else {
                        continue;
                    };
                    if let Some(t) = self.entities.get(target) {
                        assert!(
                            t.is_targetable() && t.hp > 0,
                            "invariant: entity {} Attack order targets invalid entity {} (hp {} targetable {})",
                            e.id, target, t.hp, t.is_targetable()
                        );
                    }
                }
                Order::Gather(_) => {
                    let Some(node) = e.order().gather_node() else {
                        continue;
                    };
                    if let Some(n) = self.entities.get(node) {
                        assert!(
                            n.is_node() && n.remaining().unwrap_or(0) > 0,
                            "invariant: entity {} Gather order targets invalid node {} (kind {:?} remaining {})",
                            e.id, node, n.kind, n.remaining().unwrap_or(0)
                        );
                    }
                }
                Order::Build(_) => {
                    let Some(site) = e.order().build_site() else {
                        continue;
                    };
                    if let Some(b) = self.entities.get(site) {
                        assert!(
                            b.is_building() && b.under_construction(),
                            "invariant: entity {} Build order targets invalid site {} (building {} under_construction {})",
                            e.id, site, b.is_building(), b.under_construction()
                        );
                    }
                }
                _ => {}
            }
        }

        // ------------------------------------------------------------------
        // 7. Fog grids exist for all players and never for neutral owner
        // ------------------------------------------------------------------
        for &pid in &player_ids {
            assert!(
                self.fog.has_grid(pid),
                "invariant: fog grid missing for player {}",
                pid
            );
        }
        assert!(
            !self.fog.has_grid(NEUTRAL),
            "invariant: fog grid must not exist for neutral owner (0)"
        );

        // ------------------------------------------------------------------
        // 8b. Mobile units do not stack on top of each other (PLAN §4.3).
        //     Harvesting workers are anchored to their resource node and excluded — they
        //     intentionally cannot be pushed by collision. All other mobile-unit pairs must
        //     stay at sum-of-radii apart (within `OVERLAP_TOLERANCE_PX` of floating-point and
        //     terrain-pinned residue).
        // ------------------------------------------------------------------
        let units: Vec<_> = self.entities.iter().filter(|e| e.is_unit()).collect();
        for i in 0..units.len() {
            let a = units[i];
            if is_collision_anchored(a) {
                continue;
            }
            for &b in units.iter().skip(i + 1) {
                if is_collision_anchored(b) {
                    continue;
                }
                let dx = a.pos_x - b.pos_x;
                let dy = a.pos_y - b.pos_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let min_d = a.radius() + b.radius();
                assert!(
                    dist + OVERLAP_TOLERANCE_PX >= min_d,
                    "invariant: units {} ({:?}) and {} ({:?}) overlap by {:.2}px (min sep {:.1}, dist {:.2})",
                    a.id, a.kind, b.id, b.kind,
                    min_d - dist, min_d, dist
                );
            }
        }

        // ------------------------------------------------------------------
        // 8. Snapshots never expose hidden enemy ids through entities or targets
        // ------------------------------------------------------------------
        for &pid in &player_ids {
            let snap = self.snapshot_for(pid);
            for v in &snap.entities {
                if v.owner == pid || v.owner == NEUTRAL {
                    continue;
                }
                // Enemy entity: must be on a visible tile.
                assert!(
                    self.fog.is_visible_world(pid, v.x, v.y),
                    "invariant: snapshot for player {} exposes hidden enemy entity {} at ({}, {})",
                    pid,
                    v.id,
                    v.x,
                    v.y
                );
                // If a target_id is exposed, the target must be visible too.
                if let Some(tid) = v.target_id {
                    if let Some(t) = self.entities.get(tid) {
                        let visible =
                            v.owner == pid || self.fog.is_visible_world(pid, t.pos_x, t.pos_y);
                        assert!(
                            visible,
                            "invariant: snapshot for player {} exposes hidden target_id {} (target pos {}, {})",
                            pid, tid, t.pos_x, t.pos_y
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Game, PlayerInit};

    /// A freshly-created game must satisfy every invariant before any tick runs.
    #[test]
    fn invariants_hold_at_game_start() {
        let players = [
            PlayerInit {
                id: 1,
                name: "A".into(),
                color: "#fff".into(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                name: "B".into(),
                color: "#000".into(),
                is_ai: true,
            },
        ];
        let game = Game::new(&players);
        game.assert_invariants();
    }

    /// After many ticks the invariants must still hold.
    #[test]
    fn invariants_hold_after_ai_match() {
        let players = [
            PlayerInit {
                id: 1,
                name: "A".into(),
                color: "#fff".into(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                name: "B".into(),
                color: "#000".into(),
                is_ai: true,
            },
        ];
        let mut game = Game::new(&players);
        for _ in 0..6000 {
            game.tick();
        }
        game.assert_invariants();
    }

    /// A human-only sandbox with no commands must keep invariants across ticks.
    #[test]
    fn invariants_hold_in_no_command_sandbox() {
        let players = [PlayerInit {
            id: 1,
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players);
        for _ in 0..300 {
            game.tick();
        }
        game.assert_invariants();
    }
}
