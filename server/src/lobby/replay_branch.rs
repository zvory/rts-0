use std::collections::{HashMap, HashSet};

use super::faction_validation::{
    validate_faction_request, FactionRejectReason, FactionRequestContext, FactionValidation,
};
use super::ReplayBranchSeed;
use crate::protocol::{BranchStagingOccupant, BranchStagingSeat, ServerMessage};
use rts_sim::game::Game;

pub(super) struct BranchLaunchPlan {
    pub(super) game: Game,
    pub(super) seat_by_connection: HashMap<u32, u32>,
    pub(super) match_player_count: usize,
    pub(super) map_name: String,
    pub(super) seed: u32,
    pub(super) participants: Vec<String>,
}

#[derive(Debug)]
pub(super) enum BranchLaunchError {
    NotReady,
    MissingOccupant,
    UnsupportedFaction {
        seat_player_id: u32,
        requested: Option<String>,
        reason: FactionRejectReason,
    },
}

pub(super) struct BranchStagingState {
    seed: ReplayBranchSeed,
    claimed_by_seat: HashMap<u32, u32>,
}

impl BranchStagingState {
    pub(super) fn new(seed: ReplayBranchSeed) -> Self {
        Self {
            seed,
            claimed_by_seat: HashMap::new(),
        }
    }

    pub(super) fn source_tick(&self) -> u32 {
        self.seed.source_tick
    }

    pub(super) fn can_start(&self) -> bool {
        self.seed
            .seats
            .iter()
            .filter(|seat| seat.claimable)
            .all(|seat| self.claimed_by_seat.contains_key(&seat.player_id))
    }

    pub(super) fn claimant_for_occupant(&self, occupant_id: u32) -> Option<u32> {
        self.claimed_by_seat
            .iter()
            .find_map(|(seat_player_id, claimant_id)| {
                (*claimant_id == occupant_id).then_some(*seat_player_id)
            })
    }

    #[cfg(test)]
    pub(super) fn claimant_for_seat(&self, seat_player_id: u32) -> Option<u32> {
        self.claimed_by_seat.get(&seat_player_id).copied()
    }

    pub(super) fn claim(
        &mut self,
        occupant_id: u32,
        seat_player_id: u32,
    ) -> Result<(), &'static str> {
        if !self
            .seed
            .seats
            .iter()
            .any(|seat| seat.player_id == seat_player_id && seat.claimable)
        {
            return Err("unknown branch seat");
        }
        if self.claimant_for_occupant(occupant_id).is_some() {
            return Err("occupant already claimed a branch seat");
        }
        if self.claimed_by_seat.contains_key(&seat_player_id) {
            return Err("branch seat already claimed");
        }
        self.claimed_by_seat.insert(seat_player_id, occupant_id);
        Ok(())
    }

    pub(super) fn release(&mut self, occupant_id: u32, seat_player_id: u32) -> bool {
        if self.claimed_by_seat.get(&seat_player_id) != Some(&occupant_id) {
            return false;
        }
        self.claimed_by_seat.remove(&seat_player_id);
        true
    }

    pub(super) fn release_occupant(&mut self, occupant_id: u32) {
        self.claimed_by_seat
            .retain(|_, claimant_id| *claimant_id != occupant_id);
    }

    pub(super) fn message(
        &self,
        room: String,
        host_id: u32,
        occupants: Vec<BranchStagingOccupant>,
        can_start_allowed: bool,
    ) -> ServerMessage {
        ServerMessage::BranchStaging {
            room,
            source_tick: self.source_tick(),
            host_id,
            seats: self.seats_for_message(&occupants),
            occupants,
            can_start: can_start_allowed && self.can_start(),
        }
    }

    pub(super) fn prepare_launch(
        &self,
        mut connection_exists: impl FnMut(u32) -> bool,
    ) -> Result<BranchLaunchPlan, BranchLaunchError> {
        for seat in &self.seed.seats {
            if let FactionValidation::Rejected { requested, reason } = validate_faction_request(
                FactionRequestContext::ReplayBranch,
                Some(&seat.faction_id),
            ) {
                return Err(BranchLaunchError::UnsupportedFaction {
                    seat_player_id: seat.player_id,
                    requested,
                    reason,
                });
            }
        }
        let Some(seat_by_connection) = self.connection_to_seat_map() else {
            return Err(BranchLaunchError::NotReady);
        };
        if !seat_by_connection.keys().all(|id| connection_exists(*id)) {
            return Err(BranchLaunchError::MissingOccupant);
        }

        let game = self.seed.game.clone_for_replay_keyframe();
        let active_seats: HashSet<u32> = seat_by_connection.values().copied().collect();
        let participants = self
            .seed
            .seats
            .iter()
            .filter(|seat| active_seats.contains(&seat.player_id))
            .map(|seat| seat.name.clone())
            .collect();

        Ok(BranchLaunchPlan {
            game,
            seat_by_connection,
            match_player_count: active_seats.len(),
            map_name: self.seed.source_replay.map_name.clone(),
            seed: self.seed.source_replay.seed,
            participants,
        })
    }

    fn connection_to_seat_map(&self) -> Option<HashMap<u32, u32>> {
        if !self.can_start() {
            return None;
        }
        Some(
            self.claimed_by_seat
                .iter()
                .map(|(seat_player_id, occupant_id)| (*occupant_id, *seat_player_id))
                .collect(),
        )
    }

    fn seats_for_message(&self, occupants: &[BranchStagingOccupant]) -> Vec<BranchStagingSeat> {
        let occupant_names: HashMap<u32, &str> = occupants
            .iter()
            .map(|occupant| (occupant.id, occupant.name.as_str()))
            .collect();
        self.seed
            .seats
            .iter()
            .map(|seat| {
                let claimant_id = self.claimed_by_seat.get(&seat.player_id).copied();
                let claimant_name = claimant_id
                    .and_then(|id| occupant_names.get(&id).copied())
                    .map(str::to_string);
                BranchStagingSeat {
                    player_id: seat.player_id,
                    team_id: seat.team_id,
                    faction_id: seat.faction_id.clone(),
                    name: seat.name.clone(),
                    color: seat.color.clone(),
                    claimant_id,
                    claimant_name,
                }
            })
            .collect()
    }
}
