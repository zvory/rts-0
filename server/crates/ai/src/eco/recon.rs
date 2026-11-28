//! Keep one ready Scout Plane carrier for defense; use spare sorties on public enemy sites.
use super::construction::dist2;
use crate::sdk::{
    AbilityKind, AiActions, AiBaseSiteId, AiEntity, AiFrame, EntityKind, UnitGroup, UpgradeKind,
    WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct Recon {
    launched: BTreeMap<u32, u32>,
    flights: Vec<((f32, f32), u32)>,
    visited: BTreeMap<AiBaseSiteId, u32>,
}
impl Recon {
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        incoming: Option<(f32, f32)>,
    ) {
        if !frame
            .completed_upgrades()
            .contains(&UpgradeKind::ScoutPlaneUnlock)
        {
            return;
        }
        self.flights.retain(|(_, until)| *until > frame.tick());
        self.launched
            .retain(|id, _| frame.owned().iter().any(|e| e.id == *id));
        let mut ready = frame
            .owned()
            .iter()
            .filter(|e| {
                e.kind == EntityKind::CommandCar
                    && e.health.current > 0
                    && self.launched.get(&e.id).is_none_or(|tick| {
                        frame.tick().saturating_sub(*tick)
                            >= u32::from(rts_rules::balance::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS)
                    })
                    && e.abilities.iter().any(|a| {
                        a.kind == AbilityKind::ScoutPlane
                            && a.cooldown_left == 0
                            && a.remaining_uses.is_none_or(|n| n > 0)
                            && a.available_tick.is_none_or(|t| frame.tick() >= t)
                            && a.lockout_until_tick.is_none_or(|t| frame.tick() >= t)
                    })
            })
            .collect::<Vec<_>>();
        ready.sort_by_key(|e| e.id);
        let size = frame.map().tile_size as f32;
        let covered = |p| {
            self.flights
                .iter()
                .any(|(goal, _)| dist2(*goal, p) <= (9. * size).det_powi(2))
        };
        if let Some(target) = incoming {
            // Defensive reconnaissance may spend the reserved carrier. Never substitute a
            // routine sortie when an active incident is already covered or no carrier is ready.
            if !covered(target) {
                ready.sort_by(|a, b| {
                    dist2(a.position, target)
                        .total_cmp(&dist2(b.position, target))
                        .then(a.id.cmp(&b.id))
                });
                for car in ready {
                    if self.launch(frame, actions, car, target) {
                        break;
                    }
                }
            }
            return;
        }
        // A reserve means one ready launch, not a stockpiled plane entity. With only one
        // available car it stays ready; additional ready cars scout different base sites.
        if ready.len() < 2 {
            return;
        }
        let q = WorldQueries::new(frame);
        let mut sites = Vec::new();
        for player in frame.players().iter().filter(|p| {
            p.is_alive
                && p.id != frame.player_id()
                && (p.team_id == 0 || p.team_id != frame.team_id())
        }) {
            for base in q
                .ranked_bases(player.id)
                .into_iter()
                .take(frame.base_sites().len() / 2)
            {
                let friendly = q.known_base_depots(base.site.id).iter().any(|depot| {
                    frame.players().iter().any(|p| {
                        p.id == depot.owner
                            && (p.id == frame.player_id()
                                || p.team_id != 0 && p.team_id == frame.team_id())
                    })
                });
                if !friendly && !covered(base.site.position) {
                    sites.push(base.site);
                }
            }
        }
        let Some(car) = ready.get(1).copied() else {
            return;
        };
        sites.sort_by(|a, b| {
            self.visited
                .get(&a.id)
                .copied()
                .cmp(&self.visited.get(&b.id).copied())
                .then(dist2(car.position, a.position).total_cmp(&dist2(car.position, b.position)))
                .then(a.id.cmp(&b.id))
        });
        if let Some(site) = sites.first() {
            // Reserve part of the sortie lifetime for observation, rather than spending
            // the entire flight trying to reach an extremely distant main base.
            let length = dist2(car.position, site.position).det_sqrt().max(1.);
            let travel = rts_rules::balance::SCOUT_PLANE_SPEED_PX_PER_TICK
                * f32::from(rts_rules::balance::SCOUT_PLANE_LIFETIME_TICKS)
                * 0.8;
            let scale = (travel / length).min(1.);
            let target = (
                car.position.0 + (site.position.0 - car.position.0) * scale,
                car.position.1 + (site.position.1 - car.position.1) * scale,
            );
            if self.launch(frame, actions, car, target) {
                self.visited.insert(site.id, frame.tick());
            }
        }
    }
    fn launch(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        car: &AiEntity,
        target: (f32, f32),
    ) -> bool {
        let Ok(group) = UnitGroup::new([car.id]) else {
            return false;
        };
        if actions
            .use_ability(&group, AbilityKind::ScoutPlane, Some(target), false)
            .is_err()
        {
            return false;
        }
        self.launched.insert(car.id, frame.tick());
        self.flights.push((
            target,
            frame
                .tick()
                .saturating_add(u32::from(rts_rules::balance::SCOUT_PLANE_LIFETIME_TICKS)),
        ));
        true
    }
}
#[cfg(test)]
mod tests;
