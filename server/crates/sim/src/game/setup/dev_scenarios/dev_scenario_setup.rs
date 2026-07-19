use super::*;

pub struct DevScenarioSetup {
    pub game: Game,
    pub player_id: u32,
    pub units: Vec<u32>,
    pub goal: (f32, f32),
    pub issue_after_ticks: u32,
    pub(super) order: DevScenarioOrder,
}

#[derive(Clone, Copy)]
pub(super) enum DevScenarioOrder {
    Move,
    AttackMove,
}

impl DevScenarioSetup {
    pub fn command(&self) -> SimCommand {
        match self.order {
            DevScenarioOrder::Move => SimCommand::Move {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
            DevScenarioOrder::AttackMove => SimCommand::AttackMove {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
        }
    }

    pub(super) fn checkpoint_backed(mut self, label: &str) -> Result<Self, String> {
        self.game = Game::checkpoint_backed_start_from_direct_for_setup(self.game, label)
            .map_err(|err| format!("failed to build checkpoint-backed {label} start: {err}"))?;
        Ok(self)
    }
}
