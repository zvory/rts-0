//! Internal gameplay commands.
//!
//! Wire commands are decoded in `protocol.rs` and translated into this domain shape at the
//! networking/replay boundary. Simulation services consume `SimCommand` so they can work with
//! typed entity kinds instead of JSON-facing strings.

use crate::game::entity::EntityKind;
use crate::protocol;

#[derive(Debug, Clone, PartialEq)]
pub enum SimCommand {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
    },
    Build {
        worker: u32,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
    },
    Train {
        building: u32,
        unit: EntityKind,
    },
    Cancel {
        building: u32,
    },
    Stop {
        units: Vec<u32>,
    },
    Rejected {
        reason: CommandRejection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRejection {
    UnknownBuilding,
    UnknownUnit,
}

impl CommandRejection {
    pub(crate) fn notice_message(self) -> &'static str {
        match self {
            CommandRejection::UnknownBuilding => "Unknown building",
            CommandRejection::UnknownUnit => "Unknown unit",
        }
    }
}

impl SimCommand {
    pub fn from_protocol(command: protocol::Command) -> Self {
        match command {
            protocol::Command::Move { units, x, y } => SimCommand::Move { units, x, y },
            protocol::Command::AttackMove { units, x, y } => SimCommand::AttackMove { units, x, y },
            protocol::Command::Attack { units, target } => SimCommand::Attack { units, target },
            protocol::Command::Gather { units, node } => SimCommand::Gather { units, node },
            protocol::Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } => match building.parse::<EntityKind>() {
                Ok(building) if building.is_building() => SimCommand::Build {
                    worker,
                    building,
                    tile_x,
                    tile_y,
                },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::UnknownBuilding,
                },
            },
            protocol::Command::Train { building, unit } => match unit.parse::<EntityKind>() {
                Ok(unit) if unit.is_unit() => SimCommand::Train { building, unit },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::UnknownUnit,
                },
            },
            protocol::Command::Cancel { building } => SimCommand::Cancel { building },
            protocol::Command::Stop { units } => SimCommand::Stop { units },
        }
    }

    pub fn to_protocol(&self) -> Option<protocol::Command> {
        Some(match self {
            SimCommand::Move { units, x, y } => protocol::Command::Move {
                units: units.clone(),
                x: *x,
                y: *y,
            },
            SimCommand::AttackMove { units, x, y } => protocol::Command::AttackMove {
                units: units.clone(),
                x: *x,
                y: *y,
            },
            SimCommand::Attack { units, target } => protocol::Command::Attack {
                units: units.clone(),
                target: *target,
            },
            SimCommand::Gather { units, node } => protocol::Command::Gather {
                units: units.clone(),
                node: *node,
            },
            SimCommand::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } => protocol::Command::Build {
                worker: *worker,
                building: building.to_protocol_str().to_string(),
                tile_x: *tile_x,
                tile_y: *tile_y,
            },
            SimCommand::Train { building, unit } => protocol::Command::Train {
                building: *building,
                unit: unit.to_protocol_str().to_string(),
            },
            SimCommand::Cancel { building } => protocol::Command::Cancel {
                building: *building,
            },
            SimCommand::Stop { units } => protocol::Command::Stop {
                units: units.clone(),
            },
            SimCommand::Rejected { .. } => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::kinds;

    #[test]
    fn protocol_build_command_translates_kind() {
        let command = protocol::Command::Build {
            worker: 7,
            building: kinds::BARRACKS.to_string(),
            tile_x: 4,
            tile_y: 8,
        };

        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Build {
                worker: 7,
                building: EntityKind::Barracks,
                tile_x: 4,
                tile_y: 8,
            }
        );
    }

    #[test]
    fn protocol_unknown_train_unit_becomes_rejected_command() {
        let command = protocol::Command::Train {
            building: 3,
            unit: "made_up".to_string(),
        };

        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Rejected {
                reason: CommandRejection::UnknownUnit,
            }
        );
    }
}
