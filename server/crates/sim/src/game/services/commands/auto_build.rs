use crate::game::auto_build::RESERVE_MAX;
use crate::game::{PlayerState, SimCommand};

pub(super) fn apply(players: &mut [PlayerState], player: u32, command: SimCommand) {
    let SimCommand::SetAutoBuildSettings {
        paused,
        reserve_steel,
        reserve_oil,
    } = command
    else {
        return;
    };
    if let Some(player) = players.iter_mut().find(|candidate| candidate.id == player) {
        player.auto_build.paused = paused;
        player.auto_build.reserve_steel = reserve_steel.min(RESERVE_MAX);
        player.auto_build.reserve_oil = reserve_oil.min(RESERVE_MAX);
    }
}
