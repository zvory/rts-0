use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PURE_POLICY_MODULES: &[&str] = &["services/order_planner.rs"];
const PURE_POLICY_FORBIDDEN_IMPORTS: &[&str] = &[
    "EntityStore",
    "PlayerState",
    "Fog",
    "MoveCoordinator",
    "SmokeCloudStore",
    "Event",
];

const ENTITY_FIELD_WRITE_APPROVED_PREFIXES: &[&str] = &["entity/"];
const ENTITY_FIELDS: &[&str] = &[
    "id",
    "owner",
    "kind",
    "pos_x",
    "pos_y",
    "hp",
    "max_hp",
    "movement",
    "combat",
    "production",
    "construction",
    "worker",
    "resource_node",
    "ability_cooldowns",
    "ability_uses_remaining",
];

const ALLOWED_SERVICE_IMPORTS: &[(&str, &[&str])] = &[
    (
        "ability_orders",
        &["commands", "move_coordinator", "world_query"],
    ),
    (
        "combat",
        &[
            "geometry",
            "line_of_sight",
            "move_coordinator",
            "movement",
            "occupancy",
            "spatial",
            "world_query",
        ],
    ),
    (
        "commands",
        &[
            "ability_orders",
            "construction",
            "move_coordinator",
            "movement",
            "order_planner",
            "spatial",
            "standability",
            "world_query",
        ],
    ),
    ("construction", &["occupancy", "standability"]),
    (
        "economy",
        &[
            "move_coordinator",
            "occupancy",
            "pathing",
            "spatial",
            "world_query",
        ],
    ),
    ("geometry", &["occupancy"]),
    (
        "move_coordinator",
        &["geometry", "occupancy", "pathing", "standability"],
    ),
    (
        "movement",
        &[
            "geometry",
            "move_coordinator",
            "occupancy",
            "pathing",
            "spatial",
            "standability",
        ],
    ),
    (
        "order_queue",
        &[
            "ability_orders",
            "construction",
            "line_of_sight",
            "move_coordinator",
            "movement",
            "occupancy",
            "pathing",
            "standability",
            "world_query",
        ],
    ),
    ("pathing", &["occupancy", "standability"]),
    (
        "production",
        &["move_coordinator", "occupancy", "pathing", "standability"],
    ),
    ("standability", &["geometry", "occupancy", "spatial"]),
    ("world_query", &["spatial"]),
];

#[derive(Debug, Default)]
pub struct ArchitectureReport {
    pub failures: Vec<String>,
    pub metrics: ArchitectureMetrics,
}

#[derive(Debug, Default)]
pub struct ArchitectureMetrics {
    pub line_counts: Vec<LineCount>,
    pub service_edges: Vec<ServiceEdge>,
    pub broad_mutable_signatures: Vec<FunctionSignature>,
    pub public_exports: Vec<PublicExport>,
    pub entity_field_writes: Vec<EntityFieldWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineCount {
    pub path: String,
    pub lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceEdge {
    pub source: String,
    pub target: String,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub path: String,
    pub line: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicExport {
    pub path: String,
    pub line: usize,
    pub item: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFieldWrite {
    pub path: String,
    pub line: usize,
    pub field: String,
}

#[derive(Debug, Clone)]
struct SourceFile {
    rel_path: String,
    text: String,
}

#[derive(Debug, Clone)]
struct UseStatement {
    line: usize,
    text: String,
}

pub fn check_sim_architecture(game_root: &Path) -> io::Result<ArchitectureReport> {
    let mut files = Vec::new();
    for path in rust_files(game_root)? {
        let text = fs::read_to_string(&path)?;
        let rel_path = relative_slash_path(game_root, &path);
        files.push(SourceFile { rel_path, text });
    }
    Ok(analyze_source_files(&files))
}

fn analyze_source_files(files: &[SourceFile]) -> ArchitectureReport {
    let mut report = ArchitectureReport::default();
    let service_modules = service_modules(files);
    let allowed_service_imports = allowed_service_imports();

    for file in files {
        let stripped = strip_cfg_test_modules(&file.text);
        report.metrics.line_counts.push(LineCount {
            path: file.rel_path.clone(),
            lines: file.text.lines().count(),
        });

        let use_statements = collect_use_statements(&stripped);
        check_service_imports(
            file,
            &use_statements,
            &service_modules,
            &allowed_service_imports,
            &mut report,
        );
        check_pure_policy_imports(file, &use_statements, &mut report);
        collect_broad_signatures(file, &stripped, &mut report);
        collect_public_exports(file, &stripped, &mut report);
        collect_entity_field_writes(file, &stripped, &mut report);
    }

    report.failures.sort();
    report
        .metrics
        .line_counts
        .sort_by(|a, b| a.path.cmp(&b.path));
    report.metrics.service_edges.sort();
    report
}

fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    visit_rust_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            visit_rust_files(&path, files)?;
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn service_modules(files: &[SourceFile]) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for file in files {
        let parts = file.rel_path.split('/').collect::<Vec<_>>();
        if parts.first() != Some(&"services") || parts.len() < 2 {
            continue;
        }
        if parts.len() == 2 {
            if let Some(module) = parts[1].strip_suffix(".rs") {
                if module != "mod" && module != "tests" {
                    modules.insert(module.to_string());
                }
            }
        } else {
            modules.insert(parts[1].to_string());
        }
    }
    modules
}

fn service_module_for_path(rel_path: &str) -> Option<String> {
    let parts = rel_path.split('/').collect::<Vec<_>>();
    if parts.first() != Some(&"services") || parts.len() < 2 {
        return None;
    }
    if parts.last() == Some(&"tests.rs") {
        return None;
    }
    if parts.len() == 2 {
        let module = parts[1].strip_suffix(".rs")?;
        (module != "mod" && module != "tests").then(|| module.to_string())
    } else {
        Some(parts[1].to_string())
    }
}

fn allowed_service_imports() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    ALLOWED_SERVICE_IMPORTS
        .iter()
        .map(|(source, targets)| (*source, targets.iter().copied().collect()))
        .collect()
}

fn check_service_imports(
    file: &SourceFile,
    use_statements: &[UseStatement],
    service_modules: &BTreeSet<String>,
    allowed: &BTreeMap<&str, BTreeSet<&str>>,
    report: &mut ArchitectureReport,
) {
    let Some(source) = service_module_for_path(&file.rel_path) else {
        return;
    };

    for statement in use_statements {
        for target in service_import_targets(&statement.text, service_modules) {
            if target == source {
                continue;
            }
            report.metrics.service_edges.push(ServiceEdge {
                source: source.clone(),
                target: target.clone(),
                path: file.rel_path.clone(),
                line: statement.line,
            });
            let permitted = allowed
                .get(source.as_str())
                .is_some_and(|targets| targets.contains(target.as_str()));
            if !permitted {
                report.failures.push(format!(
                    "{}:{}: service module {source} must not import services::{target} without updating the architecture allowlist",
                    file.rel_path, statement.line
                ));
            }
        }
    }
}

fn check_pure_policy_imports(
    file: &SourceFile,
    use_statements: &[UseStatement],
    report: &mut ArchitectureReport,
) {
    if !PURE_POLICY_MODULES.contains(&file.rel_path.as_str()) {
        return;
    }

    for statement in use_statements {
        for forbidden in PURE_POLICY_FORBIDDEN_IMPORTS {
            if pure_policy_import_matches(&statement.text, forbidden) {
                report.failures.push(format!(
                    "{}:{}: pure-policy module must not import {forbidden}",
                    file.rel_path, statement.line
                ));
            }
        }
    }
}

fn collect_broad_signatures(file: &SourceFile, text: &str, report: &mut ArchitectureReport) {
    for signature in collect_function_signatures(text) {
        let compact = signature.text.split_whitespace().collect::<String>();
        let has_mut_entities = compact.contains("&mutEntityStore");
        let has_mut_players = compact.contains("&mut[PlayerState]");
        if has_mut_entities && has_mut_players {
            report
                .metrics
                .broad_mutable_signatures
                .push(FunctionSignature {
                    path: file.rel_path.clone(),
                    line: signature.line,
                    name: signature.name,
                });
        }
    }
}

fn collect_public_exports(file: &SourceFile, text: &str, report: &mut ArchitectureReport) {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let export = if let Some(rest) = trimmed.strip_prefix("pub(crate) ") {
            Some(rest)
        } else {
            trimmed.strip_prefix("pub ")
        };
        let Some(rest) = export else {
            continue;
        };
        if starts_with_item_keyword(rest) {
            report.metrics.public_exports.push(PublicExport {
                path: file.rel_path.clone(),
                line: index + 1,
                item: rest.to_string(),
            });
        }
    }
}

fn collect_entity_field_writes(file: &SourceFile, text: &str, report: &mut ArchitectureReport) {
    if ENTITY_FIELD_WRITE_APPROVED_PREFIXES
        .iter()
        .any(|prefix| file.rel_path.starts_with(prefix))
    {
        return;
    }

    for (index, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for field in ENTITY_FIELDS {
            if contains_field_assignment(code, field) {
                report.metrics.entity_field_writes.push(EntityFieldWrite {
                    path: file.rel_path.clone(),
                    line: index + 1,
                    field: (*field).to_string(),
                });
            }
        }
    }
}

fn collect_use_statements(text: &str) -> Vec<UseStatement> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut start_line = 0;

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let starts_use = trimmed.starts_with("use ") || trimmed.starts_with("pub use ");
        if current.is_empty() {
            if !starts_use {
                continue;
            }
            start_line = index + 1;
        }
        current.push_str(trimmed);
        current.push(' ');
        if trimmed.ends_with(';') {
            statements.push(UseStatement {
                line: start_line,
                text: current.trim().to_string(),
            });
            current.clear();
        }
    }

    statements
}

fn service_import_targets(statement: &str, service_modules: &BTreeSet<String>) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    let prefix = "crate::game::services::";
    let mut rest = statement;
    while let Some(index) = rest.find(prefix) {
        let tail = &rest[index + prefix.len()..];
        if let Some(braced) = tail.strip_prefix('{') {
            for name in top_level_braced_names(braced) {
                if service_modules.contains(&name) {
                    targets.insert(name);
                }
            }
        } else if let Some(name) = leading_ident(tail) {
            if service_modules.contains(name) {
                targets.insert(name.to_string());
            }
        }
        rest = &tail[tail.len().min(1)..];
    }
    targets
}

fn top_level_braced_names(text_after_open_brace: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth = 1usize;
    let mut token = String::new();
    let mut chars = text_after_open_brace.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                depth += 1;
                token.clear();
            }
            '}' => {
                if depth == 1 {
                    if let Some(name) = clean_ident(&token) {
                        names.push(name.to_string());
                    }
                    break;
                }
                depth -= 1;
                token.clear();
            }
            ',' if depth == 1 => {
                if let Some(name) = clean_ident(&token) {
                    names.push(name.to_string());
                }
                token.clear();
            }
            ':' if depth == 1 && chars.peek() == Some(&':') => {
                if let Some(name) = clean_ident(&token) {
                    names.push(name.to_string());
                }
                token.clear();
            }
            _ if depth == 1 => token.push(ch),
            _ => {}
        }
    }

    names
}

fn leading_ident(text: &str) -> Option<&str> {
    let end = text
        .find(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .unwrap_or(text.len());
    (end > 0).then_some(&text[..end])
}

fn clean_ident(token: &str) -> Option<&str> {
    let trimmed = token.trim();
    let ident = leading_ident(trimmed)?;
    (ident == trimmed).then_some(ident)
}

fn pure_policy_import_matches(statement: &str, forbidden: &str) -> bool {
    if forbidden == "Event" {
        return statement.contains("crate::protocol")
            && statement
                .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                .any(|part| part == "Event");
    }
    statement
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|part| part == forbidden)
}

#[derive(Debug)]
struct ParsedSignature {
    line: usize,
    name: String,
    text: String,
}

fn collect_function_signatures(text: &str) -> Vec<ParsedSignature> {
    let mut signatures = Vec::new();
    let lines = text.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        if trimmed.starts_with("//") || !contains_fn_keyword(trimmed) {
            index += 1;
            continue;
        }

        let mut text = String::new();
        let start_line = index + 1;
        let mut end = index;
        while end < lines.len() {
            let line = lines[end].trim();
            text.push_str(line);
            text.push(' ');
            if line.contains('{') || line.ends_with(';') {
                break;
            }
            end += 1;
        }

        if let Some(name) = function_name(&text) {
            signatures.push(ParsedSignature {
                line: start_line,
                name: name.to_string(),
                text,
            });
        }
        index = end + 1;
    }

    signatures
}

fn contains_fn_keyword(text: &str) -> bool {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|part| part == "fn")
}

fn function_name(signature: &str) -> Option<&str> {
    let fn_index = signature.find("fn ")?;
    let after_fn = &signature[fn_index + 3..];
    leading_ident(after_fn.trim_start())
}

fn starts_with_item_keyword(text: &str) -> bool {
    matches!(
        text.split_whitespace().next(),
        Some("fn" | "struct" | "enum" | "const" | "mod" | "type" | "trait")
    )
}

fn contains_field_assignment(line: &str, field: &str) -> bool {
    let needle = format!(".{field}");
    let Some(start) = line.find(&needle) else {
        return false;
    };
    let before = line[..start].chars().last();
    if before.is_some_and(|ch| ch == '.' || ch.is_ascii_alphanumeric() || ch == '_') {
        return false;
    }
    let after = &line[start + needle.len()..];
    let after = after.trim_start();
    after.starts_with('=')
        || after.starts_with("+=")
        || after.starts_with("-=")
        || after.starts_with("*=")
        || after.starts_with("/=")
        || after.starts_with("%=")
}

fn strip_cfg_test_modules(text: &str) -> String {
    let mut output = String::new();
    let mut previous_cfg_test = false;
    let mut skipping = false;
    let mut brace_depth = 0isize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if !skipping && previous_cfg_test && trimmed.starts_with("mod tests") {
            skipping = true;
            brace_depth = brace_delta(line);
            output.push('\n');
            previous_cfg_test = false;
            if brace_depth <= 0 {
                skipping = false;
            }
            continue;
        }

        if skipping {
            brace_depth += brace_delta(line);
            output.push('\n');
            if brace_depth <= 0 {
                skipping = false;
            }
            continue;
        }

        output.push_str(line);
        output.push('\n');
        previous_cfg_test = trimmed.starts_with("#[cfg(test)]");
    }

    output
}

fn brace_delta(line: &str) -> isize {
    let mut delta = 0;
    for ch in line.chars() {
        match ch {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(rel_path: &str, text: &str) -> SourceFile {
        SourceFile {
            rel_path: rel_path.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn pure_policy_module_importing_entity_store_fails() {
        let report = analyze_source_files(&[source(
            "services/order_planner.rs",
            "use crate::game::entity::EntityStore;\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/order_planner.rs:1: pure-policy module must not import EntityStore"
                    .to_string()
            ]
        );
    }

    #[test]
    fn pure_policy_module_importing_protocol_event_fails() {
        let report = analyze_source_files(&[source(
            "services/order_planner.rs",
            "use crate::protocol::{Event, Snapshot};\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/order_planner.rs:1: pure-policy module must not import Event".to_string()
            ]
        );
    }

    #[test]
    fn service_import_not_on_allowlist_fails() {
        let report = analyze_source_files(&[
            source(
                "services/commands.rs",
                "use crate::game::services::death;\n",
            ),
            source("services/death.rs", ""),
        ]);

        assert_eq!(
            report.failures,
            vec!["services/commands.rs:1: service module commands must not import services::death without updating the architecture allowlist".to_string()]
        );
    }

    #[test]
    fn braced_service_imports_are_parsed() {
        let report = analyze_source_files(&[
            source(
                "services/standability.rs",
                "use crate::game::services::{geometry::{RectBody, UnitBody}, occupancy::Occupancy};\n",
            ),
            source("services/geometry.rs", ""),
            source("services/occupancy.rs", ""),
        ]);

        assert!(report.failures.is_empty());
        assert_eq!(
            report.metrics.service_edges,
            vec![
                ServiceEdge {
                    source: "standability".to_string(),
                    target: "geometry".to_string(),
                    path: "services/standability.rs".to_string(),
                    line: 1,
                },
                ServiceEdge {
                    source: "standability".to_string(),
                    target: "occupancy".to_string(),
                    path: "services/standability.rs".to_string(),
                    line: 1,
                },
            ]
        );
    }

    #[test]
    fn broad_mutable_world_signatures_are_recorded() {
        let report = analyze_source_files(&[source(
            "services/example.rs",
            "pub(crate) fn tick(\n    entities: &mut EntityStore,\n    players: &mut [PlayerState],\n) {}\n",
        )]);

        assert_eq!(
            report.metrics.broad_mutable_signatures,
            vec![FunctionSignature {
                path: "services/example.rs".to_string(),
                line: 1,
                name: "tick".to_string(),
            }]
        );
    }

    #[test]
    fn cfg_test_modules_are_ignored() {
        let report = analyze_source_files(&[
            source(
                "services/commands.rs",
                "#[cfg(test)]\nmod tests {\nuse crate::game::services::death;\n}\n",
            ),
            source("services/death.rs", ""),
        ]);

        assert!(report.failures.is_empty());
    }
}
