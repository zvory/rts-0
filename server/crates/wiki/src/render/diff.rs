//! One change between two versions of the rules, as recorded in docs/wiki/history.json (made by
//! the wiki's history tool, which compares the rules at each version field by field and finds the
//! PR behind each change).

use crate::render::util::s;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Change {
    /// "unit", "building", "research", "ability", "weapon", "node", "faction", "constant"
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    /// dotted field path; empty when the whole entry was added or removed
    pub field: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
    /// the PR (or commit) that made it, when it could be pinned to exactly one: { sha, subject }
    pub pr: Option<Value>,
}

impl Change {
    pub fn from_json(v: &Value) -> Option<Change> {
        let kind = match s(v, "kind").as_str() {
            "unit" => "unit",
            "building" => "building",
            "research" => "research",
            "ability" => "ability",
            "weapon" => "weapon",
            "node" => "node",
            "faction" => "faction",
            "constant" => "constant",
            _ => return None,
        };
        let opt = |key: &str| v.get(key).filter(|x| !x.is_null()).cloned();
        Some(Change {
            kind,
            id: s(v, "id"),
            name: s(v, "name"),
            field: s(v, "field"),
            old: opt("old"),
            new: opt("new"),
            pr: opt("pr"),
        })
    }
}
