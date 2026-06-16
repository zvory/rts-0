use std::path::{Component, Path, PathBuf};

use axum::extract::Path as AxumPath;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use rts_rules::{defs, faction, EntityKind};

const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WikiPathError {
    Traversal,
    Missing,
}

pub async fn wiki_index_handler() -> Response {
    match wiki_index_markdown() {
        Ok(markdown) => wiki_html("docs/context/README.md", &markdown).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "wiki index unavailable").into_response(),
    }
}

pub async fn wiki_page_handler(AxumPath(path): AxumPath<String>) -> Response {
    wiki_response_for(&path)
}

fn wiki_response_for(route_path: &str) -> Response {
    if is_stats_route(route_path) {
        return stats_page_html().into_response();
    }

    match resolve_wiki_doc(route_path) {
        Ok(doc) => match std::fs::read_to_string(&doc.path) {
            Ok(markdown) => wiki_html(&doc.route_path, &markdown).into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "wiki page not found").into_response(),
        },
        Err(WikiPathError::Traversal) => {
            (StatusCode::BAD_REQUEST, "invalid wiki path").into_response()
        }
        Err(WikiPathError::Missing) => {
            (StatusCode::NOT_FOUND, "wiki page not found").into_response()
        }
    }
}

fn wiki_html(route_path: &str, markdown: &str) -> impl IntoResponse {
    let title = page_title(route_path, markdown);
    let body = render_markdown(route_path, markdown);
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} - Bewegungskrieg Wiki</title>
<style>
body {{ max-width: 960px; margin: 0 auto; padding: 32px 20px; font: 16px/1.55 system-ui, sans-serif; color: #1c1f23; background: #f8f7f3; }}
main {{ background: #fff; border: 1px solid #ddd7cc; padding: 24px; }}
a {{ color: #0b5e86; }}
code, pre {{ background: #f1eee6; }}
pre {{ padding: 12px; overflow-x: auto; }}
table {{ border-collapse: collapse; }}
th, td {{ border: 1px solid #d8d2c7; padding: 4px 8px; }}
</style>
</head>
<body>
<nav><a href="/wiki">Wiki index</a></nav>
<main>
{body}
</main>
</body>
</html>"#
    );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Html(html),
    )
}

fn is_stats_route(route_path: &str) -> bool {
    matches!(route_path.trim_matches('/'), "stats" | "stats.html")
}

fn stats_page_html() -> impl IntoResponse {
    let body = render_stats_tables(&build_stats_tables());
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Gameplay Stats - Bewegungskrieg Wiki</title>
<style>
body {{ max-width: 1120px; margin: 0 auto; padding: 32px 20px; font: 16px/1.55 system-ui, sans-serif; color: #1c1f23; background: #f8f7f3; }}
main {{ background: #fff; border: 1px solid #ddd7cc; padding: 24px; }}
a {{ color: #0b5e86; }}
table {{ border-collapse: collapse; width: 100%; margin: 16px 0 28px; font-size: 14px; }}
th, td {{ border: 1px solid #d8d2c7; padding: 4px 8px; text-align: left; vertical-align: top; }}
th {{ background: #f1eee6; }}
td.numeric {{ text-align: right; font-variant-numeric: tabular-nums; }}
</style>
</head>
<body>
<nav><a href="/wiki">Wiki index</a></nav>
<main>
<h1>Gameplay Stats</h1>
{body}
</main>
</body>
</html>"#
    );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Html(html),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatsTable {
    title: &'static str,
    columns: &'static [&'static str],
    rows: Vec<Vec<String>>,
}

fn build_stats_tables() -> Vec<StatsTable> {
    vec![
        unit_stats_table(),
        building_stats_table(),
        resource_node_stats_table(),
        faction_catalog_table(),
        trainable_table(),
        buildable_table(),
        upgrade_table(),
        ability_table(),
    ]
}

fn unit_stats_table() -> StatsTable {
    StatsTable {
        title: "Units",
        columns: &[
            "Name",
            "ID",
            "HP",
            "Damage",
            "Range",
            "Cooldown",
            "Speed",
            "Sight",
            "Steel",
            "Oil",
            "Supply",
            "Build ticks",
            "Radius",
            "Armor",
            "Weapon",
            "Trained at",
            "Requires",
        ],
        rows: defs::UNITS
            .iter()
            .map(|def| {
                vec![
                    kind_label(def.kind),
                    def.kind.stable_id().to_string(),
                    def.stats.hp.to_string(),
                    def.stats.dmg.to_string(),
                    def.stats.range_tiles.to_string(),
                    def.stats.cooldown.to_string(),
                    format_float(def.stats.speed),
                    def.stats.sight_tiles.to_string(),
                    def.stats.cost_steel.to_string(),
                    def.stats.cost_oil.to_string(),
                    def.stats.supply.to_string(),
                    def.stats.build_ticks.to_string(),
                    format_float(def.stats.radius),
                    format!("{:?}", def.armor_class),
                    format!("{:?}", def.weapon),
                    optional_kind(def.trained_at),
                    kind_list(def.train_requires),
                ]
            })
            .collect(),
    }
}

fn building_stats_table() -> StatsTable {
    StatsTable {
        title: "Buildings",
        columns: &[
            "Name",
            "ID",
            "HP",
            "Sight",
            "Steel",
            "Oil",
            "Footprint",
            "Build ticks",
            "Supply",
            "Damage",
            "Range",
            "Cooldown",
            "Armor",
            "Weapon",
            "Trains",
            "Requires",
        ],
        rows: defs::BUILDINGS
            .iter()
            .map(|def| {
                vec![
                    kind_label(def.kind),
                    def.kind.stable_id().to_string(),
                    def.stats.hp.to_string(),
                    def.stats.sight_tiles.to_string(),
                    def.stats.cost_steel.to_string(),
                    def.stats.cost_oil.to_string(),
                    format!("{}x{}", def.stats.foot_w, def.stats.foot_h),
                    def.stats.build_ticks.to_string(),
                    def.stats.provides_supply.to_string(),
                    def.stats.dmg.to_string(),
                    def.stats.range_tiles.to_string(),
                    def.stats.cooldown.to_string(),
                    format!("{:?}", def.armor_class),
                    format!("{:?}", def.weapon),
                    kind_list(def.trains),
                    kind_list(def.build_requires),
                ]
            })
            .collect(),
    }
}

fn resource_node_stats_table() -> StatsTable {
    StatsTable {
        title: "Resource Nodes",
        columns: &["Name", "ID", "Amount"],
        rows: defs::NODES
            .iter()
            .map(|def| {
                vec![
                    kind_label(def.kind),
                    def.kind.stable_id().to_string(),
                    def.amount.to_string(),
                ]
            })
            .collect(),
    }
}

fn faction_catalog_table() -> StatsTable {
    StatsTable {
        title: "Faction Catalogs",
        columns: &[
            "Faction ID",
            "Loadout ID",
            "Starting steel",
            "Starting oil",
            "Starting entities",
            "Units",
            "Buildings",
            "Builders",
            "Gatherers",
            "Production anchors",
        ],
        rows: faction::CATALOGS
            .iter()
            .map(|catalog| {
                vec![
                    catalog.id.to_string(),
                    catalog.loadout.id.to_string(),
                    catalog.loadout.initial_steel.to_string(),
                    catalog.loadout.initial_oil.to_string(),
                    starting_entity_list(catalog.loadout.starting_entities),
                    kind_list(catalog.units),
                    kind_list(catalog.buildings),
                    kind_list(catalog.builders),
                    kind_list(catalog.gatherers),
                    kind_list(catalog.production_anchors),
                ]
            })
            .collect(),
    }
}

fn trainable_table() -> StatsTable {
    StatsTable {
        title: "Trainables By Faction",
        columns: &["Faction ID", "Building", "Units"],
        rows: faction::CATALOGS
            .iter()
            .flat_map(|catalog| {
                catalog.buildings.iter().filter_map(move |building| {
                    let units = catalog.trainable_units(*building);
                    (!units.is_empty()).then(|| {
                        vec![
                            catalog.id.to_string(),
                            kind_label(*building),
                            kind_vec(units.as_slice()),
                        ]
                    })
                })
            })
            .collect(),
    }
}

fn buildable_table() -> StatsTable {
    StatsTable {
        title: "Buildables By Faction",
        columns: &["Faction ID", "Building", "Requires"],
        rows: faction::CATALOGS
            .iter()
            .flat_map(|catalog| {
                catalog.buildables.iter().map(move |building| {
                    let requires = defs::building_def(*building)
                        .map(|def| def.build_requires)
                        .unwrap_or(&[]);
                    vec![
                        catalog.id.to_string(),
                        kind_label(*building),
                        kind_list(requires),
                    ]
                })
            })
            .collect(),
    }
}

fn upgrade_table() -> StatsTable {
    StatsTable {
        title: "Upgrades By Faction",
        columns: &["Faction ID", "Upgrade ID", "Researched at"],
        rows: faction::CATALOGS
            .iter()
            .flat_map(|catalog| {
                catalog.upgrades.iter().map(move |upgrade| {
                    vec![
                        catalog.id.to_string(),
                        upgrade.id.to_string(),
                        kind_label(upgrade.researched_at),
                    ]
                })
            })
            .collect(),
    }
}

fn ability_table() -> StatsTable {
    StatsTable {
        title: "Abilities By Faction",
        columns: &[
            "Faction ID",
            "Ability ID",
            "Label",
            "Title",
            "Carriers",
            "Target",
            "Range",
            "Min range",
            "Cooldown",
            "Charges",
            "Steel",
            "Oil",
            "Tech",
            "Queue",
            "Autocast",
            "Command card",
        ],
        rows: faction::CATALOGS
            .iter()
            .flat_map(|catalog| {
                catalog.abilities.iter().map(move |ability| {
                    vec![
                        catalog.id.to_string(),
                        ability.id.to_string(),
                        ability.label.to_string(),
                        ability.title.to_string(),
                        kind_list(ability.carriers),
                        ability.target_mode.stable_id().to_string(),
                        optional_u32(ability.range_tiles),
                        optional_u32(ability.min_range_tiles),
                        ability.cooldown_ticks.to_string(),
                        optional_u16(ability.charges),
                        ability.cost.steel.to_string(),
                        ability.cost.oil.to_string(),
                        optional_kind(ability.tech_requirement),
                        bool_text(ability.may_queue),
                        bool_text(ability.autocast),
                        bool_text(ability.command_card),
                    ]
                })
            })
            .collect(),
    }
}

fn render_stats_tables(tables: &[StatsTable]) -> String {
    let mut rendered = String::new();
    for table in tables {
        rendered.push_str("<section>\n<h2>");
        rendered.push_str(&escape_text(table.title));
        rendered.push_str("</h2>\n<table>\n<thead><tr>");
        for column in table.columns {
            rendered.push_str("<th>");
            rendered.push_str(&escape_text(column));
            rendered.push_str("</th>");
        }
        rendered.push_str("</tr></thead>\n<tbody>\n");
        for row in &table.rows {
            rendered.push_str("<tr>");
            for cell in row {
                let class = if is_numeric_cell(cell) {
                    r#" class="numeric""#
                } else {
                    ""
                };
                rendered.push_str("<td");
                rendered.push_str(class);
                rendered.push('>');
                rendered.push_str(&escape_text(cell));
                rendered.push_str("</td>");
            }
            rendered.push_str("</tr>\n");
        }
        rendered.push_str("</tbody>\n</table>\n</section>\n");
    }
    rendered
}

fn kind_label(kind: EntityKind) -> String {
    kind.stable_id()
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn kind_list(kinds: &[EntityKind]) -> String {
    if kinds.is_empty() {
        "None".to_string()
    } else {
        kinds
            .iter()
            .map(|kind| kind_label(*kind))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn kind_vec(kinds: &[EntityKind]) -> String {
    kind_list(kinds)
}

fn optional_kind(kind: Option<EntityKind>) -> String {
    kind.map(kind_label).unwrap_or_else(|| "None".to_string())
}

fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "None".to_string())
}

fn optional_u16(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "None".to_string())
}

fn bool_text(value: bool) -> String {
    if value {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

fn starting_entity_list(groups: &[faction::StartingEntityGroup]) -> String {
    if groups.is_empty() {
        return "None".to_string();
    }
    groups
        .iter()
        .map(|group| format!("{} x{}", kind_label(group.kind), group.count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_float(value: f32) -> String {
    let formatted = format!("{value:.3}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn is_numeric_cell(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WikiDoc {
    route_path: String,
    path: PathBuf,
}

fn resolve_wiki_doc(route_path: &str) -> Result<WikiDoc, WikiPathError> {
    let normalized = normalize_wiki_route_path(route_path)?;
    let path = Path::new(REPO_ROOT).join(&normalized);
    Ok(WikiDoc {
        route_path: normalized.to_string_lossy().into_owned(),
        path,
    })
}

fn normalize_wiki_route_path(route_path: &str) -> Result<PathBuf, WikiPathError> {
    let clean = route_path.trim_start_matches('/');
    if clean.is_empty() {
        return normalize_wiki_route_path("docs/context/README.md");
    }
    let relative = Path::new(clean);
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(WikiPathError::Traversal);
            }
        }
    }
    normalized = canonicalize_legacy_docs_route(normalized);
    if normalized
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        return Err(WikiPathError::Missing);
    }

    if !is_allowlisted_doc_route(&normalized) {
        return Err(WikiPathError::Missing);
    }

    Ok(normalized)
}

fn canonicalize_legacy_docs_route(mut normalized: PathBuf) -> PathBuf {
    let first = normalized
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        });
    if matches!(first, Some("context" | "design")) {
        let mut canonical = PathBuf::from("docs");
        canonical.push(normalized);
        normalized = canonical;
    }
    normalized
}

fn is_allowlisted_doc_route(path: &Path) -> bool {
    let mut components = path.components().filter_map(|component| match component {
        Component::Normal(part) => part.to_str(),
        _ => None,
    });
    matches!(
        (components.next(), components.next(), components.next()),
        (Some("docs"), Some("context" | "design"), Some(_))
    )
}

fn render_markdown(route_path: &str, markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all()).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => {
            Event::Text(CowStr::Boxed(raw.into_string().into_boxed_str()))
        }
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: rewrite_markdown_link(route_path, &dest_url)
                .map(CowStr::Boxed)
                .unwrap_or(dest_url),
            title,
            id,
        }),
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn rewrite_markdown_link(route_path: &str, dest_url: &str) -> Option<Box<str>> {
    if dest_url.starts_with('#') || is_external_or_absolute_url(dest_url) {
        return None;
    }
    let (path_part, anchor) = dest_url
        .split_once('#')
        .map(|(path, anchor)| (path, Some(anchor)))
        .unwrap_or((dest_url, None));
    if Path::new(path_part)
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        return None;
    }

    let base = normalize_wiki_route_path(route_path).ok()?;
    let base_dir = base.parent()?;
    let target = base_dir.join(path_part);
    let target = normalize_path_components(&target)?;
    if !is_allowlisted_doc_route(&target) {
        return None;
    }

    let mut rewritten = format!("/wiki/{}", target.to_string_lossy());
    if let Some(anchor) = anchor {
        rewritten.push('#');
        rewritten.push_str(anchor);
    }
    Some(rewritten.into_boxed_str())
}

fn is_external_or_absolute_url(url: &str) -> bool {
    url.starts_with('/')
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
}

fn normalize_path_components(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

fn wiki_index_markdown() -> std::io::Result<String> {
    let mut markdown = String::from("# Bewegungskrieg Wiki\n\n## Context Capsules\n\n");
    for doc in docs_in_root("docs/context")? {
        markdown.push_str(&format_doc_link(&doc)?);
    }
    markdown.push_str("\n## Design Docs\n\n");
    for doc in docs_in_root("docs/design")? {
        markdown.push_str(&format_doc_link(&doc)?);
    }
    markdown.push_str("\n## Generated References\n\n- [Gameplay Stats](/wiki/stats)\n");
    Ok(markdown)
}

fn docs_in_root(root: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut docs = Vec::new();
    for entry in std::fs::read_dir(Path::new(REPO_ROOT).join(root))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            let relative = path
                .strip_prefix(REPO_ROOT)
                .expect("wiki docs should be inside repo root")
                .to_path_buf();
            docs.push(relative);
        }
    }
    docs.sort();
    Ok(docs)
}

fn format_doc_link(route_path: &Path) -> std::io::Result<String> {
    let path = Path::new(REPO_ROOT).join(route_path);
    let markdown = std::fs::read_to_string(path)?;
    let route_path = route_path.to_string_lossy();
    let title = raw_page_title(&route_path, &markdown);
    Ok(format!(
        "- [{}](/wiki/{})\n",
        escape_markdown_link_text(&title),
        route_path
    ))
}

fn page_title(route_path: &str, markdown: &str) -> String {
    escape_text(&raw_page_title(route_path, markdown))
}

fn raw_page_title(route_path: &str, markdown: &str) -> String {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::to_owned)
        .unwrap_or_else(|| route_path.to_owned())
}

fn escape_markdown_link_text(text: &str) -> String {
    text.replace('[', r"\[").replace(']', r"\]")
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn wiki_resolver_allows_context_doc() {
        let doc =
            resolve_wiki_doc("docs/context/README.md").expect("context readme should resolve");
        assert!(doc.path.ends_with("docs/context/README.md"));
    }

    #[test]
    fn wiki_resolver_keeps_legacy_context_route_working() {
        let doc = resolve_wiki_doc("context/README.md").expect("context readme should resolve");
        assert_eq!(doc.route_path, "docs/context/README.md");
        assert!(doc.path.ends_with("docs/context/README.md"));
    }

    #[test]
    fn wiki_resolver_blocks_traversal() {
        assert_eq!(
            resolve_wiki_doc("../server/Cargo.toml"),
            Err(WikiPathError::Traversal)
        );
        assert_eq!(
            resolve_wiki_doc("context/../../server/Cargo.toml"),
            Err(WikiPathError::Traversal)
        );
    }

    #[test]
    fn wiki_renderer_escapes_inline_html() {
        let rendered = render_markdown(
            "docs/context/README.md",
            "# Hello\n\n<script>alert(1)</script>\n",
        );
        assert!(rendered.contains("<h1>Hello</h1>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!rendered.contains("<script>"));
    }

    #[test]
    fn wiki_renderer_rewrites_relative_doc_links() {
        let rendered = render_markdown(
            "docs/context/protocol.md",
            "[design](../design/protocol.md#snapshot) [local](client-ui.md)",
        );
        assert!(rendered.contains(r#"href="/wiki/docs/design/protocol.md#snapshot""#));
        assert!(rendered.contains(r#"href="/wiki/docs/context/client-ui.md""#));
    }

    #[test]
    fn wiki_renderer_preserves_external_same_page_and_non_doc_links() {
        let rendered = render_markdown(
            "docs/context/testing.md",
            "[anchor](#manual) [site](https://example.com) [asset](../image.png)",
        );
        assert!(rendered.contains(r##"href="#manual""##));
        assert!(rendered.contains(r#"href="https://example.com""#));
        assert!(rendered.contains(r#"href="../image.png""#));
    }

    #[test]
    fn generated_stats_tables_cover_authoritative_rules_data() {
        let tables = build_stats_tables();
        let units = table(&tables, "Units");
        let buildings = table(&tables, "Buildings");
        let nodes = table(&tables, "Resource Nodes");
        let factions = table(&tables, "Faction Catalogs");
        let trainables = table(&tables, "Trainables By Faction");
        let buildables = table(&tables, "Buildables By Faction");
        let upgrades = table(&tables, "Upgrades By Faction");
        let abilities = table(&tables, "Abilities By Faction");

        assert_eq!(units.rows.len(), defs::UNITS.len());
        let tank = defs::unit_def(EntityKind::Tank).expect("tank def");
        assert!(units.rows.contains(&vec![
            "Tank".to_string(),
            EntityKind::Tank.stable_id().to_string(),
            tank.stats.hp.to_string(),
            tank.stats.dmg.to_string(),
            tank.stats.range_tiles.to_string(),
            tank.stats.cooldown.to_string(),
            format_float(tank.stats.speed),
            tank.stats.sight_tiles.to_string(),
            tank.stats.cost_steel.to_string(),
            tank.stats.cost_oil.to_string(),
            tank.stats.supply.to_string(),
            tank.stats.build_ticks.to_string(),
            format_float(tank.stats.radius),
            format!("{:?}", tank.armor_class),
            format!("{:?}", tank.weapon),
            optional_kind(tank.trained_at),
            kind_list(tank.train_requires),
        ]));

        assert_eq!(buildings.rows.len(), defs::BUILDINGS.len());
        let depot = defs::building_def(EntityKind::Depot).expect("depot def");
        assert!(buildings.rows.contains(&vec![
            "Depot".to_string(),
            EntityKind::Depot.stable_id().to_string(),
            depot.stats.hp.to_string(),
            depot.stats.sight_tiles.to_string(),
            depot.stats.cost_steel.to_string(),
            depot.stats.cost_oil.to_string(),
            format!("{}x{}", depot.stats.foot_w, depot.stats.foot_h),
            depot.stats.build_ticks.to_string(),
            depot.stats.provides_supply.to_string(),
            depot.stats.dmg.to_string(),
            depot.stats.range_tiles.to_string(),
            depot.stats.cooldown.to_string(),
            format!("{:?}", depot.armor_class),
            format!("{:?}", depot.weapon),
            kind_list(depot.trains),
            kind_list(depot.build_requires),
        ]));

        assert_eq!(nodes.rows.len(), defs::NODES.len());
        assert!(nodes.rows.contains(&vec![
            "Steel".to_string(),
            EntityKind::Steel.stable_id().to_string(),
            defs::node_def(EntityKind::Steel)
                .unwrap()
                .amount
                .to_string(),
        ]));

        assert_eq!(factions.rows.len(), faction::CATALOGS.len());
        assert!(factions
            .rows
            .iter()
            .any(|row| row[0] == faction::DEFAULT_FACTION_ID && row[1] == "kriegsia.standard"));

        for catalog in faction::CATALOGS {
            for building in catalog.buildings {
                let units = catalog.trainable_units(*building);
                if units.is_empty() {
                    continue;
                }
                assert!(trainables.rows.contains(&vec![
                    catalog.id.to_string(),
                    kind_label(*building),
                    kind_vec(units.as_slice()),
                ]));
            }
            for building in catalog.buildables {
                let requires = defs::building_def(*building)
                    .map(|def| def.build_requires)
                    .unwrap_or(&[]);
                assert!(buildables.rows.contains(&vec![
                    catalog.id.to_string(),
                    kind_label(*building),
                    kind_list(requires),
                ]));
            }
            for upgrade in catalog.upgrades {
                assert!(upgrades.rows.contains(&vec![
                    catalog.id.to_string(),
                    upgrade.id.to_string(),
                    kind_label(upgrade.researched_at),
                ]));
            }
            for ability in catalog.abilities {
                assert!(abilities.rows.contains(&vec![
                    catalog.id.to_string(),
                    ability.id.to_string(),
                    ability.label.to_string(),
                    ability.title.to_string(),
                    kind_list(ability.carriers),
                    ability.target_mode.stable_id().to_string(),
                    optional_u32(ability.range_tiles),
                    optional_u32(ability.min_range_tiles),
                    ability.cooldown_ticks.to_string(),
                    optional_u16(ability.charges),
                    ability.cost.steel.to_string(),
                    ability.cost.oil.to_string(),
                    optional_kind(ability.tech_requirement),
                    bool_text(ability.may_queue),
                    bool_text(ability.autocast),
                    bool_text(ability.command_card),
                ]));
            }
        }
    }

    #[test]
    fn generated_stats_table_set_is_complete_and_nonempty() {
        let tables = build_stats_tables();
        let titles = tables.iter().map(|table| table.title).collect::<Vec<_>>();
        assert_eq!(
            titles,
            vec![
                "Units",
                "Buildings",
                "Resource Nodes",
                "Faction Catalogs",
                "Trainables By Faction",
                "Buildables By Faction",
                "Upgrades By Faction",
                "Abilities By Faction",
            ]
        );
        for table in tables {
            assert!(
                !table.rows.is_empty(),
                "{} should expose at least one generated row",
                table.title
            );
            for row in &table.rows {
                assert_eq!(
                    row.len(),
                    table.columns.len(),
                    "{} row should match column count",
                    table.title
                );
            }
        }
    }

    #[test]
    fn generated_stats_table_renderer_escapes_cells() {
        let rendered = render_stats_tables(&[StatsTable {
            title: "Escaping",
            columns: &["Name"],
            rows: vec![vec!["<script>alert(\"x\")</script>".to_string()]],
        }]);

        assert!(rendered.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"));
        assert!(!rendered.contains("<script>"));
    }

    #[tokio::test]
    async fn wiki_index_route_renders_docs_readme() {
        let response = wiki_router()
            .oneshot(Request::builder().uri("/wiki").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("<title>Bewegungskrieg Wiki - Bewegungskrieg Wiki</title>"));
        assert!(body.contains(r#"href="/wiki/docs/context/balance.md""#));
        assert!(body.contains(r#"href="/wiki/docs/design/balance.md""#));
        assert!(body.contains(r#"href="/wiki/stats""#));
        assert!(body.contains("<main>"));
    }

    #[tokio::test]
    async fn wiki_doc_route_renders_allowlisted_page() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/server-sim.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Capsule: server simulation"));
        assert!(body.contains(r#"href="/wiki/docs/design/server-sim.md""#));
    }

    #[tokio::test]
    async fn wiki_doc_route_renders_canonical_docs_path() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/docs/context/balance.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Capsule: balance"));
        assert!(body.contains(r#"href="/wiki/docs/design/balance.md""#));
    }

    #[tokio::test]
    async fn wiki_missing_doc_is_not_found() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/missing.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wiki_traversal_route_is_bad_request() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/%2e%2e/%2e%2e/server/Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn wiki_unsupported_paths_are_not_found() {
        for uri in [
            "/wiki/server/Cargo.toml",
            "/wiki/docs/context/README.txt",
            "/wiki/docs/assets/logo.md",
        ] {
            let response = wiki_router()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }

    #[tokio::test]
    async fn wiki_stats_route_renders_generated_rules_tables() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("<title>Gameplay Stats - Bewegungskrieg Wiki</title>"));
        assert!(body.contains("<h2>Units</h2>"));
        assert!(body.contains("<td>Tank</td>"));
        assert!(body.contains("<td>kriegsia</td>"));
        assert!(body.contains("<td>Smoke</td>"));
        assert!(body.contains("<td>Magic Anchor</td>"));
    }

    #[test]
    fn rewritten_internal_wiki_links_resolve() {
        for doc in allowlisted_docs().expect("allowlisted docs should enumerate") {
            let markdown = std::fs::read_to_string(Path::new(REPO_ROOT).join(&doc))
                .expect("allowlisted doc should read");
            let rendered = render_markdown(&doc.to_string_lossy(), &markdown);
            for href in wiki_hrefs(&rendered) {
                let path_without_anchor = href
                    .trim_start_matches("/wiki/")
                    .split_once('#')
                    .map(|(path, _)| path)
                    .unwrap_or_else(|| href.trim_start_matches("/wiki/"));
                let resolved = resolve_wiki_doc(path_without_anchor)
                    .map(|doc| doc.path)
                    .unwrap_or_else(|error| {
                        panic!("{href} from {} failed: {error:?}", doc.display())
                    });
                assert!(
                    resolved.exists(),
                    "{href} from {} resolved to missing file {}",
                    doc.display(),
                    resolved.display()
                );
            }
        }
    }

    fn allowlisted_docs() -> std::io::Result<Vec<PathBuf>> {
        let mut docs = docs_in_root("docs/context")?;
        docs.extend(docs_in_root("docs/design")?);
        docs.sort();
        Ok(docs)
    }

    fn wiki_hrefs(rendered: &str) -> Vec<&str> {
        let mut hrefs = Vec::new();
        let mut rest = rendered;
        while let Some(start) = rest.find(r#"href="/wiki/"#) {
            rest = &rest[start + r#"href=""#.len()..];
            if let Some(end) = rest.find('"') {
                hrefs.push(&rest[..end]);
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        hrefs
    }

    fn wiki_router() -> Router {
        Router::new()
            .route("/wiki", get(wiki_index_handler))
            .route("/wiki/{*path}", get(wiki_page_handler))
    }

    fn table<'a>(tables: &'a [StatsTable], title: &str) -> &'a StatsTable {
        tables
            .iter()
            .find(|table| table.title == title)
            .unwrap_or_else(|| panic!("missing table {title}"))
    }
}
