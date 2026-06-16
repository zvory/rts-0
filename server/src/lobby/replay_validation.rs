use super::faction_validation::{
    validate_faction_request, FactionRejectReason, FactionRequestContext, FactionValidation,
};
use rts_sim::game::replay::ReplayArtifactV1;
use std::collections::HashSet;

pub fn faction_loadout_incompatibility_reason(artifact: &ReplayArtifactV1) -> Option<String> {
    validate_faction_loadouts(artifact).err()
}

pub(super) fn validate_faction_loadouts(artifact: &ReplayArtifactV1) -> Result<(), String> {
    let mut seen_players = HashSet::new();
    for player in &artifact.players {
        if !seen_players.insert(player.id) {
            return Err(format!(
                "Replay artifact has duplicate player id {}.",
                player.id
            ));
        }
        if let FactionValidation::Rejected { requested, reason } = validate_faction_request(
            FactionRequestContext::ReplayPlayback,
            Some(&player.faction_id),
        ) {
            return Err(faction_rejection_message(player.id, requested, reason));
        }
    }

    let mut seen_loadouts = HashSet::new();
    for loadout in &artifact.player_loadouts {
        if !seen_loadouts.insert(loadout.player_id) {
            return Err(format!(
                "Replay has duplicate loadout records for player {}.",
                loadout.player_id
            ));
        }
        let Some(player) = artifact
            .players
            .iter()
            .find(|player| player.id == loadout.player_id)
        else {
            return Err(format!(
                "Replay loadout references unknown player {}.",
                loadout.player_id
            ));
        };
        if loadout.faction_id != player.faction_id {
            return Err(format!(
                "Replay player {} loadout faction {:?} does not match player faction {:?}.",
                loadout.player_id, loadout.faction_id, player.faction_id
            ));
        }
        if loadout.loadout_id.trim().is_empty() {
            return Err(format!(
                "Replay player {} is missing a loadout id.",
                loadout.player_id
            ));
        }
        if rts_rules::faction::catalog_loadout_for(&player.faction_id, loadout.loadout_id.trim())
            .is_none()
        {
            return Err(format!(
                "Replay player {} uses unknown loadout {:?} for faction {:?}.",
                loadout.player_id, loadout.loadout_id, player.faction_id
            ));
        }
    }

    for player in &artifact.players {
        if !seen_loadouts.contains(&player.id) {
            return Err(format!("Replay player {} is missing a loadout.", player.id));
        }
    }

    Ok(())
}

fn faction_rejection_message(
    player_id: u32,
    requested: Option<String>,
    reason: FactionRejectReason,
) -> String {
    match reason {
        FactionRejectReason::MissingFaction => {
            format!("Replay player {player_id} is missing a faction id.")
        }
        FactionRejectReason::UnknownCatalog => format!(
            "Replay player {} has unknown faction {:?}.",
            player_id,
            requested.unwrap_or_default()
        ),
        FactionRejectReason::FixtureNotAllowed => format!(
            "Replay player {} uses fixture-only faction {:?}.",
            player_id,
            requested.unwrap_or_default()
        ),
        FactionRejectReason::FactionNotAllowedInContext => format!(
            "Replay player {} uses unsupported faction {:?}.",
            player_id,
            requested.unwrap_or_default()
        ),
    }
}
