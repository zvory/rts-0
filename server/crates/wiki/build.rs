//! Indexes the rules and simulation source at build time, so every number on the wiki can name
//! the variable and the line that sets it:
//!
//! * `constants.rs` — `constants()`: every public scalar constant of `rts-rules`, read through its
//!   real public path so the compiled value (not the source text) is used. New constants appear
//!   without code changes.
//! * `code_index.json` — where each unit/building/weapon/ability/research field is set
//!   (`definitions`), where each constant is read (`usage`), and a few player-facing private
//!   simulation constants (`sim`, evaluated at run time against the rules values).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "build/code_index.rs"]
mod code_index;

const SCALARS: &[&str] = &[
    "u8",
    "u16",
    "u32",
    "u64",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "isize",
    "f32",
    "f64",
    "bool",
    "&str",
    "&'static str",
];

/// Player-facing numbers that live (privately) in the simulation crate.
const SIM_DIR: &str = "server/crates/sim/src/game/services/combat";
const SIM_CONSTANTS: &[&str] = &[
    "TANK_STATIONARY_RANGE_MAX_TILES",
    "TANK_STATIONARY_RANGE_RAMP_TICKS",
    "TANK_TURRET_TURN_RATE_RAD_PER_TICK",
    "ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK",
    "ARTILLERY_SETUP_TURN_RATE_RAD_PER_TICK",
];

/// How a [`CodeValue`] is read once its anchors are found.
#[derive(Clone, Copy)]
enum Read {
    /// A private named constant: the expression after `=` on the anchor's line.
    Const,
    /// A number written inline: the first numeric literal after the last anchor.
    Literal,
    /// The value comes from calling the rules function in `snapshot.rs` (`called_value`); only the
    /// line is found here.
    Called,
}

/// Player-facing numbers the code sets without a public named constant. Each gets a wiki key, so
/// notes can write `{{KEY}}` and the page's constants table links the line. `anchors` are found in
/// order in `path`. A `Const` or `Literal` whose anchors are gone is left out, so the notes that use
/// it fail the broken-reference test instead of showing a stale number.
struct CodeValue {
    key: &'static str,
    /// What "Show code" names; empty means `file:line`.
    label: &'static str,
    path: &'static str,
    anchors: &'static [&'static str],
    read: Read,
    doc: &'static str,
}

const INSIGHT_FN: &str = "pub fn disciple_insight_ratio(";
const ARMOR_RS: &str = "server/crates/rules/src/combat/armor.rs";
const ARMOR_FN: &str = "pub fn effective_damage_for_armor_penetration_with_bonus(";
const CODE_VALUES: &[CodeValue] = &[
    CodeValue {
        key: "TOURNAMENT_CAPACITY",
        label: "tournament::CAPACITY",
        path: "server/crates/sim/src/game/services/tournament.rs",
        anchors: &["const CAPACITY:"],
        read: Read::Const,
        doc: "Novices that enter one tournament; it starts when the Grounds is full.",
    },
    CodeValue {
        key: "TOURNAMENT_DURATION_TICKS",
        label: "tournament::DURATION",
        path: "server/crates/sim/src/game/services/tournament.rs",
        anchors: &["const DURATION:"],
        read: Read::Const,
        doc: "How long a full tournament runs before its Adept comes out.",
    },
    CodeValue {
        key: "BARBED_WIRE_SPEED_MULTIPLIER",
        label: "",
        path: "server/crates/sim/src/game/services/movement/speed.rs",
        anchors: &["barbed_wire_affects(kind) && on_wire"],
        read: Read::Literal,
        doc: "Speed multiplier for units Barbed Wire affects while they stand on a wire tile.",
    },
    CodeValue {
        key: "BARBED_WIRE_ENTRY_DAMAGE",
        label: "",
        path: "server/crates/sim/src/game/services/movement/barbed_wire.rs",
        anchors: &["if hurts", "apply_damage("],
        read: Read::Literal,
        doc: "Damage to a unit Barbed Wire affects each time it moves onto a wire tile.",
    },
    CodeValue {
        key: "DISCIPLE_INSIGHT_PER_HP_UNITS",
        label: "",
        path: "server/crates/rules/src/balance/upgrades.rs",
        anchors: &[INSIGHT_FN, "(4, 4)"],
        read: Read::Called,
        doc: "Insight a Cultivator earns per HP of damage to a unit.",
    },
    CodeValue {
        key: "DISCIPLE_INSIGHT_PER_HP_BUILDINGS",
        label: "",
        path: "server/crates/rules/src/balance/upgrades.rs",
        anchors: &[INSIGHT_FN, "(1, 4)"],
        read: Read::Called,
        doc: "Insight a Cultivator earns per HP of damage to a building.",
    },
    CodeValue {
        key: "DISCIPLE_INSIGHT_PER_HP_WIND_EXTRACTORS",
        label: "",
        path: "server/crates/rules/src/balance/upgrades.rs",
        anchors: &[INSIGHT_FN, "is_resource_extractor()"],
        read: Read::Called,
        doc: "Insight a Wind Adept earns per HP of damage to a Steel Mine or Pump Jack.",
    },
    CodeValue {
        key: "DISCIPLE_INSIGHT_PER_HP_WAR_TANKS",
        label: "",
        path: "server/crates/rules/src/balance/upgrades.rs",
        anchors: &[INSIGHT_FN, "(8, 4)"],
        read: Read::Called,
        doc: "Insight a War Adept earns per HP of damage to a Tank.",
    },
    CodeValue {
        key: "ARMOR_SMALL_DAMAGE_TAKEN",
        label: "",
        path: ARMOR_RS,
        anchors: &[ARMOR_FN, "_ => base_dmg"],
        read: Read::Called,
        doc: "Share of a hit's damage a Small target takes.",
    },
    CodeValue {
        key: "ARMOR_ARMORED_DAMAGE_TAKEN",
        label: "",
        path: ARMOR_RS,
        anchors: &[ARMOR_FN, "if penetration <= 0.0"],
        read: Read::Called,
        doc: "Share of a hit's damage an Armored target takes from a weapon with no armor penetration.",
    },
    CodeValue {
        key: "ARMOR_ARMORED_REDUCTION",
        label: "",
        path: ARMOR_RS,
        anchors: &["Some(ArmorClass::Armored) => damage_after_armor_reduction(base_dmg,"],
        read: Read::Literal,
        doc: "Armor of an Armored target: the share of damage it stops. Armor penetration cancels that share of it.",
    },
    CodeValue {
        key: "FRONT_ARC_RAD",
        label: "combat::FRONT_ARC_RAD",
        path: "server/crates/rules/src/combat.rs",
        anchors: &["const FRONT_ARC_RAD:"],
        read: Read::Const,
        doc: "A hit is on the front when the shooter is within this angle of the Tank's facing.",
    },
    CodeValue {
        key: "SIDE_ARC_RAD",
        label: "combat::SIDE_ARC_RAD",
        path: "server/crates/rules/src/combat.rs",
        anchors: &["const SIDE_ARC_RAD:"],
        read: Read::Const,
        doc: "Past the front arc and within this angle of the facing a hit is on the side; beyond it, the rear.",
    },
    CodeValue {
        key: "ELEVATION_LEVELS_PER_SIGHT_TILE",
        label: "",
        path: "server/crates/rules/src/terrain.rs",
        anchors: &["pub fn elevation_sight_bonus_tiles(", "u32::from(elevation) /"],
        read: Read::Literal,
        doc: "Elevation levels that give one extra tile of sight.",
    },
    CodeValue {
        key: "TANK_PLUS_RANGE_RAMP_RATE",
        label: "",
        path: "server/crates/sim/src/game/services/combat/weapons.rs",
        anchors: &["let increment = if e.tank_plus() {"],
        read: Read::Literal,
        doc: "With Tank Plus, the stationary range ramp counts this many ticks per tick.",
    },
    CodeValue {
        key: "THROW_SWORD_ADEPT_COOLDOWN_MULTIPLIER",
        label: "",
        path: "server/crates/sim/src/game/services/ability_orders.rs",
        anchors: &[
            "ability == AbilityKind::ThrowSword && !e.dao_master",
            "saturating_mul(",
        ],
        read: Read::Literal,
        doc: "Throw Sword's cooldown multiplier for a War Adept below Master rank.",
    },
    CodeValue {
        key: "WIND_DASH_MASTER_CHARGES",
        label: "",
        path: "server/crates/sim/src/game/entity/entity/ability_charges.rs",
        anchors: &[
            "if ability == AbilityKind::WindDash && self.dao_master {",
            "Some(",
        ],
        read: Read::Literal,
        doc: "Blink charges a Wind Master holds.",
    },
    CodeValue {
        key: "DOUBLE_XP_INSIGHT_MULTIPLIER",
        label: "",
        path: "server/crates/sim/src/game/entity/entity/dao.rs",
        anchors: &["shop_item_active(AbilityKind::DoubleXp) {"],
        read: Read::Literal,
        doc: "Insight multiplier while the 2X XP item is active.",
    },
];

struct Found {
    name: String,
    module: String,
    file: String,
    line: usize,
    ty: String,
    doc: String,
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo = manifest.join("../../..");
    let rules = repo.join("server/crates/rules/src");
    println!("cargo:rerun-if-changed=../rules/src");
    println!("cargo:rerun-if-changed=../sim/src");

    // Optional rules APIs (src/compat.rs): each is compiled in only when the rules source has it,
    // so a renamed helper blanks one wiki field instead of breaking the server build.
    let optional: &[(&str, &str, &str)] = &[
        (
            "has_engagement_range",
            "combat.rs",
            "pub fn engagement_range_tiles",
        ),
        (
            "has_carrier_range",
            "faction.rs",
            "pub fn ability_range_for_carrier",
        ),
        (
            "has_carrier_cost",
            "faction.rs",
            "pub fn ability_cost_for_carrier",
        ),
        ("has_tank_coax", "combat.rs", "TankCoax"),
        ("has_panzerfaust_shot", "combat.rs", "PanzerfaustLoadedShot"),
        ("has_burst_weapons", "combat.rs", "ScoutCarMg"),
        ("has_disciples", "kind.rs", "pub const fn is_disciple"),
        (
            "has_target_priority",
            "combat.rs",
            "pub fn default_target_priority_policy",
        ),
        ("has_threat_role", "combat.rs", "pub fn target_threat_role"),
        (
            "has_armor_reaction",
            "combat.rs",
            "pub fn weapon_triggers_tank_armor_reaction",
        ),
        ("has_queue_policy", "faction.rs", "pub queue_policy"),
        ("has_charges", "faction.rs", "pub charge_recharge_ticks"),
        (
            "has_entrenchment",
            "balance/entrenchment.rs",
            "pub fn is_entrenchment_eligible_infantry",
        ),
        (
            "has_support_setup",
            "balance/support_weapons.rs",
            "pub const fn support_weapon_setup_ticks",
        ),
        (
            "has_extractor_kind",
            "kind.rs",
            "pub const fn extracted_resource_kind",
        ),
        ("has_barbed_wire", "lib.rs", "barbed_wire_affects"),
        ("has_crushes_wire", "lib.rs", "crushes_barbed_wire"),
        (
            "has_insight_ratio",
            "balance/upgrades.rs",
            "pub fn disciple_insight_ratio",
        ),
        ("has_fires_while_moving", "lib.rs", "fires_while_moving"),
        ("has_rifle_infantry", "lib.rs", "is_rifle_infantry"),
        (
            "has_manual_emplacement",
            "lib.rs",
            "supports_manual_emplacement",
        ),
        ("has_blocks_sight", "lib.rs", "blocks_line_of_sight"),
        ("has_body_class", "lib.rs", "movement_body_class"),
        (
            "has_infantry_policy",
            "combat.rs",
            "pub infantry_target_policy",
        ),
        (
            "has_entrenchment_damage",
            "combat.rs",
            "pub fn direct_damage_after_entrenchment",
        ),
    ];
    for (cfg, file, needle) in optional {
        println!("cargo:rustc-check-cfg=cfg({cfg})");
        if fs::read_to_string(rules.join(file)).is_ok_and(|t| t.contains(needle)) {
            println!("cargo:rustc-cfg={cfg}");
        }
    }

    // ---- rules constants
    let mut found = Vec::new();
    let lib = fs::read_to_string(rules.join("lib.rs")).expect("rules crate source");
    for (module, is_pub) in module_decls(&lib) {
        if is_pub {
            if let Some(file) = module_file(&rules, &module) {
                scan(&rules, &file, &module, &mut found);
            }
        }
    }
    let mut out = String::from("// @generated by build.rs\npub fn constants() -> Vec<crate::snapshot::ConstEntry> {\n    vec![\n");
    for f in &found {
        let path = format!("rts_rules::{}::{}", f.module.replace('/', "::"), f.name);
        let value = match f.ty.as_str() {
            "bool" => format!("crate::snapshot::Val::Bool({path})"),
            "&str" | "&'static str" => format!("crate::snapshot::Val::Str({path}.to_string())"),
            "f32" => format!("crate::snapshot::Val::num_f32({path})"),
            _ => format!("crate::snapshot::Val::Num({path} as f64)"),
        };
        let _ = writeln!(
            out,
            "        crate::snapshot::ConstEntry {{ name: {:?}, file: {:?}, line: {}, ty: {:?}, value: {value}, doc: {:?} }},",
            f.name, f.file, f.line, f.ty, f.doc
        );
    }
    out.push_str("    ]\n}\n");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("constants.rs"), out).unwrap();

    // ---- private simulation constants (expression text; evaluated at run time)
    let mut sim = sim_constants(&repo);
    sim.extend(code_values(&repo));
    let mut names: std::collections::HashSet<String> =
        found.iter().map(|f| f.name.clone()).collect();
    names.extend(
        sim.iter()
            .filter_map(|v| v["name"].as_str().map(str::to_string)),
    );

    // ---- definitions and usage
    let index = serde_json::json!({
        "definitions": code_index::definitions_raw(&repo, &names),
        "usage": code_index::usage(&repo, &names, 12),
        "sim": sim,
        "codeValueKeys": CODE_VALUES.iter().map(|cv| cv.key).collect::<Vec<_>>(),
    });
    fs::write(
        out_dir.join("code_index.json"),
        serde_json::to_string(&index).unwrap(),
    )
    .unwrap();
}

fn sim_constants(repo: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let Ok(dir) = fs::read_dir(repo.join(SIM_DIR)) else {
        return out;
    };
    let mut files: Vec<_> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    for file in files {
        let text = fs::read_to_string(&file).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let fname = file.file_name().unwrap().to_string_lossy().to_string();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            let t = ["pub(super) ", "pub(crate) ", "pub "]
                .iter()
                .fold(t, |acc, p| acc.strip_prefix(p).unwrap_or(acc));
            let Some(rest) = t.strip_prefix("const ") else {
                continue;
            };
            let Some((name, after)) = rest.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if !SIM_CONSTANTS.contains(&name) || seen.contains(name) {
                continue;
            }
            let Some((ty, expr)) = after.split_once('=') else {
                continue;
            };
            let Some(expr) = expr.trim().strip_suffix(';') else {
                continue;
            };
            seen.insert(name.to_string());
            let doc: Vec<&str> = lines[..i]
                .iter()
                .rev()
                .take_while(|l| l.trim_start().starts_with("//"))
                .map(|l| l.trim_start().trim_start_matches('/').trim())
                .collect();
            out.push(serde_json::json!({
                "name": name,
                "type": ty.trim(),
                "expr": expr.replace("config::", "").replace(" as u16", "").replace(" as u32", "").replace(" as f32", ""),
                "file": format!("sim: combat/{fname}"),
                "path": format!("{SIM_DIR}/{fname}"),
                "line": i + 1,
                "doc": doc.into_iter().rev().collect::<Vec<_>>().join(" "),
            }));
        }
    }
    out
}

/// [`CODE_VALUES`] found in the source: `{ name, label, read, expr, file, path, line, fn, doc }`.
fn code_values(repo: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for cv in CODE_VALUES {
        let Ok(text) = fs::read_to_string(repo.join(cv.path)) else {
            continue;
        };
        // byte offset just past each anchor in turn; a Called value keeps the last one found
        let mut at = None;
        let mut pos = 0;
        let mut all = true;
        for anchor in cv.anchors {
            match text[pos..].find(anchor) {
                Some(i) => {
                    at = Some(pos + i);
                    pos += i + anchor.len();
                }
                None => {
                    all = false;
                    break;
                }
            }
        }
        let Some(start) = at else {
            continue;
        };
        let line_of = |offset: usize| text[..offset].matches('\n').count() + 1;
        let (expr, line) = match cv.read {
            Read::Called => (String::new(), line_of(start)),
            _ if !all => continue,
            Read::Const => {
                let line_end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
                let Some(expr) = text[pos..line_end]
                    .split_once('=')
                    .and_then(|(_, e)| e.trim().strip_suffix(';'))
                else {
                    continue;
                };
                (expr.trim().to_string(), line_of(start))
            }
            Read::Literal => {
                let Some(i) = text[pos..].find(|c: char| c.is_ascii_digit()) else {
                    continue;
                };
                let digits: String = text[pos + i..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_')
                    .filter(|c| *c != '_')
                    .collect();
                (digits, line_of(pos + i))
            }
        };
        let lines: Vec<&str> = text.lines().collect();
        let doc = match cv.doc {
            "" => doc_above(&lines, line - 1),
            d => d.to_string(),
        };
        // the function the number sits in, for "literal in fn()"
        let func = match cv.read {
            Read::Const => String::new(),
            _ => lines[..line]
                .iter()
                .rev()
                .find_map(|l| fn_name(l))
                .unwrap_or_default(),
        };
        let krate = cv
            .path
            .strip_prefix("server/crates/")
            .and_then(|p| p.split('/').next())
            .unwrap_or("");
        let short: Vec<&str> = cv.path.rsplit('/').take(2).collect();
        let file = match krate {
            "rules" => cv
                .path
                .strip_prefix("server/crates/rules/src/")
                .unwrap_or(cv.path)
                .to_string(),
            _ => format!("{krate}: {}/{}", short[1], short[0]),
        };
        out.push(serde_json::json!({
            "name": cv.key,
            "label": cv.label,
            "read": match cv.read { Read::Const => "const", Read::Literal => "literal", Read::Called => "called" },
            "expr": rust_expr(&expr),
            "file": file,
            "path": cv.path,
            "line": line,
            "fn": func,
            "doc": doc,
        }));
    }
    out
}

/// A Rust constant expression in the form `client_text::eval` reads.
fn rust_expr(expr: &str) -> String {
    let mut e = expr.replace("config::", "");
    for float in ["f32", "f64"] {
        for (name, value) in [
            ("FRAC_PI_2", "(3.141592653589793 / 2)"),
            ("FRAC_PI_4", "(3.141592653589793 / 4)"),
            ("PI", "3.141592653589793"),
        ] {
            e = e.replace(&format!("std::{float}::consts::{name}"), value);
        }
    }
    e
}

/// `foo` for a line declaring `fn foo(` (with any visibility, or `const`).
fn fn_name(line: &str) -> Option<String> {
    let mut t = line.trim_start();
    if t.starts_with("pub(") {
        t = t.split_once(") ")?.1;
    }
    for p in ["pub ", "const ", "async ", "unsafe "] {
        t = t.strip_prefix(p).unwrap_or(t);
    }
    let name = t.strip_prefix("fn ")?.split(['(', '<']).next()?.trim();
    (!name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then(|| name.to_string())
}

/// `mod x;` / `pub mod x;` declarations at the top level of a file.
fn module_decls(text: &str) -> Vec<(String, bool)> {
    let mut mods = Vec::new();
    for line in text.lines() {
        let (is_pub, rest) = if let Some(r) = line.strip_prefix("pub mod ") {
            (true, r)
        } else if let Some(r) = line.strip_prefix("mod ") {
            (false, r)
        } else {
            continue;
        };
        if let Some(name) = rest.trim().strip_suffix(';') {
            mods.push((name.trim().to_string(), is_pub));
        }
    }
    mods
}

fn module_file(src: &Path, module: &str) -> Option<PathBuf> {
    let flat = src.join(format!("{module}.rs"));
    if flat.exists() {
        return Some(flat);
    }
    let nested = src.join(module).join("mod.rs");
    nested.exists().then_some(nested)
}

/// Top-level `pub const NAME: <scalar>` items of a public module, and of private child modules
/// re-exported with `pub use child::*`.
fn scan(src: &Path, file: &Path, module: &str, found: &mut Vec<Found>) {
    let Ok(text) = fs::read_to_string(file) else {
        return;
    };
    let rel = file
        .strip_prefix(src)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        if rest.starts_with("fn ") {
            continue;
        }
        let Some((name, after)) = rest.split_once(':') else {
            continue;
        };
        let Some((ty, _)) = after.split_once('=') else {
            continue;
        };
        let ty = ty.trim().to_string();
        let name = name.trim().to_string();
        if !SCALARS.contains(&ty.as_str())
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        found.push(Found {
            name,
            module: module.to_string(),
            file: rel.clone(),
            line: index + 1,
            ty,
            doc: doc_above(&lines, index),
        });
    }
    let parent_dir = if file.file_name().is_some_and(|n| n == "mod.rs") {
        file.parent().unwrap().to_path_buf()
    } else {
        file.with_extension("")
    };
    for (child, is_pub) in module_decls(&text) {
        let glob = text.contains(&format!("pub use {child}::*;"));
        if !is_pub && !glob {
            continue;
        }
        let flat = parent_dir.join(format!("{child}.rs"));
        let child_file = if flat.exists() {
            flat
        } else {
            parent_dir.join(&child).join("mod.rs")
        };
        if !child_file.exists() {
            continue;
        }
        let child_module = if is_pub {
            format!("{module}/{child}")
        } else {
            module.to_string()
        };
        scan(src, &child_file, &child_module, found);
    }
}

fn doc_above(lines: &[&str], index: usize) -> String {
    let mut doc = Vec::new();
    let mut i = index;
    while i > 0 {
        i -= 1;
        let t = lines[i].trim();
        if t.starts_with("#[") {
            continue;
        }
        if let Some(c) = t.strip_prefix("///").or_else(|| t.strip_prefix("//")) {
            if c.starts_with('!') || c.trim_start().starts_with("---") {
                break;
            }
            doc.push(c.trim().to_string());
        } else {
            break;
        }
    }
    doc.reverse();
    doc.join(" ")
}
