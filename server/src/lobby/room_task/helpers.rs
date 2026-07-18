use std::time::Duration;

use rts_ai::AiController;
use rts_sim::game::PlayerInit;

use super::types::AiSlot;

const AUTOMATED_MATCH_HISTORY_ROOM_PREFIXES: [&str; 4] =
    ["itest-", "ai-itest-", "client-smoke-", "reg-"];
pub(super) const MATCH_COUNTDOWN_WORDS: [&str; 3] = ["Drei!", "Zwei!", "Eins!"];
pub(super) const LAB_PLAYER_ONE_ID: u32 = 1;
#[cfg(test)]
pub(super) const LAB_PLAYER_TWO_ID: u32 = 2;
pub(super) const LIVE_PAUSE_LIMIT: u8 = 3;
pub(super) const DRAINING_NEW_MATCHES_DISABLED_MSG: &str =
    "Server is draining for deploy; new matches are disabled.";

pub(super) fn match_countdown_duration() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(1)
    }
    #[cfg(not(test))]
    {
        Duration::from_secs(3)
    }
}

pub(super) fn server_build_sha() -> &'static str {
    crate::build_info::build_id()
}

pub(in crate::lobby) fn is_automated_match_history_room(room: &str) -> bool {
    AUTOMATED_MATCH_HISTORY_ROOM_PREFIXES
        .iter()
        .any(|prefix| room.starts_with(prefix))
}

pub(in crate::lobby) fn match_history_participants_are_automated(participants: &[String]) -> bool {
    let mut has_alpha = false;
    let mut has_bravo = false;
    for participant in participants {
        let name = participant.trim();
        if name.eq_ignore_ascii_case("smoke") {
            return true;
        }
        has_alpha |= name == "Alpha";
        has_bravo |= name == "Bravo";
    }
    has_alpha && has_bravo
}

pub(super) fn late_spectator_notice_name(name: &str) -> String {
    let cleaned: String = name.trim().chars().filter(|ch| !ch.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Commander".to_string()
    } else {
        cleaned.to_string()
    }
}

pub(super) fn live_ai_controllers(
    players: &[PlayerInit],
    ai_slots: &[AiSlot],
) -> Vec<AiController> {
    players
        .iter()
        .filter(|player| player.is_ai)
        .map(|player| {
            let profile_id = ai_slots
                .iter()
                .find(|ai| ai.id == player.id)
                .map(|ai| ai.profile_id)
                .unwrap_or(rts_ai::DEFAULT_LIVE_PROFILE_ID);
            let profile_id = rts_ai::resolve_live_profile_id_for_match(profile_id);
            AiController::with_profile_id(player.id, profile_id)
        })
        .collect()
}
