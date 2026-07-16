use std::collections::BTreeSet;

use crate::config;
use crate::game::commands::PendingCommand;
use crate::game::entity::{Entity, MAX_PRODUCTION_QUEUE, MAX_QUEUED_ORDERS};
use crate::game::firing_reveal::FiringRevealSource;
use crate::game::fog::LingeringSightSource;
use crate::game::map::Map;
use crate::game::panzerfaust_shot::PanzerfaustShotStore;
use crate::game::replay::CommandLogEntry;
use crate::game::SimCommand;
use crate::rules;

use super::{
    BuildingMemoryV1, CheckpointPayloadError, EntityStoreV1, FogStateV1, PlayerStateV1,
    MAX_COMPLETED_UPGRADES_PER_PLAYER, MAX_UNITS_PER_CHECKPOINT_COMMAND,
};

mod firing_reveal;
pub(super) use firing_reveal::validate_reaction_gates_against_visibility;
use firing_reveal::{validate_firing_reveal_reaction_gates, validate_firing_reveal_visibility};

const MAX_RESOURCE_INCOME_HISTORY_PER_PLAYER: usize = (config::TICK_HZ as usize * 60) + 1;

pub(super) fn validate_supplied_map(map: &Map) -> Result<(), CheckpointPayloadError> {
    if map.size == 0 {
        return Err(CheckpointPayloadError::InvalidValue { field: "map.size" });
    }
    let cells = (map.size as usize)
        .checked_mul(map.size as usize)
        .ok_or(CheckpointPayloadError::InvalidValue { field: "map.size" })?;
    if map.terrain.len() != cells {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "map.terrain",
        });
    }
    Ok(())
}

pub(super) fn validate_count(
    field: &'static str,
    count: usize,
    max: usize,
) -> Result<(), CheckpointPayloadError> {
    if count > max {
        Err(CheckpointPayloadError::CountCapExceeded { field, count, max })
    } else {
        Ok(())
    }
}

pub(super) fn validate_players(
    players: &[PlayerStateV1],
    tick: u32,
) -> Result<BTreeSet<u32>, CheckpointPayloadError> {
    let mut ids = BTreeSet::new();
    for player in players {
        if player.id == 0 || !ids.insert(player.id) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "players",
                id: player.id,
            });
        }
        validate_count(
            "players.upgrades",
            player.upgrades.len(),
            MAX_COMPLETED_UPGRADES_PER_PLAYER,
        )?;
        validate_count(
            "players.score.resourceIncomeHistory",
            player.resource_income_history_len(),
            MAX_RESOURCE_INCOME_HISTORY_PER_PLAYER,
        )?;
        let mut income_ticks = BTreeSet::new();
        for income_tick in player.resource_income_history_ticks() {
            if income_tick > tick {
                return Err(CheckpointPayloadError::InvalidValue {
                    field: "players.score.resourceIncomeHistory.tick",
                });
            }
            if !income_ticks.insert(income_tick) {
                return Err(CheckpointPayloadError::DuplicateId {
                    field: "players.score.resourceIncomeHistory",
                    id: income_tick,
                });
            }
        }
    }
    Ok(ids)
}

pub(super) fn validate_entities(
    entities: &EntityStoreV1,
    player_ids: &BTreeSet<u32>,
    map: &Map,
    tick: u32,
) -> Result<BTreeSet<u32>, CheckpointPayloadError> {
    if entities.next_id == 0 {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.nextId",
        });
    }
    let mut ids = BTreeSet::new();
    let world = map.world_size_px();
    for entity in &entities.entities {
        validate_entity(entity, entities.next_id, player_ids, world, tick, &mut ids)?;
    }
    Ok(ids)
}

pub(super) fn validate_player_supply(
    players: &[PlayerStateV1],
    entities: &EntityStoreV1,
) -> Result<(), CheckpointPayloadError> {
    for player in players {
        let catalog = rules::faction::catalog_for(&player.faction_id);
        let mut expected_used = 0u32;
        let expected_cap = config::PLAYER_SUPPLY_CAP;
        for entity in &entities.entities {
            if entity.owner != player.id {
                continue;
            }
            if entity.is_building() && !entity.under_construction() {
                for item in entity.prod_queue().iter().filter(|item| item.paid) {
                    if catalog.is_some_and(|catalog| catalog.allows_unit(item.unit)) {
                        expected_used =
                            expected_used.saturating_add(rules::economy::supply_cost(item.unit));
                    }
                }
            } else if entity.is_unit()
                && catalog.is_some_and(|catalog| catalog.allows_unit(entity.kind))
            {
                expected_used =
                    expected_used.saturating_add(rules::economy::supply_cost(entity.kind));
            }
        }
        if player.supply_used != expected_used {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "players.supplyUsed",
            });
        }
        if player.supply_cap != expected_cap {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "players.supplyCap",
            });
        }
    }
    Ok(())
}

fn validate_entity(
    entity: &Entity,
    next_id: u32,
    player_ids: &BTreeSet<u32>,
    world: f32,
    tick: u32,
    ids: &mut BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    if entity.id == 0 || !ids.insert(entity.id) {
        return Err(CheckpointPayloadError::DuplicateId {
            field: "entities",
            id: entity.id,
        });
    }
    if entity.id >= next_id {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.id",
        });
    }
    if entity.owner != 0 && !player_ids.contains(&entity.owner) {
        return Err(CheckpointPayloadError::InvalidReference {
            field: "entities.owner",
            id: entity.owner,
        });
    }
    if !in_world(entity.pos_x, entity.pos_y, world) {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.position",
        });
    }
    if entity.max_hp == 0 || entity.hp > entity.max_hp {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.hp",
        });
    }
    if entity.queued_orders().len() > MAX_QUEUED_ORDERS {
        return Err(CheckpointPayloadError::CountCapExceeded {
            field: "entities.queuedOrders",
            count: entity.queued_orders().len(),
            max: MAX_QUEUED_ORDERS,
        });
    }
    validate_count(
        "entities.production.queue",
        entity.prod_queue().len(),
        MAX_PRODUCTION_QUEUE,
    )?;
    validate_count(
        "entities.production.researchQueue",
        entity.research_queue().len(),
        MAX_PRODUCTION_QUEUE,
    )?;
    validate_tank_armor_reaction_lock(entity, world, tick)?;
    validate_firing_reveal_reaction_gates(entity, next_id, tick)?;
    Ok(())
}

fn validate_tank_armor_reaction_lock(
    entity: &Entity,
    world: f32,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    let Some(combat) = entity.combat.as_ref() else {
        return Ok(());
    };
    let Some(lock) = combat.tank_armor_reaction_lock else {
        return Ok(());
    };
    if !rules::combat::unit_uses_tank_armor_reaction(entity.kind) || entity.hp == 0 {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.combat.tankArmorReactionLock",
        });
    }
    if !in_world(lock.source_x, lock.source_y, world) {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.combat.tankArmorReactionLock.source",
        });
    }
    if lock.acquired_tick > tick
        || tick.saturating_sub(lock.acquired_tick) >= rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS
    {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.combat.tankArmorReactionLock.acquiredTick",
        });
    }
    Ok(())
}

pub(super) fn validate_fog(
    fog: &FogStateV1,
    player_ids: &BTreeSet<u32>,
    entity_ids: &BTreeSet<u32>,
    firing_reveals: &[FiringRevealSource],
    map: &Map,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    if fog.size != map.size {
        return Err(CheckpointPayloadError::InvalidValue { field: "fog.size" });
    }
    let cells = (map.size as usize)
        .checked_mul(map.size as usize)
        .ok_or(CheckpointPayloadError::InvalidValue { field: "fog.size" })?;
    for (&player, grid) in &fog.base_grids {
        if !player_ids.contains(&player) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "fog.baseGrids",
                id: player,
            });
        }
        if grid.len() != cells {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "fog.baseGrids",
            });
        }
    }
    for (&player, grid) in &fog.grids {
        if !player_ids.contains(&player) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "fog.grids",
                id: player,
            });
        }
        if grid.len() != cells {
            return Err(CheckpointPayloadError::InvalidValue { field: "fog.grids" });
        }
    }
    validate_firing_reveal_visibility(fog, player_ids, entity_ids, firing_reveals, tick)?;
    Ok(())
}

pub(super) fn validate_building_memory(
    memory: &BuildingMemoryV1,
    player_ids: &BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    let mut keys = BTreeSet::new();
    for entry in &memory.entries {
        if !player_ids.contains(&entry.player_id) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "buildingMemory.playerId",
                id: entry.player_id,
            });
        }
        if entry.entry.id != entry.building_id {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "buildingMemory.entry.id",
            });
        }
        if !keys.insert((entry.player_id, entry.building_id)) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "buildingMemory",
                id: entry.building_id,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_pending_commands(
    pending: &[PendingCommand],
    player_ids: &BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    for command in pending {
        if !player_ids.contains(&command.player) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "pendingCommands.player",
                id: command.player,
            });
        }
        validate_command_units(&command.command)?;
    }
    Ok(())
}

pub(super) fn validate_command_log(
    command_log: &[CommandLogEntry],
    tick: u32,
    player_ids: &BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    let mut last_tick = 0;
    for entry in command_log {
        if entry.tick < last_tick || entry.tick > tick {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLog.tick",
            });
        }
        if !player_ids.contains(&entry.player_id) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "commandLog.playerId",
                id: entry.player_id,
            });
        }
        last_tick = entry.tick;
    }
    Ok(())
}

pub(super) fn validate_active_sources(
    lingering_sight: &[LingeringSightSource],
    firing_reveals: &[FiringRevealSource],
    tick: u32,
    player_ids: &BTreeSet<u32>,
    entity_ids: &BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    for source in lingering_sight {
        if !player_ids.contains(&source.owner()) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "lingeringSight.owner",
                id: source.owner(),
            });
        }
        if !source.is_active_at(tick) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "lingeringSight.expiresAtTick",
            });
        }
    }
    for source in firing_reveals {
        if !player_ids.contains(&source.viewer()) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "firingReveals.viewer",
                id: source.viewer(),
            });
        }
        if !entity_ids.contains(&source.entity_id()) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "firingReveals.entityId",
                id: source.entity_id(),
            });
        }
        if source.started_at_tick() > tick || !source.is_active_at(tick) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "firingReveals.startedAtTick",
            });
        }
    }
    Ok(())
}

pub(super) fn validate_panzerfaust_shots(
    shots: &PanzerfaustShotStore,
    player_ids: &BTreeSet<u32>,
    next_entity_id: u32,
    map: &Map,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    for (owner, attacker, target, source_x, source_y, impact_x, impact_y, impact_tick) in
        shots.checkpoint_entries()
    {
        if !player_ids.contains(&owner) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "panzerfaustShots.owner",
                id: owner,
            });
        }
        validate_allocated_entity_ref("panzerfaustShots.attacker", attacker, next_entity_id)?;
        validate_allocated_entity_ref("panzerfaustShots.target", target, next_entity_id)?;
        let world = map.world_size_px();
        if !in_world(source_x, source_y, world) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "panzerfaustShots.source",
            });
        }
        if !in_world(impact_x, impact_y, world) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "panzerfaustShots.impact",
            });
        }
        if impact_tick <= tick {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "panzerfaustShots.impactTick",
            });
        }
    }
    Ok(())
}

pub(super) fn validate_id_set(
    field: &'static str,
    ids: &BTreeSet<u32>,
    valid_ids: &BTreeSet<u32>,
) -> Result<(), CheckpointPayloadError> {
    for id in ids {
        if !valid_ids.contains(id) {
            return Err(CheckpointPayloadError::InvalidReference { field, id: *id });
        }
    }
    Ok(())
}

fn validate_allocated_entity_ref(
    field: &'static str,
    id: u32,
    next_entity_id: u32,
) -> Result<(), CheckpointPayloadError> {
    if id == 0 || id >= next_entity_id {
        Err(CheckpointPayloadError::InvalidReference { field, id })
    } else {
        Ok(())
    }
}

fn validate_command_units(command: &SimCommand) -> Result<(), CheckpointPayloadError> {
    let units = match command {
        SimCommand::Move { units, .. }
        | SimCommand::AttackMove { units, .. }
        | SimCommand::Attack { units, .. }
        | SimCommand::Deconstruct { units, .. }
        | SimCommand::SetupAntiTankGuns { units, .. }
        | SimCommand::TearDownAntiTankGuns { units }
        | SimCommand::UseAbility { units, .. }
        | SimCommand::RecastAbility { units, .. }
        | SimCommand::SetAutocast { units, .. }
        | SimCommand::Gather { units, .. }
        | SimCommand::Build { units, .. }
        | SimCommand::Stop { units }
        | SimCommand::HoldPosition { units, .. } => Some(units),
        SimCommand::AdjustProductionRepeat { buildings, .. } => Some(buildings),
        SimCommand::Train { .. }
        | SimCommand::Research { .. }
        | SimCommand::Cancel { .. }
        | SimCommand::SetRally { .. }
        | SimCommand::Rejected { .. } => None,
    };
    if let Some(units) = units {
        validate_count(
            "command.units",
            units.len(),
            MAX_UNITS_PER_CHECKPOINT_COMMAND,
        )?;
    }
    Ok(())
}

fn in_world(x: f32, y: f32, world: f32) -> bool {
    x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x < world && y < world
}
