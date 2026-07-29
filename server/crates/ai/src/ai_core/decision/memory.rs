use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::{AiProfile, AttackPolicy};
use crate::config;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

use super::defense::{
    DefensivePanic, DefensivePanicResponse, DEFENSIVE_PANIC_GRACE_TICKS,
    DEFENSIVE_PANIC_SUSTAINED_TICKS,
};

const CITY_CENTRE_RESUME_SAFE_TICKS: u32 = config::TICK_HZ * 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IncompleteCityCentreMemory {
    hp: u32,
    last_damage_tick: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiDecisionMemory {
    profile_id: Option<&'static str>,
    attack_first_size: Option<usize>,
    next_attack_size: usize,
    last_attack_tick: Option<u32>,
    defensive_panic_started_tick: Option<u32>,
    defensive_panic_last_tick: Option<u32>,
    defensive_panic_response: DefensivePanicResponse,
    pub(super) pending_upgrades: BTreeSet<UpgradeKind>,
    launched_frontal_units: BTreeMap<u32, u32>,
    pub(super) turtle_opening_riflemen_ordered: usize,
    incomplete_city_centres: BTreeMap<u32, IncompleteCityCentreMemory>,
}

impl AiDecisionMemory {
    pub(crate) fn for_profile(profile: &AiProfile) -> Self {
        Self {
            profile_id: Some(profile.id),
            attack_first_size: Some(profile.attack.first_attack_size),
            next_attack_size: profile.attack.first_attack_size,
            last_attack_tick: None,
            defensive_panic_started_tick: None,
            defensive_panic_last_tick: None,
            defensive_panic_response: DefensivePanicResponse::Riflemen,
            pending_upgrades: BTreeSet::new(),
            launched_frontal_units: BTreeMap::new(),
            turtle_opening_riflemen_ordered: 0,
            incomplete_city_centres: BTreeMap::new(),
        }
    }

    pub(crate) fn desired_attack_size(&mut self, profile: &AiProfile, tick: u32) -> usize {
        self.desired_attack_size_for(profile, profile.attack, tick)
    }

    pub(super) fn desired_attack_size_for(
        &mut self,
        profile: &AiProfile,
        attack: AttackPolicy,
        tick: u32,
    ) -> usize {
        self.ensure_attack_policy(profile, attack);
        if self
            .last_attack_tick
            .map(|last| tick.saturating_sub(last) >= attack.regroup_reset_ticks)
            .unwrap_or(false)
        {
            self.next_attack_size = attack.first_attack_size;
        }
        self.next_attack_size
    }

    pub(super) fn note_attack_for(
        &mut self,
        profile: &AiProfile,
        attack: AttackPolicy,
        tick: u32,
        units: &[u32],
    ) {
        self.ensure_attack_policy(profile, attack);
        self.last_attack_tick = Some(tick);
        self.next_attack_size = self.next_attack_size.saturating_add(attack.wave_growth);
        if profile.frontal_wave.exclude_launched_ticks.is_some() {
            for unit in units {
                self.launched_frontal_units.insert(*unit, tick);
            }
        }
    }

    pub(super) fn attack_due_for(
        &mut self,
        profile: &AiProfile,
        attack: AttackPolicy,
        tick: u32,
    ) -> bool {
        self.ensure_attack_policy(profile, attack);
        self.last_attack_tick
            .map(|last| tick.saturating_sub(last) >= attack.reissue_cadence_ticks)
            .unwrap_or(true)
    }

    pub(super) fn ensure_profile(&mut self, profile: &AiProfile) {
        if self.profile_id == Some(profile.id) && self.next_attack_size != 0 {
            return;
        }
        self.profile_id = Some(profile.id);
        self.attack_first_size = Some(profile.attack.first_attack_size);
        self.next_attack_size = profile.attack.first_attack_size;
        self.last_attack_tick = None;
        self.defensive_panic_started_tick = None;
        self.defensive_panic_last_tick = None;
        self.defensive_panic_response = DefensivePanicResponse::Riflemen;
        self.pending_upgrades.clear();
        self.launched_frontal_units.clear();
        self.turtle_opening_riflemen_ordered = 0;
        self.incomplete_city_centres.clear();
    }

    fn ensure_attack_policy(&mut self, profile: &AiProfile, attack: AttackPolicy) {
        self.ensure_profile(profile);
        if self.attack_first_size == Some(attack.first_attack_size) && self.next_attack_size != 0 {
            return;
        }
        self.attack_first_size = Some(attack.first_attack_size);
        self.next_attack_size = attack.first_attack_size;
        self.last_attack_tick = None;
        self.launched_frontal_units.clear();
    }

    pub(super) fn launched_frontal_unit_exclusions(
        &mut self,
        profile: &AiProfile,
        tick: u32,
        owned_units: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let Some(exclude_ticks) = profile.frontal_wave.exclude_launched_ticks else {
            self.launched_frontal_units.clear();
            return BTreeSet::new();
        };
        self.launched_frontal_units.retain(|unit, launched_tick| {
            owned_units.contains(unit) && tick.saturating_sub(*launched_tick) < exclude_ticks
        });
        self.launched_frontal_units.keys().copied().collect()
    }

    pub(super) fn defensive_panic(
        &mut self,
        threat_response: Option<DefensivePanicResponse>,
        tick: u32,
    ) -> DefensivePanic {
        if let Some(response) = threat_response {
            let should_restart = self
                .defensive_panic_last_tick
                .map(|last| tick.saturating_sub(last) > DEFENSIVE_PANIC_GRACE_TICKS)
                .unwrap_or(true);
            if should_restart {
                self.defensive_panic_started_tick = Some(tick);
            }
            self.defensive_panic_last_tick = Some(tick);
            self.defensive_panic_response = response;
        }

        let active = self
            .defensive_panic_last_tick
            .map(|last| tick.saturating_sub(last) <= DEFENSIVE_PANIC_GRACE_TICKS)
            .unwrap_or(false);
        if !active {
            self.defensive_panic_started_tick = None;
            self.defensive_panic_response = DefensivePanicResponse::Riflemen;
        }
        let sustained = active
            && self
                .defensive_panic_started_tick
                .map(|started| tick.saturating_sub(started) >= DEFENSIVE_PANIC_SUSTAINED_TICKS)
                .unwrap_or(false);
        DefensivePanic {
            active,
            sustained,
            response: self.defensive_panic_response,
        }
    }

    pub(super) fn sync_turtle_opening(&mut self, profile: &AiProfile, observation: &AiObservation) {
        let Some(policy) = profile.turtle_defense else {
            self.turtle_opening_riflemen_ordered = 0;
            return;
        };
        self.turtle_opening_riflemen_ordered = self
            .turtle_opening_riflemen_ordered
            .max(unit_and_queue_count(observation, EntityKind::Rifleman))
            .min(policy.opening_riflemen);
    }

    pub(super) fn sync_incomplete_city_centres(&mut self, observation: &AiObservation) {
        let mut active_sites = BTreeMap::new();
        for site in observation
            .owned
            .iter()
            .filter(|entity| entity.kind == EntityKind::CityCentre && !entity.is_complete)
        {
            active_sites.insert(site.id, site.hp);
        }
        self.incomplete_city_centres
            .retain(|site_id, _| active_sites.contains_key(site_id));
        for (site_id, hp) in active_sites {
            self.incomplete_city_centres
                .entry(site_id)
                .and_modify(|site| {
                    if hp < site.hp {
                        site.last_damage_tick = observation.tick;
                    }
                    site.hp = hp;
                })
                .or_insert(IncompleteCityCentreMemory {
                    hp,
                    last_damage_tick: observation.tick,
                });
        }
    }

    pub(super) fn city_centre_is_safe_to_resume(&self, site_id: u32, tick: u32) -> bool {
        self.incomplete_city_centres
            .get(&site_id)
            .map(|site| tick.saturating_sub(site.last_damage_tick) >= CITY_CENTRE_RESUME_SAFE_TICKS)
            .unwrap_or(false)
    }

    pub(super) fn note_turtle_train(&mut self, profile: &AiProfile, unit: EntityKind) {
        let Some(policy) = profile.turtle_defense else {
            return;
        };
        if unit == EntityKind::Rifleman {
            self.turtle_opening_riflemen_ordered = self
                .turtle_opening_riflemen_ordered
                .saturating_add(1)
                .min(policy.opening_riflemen);
        }
    }
}

fn unit_and_queue_count(observation: &AiObservation, kind: EntityKind) -> usize {
    let units = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == kind)
        .count();
    let queued = observation
        .owned
        .iter()
        .filter(|entity| entity.is_complete)
        .filter(|entity| entity.production_kind == Some(kind))
        .map(|entity| entity.production_queue_len.unwrap_or(0))
        .sum::<usize>();
    units.saturating_add(queued)
}
