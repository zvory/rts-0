use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum CommandAdmission {
    Normal,
    LabIgnoreCommandLimits,
}

#[derive(Debug, Clone)]
pub(in crate::game) struct PendingCommand {
    pub(in crate::game) player: u32,
    pub(in crate::game) command: SimCommand,
    pub(in crate::game) admission: CommandAdmission,
}

impl PendingCommand {
    fn normal(player: u32, command: SimCommand) -> Self {
        Self {
            player,
            command,
            admission: CommandAdmission::Normal,
        }
    }

    fn lab_ignore_command_limits(player: u32, command: SimCommand) -> Self {
        Self {
            player,
            command,
            admission: CommandAdmission::LabIgnoreCommandLimits,
        }
    }
}

impl Game {
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand) {
        self.pending.push(PendingCommand::normal(player, cmd));
    }

    pub(in crate::game) fn enqueue_lab_command_ignoring_limits(
        &mut self,
        player: u32,
        cmd: SimCommand,
    ) {
        self.pending
            .push(PendingCommand::lab_ignore_command_limits(player, cmd));
    }

    pub(super) fn record_commands_for_tick(&mut self, pending: &[PendingCommand]) {
        self.command_log
            .extend(pending.iter().filter_map(|pending| {
                pending.command.to_protocol().map(|command| CommandLogEntry {
                    tick: self.tick,
                    player_id: pending.player,
                    command,
                })
            }));
    }
}
