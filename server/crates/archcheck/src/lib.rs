use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const LINE_GROWTH_BUFFER: usize = 80;
const TEST_LINE_GROWTH_BUFFER: usize = 250;
const NEW_FILE_LINE_BUDGET: usize = 500;
const NEW_TEST_FILE_LINE_BUDGET: usize = 1_500;
const PUBLIC_EXPORT_GROWTH_BUFFER: usize = 2;
const NEW_MODULE_PUBLIC_EXPORT_BUDGET: usize = 8;

const PURE_POLICY_FORBIDDEN_IMPORTS: &[&str] = &[
    "EntityStore",
    "PlayerState",
    "Fog",
    "MoveCoordinator",
    "SmokeCloudStore",
    "Event",
];

const SERVICE_ROLES: &[(&str, ServiceRole)] = &[
    ("ability_orders", ServiceRole::MutationHelper),
    ("combat", ServiceRole::TickSystem),
    ("commands", ServiceRole::CommandAdapter),
    ("construction", ServiceRole::TickSystem),
    ("death", ServiceRole::TickSystem),
    ("economy", ServiceRole::TickSystem),
    ("geometry", ServiceRole::QueryIndex),
    ("hero", ServiceRole::TickSystem),
    ("line_of_sight", ServiceRole::QueryIndex),
    ("move_coordinator", ServiceRole::MutationHelper),
    ("movement", ServiceRole::TickSystem),
    ("occupancy", ServiceRole::QueryIndex),
    ("order_execution", ServiceRole::MutationHelper),
    ("order_planner", ServiceRole::PurePolicy),
    ("order_queue", ServiceRole::CommandAdapter),
    ("pathing", ServiceRole::QueryIndex),
    ("production", ServiceRole::TickSystem),
    ("spatial", ServiceRole::QueryIndex),
    ("standability", ServiceRole::QueryIndex),
    ("supply", ServiceRole::TickSystem),
    ("world_query", ServiceRole::QueryIndex),
];

const GRANDFATHERED_BROAD_ADAPTERS: &[&str] = &["commands", "order_queue"];

const ROLE_EDGE_ALLOWLIST: &[(&str, &str)] = &[
    // Ability execution still reuses command notice constructors. New command families should
    // prefer facts -> pure plan -> narrow executor instead of adding more adapter back-edges.
    ("ability_orders", "commands"),
    // Ability execution may delegate movement/path staging through the coordinator boundary.
    ("ability_orders", "move_coordinator"),
    // Combat uses movement's shared facing helpers while combat policy is being split out.
    ("combat", "movement"),
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
const PLAYER_STATE_FIELD_WRITE_APPROVED_PATHS: &[&str] = &["player_state.rs"];
const PLAYER_STATE_FIELDS: &[&str] = &["steel", "oil", "supply_used", "supply_cap", "score"];

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
            "order_execution",
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
            "order_execution",
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
    pub ratchet_notes: Vec<String>,
    pub metrics: ArchitectureMetrics,
}

#[derive(Debug, Default)]
pub struct ArchitectureMetrics {
    pub line_counts: Vec<LineCount>,
    pub service_edges: Vec<ServiceEdge>,
    pub broad_mutable_signatures: Vec<FunctionSignature>,
    pub player_state_usages: Vec<PlayerStateUsage>,
    pub player_state_field_writes: Vec<PlayerStateFieldWrite>,
    pub public_exports: Vec<PublicExport>,
    pub entity_field_writes: Vec<EntityFieldWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCount {
    pub path: String,
    pub lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ServiceEdge {
    pub source: String,
    pub target: String,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub path: String,
    pub line: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerStateUsage {
    pub path: String,
    pub line: usize,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerStateFieldWrite {
    pub path: String,
    pub line: usize,
    pub field: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicExport {
    pub path: String,
    pub line: usize,
    pub item: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityFieldWrite {
    pub path: String,
    pub line: usize,
    pub field: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureBaseline {
    pub reason: String,
    pub line_counts: Vec<LineCount>,
    pub service_edges: Vec<ServiceEdge>,
    pub broad_mutable_signatures: Vec<FunctionSignature>,
    pub player_state_usages: Vec<PlayerStateUsage>,
    #[serde(default)]
    pub player_state_field_writes: Vec<PlayerStateFieldWrite>,
    pub entity_field_writes: Vec<EntityFieldWrite>,
    pub public_export_counts: Vec<PublicExportCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicExportCount {
    pub path: String,
    pub count: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceRole {
    TickSystem,
    CommandAdapter,
    PurePolicy,
    QueryIndex,
    MutationHelper,
}

impl ServiceRole {
    fn label(self) -> &'static str {
        match self {
            ServiceRole::TickSystem => "tick system",
            ServiceRole::CommandAdapter => "command adapter",
            ServiceRole::PurePolicy => "pure policy",
            ServiceRole::QueryIndex => "query/index service",
            ServiceRole::MutationHelper => "mutation helper",
        }
    }
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

pub fn check_sim_architecture_with_baseline(
    game_root: &Path,
    baseline_path: &Path,
) -> io::Result<ArchitectureReport> {
    let mut report = check_sim_architecture(game_root)?;
    let baseline = read_baseline(baseline_path)?;
    compare_to_baseline(&baseline, &mut report);
    Ok(report)
}

pub fn bless_sim_architecture_baseline(
    game_root: &Path,
    baseline_path: &Path,
    reason: &str,
) -> io::Result<Vec<String>> {
    if reason.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "baseline updates require a non-empty reason",
        ));
    }

    let report = check_sim_architecture(game_root)?;
    let previous = read_baseline(baseline_path).ok();
    let baseline = ArchitectureBaseline::from_metrics(reason.trim(), &report.metrics);
    let summary = previous
        .as_ref()
        .map(|old| baseline_change_summary(old, &baseline))
        .unwrap_or_else(|| vec!["created baseline".to_string()]);
    let text = serde_json::to_string_pretty(&baseline)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(baseline_path, format!("{text}\n"))?;
    Ok(summary)
}

fn analyze_source_files(files: &[SourceFile]) -> ArchitectureReport {
    let mut report = ArchitectureReport::default();
    let service_modules = service_modules(files);
    let allowed_service_imports = allowed_service_imports();
    let service_roles = service_roles();

    check_service_roles(&service_modules, &service_roles, &mut report);

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
            &service_roles,
            &mut report,
        );
        check_pure_policy_imports(file, &use_statements, &service_roles, &mut report);
        check_role_state_boundaries(file, &stripped, &service_roles, &mut report);
        collect_broad_signatures(file, &stripped, &mut report);
        collect_player_state_usages(file, &stripped, &mut report);
        collect_player_state_field_writes(file, &stripped, &mut report);
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
        .metrics
        .broad_mutable_signatures
        .sort_by_key(function_key);
    report
        .metrics
        .player_state_usages
        .sort_by_key(player_state_usage_key);
    report
        .metrics
        .player_state_field_writes
        .sort_by_key(player_state_field_write_key);
    report
        .metrics
        .public_exports
        .sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    report
        .metrics
        .entity_field_writes
        .sort_by_key(entity_field_write_key);
    report
}

impl ArchitectureBaseline {
    fn from_metrics(reason: &str, metrics: &ArchitectureMetrics) -> Self {
        Self {
            reason: reason.to_string(),
            line_counts: metrics.line_counts.clone(),
            service_edges: metrics.service_edges.clone(),
            broad_mutable_signatures: metrics.broad_mutable_signatures.clone(),
            player_state_usages: metrics.player_state_usages.clone(),
            player_state_field_writes: metrics.player_state_field_writes.clone(),
            entity_field_writes: metrics.entity_field_writes.clone(),
            public_export_counts: public_export_counts(metrics),
        }
    }
}

fn read_baseline(path: &Path) -> io::Result<ArchitectureBaseline> {
    let text = fs::read_to_string(path)?;
    let baseline = serde_json::from_str::<ArchitectureBaseline>(&text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if baseline.reason.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "architecture baseline reason must not be empty",
        ));
    }
    Ok(baseline)
}

fn compare_to_baseline(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    compare_line_counts(baseline, report);
    compare_service_edges(baseline, report);
    compare_broad_mutable_signatures(baseline, report);
    compare_player_state_usages(baseline, report);
    compare_player_state_field_writes(baseline, report);
    compare_entity_field_writes(baseline, report);
    compare_public_export_counts(baseline, report);
    report.failures.sort();
    report.ratchet_notes.sort();
}

fn compare_line_counts(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    let baselines = baseline
        .line_counts
        .iter()
        .map(|entry| (entry.path.as_str(), entry.lines))
        .collect::<BTreeMap<_, _>>();

    for current in &report.metrics.line_counts {
        match baselines.get(current.path.as_str()).copied() {
            Some(old_lines) => {
                let budget = old_lines + line_growth_buffer(&current.path, old_lines);
                if current.lines > budget {
                    report.failures.push(format!(
                        "{}: line count grew to {} lines; baseline is {} with budget {}. Run --bless with a reason if this growth is intentional.",
                        current.path, current.lines, old_lines, budget
                    ));
                } else if current.lines < old_lines {
                    report.ratchet_notes.push(format!(
                        "{} shrank from {} to {} lines; --bless would lower the future budget",
                        current.path, old_lines, current.lines
                    ));
                }
            }
            None => {
                let budget = if is_test_path(&current.path) {
                    NEW_TEST_FILE_LINE_BUDGET
                } else {
                    NEW_FILE_LINE_BUDGET
                };
                if current.lines > budget {
                    report.failures.push(format!(
                        "{}: new file has {} lines, above the unbaselined budget of {}. Run --bless with a reason if this size is intentional.",
                        current.path, current.lines, budget
                    ));
                } else {
                    report.ratchet_notes.push(format!(
                        "{} is new with {} lines; --bless would start tracking it",
                        current.path, current.lines
                    ));
                }
            }
        }
    }
}

fn line_growth_buffer(path: &str, baseline_lines: usize) -> usize {
    let fixed = if is_test_path(path) {
        TEST_LINE_GROWTH_BUFFER
    } else {
        LINE_GROWTH_BUFFER
    };
    fixed.max(baseline_lines / 10)
}

fn is_test_path(path: &str) -> bool {
    path.ends_with("tests.rs") || path.contains("/tests/")
}

fn compare_service_edges(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    let old = baseline
        .service_edges
        .iter()
        .map(service_edge_key)
        .collect::<BTreeSet<_>>();
    for edge in &report.metrics.service_edges {
        if !old.contains(&service_edge_key(edge)) {
            report.failures.push(format!(
                "{}:{}: new service import {} -> {} exceeds the architecture baseline",
                edge.path, edge.line, edge.source, edge.target
            ));
        }
    }
}

fn compare_broad_mutable_signatures(
    baseline: &ArchitectureBaseline,
    report: &mut ArchitectureReport,
) {
    let old = baseline
        .broad_mutable_signatures
        .iter()
        .map(function_key)
        .collect::<BTreeSet<_>>();
    for signature in &report.metrics.broad_mutable_signatures {
        if !old.contains(&function_key(signature)) {
            report.failures.push(format!(
                "{}:{}: new broad mutable world function `{}` exceeds the architecture baseline",
                signature.path, signature.line, signature.name
            ));
        }
    }
}

fn compare_player_state_usages(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    let old = baseline
        .player_state_usages
        .iter()
        .map(player_state_usage_key)
        .collect::<BTreeSet<_>>();
    for usage in &report.metrics.player_state_usages {
        if !old.contains(&player_state_usage_key(usage)) {
            report.failures.push(format!(
                "{}:{}: new direct PlayerState usage exceeds the architecture baseline",
                usage.path, usage.line
            ));
        }
    }
}

fn compare_player_state_field_writes(
    baseline: &ArchitectureBaseline,
    report: &mut ArchitectureReport,
) {
    let old = baseline
        .player_state_field_writes
        .iter()
        .map(player_state_field_write_key)
        .collect::<BTreeSet<_>>();
    for write in &report.metrics.player_state_field_writes {
        if !old.contains(&player_state_field_write_key(write)) {
            report.failures.push(format!(
                "{}:{}: new direct PlayerState.{} write exceeds the architecture baseline",
                write.path, write.line, write.field
            ));
        }
    }
}

fn compare_entity_field_writes(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    let old = baseline
        .entity_field_writes
        .iter()
        .map(entity_field_write_key)
        .collect::<BTreeSet<_>>();
    for write in &report.metrics.entity_field_writes {
        if !old.contains(&entity_field_write_key(write)) {
            report.failures.push(format!(
                "{}:{}: new direct Entity.{} write exceeds the architecture baseline",
                write.path, write.line, write.field
            ));
        }
    }
}

fn compare_public_export_counts(baseline: &ArchitectureBaseline, report: &mut ArchitectureReport) {
    let old = baseline
        .public_export_counts
        .iter()
        .map(|entry| (entry.path.as_str(), entry.count))
        .collect::<BTreeMap<_, _>>();
    for current in public_export_counts(&report.metrics) {
        match old.get(current.path.as_str()).copied() {
            Some(old_count) => {
                let budget = old_count + PUBLIC_EXPORT_GROWTH_BUFFER;
                if current.count > budget {
                    report.failures.push(format!(
                        "{}: public exports grew to {}; baseline is {} with budget {}. Run --bless with a reason if this API growth is intentional.",
                        current.path, current.count, old_count, budget
                    ));
                } else if current.count < old_count {
                    report.ratchet_notes.push(format!(
                        "{} public exports shrank from {} to {}; --bless would lower the future budget",
                        current.path, old_count, current.count
                    ));
                }
            }
            None if current.count > NEW_MODULE_PUBLIC_EXPORT_BUDGET => {
                report.failures.push(format!(
                    "{}: new module has {} public exports, above the unbaselined budget of {}",
                    current.path, current.count, NEW_MODULE_PUBLIC_EXPORT_BUDGET
                ));
            }
            None => {
                report.ratchet_notes.push(format!(
                    "{} is new with {} public exports; --bless would start tracking it",
                    current.path, current.count
                ));
            }
        }
    }
}

fn public_export_counts(metrics: &ArchitectureMetrics) -> Vec<PublicExportCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for export in &metrics.public_exports {
        *counts.entry(export.path.clone()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(path, count)| PublicExportCount { path, count })
        .collect()
}

fn baseline_change_summary(
    previous: &ArchitectureBaseline,
    current: &ArchitectureBaseline,
) -> Vec<String> {
    let mut summary = Vec::new();
    summarize_count_change(
        "line-count entries",
        previous.line_counts.len(),
        current.line_counts.len(),
        &mut summary,
    );
    summarize_count_change(
        "service import edges",
        previous.service_edges.len(),
        current.service_edges.len(),
        &mut summary,
    );
    summarize_count_change(
        "broad mutable functions",
        previous.broad_mutable_signatures.len(),
        current.broad_mutable_signatures.len(),
        &mut summary,
    );
    summarize_count_change(
        "PlayerState usage sites",
        previous.player_state_usages.len(),
        current.player_state_usages.len(),
        &mut summary,
    );
    summarize_count_change(
        "PlayerState field write sites",
        previous.player_state_field_writes.len(),
        current.player_state_field_writes.len(),
        &mut summary,
    );
    summarize_count_change(
        "Entity field write sites",
        previous.entity_field_writes.len(),
        current.entity_field_writes.len(),
        &mut summary,
    );
    summarize_public_export_change(previous, current, &mut summary);
    if summary.is_empty() {
        summary.push("baseline values unchanged; reason updated".to_string());
    }
    summary
}

fn summarize_count_change(label: &str, old: usize, new: usize, summary: &mut Vec<String>) {
    if old != new {
        summary.push(format!("{label}: {old} -> {new}"));
    }
}

fn summarize_public_export_change(
    previous: &ArchitectureBaseline,
    current: &ArchitectureBaseline,
    summary: &mut Vec<String>,
) {
    let old_total: usize = previous
        .public_export_counts
        .iter()
        .map(|entry| entry.count)
        .sum();
    let new_total: usize = current
        .public_export_counts
        .iter()
        .map(|entry| entry.count)
        .sum();
    summarize_count_change("public exports", old_total, new_total, summary);
}

fn service_edge_key(edge: &ServiceEdge) -> (String, String, String) {
    (edge.source.clone(), edge.target.clone(), edge.path.clone())
}

fn function_key(signature: &FunctionSignature) -> (String, String) {
    (signature.path.clone(), signature.name.clone())
}

fn player_state_usage_key(usage: &PlayerStateUsage) -> (String, String) {
    (usage.path.clone(), usage.code.clone())
}

fn player_state_field_write_key(write: &PlayerStateFieldWrite) -> (String, String, String) {
    (write.path.clone(), write.field.clone(), write.code.clone())
}

fn entity_field_write_key(write: &EntityFieldWrite) -> (String, String, String) {
    (write.path.clone(), write.field.clone(), write.code.clone())
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

fn service_roles() -> BTreeMap<&'static str, ServiceRole> {
    SERVICE_ROLES.iter().copied().collect()
}

fn check_service_roles(
    service_modules: &BTreeSet<String>,
    roles: &BTreeMap<&str, ServiceRole>,
    report: &mut ArchitectureReport,
) {
    for module in service_modules {
        if !roles.contains_key(module.as_str()) {
            report.failures.push(format!(
                "services/{module}: service module must be classified in SERVICE_ROLES before it can participate in dependency checks"
            ));
        }
    }
}

fn check_service_imports(
    file: &SourceFile,
    use_statements: &[UseStatement],
    service_modules: &BTreeSet<String>,
    allowed: &BTreeMap<&str, BTreeSet<&str>>,
    roles: &BTreeMap<&str, ServiceRole>,
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
            if let Some(reason) =
                service_role_edge_rejection(source.as_str(), target.as_str(), roles)
            {
                report.failures.push(format!(
                    "{}:{}: service module {source} must not import services::{target}: {reason}",
                    file.rel_path, statement.line
                ));
            }
        }
    }
}

fn check_pure_policy_imports(
    file: &SourceFile,
    use_statements: &[UseStatement],
    roles: &BTreeMap<&str, ServiceRole>,
    report: &mut ArchitectureReport,
) {
    if role_for_file(file, roles) != Some(ServiceRole::PurePolicy) {
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

fn check_role_state_boundaries(
    file: &SourceFile,
    text: &str,
    roles: &BTreeMap<&str, ServiceRole>,
    report: &mut ArchitectureReport,
) {
    let Some(role) = role_for_file(file, roles) else {
        return;
    };

    for signature in collect_function_signatures(text) {
        let compact = signature.text.split_whitespace().collect::<String>();
        if role == ServiceRole::PurePolicy && signature_accepts_mutable_world_state(&compact) {
            report.failures.push(format!(
                "{}:{}: pure-policy module must not expose mutable world state; use facts-in, decisions-out planning instead",
                file.rel_path, signature.line
            ));
        }
        if role == ServiceRole::QueryIndex
            && (compact.contains("&mutEntityStore") || compact.contains("&mut[PlayerState]"))
        {
            report.failures.push(format!(
                "{}:{}: query/index service may read world state but must not accept mutable EntityStore or PlayerState",
                file.rel_path, signature.line
            ));
        }
    }
}

fn role_for_file(file: &SourceFile, roles: &BTreeMap<&str, ServiceRole>) -> Option<ServiceRole> {
    let module = service_module_for_path(&file.rel_path)?;
    roles.get(module.as_str()).copied()
}

fn service_role_edge_rejection(
    source: &str,
    target: &str,
    roles: &BTreeMap<&str, ServiceRole>,
) -> Option<String> {
    let source_role = roles.get(source).copied()?;
    let target_role = roles.get(target).copied()?;
    if GRANDFATHERED_BROAD_ADAPTERS.contains(&source) {
        return None;
    }
    if service_role_edge_allowed(source, target, source_role, target_role) {
        return None;
    }
    Some(format!(
        "{} -> {} edges are forbidden by the service role matrix; route orchestration through systems.rs or introduce facts/plans plus a narrow executor",
        source_role.label(),
        target_role.label()
    ))
}

fn signature_accepts_mutable_world_state(compact_signature: &str) -> bool {
    [
        "&mutEntityStore",
        "&mut[PlayerState]",
        "&mutFog",
        "&mutMoveCoordinator",
        "&mutSmokeCloudStore",
    ]
    .iter()
    .any(|needle| compact_signature.contains(needle))
}

fn service_role_edge_allowed(
    source: &str,
    target: &str,
    source_role: ServiceRole,
    target_role: ServiceRole,
) -> bool {
    if ROLE_EDGE_ALLOWLIST.contains(&(source, target)) {
        return true;
    }
    if source_role == ServiceRole::CommandAdapter
        && target_role == ServiceRole::CommandAdapter
        && GRANDFATHERED_BROAD_ADAPTERS.contains(&source)
    {
        return true;
    }

    matches!(
        (source_role, target_role),
        (ServiceRole::TickSystem, ServiceRole::PurePolicy)
            | (ServiceRole::TickSystem, ServiceRole::QueryIndex)
            | (ServiceRole::TickSystem, ServiceRole::MutationHelper)
            | (ServiceRole::CommandAdapter, ServiceRole::PurePolicy)
            | (ServiceRole::CommandAdapter, ServiceRole::QueryIndex)
            | (ServiceRole::CommandAdapter, ServiceRole::MutationHelper)
            | (ServiceRole::PurePolicy, ServiceRole::PurePolicy)
            | (ServiceRole::QueryIndex, ServiceRole::PurePolicy)
            | (ServiceRole::QueryIndex, ServiceRole::QueryIndex)
            | (ServiceRole::MutationHelper, ServiceRole::PurePolicy)
            | (ServiceRole::MutationHelper, ServiceRole::QueryIndex)
    )
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

fn collect_player_state_usages(file: &SourceFile, text: &str, report: &mut ArchitectureReport) {
    for (index, line) in text.lines().enumerate() {
        let code = code_before_comment(line);
        if code
            .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
            .any(|part| part == "PlayerState")
        {
            report.metrics.player_state_usages.push(PlayerStateUsage {
                path: file.rel_path.clone(),
                line: index + 1,
                code: normalized_code(code),
            });
        }
    }
}

fn collect_player_state_field_writes(
    file: &SourceFile,
    text: &str,
    report: &mut ArchitectureReport,
) {
    if PLAYER_STATE_FIELD_WRITE_APPROVED_PATHS.contains(&file.rel_path.as_str()) {
        return;
    }

    for (index, line) in text.lines().enumerate() {
        let code = code_before_comment(line);
        for field in PLAYER_STATE_FIELDS {
            if contains_field_assignment(code, field) {
                report
                    .metrics
                    .player_state_field_writes
                    .push(PlayerStateFieldWrite {
                        path: file.rel_path.clone(),
                        line: index + 1,
                        field: (*field).to_string(),
                        code: normalized_code(code),
                    });
            }
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
        let code = code_before_comment(line);
        for field in ENTITY_FIELDS {
            if contains_field_assignment(code, field) {
                report.metrics.entity_field_writes.push(EntityFieldWrite {
                    path: file.rel_path.clone(),
                    line: index + 1,
                    field: (*field).to_string(),
                    code: normalized_code(code),
                });
            }
        }
    }
}

fn code_before_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or_default()
}

fn normalized_code(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
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
    if before.is_some_and(|ch| ch == '.') {
        return false;
    }
    let after = &line[start + needle.len()..];
    let after = after.trim_start();
    starts_with_assignment_operator(after)
}

fn starts_with_assignment_operator(text: &str) -> bool {
    if text.starts_with("==") || text.starts_with("=>") {
        return false;
    }
    text.starts_with('=')
        || text.starts_with("+=")
        || text.starts_with("-=")
        || text.starts_with("*=")
        || text.starts_with("/=")
        || text.starts_with("%=")
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

    fn baseline(reason: &str, metrics: &ArchitectureMetrics) -> ArchitectureBaseline {
        ArchitectureBaseline::from_metrics(reason, metrics)
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
    fn role_allowed_service_import_passes_when_allowlisted() {
        let report = analyze_source_files(&[
            source(
                "services/production.rs",
                "use crate::game::services::standability;\n",
            ),
            source("services/standability.rs", ""),
        ]);

        assert!(report.failures.is_empty());
        assert_eq!(
            report.metrics.service_edges,
            vec![ServiceEdge {
                source: "production".to_string(),
                target: "standability".to_string(),
                path: "services/production.rs".to_string(),
                line: 1,
            }]
        );
    }

    #[test]
    fn edge_forbidden_by_role_matrix_fails_with_reason() {
        let report = analyze_source_files(&[
            source(
                "services/production.rs",
                "use crate::game::services::combat;\n",
            ),
            source("services/combat/mod.rs", ""),
        ]);

        assert_eq!(
            report.failures,
            vec![
                "services/production.rs:1: service module production must not import services::combat without updating the architecture allowlist".to_string(),
                "services/production.rs:1: service module production must not import services::combat: tick system -> tick system edges are forbidden by the service role matrix; route orchestration through systems.rs or introduce facts/plans plus a narrow executor".to_string(),
            ]
        );
    }

    #[test]
    fn new_service_module_must_be_classified() {
        let report = analyze_source_files(&[source("services/new_policy.rs", "")]);

        assert_eq!(
            report.failures,
            vec![
                "services/new_policy: service module must be classified in SERVICE_ROLES before it can participate in dependency checks".to_string()
            ]
        );
    }

    #[test]
    fn query_index_module_accepting_mutable_world_state_fails() {
        let report = analyze_source_files(&[source(
            "services/world_query.rs",
            "pub(crate) fn mutate(entities: &mut EntityStore) {}\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/world_query.rs:1: query/index service may read world state but must not accept mutable EntityStore or PlayerState".to_string()
            ]
        );
    }

    #[test]
    fn pure_policy_module_accepting_mutable_inputs_fails() {
        let report = analyze_source_files(&[source(
            "services/order_planner.rs",
            "pub(crate) fn plan(entities: &mut EntityStore) {}\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/order_planner.rs:1: pure-policy module must not expose mutable world state; use facts-in, decisions-out planning instead".to_string()
            ]
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
    fn direct_player_state_usages_are_recorded() {
        let report = analyze_source_files(&[source(
            "services/example.rs",
            "use crate::game::PlayerState;\nfn update(player: &mut PlayerState) {}\n",
        )]);

        assert_eq!(report.metrics.player_state_usages.len(), 2);
        assert!(report
            .metrics
            .player_state_usages
            .contains(&PlayerStateUsage {
                path: "services/example.rs".to_string(),
                line: 2,
                code: "fn update(player: &mut PlayerState) {}".to_string(),
            }));
    }

    #[test]
    fn direct_player_state_field_writes_are_recorded() {
        let report = analyze_source_files(&[source(
            "services/example.rs",
            "fn update(player: &mut PlayerState) { player.steel = 0; player.supply_used += 1; }\n",
        )]);

        assert_eq!(
            report.metrics.player_state_field_writes,
            vec![
                PlayerStateFieldWrite {
                    path: "services/example.rs".to_string(),
                    line: 1,
                    field: "steel".to_string(),
                    code: "fn update(player: &mut PlayerState) { player.steel = 0; player.supply_used += 1; }".to_string(),
                },
                PlayerStateFieldWrite {
                    path: "services/example.rs".to_string(),
                    line: 1,
                    field: "supply_used".to_string(),
                    code: "fn update(player: &mut PlayerState) { player.steel = 0; player.supply_used += 1; }".to_string(),
                },
            ]
        );
    }

    #[test]
    fn line_growth_has_a_generous_buffer_before_failing() {
        let original = analyze_source_files(&[source("services/commands.rs", "line\n")]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut within_budget =
            analyze_source_files(&[source("services/commands.rs", &"line\n".repeat(81))]);
        compare_to_baseline(&baseline, &mut within_budget);
        assert!(within_budget.failures.is_empty());

        let mut over_budget =
            analyze_source_files(&[source("services/commands.rs", &"line\n".repeat(82))]);
        compare_to_baseline(&baseline, &mut over_budget);
        assert_eq!(over_budget.failures.len(), 1);
        assert!(over_budget.failures[0].contains("line count grew"));
    }

    #[test]
    fn test_files_get_a_larger_line_growth_buffer() {
        let original = analyze_source_files(&[source("services/tests.rs", "line\n")]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut report =
            analyze_source_files(&[source("services/tests.rs", &"line\n".repeat(251))]);

        compare_to_baseline(&baseline, &mut report);

        assert!(report.failures.is_empty());
    }

    #[test]
    fn shrinking_a_file_suggests_lowering_the_future_budget() {
        let original = analyze_source_files(&[source("services/commands.rs", &"line\n".repeat(5))]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut report = analyze_source_files(&[source("services/commands.rs", "line\n")]);

        compare_to_baseline(&baseline, &mut report);

        assert!(report.failures.is_empty());
        assert_eq!(
            report.ratchet_notes,
            vec![
                "services/commands.rs shrank from 5 to 1 lines; --bless would lower the future budget"
                    .to_string()
            ]
        );
    }

    #[test]
    fn new_player_state_usage_exceeds_the_baseline_without_line_number_noise() {
        let original = analyze_source_files(&[source(
            "services/commands.rs",
            "fn update(player: &mut PlayerState) {}\n",
        )]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut same_usage_moved = analyze_source_files(&[source(
            "services/commands.rs",
            "\n\nfn update(player: &mut PlayerState) {}\n",
        )]);

        compare_to_baseline(&baseline, &mut same_usage_moved);

        assert!(same_usage_moved.failures.is_empty());

        let mut new_usage = analyze_source_files(&[source(
            "services/commands.rs",
            "fn update(player: &mut PlayerState) {}\nfn reset(player: PlayerState) {}\n",
        )]);
        compare_to_baseline(&baseline, &mut new_usage);
        assert_eq!(new_usage.failures.len(), 1);
        assert!(new_usage.failures[0].contains("new direct PlayerState usage"));
    }

    #[test]
    fn new_player_state_field_write_exceeds_the_baseline() {
        let original = analyze_source_files(&[source(
            "services/commands.rs",
            "fn update(player: &mut P) { player.steel = 0; }\n",
        )]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut same_write_moved = analyze_source_files(&[source(
            "services/commands.rs",
            "\n\nfn update(player: &mut P) { player.steel = 0; }\n",
        )]);

        compare_to_baseline(&baseline, &mut same_write_moved);

        assert!(same_write_moved.failures.is_empty());

        let mut new_write = analyze_source_files(&[source(
            "services/commands.rs",
            "fn update(player: &mut P) { player.steel = 0; }\nfn reset(player: &mut P) { player.oil += 1; }\n",
        )]);
        compare_to_baseline(&baseline, &mut new_write);
        assert_eq!(new_write.failures.len(), 1);
        assert!(new_write.failures[0].contains("new direct PlayerState.oil write"));
    }

    #[test]
    fn public_exports_have_a_small_growth_buffer() {
        let original = analyze_source_files(&[source("services/commands.rs", "pub fn a() {}\n")]);
        let baseline = baseline("test baseline", &original.metrics);
        let mut within_budget = analyze_source_files(&[source(
            "services/commands.rs",
            "pub fn a() {}\npub fn b() {}\npub fn c() {}\n",
        )]);
        compare_to_baseline(&baseline, &mut within_budget);
        assert!(within_budget.failures.is_empty());

        let mut over_budget = analyze_source_files(&[source(
            "services/commands.rs",
            "pub fn a() {}\npub fn b() {}\npub fn c() {}\npub fn d() {}\n",
        )]);
        compare_to_baseline(&baseline, &mut over_budget);
        assert_eq!(over_budget.failures.len(), 1);
        assert!(over_budget.failures[0].contains("public exports grew"));
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
