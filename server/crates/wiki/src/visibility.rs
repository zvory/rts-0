//! Player-wiki presentation policy. Catalogs also retain replay/fixture and playtest-hidden
//! content, so membership alone is insufficient. Keep the raw snapshot and history intact.
use serde_json::Value;
use std::collections::HashSet;

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

/// Filter the current presentation, including relations, before any pages or search are built.
pub(crate) fn filter_current(cur: &mut Value) {
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
