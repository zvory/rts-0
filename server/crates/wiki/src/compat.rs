//! Rules APIs that not every game version has. build.rs looks for each in the copied source and
//! sets a `has_*` cfg; without it the value is simply left out (null), so older versions export.

#![allow(unused_variables, unused_imports)]

use crate::snapshot::{f, id};
use rts_rules::combat::{self, WeaponProfile};
use rts_rules::faction::{self, AbilityCatalogEntry, AbilityKind};
use rts_rules::{balance, EntityKind};
use serde_json::{json, Value};

pub fn engagement(k: EntityKind) -> Value {
    #[cfg(has_engagement_range)]
    return crate::snapshot::v(combat::engagement_range_tiles(
        k,
        combat::attack_profile(k).range_tiles,
    ));
    #[cfg(not(has_engagement_range))]
    return crate::snapshot::v(combat::attack_profile(k).range_tiles);
}

pub fn movement_body(k: EntityKind) -> Value {
    #[cfg(has_body_class)]
    return json!(format!("{:?}", rts_rules::movement_body_class(k)));
    #[cfg(not(has_body_class))]
    return Value::Null;
}

pub fn threat_role(k: EntityKind) -> Value {
    #[cfg(has_threat_role)]
    return json!(format!("{:?}", combat::target_threat_role(k)));
    #[cfg(not(has_threat_role))]
    return Value::Null;
}

pub fn target_priority(k: EntityKind) -> Value {
    #[cfg(has_target_priority)]
    return json!(combat::default_target_priority_policy(k).stable_id());
    #[cfg(not(has_target_priority))]
    return Value::Null;
}

pub fn fires_while_moving(k: EntityKind) -> bool {
    #[cfg(has_fires_while_moving)]
    return rts_rules::fires_while_moving(k);
    #[cfg(not(has_fires_while_moving))]
    return false;
}

pub fn rifle_infantry(k: EntityKind) -> bool {
    #[cfg(has_rifle_infantry)]
    return rts_rules::is_rifle_infantry(k);
    #[cfg(not(has_rifle_infantry))]
    return false;
}

pub fn entrenchment_eligible(k: EntityKind) -> bool {
    #[cfg(has_entrenchment)]
    return balance::is_entrenchment_eligible_infantry(k);
    #[cfg(not(has_entrenchment))]
    return false;
}

pub fn entrenched_damage(k: EntityKind, dmg: u32) -> Option<u32> {
    #[cfg(all(has_entrenchment, has_entrenchment_damage))]
    return entrenchment_eligible(k)
        .then(|| combat::direct_damage_after_entrenchment(k, dmg, true));
    #[cfg(not(all(has_entrenchment, has_entrenchment_damage)))]
    return None;
}

pub fn slowed_by_wire(k: EntityKind) -> bool {
    #[cfg(has_barbed_wire)]
    return rts_rules::barbed_wire_affects(k);
    #[cfg(not(has_barbed_wire))]
    return false;
}

pub fn crushes_wire(k: EntityKind) -> bool {
    #[cfg(has_crushes_wire)]
    return rts_rules::crushes_barbed_wire(k);
    #[cfg(not(has_crushes_wire))]
    return false;
}

pub fn manual_emplacement(k: EntityKind) -> bool {
    #[cfg(has_manual_emplacement)]
    return rts_rules::supports_manual_emplacement(k);
    #[cfg(not(has_manual_emplacement))]
    return false;
}

pub fn setup_ticks(k: EntityKind) -> (Value, Value) {
    #[cfg(has_support_setup)]
    return (
        json!(balance::support_weapon_setup_ticks(k)),
        json!(balance::support_weapon_teardown_ticks(k)),
    );
    #[cfg(not(has_support_setup))]
    return (Value::Null, Value::Null);
}

pub fn disciple(k: EntityKind) -> bool {
    #[cfg(has_disciples)]
    return k.is_disciple();
    #[cfg(not(has_disciples))]
    return false;
}

pub fn blocks_sight(k: EntityKind) -> bool {
    #[cfg(has_blocks_sight)]
    return rts_rules::blocks_line_of_sight(k);
    #[cfg(not(has_blocks_sight))]
    return false;
}

pub fn extracts(k: EntityKind) -> Value {
    #[cfg(has_extractor_kind)]
    return json!(k.extracted_resource_kind().map(id));
    #[cfg(not(has_extractor_kind))]
    return Value::Null;
}

pub fn infantry_policy(w: &WeaponProfile) -> Value {
    #[cfg(has_infantry_policy)]
    return json!(format!("{:?}", w.infantry_target_policy));
    #[cfg(not(has_infantry_policy))]
    return json!("None");
}

pub fn triggers_armor_reaction(w: &WeaponProfile) -> bool {
    #[cfg(has_armor_reaction)]
    return combat::weapon_triggers_tank_armor_reaction(w);
    #[cfg(not(has_armor_reaction))]
    return false;
}

pub fn carrier_cost(k: AbilityKind, carrier: EntityKind) -> Option<(u32, u32)> {
    #[cfg(has_carrier_cost)]
    return {
        let c = faction::ability_cost_for_carrier(k, carrier);
        Some((c.steel, c.oil))
    };
    #[cfg(not(has_carrier_cost))]
    return None;
}

/// The carrier's ability range as JSON (null when it has none); None if this version can't tell.
pub fn carrier_range(k: AbilityKind, carrier: EntityKind) -> Option<Value> {
    #[cfg(has_carrier_range)]
    return Some(crate::snapshot::vo(faction::ability_range_for_carrier(
        k, carrier,
    )));
    #[cfg(not(has_carrier_range))]
    return None;
}

pub fn charges(a: &AbilityCatalogEntry) -> (Value, Value) {
    #[cfg(has_charges)]
    return (json!(a.charges), json!(a.charge_recharge_ticks));
    #[cfg(not(has_charges))]
    return (Value::Null, Value::Null);
}

pub fn queue_policy(a: &AbilityCatalogEntry) -> Value {
    #[cfg(has_queue_policy)]
    return json!(a.queue_policy.stable_id());
    #[cfg(not(has_queue_policy))]
    return Value::Null;
}
