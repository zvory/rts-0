//! Player-wiki presentation policy. Catalogs also retain replay/fixture and playtest-hidden
//! content, so membership alone is insufficient. Keep the raw snapshot and history intact: hiding
//! happens on the copy the pages are built from.
use crate::render::{upper_snake, Change};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const HIDDEN: &[&str] = &[
    "ekat",
    "golem",
    "zamok",
    "artillery",
    "artillery_unlock",
    "ballistic_tables",
    "healing_seal",
    "healingSeal",
    "depot",
];

/// What the current presentation left out, so balance constants and the version history can
/// leave it out too.
#[derive(Default)]
pub(crate) struct Hidden {
    /// ids of factions, units, buildings, research, abilities and weapons the pages don't show
    ids: HashSet<String>,
    /// constants whose only home was hidden content
    constants: HashSet<String>,
    /// every constant name in the running build, hidden or not
    current_constants: HashSet<String>,
    /// UPPER_SNAKE forms of the hidden ids, for constants that no longer exist at all
    prefixes: Vec<String>,
}

/// Filter the current presentation, including relations, before any pages or search are built.
/// `owner` maps each constant to the page it would have on the unfiltered rules.
pub(crate) fn filter_current(
    cur: &mut Value,
    owner: &HashMap<String, (&'static str, String)>,
) -> Hidden {
    let before = shown_ids(cur);
    let current_constants: HashSet<String> = names(&cur["constants"]).collect();
    prune(cur, HIDDEN);
    let mut allowed = HashSet::new();
    for faction in cur["factions"].as_array().into_iter().flatten() {
        for field in ["units", "buildings", "abilities"] {
            for id in faction[field]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                allowed.insert(id.to_string());
            }
        }
        for upgrade in faction["upgrades"].as_array().into_iter().flatten() {
            if let Some(id) = upgrade["id"].as_str() {
                allowed.insert(id.to_string());
            }
        }
    }
    for field in ["units", "buildings", "upgrades", "abilities"] {
        if let Some(entries) = cur[field].as_array_mut() {
            entries.retain(|v| v["id"].as_str().is_some_and(|id| allowed.contains(id)));
        }
    }
    let unavailable: Vec<String> = cur["abilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| {
            !a["carriers"].as_array().is_some_and(|carriers| {
                carriers
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|id| allowed.contains(id))
            })
        })
        .filter_map(|a| a["id"].as_str().map(str::to_string))
        .collect();
    prune(
        cur,
        &unavailable.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    // Weapons without any current users should not appear in profiles or damage tables.
    if let Some(weapons) = cur["weapons"].as_array_mut() {
        weapons.retain(|w| {
            w["usedBy"].as_array().is_some_and(|users| {
                users
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|id| allowed.contains(id))
            })
        });
    }

    let after = shown_ids(cur);
    let mut ids: HashSet<String> = before.difference(&after).cloned().collect();
    ids.extend(HIDDEN.iter().map(|id| id.to_string()));
    // A constant goes with its hidden home page, unless a page still shown reads it.
    let still_read = constants_read_by(cur, &after);
    let constants: HashSet<String> = current_constants
        .iter()
        .filter(|name| {
            owner
                .get(*name)
                .is_some_and(|(kind, id)| *kind != "page" && ids.contains(id))
                && !still_read.contains(*name)
        })
        .cloned()
        .collect();
    if let Some(entries) = cur["constants"].as_array_mut() {
        entries.retain(|c| c["name"].as_str().is_some_and(|n| !constants.contains(n)));
    }
    let prefixes = ids
        .iter()
        .map(|id| upper_snake(id))
        .filter(|p| p.len() >= 4)
        .collect();
    Hidden {
        ids,
        constants,
        current_constants,
        prefixes,
    }
}

impl Hidden {
    /// The version history as the pages show it: changes to hidden content dropped, hidden ids
    /// taken out of list changes (a faction's units, an ability's carriers).
    pub(crate) fn filter_history(&self, diffs: Vec<Vec<Change>>) -> Vec<Vec<Change>> {
        diffs
            .into_iter()
            .map(|changes| {
                changes
                    .into_iter()
                    .filter_map(|c| self.filter_change(c))
                    .collect()
            })
            .collect()
    }

    fn filter_change(&self, mut c: Change) -> Option<Change> {
        if c.kind == "constant" {
            return (!self.hides_constant(&c.id)).then_some(c);
        }
        if self.ids.contains(&c.id) {
            return None;
        }
        if let (Some(Value::Array(old)), Some(Value::Array(new))) = (&mut c.old, &mut c.new) {
            let shown = |v: &Value| {
                let id = v.as_str().or_else(|| v.get("id").and_then(Value::as_str));
                !id.is_some_and(|id| self.ids.contains(id))
            };
            old.retain(shown);
            new.retain(shown);
            if old == new {
                return None;
            }
        }
        Some(c)
    }

    fn hides_constant(&self, name: &str) -> bool {
        if self.current_constants.contains(name) {
            return self.constants.contains(name);
        }
        // gone from the game: judge it by its name
        self.prefixes
            .iter()
            .any(|p| name == p || name.starts_with(&format!("{p}_")))
    }
}

/// Ids of everything the pages can show.
fn shown_ids(cur: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    for field in [
        "factions",
        "units",
        "buildings",
        "upgrades",
        "abilities",
        "weapons",
    ] {
        for v in cur[field].as_array().into_iter().flatten() {
            if let Some(id) = v["id"].as_str() {
                out.insert(id.to_string());
            }
        }
    }
    out
}

fn names(constants: &Value) -> impl Iterator<Item = String> + '_ {
    constants
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["name"].as_str().map(str::to_string))
}

/// Constants named in the code definitions of entries that are still shown.
fn constants_read_by(cur: &Value, shown: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(Value::Object(sources)) = cur.get("sources") else {
        return out;
    };
    for (key, fields) in sources {
        let Some((_, id)) = key.split_once(':') else {
            continue;
        };
        if !shown.contains(id) {
            continue;
        }
        for field in fields.as_object().into_iter().flat_map(|f| f.values()) {
            for name in field["consts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                out.insert(name.to_string());
            }
        }
    }
    out
}

fn prune(value: &mut Value, hidden: &[&str]) {
    match value {
        Value::Array(entries) => {
            entries.retain(|v| {
                let id = v.as_str().or_else(|| v.get("id").and_then(Value::as_str));
                !id.is_some_and(|id| hidden.contains(&id))
            });
            for entry in entries {
                prune(entry, hidden);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                prune(value, hidden);
            }
        }
        _ => {}
    }
}
