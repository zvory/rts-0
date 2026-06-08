use super::*;

impl Game {
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand) {
        self.pending.push((player, cmd));
    }

    pub(super) fn record_commands_for_tick(&mut self, pending: &[(u32, SimCommand)]) {
        self.command_log
            .extend(pending.iter().filter_map(|(player_id, command)| {
                command.to_protocol().map(|command| CommandLogEntry {
                    tick: self.tick,
                    player_id: *player_id,
                    command,
                })
            }));
    }
}
