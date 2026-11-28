use super::construction::dist2;
use crate::sdk::UnitGroup;
use crate::sdk::{
    AiActions, AiBaseSiteId, AiEntityState, AiFrame, DefensePlanner, EntityKind, PullbackMemory,
    PullbackParams,
};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug)]
pub(super) struct DefenseAssignments {
    destinations: BTreeMap<u32, (f32, f32)>,
    held: BTreeSet<u32>,
    frontier: Option<AiBaseSiteId>,
    reserved_depot: Option<crate::sdk::AiTile>,
    advance_orders: BTreeMap<u32, u32>,
    screen_units: BTreeSet<u32>,
    // Initial cover wave only; fresh auto-production must not postpone dispatch forever.
    screen_wave: BTreeSet<u32>,
    guard_posts: BTreeMap<u32, (f32, f32)>,
    armor: super::armor::ArmorAssignments,
    pullback: PullbackMemory,
    response: crate::pressure::response::PressureResponse,
    responding: BTreeSet<u32>,
}
impl Default for DefenseAssignments {
    fn default() -> Self {
        Self {
            destinations: Default::default(),
            held: Default::default(),
            frontier: None,
            reserved_depot: None,
            advance_orders: Default::default(),
            screen_units: Default::default(),
            screen_wave: Default::default(),
            guard_posts: Default::default(),
            armor: Default::default(),
            pullback: Default::default(),
            response: crate::pressure::response::PressureResponse::with_cohesive_reserve(),
            responding: Default::default(),
        }
    }
}
impl DefenseAssignments {
    pub(super) fn prepare_expansion(&mut self, site: AiBaseSiteId) {
        if self.frontier != Some(site) {
            self.frontier = Some(site);
            self.screen_units.clear();
            self.screen_wave.clear();
            self.reserved_depot = None;
            self.advance_orders.clear();
        }
    }
    pub(super) fn finish_completed_expansion(&mut self, sites: &[AiBaseSiteId]) {
        if self.frontier.is_some_and(|site| sites.contains(&site)) {
            self.frontier = None;
            self.reserved_depot = None;
            self.advance_orders.clear();
            self.screen_units.clear();
            self.screen_wave.clear();
        }
    }
    #[cfg(test)]
    pub(super) fn screen_ids(&self) -> &BTreeSet<u32> {
        &self.screen_units
    }
    #[cfg(test)]
    pub(super) fn assigned_posts(&self) -> &BTreeMap<u32, (f32, f32)> {
        &self.destinations
    }
    #[cfg(test)]
    pub(super) fn has_responders(&self) -> bool {
        !self.responding.is_empty()
    }
    pub(super) fn frontier(&self) -> Option<AiBaseSiteId> {
        self.frontier
    }
    pub(super) fn depot_tile(&self) -> Option<crate::sdk::AiTile> {
        self.reserved_depot
    }
    pub(super) fn reserve_depot(&mut self, tile: crate::sdk::AiTile) {
        if self.reserved_depot != Some(tile) {
            self.reserved_depot = Some(tile);
            self.advance_orders.clear();
            self.screen_wave.clear();
        }
    }
    fn screen_ready(&self, frame: &AiFrame, site: AiBaseSiteId) -> bool {
        let mut count = 0;
        self.frontier == Some(site)
            && frame
                .owned()
                .iter()
                .filter(|e| {
                    self.screen_wave.contains(&e.id)
                        && self.screen_units.contains(&e.id)
                        && infantry(e.kind)
                        && e.health.current > 0
                })
                .all(|e| {
                    count += 1;
                    self.advance_orders.get(&e.id).is_some_and(|sent| {
                        frame.tick().saturating_sub(*sent) >= super::SCREEN_LEAD_TICKS
                    })
                })
            && count > 0
    }
    pub(super) fn ready_for_builder(&self, frame: &AiFrame, site: AiBaseSiteId) -> bool {
        self.screen_ready(frame, site) && self.builder_route_clear(frame)
    }
    fn builder_route_clear(&self, frame: &AiFrame) -> bool {
        let q = crate::sdk::WorldQueries::new(frame);
        let Some(tile) = self.reserved_depot else {
            return false;
        };
        let Some(from) = frame
            .owned()
            .iter()
            .find(|e| e.kind == EntityKind::Worker)
            .and_then(|worker| q.world_point(worker.position.0, worker.position.1))
        else {
            return false;
        };
        q.ground_route_to_build_site(from, EntityKind::ResourceDepot, tile, EntityKind::Worker)
            .is_some_and(|route| !q.route_has_visible_ground_threat(&route, EntityKind::Worker))
    }
    #[cfg(test)]
    pub(super) fn waiting_for_builder(&self, frame: &AiFrame, site: AiBaseSiteId) -> bool {
        self.frontier == Some(site)
            && self.responding.is_empty()
            && !self.screen_units.is_empty()
            && !self.screen_ready(frame, site)
    }
    fn defended_areas(&mut self, frame: &AiFrame) -> Vec<crate::pressure::response::DefendedArea> {
        use crate::pressure::response::DefendedArea;
        self.guard_posts.retain(|id, _| {
            frame
                .owned()
                .iter()
                .any(|e| e.id == *id && e.health.current > 0)
        });
        let mut areas: Vec<_> = self
            .guard_posts
            .iter()
            .map(|(id, anchor)| DefendedArea {
                anchor: *anchor,
                radius_tiles: None,
                members: BTreeSet::from([*id]),
            })
            .collect();
        // Every living friendly building is a rear-area anchor, including scaffolds and
        // exhausted Depots. Economic weighting still independently decides normal staffing.
        for building in frame
            .owned()
            .iter()
            .chain(frame.visible_allies())
            .filter(|e| e.kind.is_building() && e.health.current > 0)
        {
            areas.push(DefendedArea {
                anchor: building.position,
                radius_tiles: Some(20.),
                members: BTreeSet::from([building.id]),
            });
        }
        areas
    }
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        planner: &mut DefensePlanner,
        sites: &[AiBaseSiteId],
    ) {
        let members: Vec<_> = frame
            .owned()
            .iter()
            .filter(|e| infantry(e.kind) || e.kind == EntityKind::Tank)
            .map(|e| e.id)
            .collect();
        // Reuse Pressure/Rush's shared wounded-unit policy. It must keep command ownership
        // during combat responses as well as ordinary defensive post maintenance.
        let pullback = self
            .pullback
            .step(frame, &members, PullbackParams::default(), actions);
        for id in pullback.active.iter().chain(&pullback.released) {
            self.held.remove(id);
            self.advance_orders.remove(id);
            self.screen_units.remove(id);
        }
        let eligible = members
            .into_iter()
            .filter(|id| !pullback.active.contains(id))
            .collect();
        let areas = self.defended_areas(frame);
        let response = self
            .response
            .step_defended(frame, &eligible, &areas, actions);
        // Ownership persists across decisions where the response emits no fresh order.
        // Never let a frontier Move/Hold steal a responder during its reissue cooldown.
        for id in response.active.iter().chain(&response.released) {
            self.held.remove(id);
        }
        // Responders are already mobilized. They retain tactical ownership, but a distant
        // response must not block the Engineer when its own build route is clear.
        for id in response
            .active
            .iter()
            .filter(|id| !pullback.active.contains(id))
        {
            self.advance_orders.entry(*id).or_insert(frame.tick());
        }
        for id in &response.released {
            self.advance_orders.remove(id);
        }
        self.responding = response.active;
        let unavailable: BTreeSet<_> = self.responding.union(&pullback.active).copied().collect();
        self.advance_orders.retain(|id, _| eligible.contains(id));
        // Borrowing a defender for combat does not remove its home post or shrink the
        // planner's capacity. Otherwise a fight can replace a broad cut with rear garrisons.
        let infantry: Vec<_> = frame
            .owned()
            .iter()
            .filter(|e| infantry(e.kind) && e.health.current > 0)
            .collect();
        if infantry.is_empty() {
            self.destinations.clear();
            self.held.clear();
        }
        if self.reserved_depot.is_none() {
            if let Some(tile) = self.frontier.and_then(|site| {
                crate::sdk::WorldQueries::new(frame)
                    .base_depot_site(site, &crate::sdk::KnownBuildSiteExclusions::default())
            }) {
                self.reserve_depot(tile);
            }
        }
        let reservations = self
            .reserved_depot
            .map(|tile| crate::sdk::DefenseReservation {
                building: EntityKind::ResourceDepot,
                tile,
            })
            .into_iter()
            .collect::<Vec<_>>();
        let plan = if let Some(site) = self.frontier {
            planner.plan_with_expansion(frame, sites, site, infantry.len(), &reservations)
        } else {
            planner.plan(frame, sites, infantry.len())
        };
        self.armor
            .step(frame, actions, &plan, &unavailable, self.reserved_depot);
        let previous_screen = self.screen_units.clone();
        self.screen_units.retain(|id| self.responding.contains(id));
        let mut available: BTreeSet<_> = (0..plan.slots.len()).collect();
        let mut assigned = BTreeMap::new();
        // Preserve exact surviving posts before matching newly produced/repositioned infantry.
        for e in &infantry {
            if let Some(old) = self.destinations.get(&e.id) {
                if let Some(i) = available
                    .iter()
                    .copied()
                    .find(|i| dist2(plan.slots[*i].position, *old) < 1.)
                {
                    available.remove(&i);
                    assigned.insert(e.id, plan.slots[i].position);
                }
            }
        }
        for e in &infantry {
            if assigned.contains_key(&e.id) {
                continue;
            }
            if let Some(i) = available.iter().copied().min_by(|a, b| {
                dist2(e.position, plan.slots[*a].position)
                    .total_cmp(&dist2(e.position, plan.slots[*b].position))
            }) {
                available.remove(&i);
                assigned.insert(e.id, plan.slots[i].position);
            }
        }
        // Only defenders allocated to the prospective sector participate in its Engineer lead.
        // Rear-sector troops retain their own posts and never need to regroup at the new base.
        for (id, point) in &assigned {
            if pullback.active.contains(id) {
                continue;
            }
            if plan
                .slots
                .iter()
                .find(|slot| slot.position == *point)
                .and_then(|slot| plan.sectors.get(slot.sector_id))
                .is_some_and(|sector| {
                    self.frontier
                        .is_some_and(|site| sector.base_ids.contains(&site))
                })
            {
                if !previous_screen.contains(id)
                    || self
                        .destinations
                        .get(id)
                        .is_none_or(|old| dist2(*old, *point) > 1.)
                {
                    self.advance_orders.remove(id);
                }
                self.screen_units.insert(*id);
            }
        }
        for e in infantry {
            if unavailable.contains(&e.id) {
                continue;
            }
            let resume = response.released.contains(&e.id) || pullback.released.contains(&e.id);
            let Some(point) = assigned.get(&e.id).copied() else {
                continue;
            };
            let changed = resume
                || self
                    .destinations
                    .get(&e.id)
                    .is_none_or(|p| dist2(*p, point) > 1.);
            let Ok(group) = UnitGroup::new([e.id]) else {
                continue;
            };
            // An expansion is an explicit regroup, including units already fighting or
            // entrenched. Every existing/new infantry unit receives an attack-move first;
            // normal post maintenance must never replace that advance with a plain Move.
            if self.frontier.is_some()
                && self.screen_units.contains(&e.id)
                && !self.advance_orders.contains_key(&e.id)
            {
                if actions.attack_move(&group, point.0, point.1, false).is_ok() {
                    self.advance_orders.insert(e.id, frame.tick());
                    self.held.remove(&e.id);
                }
                continue;
            }
            // Leave nearby fights in place. Move commands, rather than chase orders, bring the
            // defenders back to legal posts after a fight or an expansion changes the plan.
            if e.state == AiEntityState::Attack && !resume {
                continue;
            }
            if dist2(e.position, point) > (frame.map().tile_size as f32 * 0.3).det_powi(2) {
                if (changed || e.state != AiEntityState::Move)
                    && if self.frontier.is_some() {
                        actions.attack_move(&group, point.0, point.1, false).is_ok()
                    } else {
                        actions.move_group(&group, point.0, point.1, false).is_ok()
                    }
                {
                    self.held.remove(&e.id);
                }
            } else if (changed || !self.held.contains(&e.id) || e.state != AiEntityState::Idle)
                && actions.hold_position(&group, false).is_ok()
            {
                self.held.insert(e.id);
            }
        }
        self.screen_wave.retain(|id| {
            self.screen_units.contains(id)
                && frame
                    .owned()
                    .iter()
                    .any(|e| e.id == *id && e.health.current > 0)
        });
        if self.screen_wave.is_empty() {
            self.screen_wave
                .extend(self.screen_units.iter().copied().filter(|id| {
                    frame
                        .owned()
                        .iter()
                        .any(|e| e.id == *id && e.health.current > 0)
                }));
        }
        self.guard_posts
            .extend(assigned.iter().map(|(id, p)| (*id, *p)));
        self.guard_posts.extend(self.armor.guard_posts());
        self.held.retain(|id| assigned.contains_key(id));
        self.destinations = assigned;
    }
}

fn infantry(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
    )
}
