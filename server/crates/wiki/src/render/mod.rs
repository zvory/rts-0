//! The page generator: the live snapshot + docs/wiki data (notes, icons, version history) → HTML.
//! Every page is self-contained (styles, scripts and icons inlined) and links to its siblings by
//! relative file name, so the whole site is served flat under /wiki/.

mod content;
mod diff;
pub(crate) mod pages;
mod util;

pub use content::Doc;
use diff::Change;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use util::{arr, esc, get, s, title_case, upper_snake, Units};

pub struct Wiki {
    /// generated pages: file name → HTML
    pub pages: RefCell<BTreeMap<String, String>>,
    /// the running build's rules
    pub cur: Value,
    /// past versions from docs/wiki/history.json, oldest first: { game, commits }
    pub snaps: Vec<Value>,
    pub u: Units,
    pub content: BTreeMap<String, Doc>,
    /// diffs[i]: what changed from snaps[i-1] to snaps[i] (diffs[0] is the baseline: empty)
    pub diffs: Vec<Vec<Change>>,
    pub missing: RefCell<Vec<(String, String)>>,
    /// constant name -> (entity kind, entity id) whose page lists it
    pub const_owner: HashMap<String, (&'static str, String)>,
    pub images: HashMap<String, String>,
    /// unit/building id -> image data URL, drawn by the game client's own art code
    pub icons: HashMap<String, String>,
    /// the last accuracy-check summary recorded with the history
    pub verification: Value,
}

/// Build every page. `history` is docs/wiki/history.json, `icons` docs/wiki/icons.json.
pub fn render(
    mut cur: Value,
    content: BTreeMap<String, Doc>,
    icons: Option<&str>,
    history: Option<&str>,
) -> BTreeMap<String, String> {
    super::visibility::filter_current(&mut cur);
    let u = Units {
        tick_hz: util::n(get(&cur, "timing"), "tickHz").unwrap_or(30.0),
        tile: util::n(get(&cur, "timing"), "tileSize").unwrap_or(32.0),
    };
    let history: Value = history
        .and_then(|h| serde_json::from_str(h.trim_start_matches('\u{feff}')).ok())
        .unwrap_or(Value::Null);
    let mut snaps = Vec::new();
    let mut diffs = Vec::new();
    for v in arr(&history, "versions") {
        snaps.push(serde_json::json!({ "game": get(v, "game"), "commits": get(v, "commits") }));
        diffs.push(
            arr(v, "changes")
                .iter()
                .filter_map(Change::from_json)
                .collect(),
        );
    }
    if snaps.is_empty() {
        // no history yet: the running build is the baseline
        snaps.push(serde_json::json!({ "game": get(&cur, "game"), "commits": [] }));
        diffs.push(Vec::new());
    }
    let mut wiki = Wiki {
        pages: RefCell::new(BTreeMap::new()),
        cur,
        snaps,
        u,
        content,
        diffs,
        missing: RefCell::new(Vec::new()),
        const_owner: HashMap::new(),
        images: HashMap::new(),
        icons: icons.map(load_icons).unwrap_or_default(),
        verification: get(&history, "verification").clone(),
    };
    wiki.const_owner = wiki.assign_constants();
    pages::render_all(&wiki);
    wiki.pages.into_inner()
}

/// icons.json: { units: {id: data-url}, buildings: {...} }
fn load_icons(text: &str) -> HashMap<String, String> {
    let Ok(v) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for group in ["units", "buildings"] {
        if let Some(Value::Object(m)) = v.get(group) {
            for (k, url) in m {
                if let Some(u) = url.as_str().filter(|u| u.starts_with("data:image/")) {
                    map.insert(k.clone(), u.to_string());
                }
            }
        }
    }
    map
}

pub const KINDS: &[(&str, &str)] = &[
    ("unit", "units"),
    ("building", "buildings"),
    ("research", "upgrades"),
    ("ability", "abilities"),
    ("faction", "factions"),
    ("weapon", "weapons"),
    ("node", "nodes"),
];

impl Wiki {
    pub fn cur(&self) -> &Value {
        &self.cur
    }

    pub fn game(&self) -> &Value {
        get(self.cur(), "game")
    }

    pub fn list(&self, coll: &str) -> &[Value] {
        arr(self.cur(), coll)
    }

    pub fn find(&self, coll: &str, id: &str) -> Option<&Value> {
        self.list(coll).iter().find(|x| s(x, "id") == id)
    }

    pub fn unit(&self, id: &str) -> Option<&Value> {
        self.find("units", id)
    }
    pub fn building(&self, id: &str) -> Option<&Value> {
        self.find("buildings", id)
    }
    pub fn research(&self, id: &str) -> Option<&Value> {
        self.find("upgrades", id)
    }
    pub fn ability(&self, id: &str) -> Option<&Value> {
        self.find("abilities", id)
    }
    pub fn weapon(&self, id: &str) -> Option<&Value> {
        self.find("weapons", id)
    }
    pub fn faction(&self, id: &str) -> Option<&Value> {
        self.find("factions", id)
    }

    pub fn constant(&self, name: &str) -> Option<Value> {
        self.list("constants")
            .iter()
            .find(|c| s(c, "name") == name)
            .map(|c| get(c, "value").clone())
    }

    /// Entity JSON and its page link.
    pub fn entity(&self, kind: &str, id: &str) -> Option<(&Value, String)> {
        let coll = KINDS.iter().find(|(k, _)| *k == kind)?.1;
        let v = self.find(coll, id)?;
        Some((v, self.file_for(kind, id)))
    }

    pub fn file_for(&self, kind: &str, id: &str) -> String {
        match kind {
            "unit" => format!("unit-{id}.html"),
            "building" => format!("building-{id}.html"),
            "research" => format!("research-{id}.html"),
            "ability" => format!("abilities.html#{id}"),
            "faction" => format!("faction-{id}.html"),
            "weapon" => format!("weapons.html#w-{id}"),
            "constant" => format!("constants.html#c-{id}"),
            "page" => pages::doc_file(id),
            _ => "index.html".into(),
        }
    }

    pub fn entity_name(&self, kind: &str, id: &str) -> String {
        match kind {
            "faction" => self.faction_name(id),
            "page" => self
                .content
                .get(id)
                .map(|d| pages::doc_title(id, d))
                .unwrap_or_else(|| title_case(id)),
            "weapon" => weapon_name(id),
            "ability" => self
                .ability(id)
                .map(|a| s(a, "label"))
                .unwrap_or_else(|| title_case(id)),
            _ => {
                let coll = KINDS
                    .iter()
                    .find(|(k, _)| *k == kind)
                    .map(|x| x.1)
                    .unwrap_or("units");
                self.find(coll, id)
                    .map(|v| s(v, "name"))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| title_case(id))
            }
        }
    }

    pub fn faction_name(&self, id: &str) -> String {
        self.content
            .get(&format!("factions/{id}"))
            .and_then(|d| d.get("name").map(str::to_string))
            .unwrap_or_else(|| title_case(id))
    }

    /// "unit" / "building" / "node" for an entity-kind id.
    pub fn kind_of(&self, id: &str) -> &'static str {
        if self.unit(id).is_some() {
            "unit"
        } else if self.building(id).is_some() {
            "building"
        } else {
            "node"
        }
    }

    pub fn name_of(&self, id: &str) -> String {
        self.entity_name(self.kind_of(id), id)
    }

    /// Link to a unit or building page by entity id.
    pub fn link(&self, id: &str) -> String {
        let kind = self.kind_of(id);
        if kind == "node" {
            return esc(&self.name_of(id));
        }
        format!(
            "<a href=\"{}\">{}</a>",
            self.file_for(kind, id),
            esc(&self.name_of(id))
        )
    }

    pub fn link_kind(&self, kind: &str, id: &str) -> String {
        format!(
            "<a href=\"{}\">{}</a>",
            self.file_for(kind, id),
            esc(&self.entity_name(kind, id))
        )
    }

    pub fn links(&self, ids: &[String]) -> String {
        if ids.is_empty() {
            return "—".into();
        }
        ids.iter()
            .map(|i| self.link(i))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Factions whose catalog contains this unit / building / research / ability.
    pub fn factions_of(&self, kind: &str, id: &str) -> Vec<String> {
        let key = match kind {
            "unit" => "units",
            "building" => "buildings",
            "research" => "upgrades",
            "ability" => "abilities",
            _ => return vec![],
        };
        self.list("factions")
            .iter()
            .filter(|f| {
                arr(f, key).iter().any(|x| match x {
                    Value::String(v) => v == id,
                    other => s(other, "id") == id,
                })
            })
            .map(|f| s(f, "id"))
            .collect()
    }

    pub fn faction_class(&self, kind: &str, id: &str) -> String {
        self.factions_of(kind, id)
            .first()
            .map(|f| format!("fac-{f}"))
            .unwrap_or_else(|| "fac-none".into())
    }

    /// Picture (content/images/<id>.*) or the in-game icon letters.
    pub fn badge(&self, id: &str, icon: &str, size: &str) -> String {
        if let Some(src) = self.images.get(id) {
            return format!("<span class=\"art {size}\"><img src=\"{src}\" alt=\"\"></span>");
        }
        if self.icons.contains_key(id) {
            // the picture itself is added once per page by the page shell (see icon_styles)
            return format!(
                "<span class=\"art {size} ic-{}\" role=\"img\" aria-label=\"{}\"></span>",
                icon_class(id),
                esc(&self.name_of(id))
            );
        }
        let letters = if icon.is_empty() {
            self.name_of(id)
                .chars()
                .filter(|c| c.is_uppercase())
                .take(3)
                .collect::<String>()
        } else {
            icon.to_string()
        };
        format!("<span class=\"badge {size}\">{}</span>", esc(&letters))
    }

    /// Name prefixes that tie constants to an entity: its id and name in UPPER_SNAKE
    /// (`smoke` -> SMOKE, `Machine Gunner` -> MACHINE_GUNNER), plus any `constants:` front matter.
    pub fn prefixes_of(&self, kind: &str, id: &str) -> Vec<String> {
        let front_matter = |key: &str| -> Vec<String> {
            self.content
                .get(key)
                .and_then(|d| d.get("constants"))
                .map(|extra| {
                    extra
                        .split(',')
                        .map(|p| p.trim().trim_end_matches('_').to_string())
                        .filter(|p| !p.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        match kind {
            "page" => return front_matter(id),
            "faction" => return front_matter(&format!("factions/{id}")),
            _ => {}
        }
        let (coll, dir) = match kind {
            "unit" => ("units", "units"),
            "building" => ("buildings", "buildings"),
            "research" => ("upgrades", "research"),
            _ => ("abilities", "abilities"),
        };
        let Some(v) = self.find(coll, id) else {
            return vec![];
        };
        let mut ps = vec![upper_snake(id)];
        let name = if kind == "ability" {
            s(v, "label")
        } else {
            s(v, "name")
        };
        let from_name = upper_snake(name.replace(['-', '.', '!', '+', '%', '/'], " ").trim())
            .replace("__", "_");
        if from_name.len() > 1 && !ps.contains(&from_name) {
            ps.push(from_name);
        }
        if let Some(extra) = self
            .content
            .get(&format!("{dir}/{id}"))
            .and_then(|d| d.get("constants"))
        {
            ps.extend(
                extra
                    .split(',')
                    .map(|p| p.trim().trim_end_matches('_').to_string())
                    .filter(|p| !p.is_empty()),
            );
        }
        ps
    }

    /// Each constant gets one home page (its changelog entries go there):
    /// 1. the entity whose name prefix matches longest — `TANK_PLUS_*` → Tank Plus, `TANK_TRAP_*` →
    ///    Tank Trap, other `TANK_*` → Tank; a name may also sit inside (`ROCKET_BARRAGE_*` → Barrage);
    /// 2. otherwise the single entity whose code definition names it.
    fn assign_constants(&self) -> HashMap<String, (&'static str, String)> {
        let mut prefixes: Vec<(String, &'static str, String, usize)> = Vec::new();
        let kinds: [(&'static str, &str, usize); 4] = [
            ("unit", "units", 0),
            ("building", "buildings", 1),
            ("research", "upgrades", 2),
            ("ability", "abilities", 3),
        ];
        for (kind, coll, prio) in kinds {
            for v in self.list(coll) {
                let id = s(v, "id");
                for p in self.prefixes_of(kind, &id) {
                    prefixes.push((p, kind, id.clone(), prio));
                }
            }
        }
        // factions and mechanics/guide pages take what their `constants:` line names
        for f in self.list("factions") {
            let id = s(f, "id");
            for p in self.prefixes_of("faction", &id) {
                prefixes.push((p, "faction", id.clone(), 4));
            }
        }
        for key in self.content.keys().filter(|k| pages::is_page_doc(k)) {
            for p in self.prefixes_of("page", key) {
                prefixes.push((p, "page", key.clone(), 5));
            }
        }
        let mut owner = HashMap::new();
        for c in self.list("constants") {
            let name = s(c, "name");
            let best = prefixes
                .iter()
                .filter_map(|(p, kind, id, prio)| {
                    let at_start = name == *p || name.starts_with(&format!("{p}_"));
                    // only an entity's own id may match inside a name (ROCKET_BARRAGE_* → barrage);
                    // display names are too loose for that ("Anchor" vs MINING_ANCHOR_RANGE_TILES)
                    let own_id = *kind != "page" && *kind != "faction" && *p == upper_snake(id);
                    let inside = own_id
                        && p.len() >= 5
                        && (name.contains(&format!("_{p}_")) || name.ends_with(&format!("_{p}")));
                    // score: longer match wins; a match at the start beats one inside
                    (at_start || inside)
                        .then(|| (p.len() * 2 + usize::from(at_start), *prio, *kind, id))
                })
                .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            if let Some((_, _, kind, id)) = best {
                owner.insert(name, (kind, id.clone()));
            }
        }
        // fallback: the one entity whose definition names the constant
        let mut referenced: HashMap<String, Vec<(&'static str, String)>> = HashMap::new();
        if let Some(Value::Object(sources)) = self.cur().get("sources") {
            for (key, fields) in sources {
                let Some((kind, id)) = key.split_once(':') else {
                    continue;
                };
                let kind: &'static str = match kind {
                    "unit" => "unit",
                    "building" => "building",
                    "research" => "research",
                    "ability" => "ability",
                    _ => continue,
                };
                if self.entity(kind, id).is_none() {
                    continue;
                }
                if let Value::Object(fields) = fields {
                    for f in fields.values() {
                        for c in util::strs(f, "consts") {
                            let list = referenced.entry(c).or_default();
                            if !list.iter().any(|(k, i)| *k == kind && i == id) {
                                list.push((kind, id.to_string()));
                            }
                        }
                    }
                }
            }
        }
        for (c, users) in referenced {
            if users.len() == 1 && !owner.contains_key(&c) {
                owner.insert(c, users[0].clone());
            }
        }
        owner
    }

    pub fn constants_of(&self, kind: &str, id: &str) -> Vec<&Value> {
        self.list("constants")
            .iter()
            .filter(|c| {
                self.const_owner
                    .get(&s(c, "name"))
                    .is_some_and(|(k, i)| *k == kind && i == id)
            })
            .collect()
    }

    pub fn note(&self, dir: &str, id: &str) -> Option<&Doc> {
        self.content.get(&format!("{dir}/{id}"))
    }
}

/// Links between numbers and the code that sets or reads them.
impl Wiki {
    pub fn gh(&self, path: &str, line: f64) -> String {
        format!(
            "{}/blob/{}/{}#L{}",
            s(self.game(), "repo"),
            s(self.game(), "commit"),
            path,
            util::fmt(line)
        )
    }

    /// Definition of one field (e.g. "unit:tank", "stats.hp") as read from the source.
    pub fn source(&self, key: &str, field: &str) -> Option<&Value> {
        self.cur().get("sources")?.get(key)?.get(field)
    }

    pub fn const_entry(&self, name: &str) -> Option<&Value> {
        self.list("constants").iter().find(|c| s(c, "name") == name)
    }

    pub fn const_path(c: &Value) -> String {
        match s(c, "path") {
            p if p.is_empty() => format!("server/crates/rules/src/{}", s(c, "file")),
            p => p,
        }
    }

    /// `display`, carrying where it is set: hover shows the code; "Show code" shows the variable
    /// (or file:line for a literal) linking to that line.
    pub fn coded(&self, key: &str, field: &str, display: String) -> String {
        let Some(src) = self.source(key, field) else {
            return display;
        };
        let consts = util::strs(src, "consts");
        let path = s(src, "path");
        let line = util::nz(src, "line");
        let file = path.rsplit('/').next().unwrap_or(&path).to_string();
        let title = format!("{field} = {}  ({file}:{})", s(src, "expr"), util::fmt(line));
        let (label, href) = match consts.as_slice() {
            [one] => match self.const_entry(one) {
                Some(c) => (
                    one.clone(),
                    self.gh(&Self::const_path(c), util::nz(c, "line")),
                ),
                None => (one.clone(), self.gh(&path, line)),
            },
            _ => (format!("{file}:{}", util::fmt(line)), self.gh(&path, line)),
        };
        format!(
            "<span class=\"cv\" title=\"{}\">{display}<a class=\"var\" href=\"{}\">{}</a></span>",
            esc(&title),
            esc(&href),
            esc(&label)
        )
    }

    /// `display` for a named constant.
    pub fn coded_const(&self, name: &str, display: String) -> String {
        let Some(c) = self.const_entry(name) else {
            return display;
        };
        let title = format!(
            "{name} = {}  ({}:{})",
            util::scalar(get(c, "value")),
            s(c, "file"),
            s(c, "line")
        );
        format!(
            "<span class=\"cv\" title=\"{}\">{display}<a class=\"var\" href=\"{}\">{}</a></span>",
            esc(&title),
            esc(&self.gh(&Self::const_path(c), util::nz(c, "line"))),
            esc(name)
        )
    }

    /// Constants named in any field of an entity's definition.
    pub fn referenced_constants(&self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(Value::Object(fields)) = self.cur().get("sources").and_then(|s| s.get(key)) {
            for f in fields.values() {
                for c in util::strs(f, "consts") {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out
    }

    /// Where a constant is read in the rules and simulation code.
    pub fn usage(&self, name: &str) -> &[Value] {
        self.cur()
            .get("usage")
            .and_then(|u| u.get(name))
            .and_then(Value::as_array)
            .map(|a| a.as_slice())
            .unwrap_or(&[])
    }
}

pub fn icon_class(id: &str) -> String {
    id.replace(['.', '_'], "-")
}

impl Wiki {
    /// `<style>` giving each icon class used in `body` its picture, so each image is embedded once.
    pub fn icon_styles(&self, body: &str) -> String {
        let mut used: Vec<&String> = self
            .icons
            .keys()
            .filter(|id| body.contains(&format!(" ic-{}\"", icon_class(id))))
            .collect();
        used.sort();
        if used.is_empty() {
            return String::new();
        }
        let mut css = String::from("<style>");
        for id in used {
            css.push_str(&format!(
                ".ic-{}{{--ic:url(\"{}\")}}",
                icon_class(id),
                self.icons[id]
            ));
        }
        css.push_str("</style>");
        css
    }
}

pub fn weapon_name(id: &str) -> String {
    let t = title_case(id);
    t.replace(" Mg", " MG").replace("Anti Tank", "Anti-Tank")
}

/// All `*.md` notes under docs/wiki/content.
pub fn content_load(dir: &std::path::Path) -> BTreeMap<String, Doc> {
    content::load(dir)
}
