//! The live rules snapshot: every number the wiki shows, read from the compiled `rts_rules` (and,
//! for research, the simulation's own research table) or computed with the game's own combat
//! functions. Player-facing names and tooltips come from the client rules mirror. Where each value
//! is set and used comes from the build-time code index (build.rs).

use crate::{client_text, compat};
use rts_rules::combat::{self, ArmorFacing, FacingDamagePolicy, WeaponKind, WeaponProfile};
use rts_rules::defs::{self, TechRequirement};
use rts_rules::faction::{self, AbilityKind, UpgradeKind};
use rts_rules::terrain::TerrainKind;
use rts_rules::{balance, EntityKind};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

/// Built by build.rs: { definitions, usage, sim } (see there).
const CODE_INDEX: &str = include_str!(concat!(env!("OUT_DIR"), "/code_index.json"));

pub struct ConstEntry {
    pub name: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub ty: &'static str,
    pub value: Val,
    pub doc: &'static str,
}

pub enum Val {
    Num(f64),
    Bool(bool),
    Str(String),
}

impl Val {
    pub fn num_f32(v: f32) -> Val {
        // shortest f32 text keeps 0.1 as 0.1 instead of 0.10000000149
        Val::Num(format!("{v}").parse().unwrap_or(v as f64))
    }
    fn json(&self) -> Value {
        match self {
            Val::Num(n) => num(*n),
            Val::Bool(b) => json!(b),
            Val::Str(s) => json!(s),
        }
    }
}

pub(crate) fn num(n: f64) -> Value {
    if n.is_finite() && (n - n.round()).abs() < 1e-9 && n.abs() < 9e15 {
        json!(n.round() as i64)
    } else {
        json!(n)
    }
}

pub(crate) fn f(v: f32) -> Value {
    num(format!("{v}").parse().unwrap_or(v as f64))
}

/// Any numeric field to JSON, whatever its Rust type.
pub trait Num: Copy {
    fn val(self) -> Value;
}
macro_rules! num_int {
    ($($t:ty),*) => { $(impl Num for $t { fn val(self) -> Value { json!(self) } })* };
}
num_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64);
impl Num for f32 {
    fn val(self) -> Value {
        f(self)
    }
}
impl Num for f64 {
    fn val(self) -> Value {
        num(self)
    }
}
pub fn v<T: Num>(x: T) -> Value {
    x.val()
}
pub fn vo<T: Num>(x: Option<T>) -> Value {
    x.map(Num::val).unwrap_or(Value::Null)
}

/// The snapshot for the running build. `game` is filled in by the caller.
pub fn build(client_rules_js: &str, game: Value) -> Value {
    let consts = constants();
    let mut numeric: HashMap<String, f64> = consts
        .iter()
        .filter_map(|c| match c.value {
            Val::Num(n) => Some((c.name.to_string(), n)),
            _ => None,
        })
        .collect();
    let index: Value = serde_json::from_str(CODE_INDEX).unwrap_or(Value::Null);
    let sim = sim_constants(&index, &numeric);
    for c in &sim {
        if let (Some(name), Some(v)) = (c["name"].as_str(), c["value"].as_f64()) {
            numeric.insert(name.to_string(), v);
        }
    }
    let text = client_text::read(client_rules_js, &numeric);

    let mut snapshot = Map::new();
    snapshot.insert("schema".into(), json!(1));
    snapshot.insert("game".into(), game);
    snapshot.insert("commits".into(), json!([]));
    snapshot.insert(
        "timing".into(),
        json!({ "tickHz": balance::TICK_HZ, "tileSize": balance::TILE_SIZE }),
    );
    snapshot.insert("factions".into(), factions());
    snapshot.insert("units".into(), units(&text, &numeric));
    snapshot.insert("buildings".into(), buildings(&text));
    snapshot.insert("nodes".into(), nodes(&text));
    snapshot.insert("weapons".into(), weapons(&numeric));
    snapshot.insert("upgrades".into(), upgrades(&text));
    snapshot.insert("abilities".into(), abilities());
    snapshot.insert("facing".into(), facing_table());
    snapshot.insert("sources".into(), sources(&index));
    snapshot.insert(
        "usage".into(),
        index.get("usage").cloned().unwrap_or(Value::Null),
    );
    snapshot.insert(
        "constants".into(),
        Value::Array(
            consts
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name, "file": c.file, "line": c.line,
                        "path": format!("server/crates/rules/src/{}", c.file),
                        "type": c.ty, "value": c.value.json(), "doc": c.doc,
                    })
                })
                .chain(sim)
                .collect(),
        ),
    );
    Value::Object(snapshot)
}

/// The private simulation constants indexed by build.rs, evaluated against the rules values.
fn sim_constants(index: &Value, rules: &HashMap<String, f64>) -> Vec<Value> {
    let mut out = Vec::new();
    for c in index
        .get("sim")
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
    {
        let Some(value) = c["expr"].as_str().and_then(|e| client_text::eval(e, rules)) else {
            continue;
        };
        let mut c = c.clone();
        c["value"] = num(value);
        c["module"] = json!("sim");
        if let Some(m) = c.as_object_mut() {
            m.remove("expr");
        }
        out.push(c);
    }
    out
}

/// Code definitions keyed by page ("unit:tank") instead of enum variant ("EntityKind:Tank").
fn sources(index: &Value) -> Value {
    let ids = variant_ids();
    let mut out = Map::new();
    if let Some(Value::Object(defs)) = index.get("definitions") {
        for (key, fields) in defs {
            let Some((en, var)) = key.split_once(':') else {
                continue;
            };
            if let Some((kind, id)) = ids.get(&(en.to_string(), var.to_string())) {
                out.insert(format!("{kind}:{id}"), fields.clone());
            }
        }
    }
    Value::Object(out)
}

/// (enum name, variant name) -> (page kind, stable id), for matching source definitions.
fn variant_ids() -> HashMap<(String, String), (String, String)> {
    let mut m = HashMap::new();
    for k in EntityKind::ALL {
        let kind = if defs::unit_def(k).is_some() {
            "unit"
        } else if defs::building_def(k).is_some() {
            "building"
        } else {
            "node"
        };
        m.insert(
            ("EntityKind".to_string(), format!("{k:?}")),
            (kind.to_string(), k.stable_id().to_string()),
        );
    }
    for w in WeaponKind::ALL {
        m.insert(
            ("WeaponKind".to_string(), format!("{w:?}")),
            ("weapon".to_string(), w.stable_id().to_string()),
        );
    }
    for a in AbilityKind::ALL {
        m.insert(
            ("AbilityKind".to_string(), format!("{a:?}")),
            ("ability".to_string(), a.stable_id().to_string()),
        );
    }
    for u in UpgradeKind::ALL {
        m.insert(
            ("UpgradeKind".to_string(), format!("{u:?}")),
            ("research".to_string(), u.stable_id().to_string()),
        );
    }
    m
}

pub(crate) fn id(kind: EntityKind) -> &'static str {
    kind.stable_id()
}

fn ids(kinds: &[EntityKind]) -> Value {
    Value::Array(kinds.iter().map(|k| json!(id(*k))).collect())
}

fn title_case(stable: &str) -> String {
    stable
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_fields(
    kind: EntityKind,
    text: &client_text::ClientText,
) -> (String, Value, Value, Value, Value) {
    let e = text.kinds.get(id(kind)).cloned().unwrap_or_default();
    (
        e.label.unwrap_or_else(|| title_case(id(kind))),
        json!(e.icon),
        json!(e.description),
        json!(e.upgrade_requires),
        json!(e.requires_text),
    )
}

fn factions() -> Value {
    Value::Array(
        faction::CATALOGS
            .iter()
            .filter(|c| !c.units.is_empty() && !c.id.contains("fixture"))
            .map(|c| {
                let starting: Vec<Value> = c
                    .loadout
                    .starting_entities
                    .iter()
                    .map(|g| json!({ "kind": id(g.kind), "count": g.count, "completed": g.completed, "formation": format!("{:?}", g.formation) }))
                    .collect();
                let trainables: Vec<Value> = c
                    .buildings
                    .iter()
                    .filter_map(|b| {
                        let u = c.trainable_units(*b);
                        (!u.is_empty()).then(|| json!({ "building": id(*b), "units": ids(&u) }))
                    })
                    .collect();
                json!({
                    "id": c.id,
                    "loadout": {
                        "id": c.loadout.id,
                        "steel": c.loadout.initial_steel,
                        "oil": c.loadout.initial_oil,
                        "starting": starting,
                        "openingUpgrades": c.loadout.opening_upgrades,
                    },
                    "units": ids(c.units),
                    "buildings": ids(c.buildings),
                    "buildables": ids(c.buildables),
                    "trainables": trainables,
                    "upgrades": c.upgrades.iter().map(|u| json!({ "id": u.kind.stable_id(), "researchedAt": id(u.researched_at) })).collect::<Vec<_>>(),
                    "abilities": c.abilities.iter().map(|a| json!(a.kind.stable_id())).collect::<Vec<_>>(),
                    "builders": ids(c.builders),
                    "gatherers": ids(c.gatherers),
                    "productionAnchors": ids(c.production_anchors),
                })
            })
            .collect(),
    )
}

fn requirement(req: TechRequirement) -> Value {
    match req {
        TechRequirement::All(k) => json!({ "mode": "all", "kinds": ids(k) }),
        TechRequirement::Any(k) => json!({ "mode": "any", "kinds": ids(k) }),
    }
}

/// Weapons the simulation fires as machine-gun bursts (mirrors `is_machine_gun` in
/// sim/src/game/services/combat/machine_gun.rs): MG_BURST_BULLETS rays, the first aimed at the
/// target, each hit worth MG_RAY_DAMAGE_MULTIPLIER of the profile damage, summed before armor.
/// The burst constants are read by name at run time, so rules versions without them still export.
/// (Matched by stable id, so versions without one of these weapons still compile.)
fn is_burst(w: WeaponKind) -> bool {
    matches!(
        w.stable_id(),
        "machine_gunner_mg" | "scout_car_mg" | "tank_coax"
    )
}

struct Burst {
    rays: u32,
    multiplier: f32,
}

fn burst_rules(consts: &HashMap<String, f64>) -> Option<Burst> {
    Some(Burst {
        rays: *consts.get("MG_BURST_BULLETS")? as u32,
        multiplier: *consts.get("MG_RAY_DAMAGE_MULTIPLIER")? as f32,
    })
}

fn burst_damage(w: &WeaponProfile, rays: u32, b: &Burst) -> u32 {
    (w.dmg as f32 * rays as f32 * b.multiplier).round() as u32
}

/// The attack a unit really makes with its default weapon. Cultivators take damage, range and
/// cooldown from their own stats (`combat::attack_profile`) and armor penetration from their Dao
/// rank (`combat::ranked_primary_weapon_profile`), so one shared sword hits differently per unit.
fn effective_weapon(k: EntityKind) -> Option<WeaponProfile> {
    let mut p = combat::ranked_primary_weapon_profile(k, false)?;
    let a = combat::attack_profile(k);
    p.dmg = a.dmg;
    p.range_tiles = a.range_tiles;
    p.cooldown = a.cooldown;
    Some(p)
}

fn same_attack(a: &WeaponProfile, b: &WeaponProfile) -> bool {
    a.dmg == b.dmg
        && a.range_tiles == b.range_tiles
        && a.cooldown == b.cooldown
        && a.armor_penetration == b.armor_penetration
}

/// The weapons-list entry for a unit's default attack: the profile's own id, or
/// `<profile>@<unit>` when the unit's attack differs from the shared profile.
fn weapon_entry_id(k: EntityKind) -> Option<String> {
    let kind = combat::default_weapon_kind(k)?;
    let base = combat::weapon_profile(kind)?;
    let eff = effective_weapon(k)?;
    Some(if same_attack(base, &eff) {
        kind.stable_id().to_string()
    } else {
        format!("{}@{}", kind.stable_id(), id(k))
    })
}

/// What an Adept gains on reaching Master rank (sim entity/dao.rs `advance_dao_master`,
/// `movement_speed`; rules `ranked_primary_damage`, `ranked_primary_weapon_profile`).
fn master_rank(k: EntityKind, s: &balance::UnitStats) -> Option<Value> {
    if !k.is_disciple() || k.is_novice() {
        return None;
    }
    let attack = combat::attack_profile(k);
    Some(json!({
        "insight": balance::DISCIPLE_EVOLVED_INSIGHT_CAP,
        "hp": (s.hp as f32 * balance::DAO_MASTER_HP_MULTIPLIER).round() as u32,
        "dmg": combat::ranked_primary_damage(k, true, attack.dmg),
        "armorPenetration": combat::ranked_primary_weapon_profile(k, true).map(|p| v(p.armor_penetration)),
        "speedPxPerTick": v(s.speed * balance::DAO_MASTER_SPEED_MULTIPLIER),
    }))
}

/// Extra weapons a unit carries besides its default weapon.
fn secondary_weapons(kind: EntityKind) -> Vec<WeaponKind> {
    let wanted = match kind.stable_id() {
        "tank" => "tank_coax",
        "panzerfaust" => "panzerfaust_loaded_shot",
        _ => return vec![],
    };
    WeaponKind::ALL
        .iter()
        .copied()
        .filter(|w| w.stable_id() == wanted)
        .collect()
}

fn units(text: &client_text::ClientText, consts: &HashMap<String, f64>) -> Value {
    let deployed_range = consts.get("ANTI_TANK_GUN_DEPLOYED_RANGE_TILES").copied();
    Value::Array(
        defs::UNITS
            .iter()
            .map(|d| {
                let k = d.kind;
                let s = d.stats;
                let (name, icon, desc, upg, req_text) = text_fields(k, text);
                let attack = combat::attack_profile(k);
                json!({
                    "id": id(k),
                    "name": name,
                    "icon": icon,
                    "description": desc,
                    "hp": s.hp,
                    "dmg": s.dmg,
                    "rangeTiles": v(s.range_tiles),
                    "cooldownTicks": s.cooldown,
                    "speedPxPerTick": v(s.speed),
                    "sightTiles": s.sight_tiles,
                    "steel": s.cost_steel,
                    "oil": s.cost_oil,
                    "supply": s.supply,
                    "buildTicks": s.build_ticks,
                    "radiusPx": v(s.radius),
                    "armor": format!("{:?}", d.armor_class),
                    "weaponClass": format!("{:?}", d.weapon),
                    "trainedAt": d.trained_at.map(id),
                    "trainRequirement": requirement(d.train_requirement),
                    "upgradeRequires": rts_sim::game::upgrade::required_for_unit(k).map(|u| u.stable_id().to_string()).or_else(|| upg.as_str().map(str::to_string)),
                    "requiresText": req_text,
                    "attack": { "dmg": attack.dmg, "rangeTiles": v(attack.range_tiles), "cooldownTicks": attack.cooldown },
                    "engagementRangeTiles": compat::engagement(k),
                    "weapon": weapon_entry_id(k),
                    "master": master_rank(k, &s),
                    "secondaryWeapons": secondary_weapons(k).iter().map(|w| w.stable_id()).collect::<Vec<_>>(),
                    "movementBody": compat::movement_body(k),
                    "threatRole": compat::threat_role(k),
                    "targetPriority": compat::target_priority(k),
                    "firesWhileMoving": compat::fires_while_moving(k),
                    "rifleInfantry": compat::rifle_infantry(k),
                    "entrenchmentEligible": compat::entrenchment_eligible(k),
                    "slowedByBarbedWire": compat::slowed_by_wire(k),
                    "crushesBarbedWire": compat::crushes_wire(k),
                    "manualEmplacement": compat::manual_emplacement(k),
                    "setupTicks": compat::setup_ticks(k).0,
                    "teardownTicks": compat::setup_ticks(k).1,
                    "disciple": compat::disciple(k),
                    // sim/src/game/services/combat/weapons.rs: a set-up Anti-Tank Gun fires to
                    // ANTI_TANK_GUN_DEPLOYED_RANGE_TILES; packed up it deals no damage
                    "deployedRangeTiles": (k == EntityKind::AntiTankGun).then_some(deployed_range).flatten().map(num),
                    "firesOnlyWhenSetUp": k == EntityKind::AntiTankGun,
                    // sim combat/projection.rs: a still Tank's range ramps up; moving resets it
                    "stationaryRange": (k == EntityKind::Tank)
                        .then(|| Some(json!({
                            "maxTiles": num(*consts.get("TANK_STATIONARY_RANGE_MAX_TILES")?),
                            "rampTicks": num(*consts.get("TANK_STATIONARY_RANGE_RAMP_TICKS")?),
                        })))
                        .flatten(),
                })
            })
            .collect(),
    )
}

fn buildings(text: &client_text::ClientText) -> Value {
    Value::Array(
        defs::BUILDINGS
            .iter()
            .map(|d| {
                let k = d.kind;
                let s = d.stats;
                let (name, icon, desc, upg, req_text) = text_fields(k, text);
                let researches: Vec<Value> = faction::UpgradeKind::ALL
                    .iter()
                    .filter(|u| {
                        faction::CATALOGS
                            .iter()
                            .any(|c| !c.id.contains("fixture") && c.allows_research(**u, k))
                    })
                    .map(|u| json!(u.stable_id()))
                    .collect();
                json!({
                    "id": id(k),
                    "name": name,
                    "icon": icon,
                    "description": desc,
                    "upgradeRequires": upg,
                    "requiresText": req_text,
                    "hp": s.hp,
                    "sightTiles": s.sight_tiles,
                    "steel": s.cost_steel,
                    "oil": s.cost_oil,
                    "footW": s.foot_w,
                    "footH": s.foot_h,
                    "buildTicks": s.build_ticks,
                    "dmg": s.dmg,
                    "rangeTiles": v(s.range_tiles),
                    "cooldownTicks": s.cooldown,
                    "armor": format!("{:?}", d.armor_class),
                    "weaponClass": format!("{:?}", d.weapon),
                    "trains": ids(d.trains),
                    "requires": ids(d.build_requires),
                    "researches": researches,
                    "blocksSight": compat::blocks_sight(k),
                    "extracts": compat::extracts(k),
                })
            })
            .collect(),
    )
}

fn nodes(text: &client_text::ClientText) -> Value {
    Value::Array(
        defs::NODES
            .iter()
            .map(|n| json!({ "id": id(n.kind), "name": text_fields(n.kind, text).0, "amount": n.amount }))
            .collect(),
    )
}

/// Every unit and building that can be shot at, for the damage tables.
fn targets() -> Vec<EntityKind> {
    defs::UNITS
        .iter()
        .map(|d| d.kind)
        .chain(defs::BUILDINGS.iter().map(|d| d.kind))
        .collect()
}

fn facing_damage(w: &WeaponProfile, victim: EntityKind, facing: ArmorFacing) -> u32 {
    // victim at the origin facing +x; the attacker's position picks the armor arc
    let attacker = match facing {
        ArmorFacing::Front => (100.0, 0.0),
        ArmorFacing::Side => (0.0, 100.0),
        ArmorFacing::Rear => (-100.0, 0.0),
    };
    combat::effective_damage_with_facing_for_weapon(
        w,
        victim,
        w.dmg,
        Some(TerrainKind::Open),
        Some(0.0),
        (0.0, 0.0),
        attacker,
    )
}

/// One entry per attack as units really make it: a profile's own entry for the units that fire it
/// unchanged, plus `<profile>@<unit>` for each unit whose attack differs (see `effective_weapon`).
fn weapons(consts: &HashMap<String, f64>) -> Value {
    let all_kinds: Vec<EntityKind> = EntityKind::ALL.to_vec();
    let burst = burst_rules(consts);
    let mut out = Vec::new();
    for w in combat::WEAPON_PROFILES {
        let base_id = w.id.stable_id().to_string();
        let mut used_by: Vec<Value> = all_kinds
            .iter()
            .filter(|k| weapon_entry_id(**k).as_deref() == Some(base_id.as_str()))
            .map(|k| json!(id(*k)))
            .collect();
        used_by.extend(
            all_kinds
                .iter()
                .filter(|k| secondary_weapons(**k).contains(&w.id))
                .map(|k| json!(id(*k))),
        );
        out.push(weapon_entry(
            w,
            &base_id,
            None,
            used_by,
            burst.as_ref(),
            consts,
        ));
        for k in all_kinds
            .iter()
            .filter(|k| combat::default_weapon_kind(**k) == Some(w.id))
        {
            let (Some(entry), Some(p)) = (weapon_entry_id(*k), effective_weapon(*k)) else {
                continue;
            };
            if entry != base_id {
                out.push(weapon_entry(
                    &p,
                    &entry,
                    Some(*k),
                    vec![json!(id(*k))],
                    burst.as_ref(),
                    consts,
                ));
            }
        }
    }
    Value::Array(out)
}

fn weapon_entry(
    w: &WeaponProfile,
    entry_id: &str,
    variant_of: Option<EntityKind>,
    used_by: Vec<Value>,
    burst: Option<&Burst>,
    consts: &HashMap<String, f64>,
) -> Value {
    let burst = burst.filter(|_| is_burst(w.id));
    // Mortar shells don't use the profile's damage: each one deals MORTAR_INNER_DAMAGE where it
    // lands, falling to MORTAR_OUTER_DAMAGE at MORTAR_OUTER_RADIUS_TILES (balance
    // mortar_damage_at_distance_squared). The tables count a direct hit; entrenchment cuts area
    // damage, not direct damage.
    let area = (w.id.stable_id() == "mortar_team_mortar")
        .then(|| {
            Some((
                *consts.get("MORTAR_INNER_DAMAGE")? as u32,
                *consts.get("MORTAR_OUTER_DAMAGE")?,
                *consts.get("MORTAR_OUTER_RADIUS_TILES")?,
            ))
        })
        .flatten();
    let profile = WeaponProfile {
        dmg: area.map_or(w.dmg, |(inner, _, _)| inner),
        ..*w
    };
    let w = &profile;
    let vs: Vec<Value> = targets()
        .into_iter()
        .map(|t| {
            let open = combat::effective_damage_for_weapon(w, t, w.dmg, Some(TerrainKind::Open));
            let mut row = json!({ "target": id(t), "dmg": open });
            if combat::facing_damage_multiplier_for_weapon(w, t, ArmorFacing::Rear) != 1.0 {
                row["front"] = json!(facing_damage(w, t, ArmorFacing::Front));
                row["side"] = json!(facing_damage(w, t, ArmorFacing::Side));
                row["rear"] = json!(facing_damage(w, t, ArmorFacing::Rear));
            }
            if let Some(b) = burst {
                let one = combat::effective_damage_for_weapon(
                    w,
                    t,
                    burst_damage(w, 1, b),
                    Some(TerrainKind::Open),
                );
                let all = combat::effective_damage_for_weapon(
                    w,
                    t,
                    burst_damage(w, b.rays, b),
                    Some(TerrainKind::Open),
                );
                row["burstMin"] = json!(one);
                row["burstMax"] = json!(all);
                if let Some(e) = compat::entrenched_damage(t, all) {
                    row["entrenched"] = json!(e);
                }
            } else if area.is_some() {
                if compat::entrenchment_eligible(t) {
                    row["entrenched"] =
                        json!(combat::area_damage_after_entrenchment(t, open, true));
                }
            } else if let Some(e) = compat::entrenched_damage(t, open) {
                row["entrenched"] = json!(e);
            }
            row
        })
        .collect();
    json!({
        "id": entry_id,
        // a unit's own version of a shared profile: which profile, and whose attack it is
        "base": variant_of.map(|_| w.id.stable_id()),
        "variantOf": variant_of.map(id),
        "rangeTiles": v(w.range_tiles),
        "dmg": w.dmg,
        "cooldownTicks": w.cooldown,
        "weaponClass": format!("{:?}", w.weapon_class),
        "armorPenetration": v(w.armor_penetration),
        "facingArmor": w.facing_damage_policy == FacingDamagePolicy::TankArmorFacing,
        "infantryPolicy": compat::infantry_policy(w),
        "triggersTankArmorReaction": compat::triggers_armor_reaction(w),
        "burst": burst.map(|b| json!({
            "rays": b.rays,
            "rayMultiplier": f(b.multiplier),
            "min": burst_damage(w, 1, b),
            "max": burst_damage(w, b.rays, b),
        })),
        "area": area.map(|(inner, outer, radius)| json!({ "max": inner, "min": num(outer), "radiusTiles": num(radius) })),
        // sim combat/weapons.rs: a set-up Anti-Tank Gun fires farther; a still Tank's range ramps up
        "deployedRangeTiles": (w.id.stable_id() == "anti_tank_gun").then(|| consts.get("ANTI_TANK_GUN_DEPLOYED_RANGE_TILES").copied().map(num)).flatten(),
        "stationaryRangeTiles": (w.id.stable_id() == "tank_cannon").then(|| consts.get("TANK_STATIONARY_RANGE_MAX_TILES").copied().map(num)).flatten(),
        "usedBy": used_by,
        "vs": vs,
    })
}

fn facing_table() -> Value {
    // Read the arc multipliers back through the public function, using an armor-facing weapon.
    let profile = combat::WEAPON_PROFILES
        .iter()
        .find(|w| w.facing_damage_policy == FacingDamagePolicy::TankArmorFacing);
    match profile {
        Some(w) => json!({
            "front": v(combat::facing_damage_multiplier_for_weapon(w, EntityKind::Tank, ArmorFacing::Front)),
            "side": v(combat::facing_damage_multiplier_for_weapon(w, EntityKind::Tank, ArmorFacing::Side)),
            "rear": v(combat::facing_damage_multiplier_for_weapon(w, EntityKind::Tank, ArmorFacing::Rear)),
        }),
        None => json!(null),
    }
}

/// Research, from the simulation's own research table (`rts_sim::game::upgrade::definition`):
/// the cost, time and prerequisite the game actually charges and checks.
fn upgrades(text: &client_text::ClientText) -> Value {
    Value::Array(
        UpgradeKind::ALL
            .iter()
            .filter(|u| {
                faction::CATALOGS.iter().any(|c| {
                    !c.id.contains("fixture") && c.upgrades.iter().any(|entry| entry.kind == **u)
                })
            })
            .map(|u| {
                let sid = u.stable_id();
                let def = rts_sim::game::upgrade::definition(*u);
                let e = text.upgrades.get(sid).cloned().unwrap_or_default();
                let factions: Vec<Value> = faction::CATALOGS
                    .iter()
                    .filter(|c| {
                        c.upgrades.iter().any(|x| x.kind == *u) && !c.id.contains("fixture")
                    })
                    .map(|c| json!(c.id))
                    .collect();
                json!({
                    "id": sid,
                    "name": e.label.unwrap_or_else(|| title_case(sid)),
                    "icon": e.icon,
                    "description": e.description,
                    "researchedAt": id(def.researched_at),
                    "steel": def.cost_steel,
                    "oil": def.cost_oil,
                    "researchTicks": def.research_ticks,
                    "requiresUpgrade": def.requires_upgrade.map(UpgradeKind::stable_id),
                    "factions": factions,
                })
            })
            .collect(),
    )
}

fn abilities() -> Value {
    Value::Array(
        AbilityKind::ALL
            .iter()
            .map(|k| {
                let factions: Vec<&faction::FactionCatalog> = faction::CATALOGS
                    .iter()
                    .filter(|c| c.ability(*k).is_some() && !c.id.contains("fixture"))
                    .collect();
                let a = factions
                    .first()
                    .and_then(|c| c.ability(*k))
                    .unwrap_or_else(|| faction::ability_definition(*k));
                let carrier_costs: Map<String, Value> = a
                    .carriers
                    .iter()
                    .filter_map(|c| {
                        let (steel, oil) = compat::carrier_cost(*k, *c)?;
                        (steel != a.cost.steel || oil != a.cost.oil)
                            .then(|| (id(*c).to_string(), json!({ "steel": steel, "oil": oil })))
                    })
                    .collect();
                let carrier_ranges: Map<String, Value> = a
                    .carriers
                    .iter()
                    .filter_map(|c| {
                        let r = compat::carrier_range(*k, *c)?;
                        (r != vo(a.range_tiles)).then(|| (id(*c).to_string(), r))
                    })
                    .collect();
                let (charges, recharge) = compat::charges(&a);
                json!({
                    "id": k.stable_id(),
                    "label": a.label,
                    "icon": a.icon,
                    "hotkey": a.hotkey,
                    "title": a.title,
                    "carriers": ids(a.carriers),
                    "carrierCosts": carrier_costs,
                    "carrierRanges": carrier_ranges,
                    "targetMode": a.target_mode.stable_id(),
                    "rangeTiles": vo(a.range_tiles),
                    "minRangeTiles": a.min_range_tiles,
                    "cooldownTicks": a.cooldown_ticks,
                    // sim services/ability_orders.rs: Throw Sword's cooldown doubles below Master rank
                    "adeptCooldownTicks": (*k == AbilityKind::ThrowSword).then(|| a.cooldown_ticks.saturating_mul(2)),
                    "charges": charges,
                    "chargeRechargeTicks": recharge,
                    "steel": a.cost.steel,
                    "oil": a.cost.oil,
                    "techRequirement": a.tech_requirement.map(id),
                    "upgradeRequirement": a.upgrade_requirement.map(UpgradeKind::stable_id),
                    "queuePolicy": compat::queue_policy(&a),
                    "autocast": a.autocast,
                    "commandCard": a.command_card,
                    "factions": factions.iter().map(|c| json!(c.id)).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The weapon row a unit page shows must be the attack the simulation makes: Cultivators'
    /// own damage, range and cooldown, and their Dao rank's armor penetration.
    #[test]
    fn unit_weapon_entries_match_the_attack_the_sim_uses() {
        let weapons = weapons(&HashMap::new());
        let weapons = weapons.as_array().expect("weapons list");
        for d in defs::UNITS {
            let k = d.kind;
            let Some(entry) = weapon_entry_id(k) else {
                continue;
            };
            let w = weapons
                .iter()
                .find(|w| w["id"] == entry.as_str())
                .unwrap_or_else(|| panic!("no weapon entry {entry} for {}", id(k)));
            let attack = combat::attack_profile(k);
            let ranked = combat::ranked_primary_weapon_profile(k, false).expect("profile");
            assert_eq!(w["dmg"], json!(attack.dmg), "{} damage", id(k));
            assert_eq!(w["rangeTiles"], v(attack.range_tiles), "{} range", id(k));
            assert_eq!(
                w["cooldownTicks"],
                json!(attack.cooldown),
                "{} cooldown",
                id(k)
            );
            assert_eq!(
                w["armorPenetration"],
                v(ranked.armor_penetration),
                "{} armor penetration",
                id(k)
            );
            assert!(
                w["usedBy"]
                    .as_array()
                    .is_some_and(|u| u.contains(&json!(id(k)))),
                "{} not listed as a user of {entry}",
                id(k)
            );
        }
    }
}
