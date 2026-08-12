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
    MoveSequence(&'static [(u32, (f32, f32))]),
    AttackMove,
    MoveWithPanzerfaustWindup {
        attacker: u32,
        victim: u32,
        windup_ticks: u16,
    },
}

impl DevScenarioSetup {
    pub fn command(&self) -> SimCommand {
        match self.order {
            DevScenarioOrder::Move | DevScenarioOrder::MoveSequence(&[]) => SimCommand::Move {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
            DevScenarioOrder::MoveSequence(sequence) => {
                let (_, (x, y)) = sequence[0];
                SimCommand::Move {
                    units: self.units.clone(),
                    x,
                    y,
                    queued: false,
                }
            }
            DevScenarioOrder::AttackMove => SimCommand::AttackMove {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
            DevScenarioOrder::MoveWithPanzerfaustWindup { .. } => SimCommand::Move {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
        }
    }

    pub fn scheduled_commands(&self) -> Vec<(u32, SimCommand)> {
        match self.order {
            DevScenarioOrder::MoveSequence(&[]) => {
                vec![(self.issue_after_ticks, self.command())]
            }
            DevScenarioOrder::MoveSequence(sequence) => sequence
                .iter()
                .map(|&(tick, (x, y))| {
                    (
                        tick,
                        SimCommand::Move {
                            units: self.units.clone(),
                            x,
                            y,
                            queued: false,
                        },
                    )
                })
                .collect(),
            _ => vec![(self.issue_after_ticks, self.command())],
        }
    }

    pub fn panzerfaust_windup(&self) -> Option<(u32, u32, u16)> {
        match self.order {
            DevScenarioOrder::MoveWithPanzerfaustWindup {
                attacker,
                victim,
                windup_ticks,
            } => Some((attacker, victim, windup_ticks)),
            _ => None,
        }
    }

    pub(super) fn checkpoint_backed(mut self, label: &str) -> Result<Self, String> {
        self.game = Game::checkpoint_backed_start_from_direct_for_setup(self.game, label)
            .map_err(|err| format!("failed to build checkpoint-backed {label} start: {err}"))?;
        Ok(self)
    }
}
