//! Internal gameplay commands.
//!
//! Wire commands are decoded in `protocol.rs` and translated into this domain shape at the
//! networking/replay boundary. Simulation services consume `SimCommand` so they can work with
//! typed entity kinds instead of JSON-facing strings.

use crate::game::ability::AbilityKind;
use crate::game::entity::{EntityKind, RallyKind};
use crate::game::upgrade::UpgradeKind;
use crate::protocol;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimCommand {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        queued: bool,
    },
    Deconstruct {
        units: Vec<u32>,
        target: u32,
        queued: bool,
    },
    SetupAntiTankGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    TearDownAntiTankGuns {
        units: Vec<u32>,
    },
    UseAbility {
        ability: AbilityKind,
        units: Vec<u32>,
        x: Option<f32>,
        y: Option<f32>,
        queued: bool,
    },
    RecastAbility {
        ability: AbilityKind,
        units: Vec<u32>,
        target_object_id: Option<u32>,
        queued: bool,
    },
    SetAutocast {
        ability: AbilityKind,
        units: Vec<u32>,
        enabled: bool,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        queued: bool,
    },
    Build {
        units: Vec<u32>,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
        queued: bool,
    },
    Train {
        building: u32,
        unit: EntityKind,
    },
    Research {
        building: u32,
        upgrade: UpgradeKind,
    },
    QueueBuild {
        units: Vec<u32>,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
        queued: bool,
    },
    QueueTrain {
        building: u32,
        unit: EntityKind,
        quantity: u32,
        automatic: bool,
    },
    QueueResearch {
        building: u32,
        upgrade: UpgradeKind,
    },
    Cancel {
        building: u32,
    },
    Stop {
        units: Vec<u32>,
    },
    HoldPosition {
        units: Vec<u32>,
    },
    SetRally {
        building: u32,
        x: f32,
        y: f32,
        kind: RallyKind,
        queued: bool,
    },
    Rejected {
        reason: CommandRejection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandRejection {
    Building,
    Unit,
    Upgrade,
    Ability,
}

impl CommandRejection {
    pub(crate) fn notice_message(self) -> &'static str {
        match self {
            CommandRejection::Building => "Unknown building",
            CommandRejection::Unit => "Unknown unit",
            CommandRejection::Upgrade => "Unknown upgrade",
            CommandRejection::Ability => "Unknown ability",
        }
    }
}

impl SimCommand {
    pub fn from_protocol(command: protocol::Command) -> Self {
        match command {
            protocol::Command::Move {
                units,
                x,
                y,
                queued,
            } => SimCommand::Move {
                units,
                x,
                y,
                queued,
            },
            protocol::Command::AttackMove {
                units,
                x,
                y,
                queued,
            } => SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            },
            protocol::Command::Attack {
                units,
                target,
                queued,
            } => SimCommand::Attack {
                units,
                target,
                queued,
            },
            protocol::Command::Deconstruct {
                units,
                target,
                queued,
            } => SimCommand::Deconstruct {
                units,
                target,
                queued,
            },
            protocol::Command::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            } => SimCommand::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            },
            protocol::Command::TearDownAntiTankGuns { units } => {
                SimCommand::TearDownAntiTankGuns { units }
            }
            protocol::Command::Charge { units } => SimCommand::UseAbility {
                ability: AbilityKind::Charge,
                units,
                x: None,
                y: None,
                queued: false,
            },
            protocol::Command::UseAbility {
                ability,
                units,
                x,
                y,
                queued,
            } => match ability.parse::<AbilityKind>() {
                Ok(ability) => SimCommand::UseAbility {
                    ability,
                    units,
                    x,
                    y,
                    queued,
                },
                Err(_) => SimCommand::Rejected {
                    reason: CommandRejection::Ability,
                },
            },
            protocol::Command::RecastAbility {
                ability,
                units,
                target_object_id,
                queued,
            } => match ability.parse::<AbilityKind>() {
                Ok(ability) => SimCommand::RecastAbility {
                    ability,
                    units,
                    target_object_id,
                    queued,
                },
                Err(_) => SimCommand::Rejected {
                    reason: CommandRejection::Ability,
                },
            },
            protocol::Command::SetAutocast {
                ability,
                units,
                enabled,
            } => match ability.parse::<AbilityKind>() {
                Ok(ability) => SimCommand::SetAutocast {
                    ability,
                    units,
                    enabled,
                },
                Err(_) => SimCommand::Rejected {
                    reason: CommandRejection::Ability,
                },
            },
            protocol::Command::Gather {
                units,
                node,
                queued,
            } => SimCommand::Gather {
                units,
                node,
                queued,
            },
            protocol::Command::Build {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => match building.parse::<EntityKind>() {
                Ok(building) if building.is_building() => SimCommand::Build {
                    units,
                    building,
                    tile_x,
                    tile_y,
                    queued,
                },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::Building,
                },
            },
            protocol::Command::Train { building, unit } => match unit.parse::<EntityKind>() {
                Ok(unit) if unit.is_unit() => SimCommand::Train { building, unit },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::Unit,
                },
            },
            protocol::Command::Research { building, upgrade } => {
                match upgrade.parse::<UpgradeKind>() {
                    Ok(upgrade) => SimCommand::Research { building, upgrade },
                    _ => SimCommand::Rejected {
                        reason: CommandRejection::Upgrade,
                    },
                }
            }
            protocol::Command::QueueBuild {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => match building.parse::<EntityKind>() {
                Ok(building) if building.is_building() => SimCommand::QueueBuild {
                    units,
                    building,
                    tile_x,
                    tile_y,
                    queued,
                },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::Building,
                },
            },
            protocol::Command::QueueTrain {
                building,
                unit,
                quantity,
                automatic,
            } => match unit.parse::<EntityKind>() {
                Ok(unit) if unit.is_unit() => SimCommand::QueueTrain {
                    building,
                    unit,
                    quantity,
                    automatic,
                },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::Unit,
                },
            },
            protocol::Command::QueueResearch { building, upgrade } => {
                match upgrade.parse::<UpgradeKind>() {
                    Ok(upgrade) => SimCommand::QueueResearch { building, upgrade },
                    _ => SimCommand::Rejected {
                        reason: CommandRejection::Upgrade,
                    },
                }
            }
            protocol::Command::Cancel { building } => SimCommand::Cancel { building },
            protocol::Command::Stop { units } => SimCommand::Stop { units },
            protocol::Command::HoldPosition { units } => SimCommand::HoldPosition { units },
            protocol::Command::SetRally {
                building,
                x,
                y,
                kind,
                queued,
            } => match RallyKind::from_protocol_str(kind.as_deref()) {
                Some(kind) => SimCommand::SetRally {
                    building,
                    x,
                    y,
                    kind,
                    queued,
                },
                None => SimCommand::Rejected {
                    reason: CommandRejection::Unit,
                },
            },
        }
    }

    pub fn to_protocol(&self) -> Option<protocol::Command> {
        Some(match self {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => protocol::Command::Move {
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => protocol::Command::AttackMove {
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::Attack {
                units,
                target,
                queued,
            } => protocol::Command::Attack {
                units: units.clone(),
                target: *target,
                queued: *queued,
            },
            SimCommand::Deconstruct {
                units,
                target,
                queued,
            } => protocol::Command::Deconstruct {
                units: units.clone(),
                target: *target,
                queued: *queued,
            },
            SimCommand::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            } => protocol::Command::SetupAntiTankGuns {
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::TearDownAntiTankGuns { units } => protocol::Command::TearDownAntiTankGuns {
                units: units.clone(),
            },
            SimCommand::UseAbility {
                ability,
                units,
                x,
                y,
                queued,
            } => protocol::Command::UseAbility {
                ability: ability.to_protocol_str().to_string(),
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::RecastAbility {
                ability,
                units,
                target_object_id,
                queued,
            } => protocol::Command::RecastAbility {
                ability: ability.to_protocol_str().to_string(),
                units: units.clone(),
                target_object_id: *target_object_id,
                queued: *queued,
            },
            SimCommand::SetAutocast {
                ability,
                units,
                enabled,
            } => protocol::Command::SetAutocast {
                ability: ability.to_protocol_str().to_string(),
                units: units.clone(),
                enabled: *enabled,
            },
            SimCommand::Gather {
                units,
                node,
                queued,
            } => protocol::Command::Gather {
                units: units.clone(),
                node: *node,
                queued: *queued,
            },
            SimCommand::Build {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => protocol::Command::Build {
                units: units.clone(),
                building: protocol::kind_to_wire(*building).to_string(),
                tile_x: *tile_x,
                tile_y: *tile_y,
                queued: *queued,
            },
            SimCommand::Train { building, unit } => protocol::Command::Train {
                building: *building,
                unit: protocol::kind_to_wire(*unit).to_string(),
            },
            SimCommand::Research { building, upgrade } => protocol::Command::Research {
                building: *building,
                upgrade: upgrade.to_protocol_str().to_string(),
            },
            SimCommand::QueueBuild {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => protocol::Command::QueueBuild {
                units: units.clone(),
                building: protocol::kind_to_wire(*building).to_string(),
                tile_x: *tile_x,
                tile_y: *tile_y,
                queued: *queued,
            },
            SimCommand::QueueTrain {
                building,
                unit,
                quantity,
                automatic,
            } => protocol::Command::QueueTrain {
                building: *building,
                unit: protocol::kind_to_wire(*unit).to_string(),
                quantity: *quantity,
                automatic: *automatic,
            },
            SimCommand::QueueResearch { building, upgrade } => protocol::Command::QueueResearch {
                building: *building,
                upgrade: upgrade.to_protocol_str().to_string(),
            },
            SimCommand::Cancel { building } => protocol::Command::Cancel {
                building: *building,
            },
            SimCommand::Stop { units } => protocol::Command::Stop {
                units: units.clone(),
            },
            SimCommand::HoldPosition { units } => protocol::Command::HoldPosition {
                units: units.clone(),
            },
            SimCommand::SetRally {
                building,
                x,
                y,
                kind,
                queued,
            } => protocol::Command::SetRally {
                building: *building,
                x: *x,
                y: *y,
                kind: Some(kind.to_protocol_str().to_string()),
                queued: *queued,
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
            units: vec![7],
            building: kinds::BARRACKS.to_string(),
            tile_x: 4,
            tile_y: 8,
            queued: false,
        };

        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Build {
                units: vec![7],
                building: EntityKind::Barracks,
                tile_x: 4,
                tile_y: 8,
                queued: false,
            }
        );
    }

    #[test]
    fn protocol_build_command_requires_units() {
        let decoded = serde_json::from_str::<protocol::Command>(
            r#"{"c":"build","worker":7,"building":"barracks","tileX":4,"tileY":8}"#,
        );

        assert!(
            decoded.is_err(),
            "build commands must use the current units[] payload"
        );
    }

    #[test]
    fn protocol_queued_flag_defaults_false_and_round_trips_when_true() {
        let decoded: protocol::Command =
            serde_json::from_str(r#"{"c":"move","units":[1],"x":10.0,"y":20.0}"#)
                .expect("omitted queued flag should deserialize");
        assert_eq!(
            SimCommand::from_protocol(decoded),
            SimCommand::Move {
                units: vec![1],
                x: 10.0,
                y: 20.0,
                queued: false,
            }
        );

        let command = protocol::Command::Move {
            units: vec![1],
            x: 10.0,
            y: 20.0,
            queued: true,
        };
        let encoded = serde_json::to_string(&command).expect("queued command should serialize");
        assert!(
            encoded.contains(r#""queued":true"#),
            "serialized command log entry should preserve queued=true"
        );
        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Move {
                units: vec![1],
                x: 10.0,
                y: 20.0,
                queued: true,
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
                reason: CommandRejection::Unit,
            }
        );
    }

    #[test]
    fn protocol_anti_tank_gun_setup_commands_round_trip() {
        let setup = protocol::Command::SetupAntiTankGuns {
            units: vec![3, 5],
            x: 100.0,
            y: 200.0,
            queued: true,
        };
        assert_eq!(
            SimCommand::from_protocol(setup.clone()),
            SimCommand::SetupAntiTankGuns {
                units: vec![3, 5],
                x: 100.0,
                y: 200.0,
                queued: true,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(setup.clone()).to_protocol(),
            Some(setup)
        );

        let teardown = protocol::Command::TearDownAntiTankGuns { units: vec![7] };
        assert_eq!(
            SimCommand::from_protocol(teardown.clone()),
            SimCommand::TearDownAntiTankGuns { units: vec![7] }
        );
        assert_eq!(
            SimCommand::from_protocol(teardown.clone()).to_protocol(),
            Some(teardown)
        );
    }

    #[test]
    fn protocol_deconstruct_command_round_trips() {
        let command = protocol::Command::Deconstruct {
            units: vec![3, 5],
            target: 77,
            queued: true,
        };

        assert_eq!(
            SimCommand::from_protocol(command.clone()),
            SimCommand::Deconstruct {
                units: vec![3, 5],
                target: 77,
                queued: true,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(command.clone()).to_protocol(),
            Some(command)
        );
    }

    #[test]
    fn protocol_use_ability_command_round_trips() {
        let command = protocol::Command::UseAbility {
            ability: protocol::abilities::SMOKE.to_string(),
            units: vec![10, 11],
            x: Some(320.0),
            y: Some(384.0),
            queued: true,
        };

        assert_eq!(
            SimCommand::from_protocol(command.clone()),
            SimCommand::UseAbility {
                ability: AbilityKind::Smoke,
                units: vec![10, 11],
                x: Some(320.0),
                y: Some(384.0),
                queued: true,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(command.clone()).to_protocol(),
            Some(command)
        );
    }

    #[test]
    fn blanket_fire_protocol_command_round_trips() {
        let command = protocol::Command::UseAbility {
            ability: protocol::abilities::BLANKET_FIRE.to_string(),
            units: vec![10],
            x: Some(704.0),
            y: Some(768.0),
            queued: true,
        };

        assert_eq!(
            SimCommand::from_protocol(command.clone()),
            SimCommand::UseAbility {
                ability: AbilityKind::BlanketFire,
                units: vec![10],
                x: Some(704.0),
                y: Some(768.0),
                queued: true,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(command.clone()).to_protocol(),
            Some(command)
        );
    }

    #[test]
    fn protocol_recast_ability_command_round_trips() {
        let command = protocol::Command::RecastAbility {
            ability: protocol::abilities::EKAT_TELEPORT.to_string(),
            units: vec![21],
            target_object_id: Some(77),
            queued: false,
        };

        assert_eq!(
            SimCommand::from_protocol(command.clone()),
            SimCommand::RecastAbility {
                ability: AbilityKind::EkatTeleport,
                units: vec![21],
                target_object_id: Some(77),
                queued: false,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(command.clone()).to_protocol(),
            Some(command)
        );
    }
}
