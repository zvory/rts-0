use std::collections::HashMap;

use crate::config;
use crate::protocol::{AttackReveal, Event};
use serde::{Deserialize, Serialize};

use super::fog::Fog;
use super::teams::TeamRelations;

/// Temporary actionable sight granted to a recipient when a hostile unit exposes itself by firing.
///
/// Like lingering death sight, this is stamped into live fog so command validation, combat
/// targeting, and snapshot projection all treat the revealed unit as currently visible. The
/// stable start tick identifies one continuous episode even when later shots extend its expiry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::game) struct FiringRevealSource {
    viewer: u32,
    entity_id: u32,
    started_at_tick: u32,
    expires_at_tick: u32,
}

impl FiringRevealSource {
    fn new(
        viewer: u32,
        entity_id: u32,
        started_at_tick: u32,
        expires_at_tick: u32,
    ) -> Option<Self> {
        if viewer == 0 || entity_id == 0 {
            return None;
        }
        Some(Self {
            viewer,
            entity_id,
            started_at_tick,
            expires_at_tick,
        })
    }

    pub(in crate::game) fn is_active_at(self, tick: u32) -> bool {
        self.expires_at_tick > tick
    }

    pub(in crate::game) fn viewer(self) -> u32 {
        self.viewer
    }

    pub(in crate::game) fn entity_id(self) -> u32 {
        self.entity_id
    }

    pub(in crate::game) fn started_at_tick(self) -> u32 {
        self.started_at_tick
    }

    fn expires_at_tick(self) -> u32 {
        self.expires_at_tick
    }

    fn upsert(
        sources: &mut Vec<Self>,
        viewer: u32,
        entity_id: u32,
        started_at_tick: u32,
        expires_at_tick: u32,
    ) {
        let Some(source) = Self::new(viewer, entity_id, started_at_tick, expires_at_tick) else {
            return;
        };
        match sources.iter_mut().find(|existing| {
            existing.viewer() == source.viewer() && existing.entity_id() == source.entity_id()
        }) {
            Some(existing) if !existing.is_active_at(started_at_tick) => {
                *existing = source;
            }
            Some(existing) if source.expires_at_tick() > existing.expires_at_tick() => {
                // Repeated shots extend one continuous reveal episode. Keeping the original
                // start tick prevents target switching or move spam from charging a fresh
                // reaction delay for every extension.
                existing.expires_at_tick = source.expires_at_tick();
            }
            Some(_) => {}
            None => sources.push(source),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn record_firing_reveals_for_victim_team(
    firing_reveals: &mut Vec<FiringRevealSource>,
    player_ids: impl IntoIterator<Item = u32>,
    fog: &Fog,
    teams: &TeamRelations,
    victim_owner: u32,
    attacker_owner: u32,
    entity_id: u32,
    attacker_pos: (f32, f32),
    fired_at_tick: u32,
    firing_cycle_ticks: u32,
) {
    if victim_owner == 0 || !teams.is_enemy_owner(attacker_owner, victim_owner) {
        return;
    }
    let expires_at_tick = fired_at_tick
        .saturating_add(firing_cycle_ticks)
        .saturating_add(config::TICK_HZ / 2);
    for viewer in player_ids {
        if !teams.same_team_or_same_owner(viewer, victim_owner) {
            continue;
        }
        let already_visible_without_reveal =
            fog.is_visible_without_firing_reveal_world(viewer, attacker_pos.0, attacker_pos.1);
        if already_visible_without_reveal {
            continue;
        }
        FiringRevealSource::upsert(
            firing_reveals,
            viewer,
            entity_id,
            fired_at_tick,
            expires_at_tick,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn record_firing_reveals_for_victim_teams(
    firing_reveals: &mut Vec<FiringRevealSource>,
    player_ids: &[u32],
    fog: &Fog,
    teams: &TeamRelations,
    victim_owners: &[u32],
    attacker_owner: u32,
    entity_id: u32,
    attacker_pos: (f32, f32),
    fired_at_tick: u32,
    firing_cycle_ticks: u32,
) {
    for &victim_owner in victim_owners {
        record_firing_reveals_for_victim_team(
            firing_reveals,
            player_ids.iter().copied(),
            fog,
            teams,
            victim_owner,
            attacker_owner,
            entity_id,
            attacker_pos,
            fired_at_tick,
            firing_cycle_ticks,
        );
    }
}

pub(in crate::game) fn record_global_firing_reveals_for_enemy_players(
    firing_reveals: &mut Vec<FiringRevealSource>,
    player_ids: &[u32],
    teams: &TeamRelations,
    attacker_owner: u32,
    entity_id: u32,
    fired_at_tick: u32,
    firing_cycle_ticks: u32,
) {
    if attacker_owner == 0 {
        return;
    }
    let expires_at_tick = fired_at_tick
        .saturating_add(firing_cycle_ticks)
        .saturating_add(config::TICK_HZ / 2);
    for &viewer in player_ids {
        if teams.is_enemy_owner(attacker_owner, viewer) {
            FiringRevealSource::upsert(
                firing_reveals,
                viewer,
                entity_id,
                fired_at_tick,
                expires_at_tick,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn record_mortar_impact_firing_reveals(
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &HashMap<u32, Vec<Event>>,
    fog: &Fog,
    teams: &TeamRelations,
    victim_owners: &[u32],
    attacker_owner: u32,
    attacker: u32,
    reveal: Option<&AttackReveal>,
    tick: u32,
    firing_cycle_ticks: u32,
) {
    let Some(reveal) = reveal else {
        return;
    };
    let player_ids = events.keys().copied().collect::<Vec<_>>();
    record_firing_reveals_for_victim_teams(
        firing_reveals,
        &player_ids,
        fog,
        teams,
        victim_owners,
        attacker_owner,
        attacker,
        (reveal.x, reveal.y),
        tick,
        firing_cycle_ticks,
    );
}
