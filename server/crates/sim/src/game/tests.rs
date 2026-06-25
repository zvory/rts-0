use std::collections::HashMap;

use super::scoring::entity_score_value;
use super::*;
use crate::game::command::SimCommand as Command;
use crate::game::entity::{Entity, EntityKind, GatherPhase, Order, OrderIntent, WeaponSetup};
use crate::protocol::{kinds, terrain, AbilityCooldownView, EntityView, Event, OrderPlanMarker};
use crate::rules::{combat, terrain::TerrainKind};

mod ability_runtime_tests;
mod ai_identity_tests;
mod artillery_tests;
mod determinism_tests;
mod ekat_tests;
mod faction_ability_tests;
mod fixtures;
mod meth_movement_tests;
mod movement_replay_tests;
mod resources_mining_tests;
mod scoring_projection_tests;
mod smoke_mortar_tests;
mod tank_trap_tests;
