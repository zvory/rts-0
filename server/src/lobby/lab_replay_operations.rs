//! Translation between simulation Lab operations and their replay protocol representation.

use std::str::FromStr;

use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabCommandOptions, LabMoveEntity, LabOp, LabSetCompletedResearch, LabSetEntityOwner,
    LabSetPlayerResources, LabSpawnEntity, LabUpdate,
};
use rts_sim::game::upgrade::UpgradeKind;

use super::lab_timeline::LabTimelineEntryKind;
use crate::protocol::{LabReplayOperation, LabSpawnEntitySpec, LabUpdateSpec};

pub(super) fn lab_op_to_replay_operation(op: &LabOp) -> Option<LabReplayOperation> {
    match op {
        LabOp::SpawnEntities(spawns) => Some(LabReplayOperation::SpawnEntities {
            spawns: spawns
                .iter()
                .map(|input| LabSpawnEntitySpec {
                    owner: input.owner,
                    kind: input.kind.stable_id().to_string(),
                    x: input.x,
                    y: input.y,
                    completed: input.completed,
                })
                .collect(),
        }),
        LabOp::ApplyUpdates(updates) => Some(LabReplayOperation::ApplyUpdates {
            updates: updates.iter().map(update_to_protocol).collect(),
        }),
        LabOp::DeleteEntities(entity_ids) => Some(LabReplayOperation::DeleteEntities {
            entity_ids: entity_ids.clone(),
        }),
        LabOp::SpawnEntity(input) => Some(LabReplayOperation::SpawnEntities {
            spawns: vec![LabSpawnEntitySpec {
                owner: input.owner,
                kind: input.kind.stable_id().to_string(),
                x: input.x,
                y: input.y,
                completed: input.completed,
            }],
        }),
        LabOp::DeleteEntity { entity_id } => Some(LabReplayOperation::DeleteEntities {
            entity_ids: vec![*entity_id],
        }),
        LabOp::MoveEntity(input) => Some(LabReplayOperation::ApplyUpdates {
            updates: vec![update_to_protocol(&LabUpdate::Move(*input))],
        }),
        LabOp::SetEntityOwner(input) => Some(LabReplayOperation::ApplyUpdates {
            updates: vec![update_to_protocol(&LabUpdate::SetEntityOwner(*input))],
        }),
        LabOp::SetPlayerResources(input) => Some(LabReplayOperation::ApplyUpdates {
            updates: vec![update_to_protocol(&LabUpdate::SetPlayerResources(*input))],
        }),
        LabOp::SetPlayerGodMode { player_id, enabled } => Some(LabReplayOperation::ApplyUpdates {
            updates: vec![update_to_protocol(&LabUpdate::SetPlayerGodMode {
                player_id: *player_id,
                enabled: *enabled,
            })],
        }),
        LabOp::SetCompletedResearch(input) => Some(LabReplayOperation::ApplyUpdates {
            updates: vec![update_to_protocol(&LabUpdate::SetCompletedResearch(*input))],
        }),
        LabOp::ApplyMapDraft(_) | LabOp::RestoreCheckpointScenario(_) => None,
    }
}

fn update_to_protocol(update: &LabUpdate) -> LabUpdateSpec {
    match update {
        LabUpdate::Move(input) => LabUpdateSpec::Move {
            entity_id: input.entity_id,
            x: input.x,
            y: input.y,
        },
        LabUpdate::SetEntityOwner(input) => LabUpdateSpec::Reassign {
            entity_id: input.entity_id,
            owner: input.owner,
        },
        LabUpdate::SetPlayerResources(input) => LabUpdateSpec::Resources {
            player_id: input.player_id,
            steel: input.steel,
            oil: input.oil,
        },
        LabUpdate::SetPlayerGodMode { player_id, enabled } => LabUpdateSpec::GodMode {
            player_id: *player_id,
            enabled: *enabled,
        },
        LabUpdate::SetCompletedResearch(input) => LabUpdateSpec::Research {
            player_id: input.player_id,
            upgrade: input.upgrade.to_protocol_str().to_string(),
            completed: input.completed,
        },
    }
}

fn lab_replay_operation_kind(op: &LabReplayOperation) -> &'static str {
    match op {
        LabReplayOperation::SpawnEntities { .. } => "spawnEntities",
        LabReplayOperation::ApplyUpdates { .. } => "applyUpdates",
        LabReplayOperation::DeleteEntities { .. } => "deleteEntities",
        LabReplayOperation::SpawnEntity { .. } => "spawnEntity",
        LabReplayOperation::DeleteEntity { .. } => "deleteEntity",
        LabReplayOperation::MoveEntity { .. } => "moveEntity",
        LabReplayOperation::SetEntityOwner { .. } => "setEntityOwner",
        LabReplayOperation::SetPlayerResources { .. } => "setPlayerResources",
        LabReplayOperation::SetPlayerGodMode { .. } => "setPlayerGodMode",
        LabReplayOperation::SetCompletedResearch { .. } => "setCompletedResearch",
        LabReplayOperation::IssueCommandAs { .. } => "issueCommandAs",
    }
}

pub(super) fn lab_replay_operation_to_entry_kind(
    replay_op: &LabReplayOperation,
) -> Result<LabTimelineEntryKind, String> {
    match replay_op {
        LabReplayOperation::SpawnEntities { spawns } => Ok(LabTimelineEntryKind::LabOperation {
            op_kind: lab_replay_operation_kind(replay_op).to_string(),
            op: LabOp::SpawnEntities(
                spawns
                    .iter()
                    .map(|spawn| {
                        let kind = EntityKind::from_str(&spawn.kind)
                            .map_err(|_| "unknown entity kind".to_string())?;
                        Ok(LabSpawnEntity {
                            owner: spawn.owner,
                            kind,
                            x: spawn.x,
                            y: spawn.y,
                            completed: spawn.completed,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            ),
        }),
        LabReplayOperation::ApplyUpdates { updates } => Ok(LabTimelineEntryKind::LabOperation {
            op_kind: lab_replay_operation_kind(replay_op).to_string(),
            op: LabOp::ApplyUpdates(
                updates
                    .iter()
                    .map(update_from_protocol)
                    .collect::<Result<Vec<_>, String>>()?,
            ),
        }),
        LabReplayOperation::DeleteEntities { entity_ids } => {
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::DeleteEntities(entity_ids.clone()),
            })
        }
        LabReplayOperation::SpawnEntity {
            owner,
            kind,
            x,
            y,
            completed,
        } => {
            let kind = EntityKind::from_str(kind).map_err(|_| "unknown entity kind".to_string())?;
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::SpawnEntity(LabSpawnEntity {
                    owner: *owner,
                    kind,
                    x: *x,
                    y: *y,
                    completed: *completed,
                }),
            })
        }
        LabReplayOperation::DeleteEntity { entity_id } => Ok(LabTimelineEntryKind::LabOperation {
            op_kind: lab_replay_operation_kind(replay_op).to_string(),
            op: LabOp::DeleteEntity {
                entity_id: *entity_id,
            },
        }),
        LabReplayOperation::MoveEntity { entity_id, x, y } => {
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::MoveEntity(LabMoveEntity {
                    entity_id: *entity_id,
                    x: *x,
                    y: *y,
                }),
            })
        }
        LabReplayOperation::SetEntityOwner { entity_id, owner } => {
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::SetEntityOwner(LabSetEntityOwner {
                    entity_id: *entity_id,
                    owner: *owner,
                }),
            })
        }
        LabReplayOperation::SetPlayerResources {
            player_id,
            steel,
            oil,
        } => Ok(LabTimelineEntryKind::LabOperation {
            op_kind: lab_replay_operation_kind(replay_op).to_string(),
            op: LabOp::SetPlayerResources(LabSetPlayerResources {
                player_id: *player_id,
                steel: *steel,
                oil: *oil,
            }),
        }),
        LabReplayOperation::SetPlayerGodMode { player_id, enabled } => {
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::SetPlayerGodMode {
                    player_id: *player_id,
                    enabled: *enabled,
                },
            })
        }
        LabReplayOperation::SetCompletedResearch {
            player_id,
            upgrade,
            completed,
        } => {
            let upgrade =
                UpgradeKind::from_str(upgrade).map_err(|_| "unknown research id".to_string())?;
            Ok(LabTimelineEntryKind::LabOperation {
                op_kind: lab_replay_operation_kind(replay_op).to_string(),
                op: LabOp::SetCompletedResearch(LabSetCompletedResearch {
                    player_id: *player_id,
                    upgrade,
                    completed: *completed,
                }),
            })
        }
        LabReplayOperation::IssueCommandAs {
            player_id,
            cmd,
            ignore_command_limits,
        } => Ok(LabTimelineEntryKind::IssueCommandAs {
            player_id: *player_id,
            command: cmd.clone(),
            options: LabCommandOptions {
                ignore_command_limits: *ignore_command_limits,
            },
        }),
    }
}

fn update_from_protocol(update: &LabUpdateSpec) -> Result<LabUpdate, String> {
    Ok(match update {
        LabUpdateSpec::Move { entity_id, x, y } => LabUpdate::Move(LabMoveEntity {
            entity_id: *entity_id,
            x: *x,
            y: *y,
        }),
        LabUpdateSpec::Reassign { entity_id, owner } => {
            LabUpdate::SetEntityOwner(LabSetEntityOwner {
                entity_id: *entity_id,
                owner: *owner,
            })
        }
        LabUpdateSpec::Resources {
            player_id,
            steel,
            oil,
        } => LabUpdate::SetPlayerResources(LabSetPlayerResources {
            player_id: *player_id,
            steel: *steel,
            oil: *oil,
        }),
        LabUpdateSpec::Research {
            player_id,
            upgrade,
            completed,
        } => LabUpdate::SetCompletedResearch(LabSetCompletedResearch {
            player_id: *player_id,
            upgrade: UpgradeKind::from_str(upgrade)
                .map_err(|_| "unknown research id".to_string())?,
            completed: *completed,
        }),
        LabUpdateSpec::GodMode { player_id, enabled } => LabUpdate::SetPlayerGodMode {
            player_id: *player_id,
            enabled: *enabled,
        },
    })
}
