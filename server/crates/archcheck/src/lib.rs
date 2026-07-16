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
const STATE_REGISTRY_DOC: &str = "docs/design/server-sim.md";
const STATE_REGISTRY_HEADING: &str = "#### 3.1.1 `Game` State Ownership Registry";
const GAME_STATE_ALLOWED_CATEGORIES: &[&str] =
    &["authoritative/serialized", "compatibility metadata"];
const DERIVED_STATE_ALLOWED_CATEGORY: &str = "derived/rebuildable";

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
    ("entrenchment", ServiceRole::TickSystem),
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
    ("scout_plane", ServiceRole::MutationHelper),
    ("spatial", ServiceRole::QueryIndex),
    ("standability", ServiceRole::QueryIndex),
    ("supply", ServiceRole::TickSystem),
    ("world_query", ServiceRole::QueryIndex),
];

const ROLE_EDGE_ALLOWLIST: &[(&str, &str)] = &[
    // Ability execution still reuses command notice constructors. New command families should
    // prefer facts -> pure plan -> narrow executor instead of adding more adapter back-edges.
    ("ability_orders", "commands"),
    // Ability execution may delegate movement/path staging through the coordinator boundary.
    ("ability_orders", "move_coordinator"),
    // Combat uses movement's shared facing helpers while combat policy is being split out.
    ("combat", "movement"),
    // Residual command/order adapter edges into tick systems. Keep these explicit so adding another
    // broad adapter dependency requires a named exception instead of inheriting a blanket bypass.
    ("commands", "construction"),
    ("commands", "movement"),
    ("order_queue", "construction"),
    ("order_queue", "movement"),
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
    "scout_plane",
];
const PLAYER_STATE_FIELD_WRITE_APPROVED_PATHS: &[&str] = &["player_state.rs"];
const PLAYER_STATE_FIELDS: &[&str] = &["steel", "oil", "supply_used", "score"];

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
            "scout_plane",
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
    ("entrenchment", &["geometry", "occupancy", "standability"]),
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
            "scout_plane",
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
        &[
            "move_coordinator",
            "occupancy",
            "pathing",
            "scout_plane",
            "standability",
        ],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StateOwner {
    Game,
    GameState,
    DerivedState,
}

impl StateOwner {
    fn struct_name(self) -> &'static str {
        match self {
            StateOwner::Game => "Game",
            StateOwner::GameState => "GameState",
            StateOwner::DerivedState => "DerivedState",
        }
    }

    fn label(self) -> &'static str {
        self.struct_name()
    }

    fn expected_path(self) -> &'static str {
        match self {
            StateOwner::Game => "mod.rs",
            StateOwner::GameState => "state.rs",
            StateOwner::DerivedState => "derived_state.rs",
        }
    }
}

#[derive(Debug, Clone)]
struct StructField {
    owner: StateOwner,
    path: String,
    line: usize,
    name: String,
    ty: String,
}

#[derive(Debug, Default)]
struct StateOwnerScan {
    found_owners: BTreeSet<StateOwner>,
    fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
struct RegistryEntry {
    line: usize,
    category: String,
    checkpoint_policy: String,
    evidence: String,
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
    let registry_text = read_state_registry_doc(game_root)?;
    Ok(analyze_source_files_with_registry(
        &files,
        Some(&registry_text),
    ))
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

#[cfg(test)]
fn analyze_source_files(files: &[SourceFile]) -> ArchitectureReport {
    analyze_source_files_with_registry(files, None)
}

fn analyze_source_files_with_registry(
    files: &[SourceFile],
    registry_text: Option<&str>,
) -> ArchitectureReport {
    let mut report = ArchitectureReport::default();
    let service_modules = service_modules(files);
    let allowed_service_imports = allowed_service_imports();
    let service_roles = service_roles();

    check_service_roles(&service_modules, &service_roles, &mut report);
    let state_owner_scan = collect_state_owner_scan(files);
    check_state_owner_tree(&state_owner_scan, registry_text.is_some(), &mut report);
    check_state_owner_registry(&state_owner_scan, registry_text, &mut report);

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
        check_module_level_mutable_state(file, &stripped, &mut report);
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

fn read_state_registry_doc(game_root: &Path) -> io::Result<String> {
    let registry_path = game_root
        .ancestors()
        .map(|ancestor| ancestor.join(STATE_REGISTRY_DOC))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "could not find {STATE_REGISTRY_DOC} above game root {}",
                    game_root.display()
                ),
            )
        })?;
    fs::read_to_string(&registry_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "could not read state ownership registry {}: {error}",
                registry_path.display()
            ),
        )
    })
}

fn collect_state_owner_scan(files: &[SourceFile]) -> StateOwnerScan {
    let mut scan = StateOwnerScan::default();
    for owner in [
        StateOwner::Game,
        StateOwner::GameState,
        StateOwner::DerivedState,
    ] {
        if let Some(file) = files
            .iter()
            .find(|file| file.rel_path == owner.expected_path())
        {
            if let Some(fields) = collect_struct_fields(file, owner) {
                scan.found_owners.insert(owner);
                scan.fields.extend(fields);
            }
        }
    }
    scan
}

fn check_state_owner_tree(
    scan: &StateOwnerScan,
    require_full_tree: bool,
    report: &mut ArchitectureReport,
) {
    if require_full_tree {
        for owner in [
            StateOwner::Game,
            StateOwner::GameState,
            StateOwner::DerivedState,
        ] {
            if !scan.found_owners.contains(&owner) {
                report.failures.push(format!(
                    "{} struct must exist so state ownership can be checked",
                    owner.label()
                ));
            }
        }
    }

    let game_fields = scan
        .fields
        .iter()
        .filter(|field| field.owner == StateOwner::Game)
        .collect::<Vec<_>>();
    if game_fields.is_empty() {
        return;
    }

    let expected = BTreeMap::from([("state", "GameState"), ("derived", "DerivedState")]);
    for field in &game_fields {
        match expected.get(field.name.as_str()) {
            Some(expected_ty) if field.ty == *expected_ty => {}
            Some(expected_ty) => report.failures.push(format!(
                "{}:{}: Game.{} must have type {}; found {}",
                field.path, field.line, field.name, expected_ty, field.ty
            )),
            None => report.failures.push(format!(
                "{}:{}: Game must only store `state: GameState` and `derived: DerivedState`; move `{}` under GameState or DerivedState, or document it as room/session/test-only state outside Game",
                field.path, field.line, field.name
            )),
        }
    }

    for (name, ty) in expected {
        if !game_fields
            .iter()
            .any(|field| field.name == name && field.ty == ty)
        {
            report.failures.push(format!(
                "Game must contain `{name}: {ty}` so authoritative and derived state stay under the explicit ownership tree"
            ));
        }
    }
}

fn check_state_owner_registry(
    scan: &StateOwnerScan,
    registry_text: Option<&str>,
    report: &mut ArchitectureReport,
) {
    let Some(registry_text) = registry_text else {
        return;
    };
    let entries = parse_state_registry(registry_text, report);
    if entries.is_empty() {
        report.failures.push(format!(
            "{STATE_REGISTRY_DOC}: {STATE_REGISTRY_HEADING} must contain a field/category registry table"
        ));
        return;
    }

    let mut code_fields = BTreeMap::<String, &StructField>::new();
    for field in scan.fields.iter().filter(|field| {
        matches!(
            field.owner,
            StateOwner::GameState | StateOwner::DerivedState
        )
    }) {
        if let Some(previous) = code_fields.insert(field.name.clone(), field) {
            report.failures.push(format!(
                "{}:{}: state owner field `{}` duplicates {}:{}; field names must be unique across GameState and DerivedState for registry checks",
                field.path, field.line, field.name, previous.path, previous.line
            ));
        }
    }

    for field in code_fields.values() {
        let Some(entry) = entries.get(field.name.as_str()) else {
            report.failures.push(format!(
                "{}:{}: {}.{} is missing from {STATE_REGISTRY_DOC} {STATE_REGISTRY_HEADING}",
                field.path,
                field.line,
                field.owner.label(),
                field.name
            ));
            continue;
        };
        match field.owner {
            StateOwner::GameState => {
                if !GAME_STATE_ALLOWED_CATEGORIES.contains(&entry.category.as_str()) {
                    report.failures.push(format!(
                        "{STATE_REGISTRY_DOC}:{}: GameState.{} is categorized as `{}`; GameState fields must be authoritative/serialized or compatibility metadata",
                        entry.line, field.name, entry.category
                    ));
                }
                check_registry_checkpoint_policy(field, entry, report);
            }
            StateOwner::DerivedState => {
                if entry.category != DERIVED_STATE_ALLOWED_CATEGORY {
                    report.failures.push(format!(
                        "{STATE_REGISTRY_DOC}:{}: DerivedState.{} is categorized as `{}`; DerivedState fields must be derived/rebuildable",
                        entry.line, field.name, entry.category
                    ));
                }
                check_registry_checkpoint_policy(field, entry, report);
            }
            StateOwner::Game => {}
        }
    }

    for (name, entry) in entries {
        if !code_fields.contains_key(name.as_str()) {
            report.failures.push(format!(
                "{STATE_REGISTRY_DOC}:{}: registry field `{}` is not a current GameState or DerivedState field",
                entry.line, name
            ));
        }
    }
}

fn check_registry_checkpoint_policy(
    field: &StructField,
    entry: &RegistryEntry,
    report: &mut ArchitectureReport,
) {
    if registry_cell_unresolved(&entry.checkpoint_policy) {
        report.failures.push(format!(
            "{STATE_REGISTRY_DOC}:{}: {}.{} registry row must include a concrete checkpoint policy",
            entry.line,
            field.owner.label(),
            field.name
        ));
    }
    if registry_cell_unresolved(&entry.evidence) {
        report.failures.push(format!(
            "{STATE_REGISTRY_DOC}:{}: {}.{} registry row must include evidence and notes",
            entry.line,
            field.owner.label(),
            field.name
        ));
    }
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

fn collect_struct_fields(file: &SourceFile, owner: StateOwner) -> Option<Vec<StructField>> {
    let lines = file.text.lines().collect::<Vec<_>>();
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        let code = code_before_comment(line);
        if struct_declares(code, owner.struct_name()) {
            start = Some(index);
            break;
        }
    }
    let start = start?;

    let mut fields = Vec::new();
    let mut depth = 0isize;
    let mut inside_body = false;
    let mut cfg_test_field_pending = false;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let code = code_before_comment(line);
        if !inside_body {
            if let Some(open_index) = code.find('{') {
                inside_body = true;
                depth = 1 + brace_delta(&code[open_index + 1..]);
                if let Some(field) =
                    parse_struct_field_line(&code[open_index + 1..], file, owner, index + 1)
                {
                    fields.push(field);
                }
                if depth <= 0 {
                    break;
                }
            }
            continue;
        }

        if depth == 1 {
            let trimmed = code.trim();
            if is_cfg_test_attribute(trimmed) {
                cfg_test_field_pending = true;
            } else if cfg_test_field_pending && trimmed.starts_with("#[") {
                // Preserve the cfg(test) skip across stacked attributes on the same field.
            } else if cfg_test_field_pending {
                if !trimmed.is_empty() {
                    cfg_test_field_pending = false;
                }
            } else if let Some(field) = parse_struct_field_line(code, file, owner, index + 1) {
                fields.push(field);
            }
        }
        depth += brace_delta(code);
        if inside_body && depth <= 0 {
            break;
        }
    }
    Some(fields)
}

fn struct_declares(line: &str, struct_name: &str) -> bool {
    let parts = line
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .collect::<Vec<_>>();
    parts
        .windows(2)
        .any(|window| window[0] == "struct" && window[1] == struct_name)
}

fn parse_struct_field_line(
    line: &str,
    file: &SourceFile,
    owner: StateOwner,
    line_number: usize,
) -> Option<StructField> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with('}')
        || trimmed.starts_with("//")
    {
        return None;
    }
    let field_text = strip_visibility_prefix(trimmed);
    let colon = field_text.find(':')?;
    let before_colon = &field_text[..colon];
    let name = before_colon
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .rfind(|part| !part.is_empty())?;
    let ty = field_text[colon + 1..]
        .trim()
        .trim_end_matches(',')
        .trim()
        .to_string();
    if name == "pub" || ty.is_empty() {
        return None;
    }
    Some(StructField {
        owner,
        path: file.rel_path.clone(),
        line: line_number,
        name: name.to_string(),
        ty,
    })
}

fn parse_state_registry(
    registry_text: &str,
    report: &mut ArchitectureReport,
) -> BTreeMap<String, RegistryEntry> {
    let Some(section_lines) = state_registry_lines(registry_text) else {
        report.failures.push(format!(
            "{STATE_REGISTRY_DOC}: missing {STATE_REGISTRY_HEADING}"
        ));
        return BTreeMap::new();
    };

    let mut entries = BTreeMap::new();
    for (line_number, line) in section_lines {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect::<Vec<_>>();
        if cells.len() < 2 || cells[0] == "Field" || cells[0].starts_with("---") {
            continue;
        }
        let field = trim_code(cells[0]);
        let category = trim_code(cells[1]);
        let checkpoint_policy = cells.get(2).map(|cell| trim_code(cell)).unwrap_or_default();
        let evidence = cells.get(3).map(|cell| trim_code(cell)).unwrap_or_default();
        if field.is_empty() || category.is_empty() {
            continue;
        }
        if let Some(previous) = entries.insert(
            field.clone(),
            RegistryEntry {
                line: line_number,
                category,
                checkpoint_policy,
                evidence,
            },
        ) {
            report.failures.push(format!(
                "{STATE_REGISTRY_DOC}:{line_number}: registry field `{field}` duplicates earlier entry on line {}",
                previous.line
            ));
        }
    }
    entries
}

fn state_registry_lines(text: &str) -> Option<Vec<(usize, &str)>> {
    let mut in_section = false;
    let mut section = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim() == STATE_REGISTRY_HEADING {
            in_section = true;
        } else if in_section && line.starts_with("###") {
            break;
        }
        if in_section {
            section.push((index + 1, line));
        }
    }
    in_section.then_some(section)
}

fn trim_code(text: &str) -> String {
    text.trim().trim_matches('`').trim().to_string()
}

fn registry_cell_unresolved(text: &str) -> bool {
    let normalized = text.trim().trim_matches('`').trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "" | "-" | "todo" | "tbd" | "n/a" | "na" | "none" | "unresolved"
    )
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

fn check_module_level_mutable_state(
    file: &SourceFile,
    text: &str,
    report: &mut ArchitectureReport,
) {
    if is_test_path(&file.rel_path) {
        return;
    }

    let mut cfg_test_item_pending = false;
    for (index, line) in text.lines().enumerate() {
        let code = code_before_comment(line).trim();
        if is_cfg_test_attribute(code) {
            cfg_test_item_pending = true;
        } else if cfg_test_item_pending && code.starts_with("#[") {
            // Preserve the cfg(test) skip across stacked attributes on the same item.
        } else {
            if module_level_mutable_state_decl(code) && !cfg_test_item_pending {
                report.failures.push(format!(
                    "{}:{}: module-level mutable simulation state is not allowed; store durable state under GameState, rebuildable state under DerivedState, or keep room/session/test-only state outside rts-sim",
                    file.rel_path,
                    index + 1
                ));
            }
            if !code.is_empty() {
                cfg_test_item_pending = false;
            }
        }
    }
}

fn module_level_mutable_state_decl(line: &str) -> bool {
    let line = strip_visibility_prefix(line.trim_start());
    if line.starts_with("static mut ") {
        return true;
    }
    if line.starts_with("static ") {
        return ["Mutex<", "RwLock<", "OnceLock<", "LazyLock<"]
            .iter()
            .any(|needle| line.contains(needle));
    }
    line.starts_with("thread_local!")
}

fn is_cfg_test_attribute(line: &str) -> bool {
    line.starts_with("#[cfg(test")
}

fn strip_visibility_prefix(line: &str) -> &str {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("pub ") {
        return rest.trim_start();
    }
    if let Some(after_pub) = line.strip_prefix("pub(") {
        if let Some(close) = after_pub.find(')') {
            return after_pub[close + 1..].trim_start();
        }
    }
    line
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

    fn registry(rows: &[(&str, &str)]) -> String {
        let mut text = format!(
            "{STATE_REGISTRY_HEADING}\n\n| Field | Category | Checkpoint policy | Evidence and notes |\n| --- | --- | --- | --- |\n"
        );
        for (field, category) in rows {
            text.push_str(&format!(
                "| `{field}` | `{category}` | test policy | test evidence |\n"
            ));
        }
        text
    }

    fn minimal_state_tree_sources(
        game_state_fields: &str,
        derived_state_fields: &str,
    ) -> Vec<SourceFile> {
        vec![
            source(
                "mod.rs",
                "pub struct Game {\n    pub(in crate::game) state: GameState,\n    pub(in crate::game) derived: DerivedState,\n}\n",
            ),
            source(
                "state.rs",
                &format!("pub(in crate::game) struct GameState {{\n{game_state_fields}\n}}\n"),
            ),
            source(
                "derived_state.rs",
                &format!(
                    "pub(in crate::game) struct DerivedState {{\n{derived_state_fields}\n}}\n"
                ),
            ),
        ]
    }

    #[test]
    fn game_must_only_store_explicit_state_tree_roots() {
        let report = analyze_source_files(&[source(
            "mod.rs",
            "pub struct Game {\n    pub(in crate::game) state: GameState,\n    pub(in crate::game) derived: DerivedState,\n    pub(in crate::game) hidden_cache: Vec<u32>,\n}\n",
        )]);

        assert!(report.failures.iter().any(|failure| failure
            .contains("Game must only store `state: GameState` and `derived: DerivedState`")));
    }

    #[test]
    fn state_registry_requires_every_game_state_and_derived_state_field() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = registry(&[("map", "authoritative/serialized")]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.iter().any(|failure| failure
            .contains("DerivedState.final_spatial is missing from docs/design/server-sim.md")));
    }

    #[test]
    fn state_registry_accepts_private_visibility_fields() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = registry(&[
            ("map", "authoritative/serialized"),
            ("final_spatial", "derived/rebuildable"),
        ]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.is_empty());
    }

    #[test]
    fn state_owner_scan_ignores_test_shadow_structs() {
        let mut files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        files.push(source(
            "tests/game_tests.rs",
            "struct Game { hidden_cache: Vec<u32> }\nstruct GameState { test_cache: Vec<u32> }\n",
        ));
        let doc = registry(&[
            ("map", "authoritative/serialized"),
            ("final_spatial", "derived/rebuildable"),
        ]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.is_empty());
    }

    #[test]
    fn state_registry_ignores_cfg_test_owner_fields() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,\n    #[cfg(test)]\n    debug_cache: Vec<u32>,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = registry(&[
            ("map", "authoritative/serialized"),
            ("final_spatial", "derived/rebuildable"),
        ]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.is_empty());
    }

    #[test]
    fn state_registry_rejects_wrong_derived_state_category() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = registry(&[
            ("map", "authoritative/serialized"),
            ("final_spatial", "authoritative/serialized"),
        ]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.iter().any(|failure| failure.contains(
            "DerivedState.final_spatial is categorized as `authoritative/serialized`; DerivedState fields must be derived/rebuildable"
        )));
    }

    #[test]
    fn state_registry_requires_checkpoint_policy_and_evidence() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = format!(
            "{STATE_REGISTRY_HEADING}\n\n\
             | Field | Category | Checkpoint policy | Evidence and notes |\n\
             | --- | --- | --- | --- |\n\
             | `map` | `authoritative/serialized` |  | test evidence |\n\
             | `final_spatial` | `derived/rebuildable` | test policy |  |\n"
        );

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report.failures.iter().any(|failure| failure
            .contains("GameState.map registry row must include a concrete checkpoint policy")));
        assert!(report.failures.iter().any(|failure| failure
            .contains("DerivedState.final_spatial registry row must include evidence and notes")));
    }

    #[test]
    fn state_registry_rejects_stale_entries() {
        let files = minimal_state_tree_sources(
            "    pub(in crate::game) map: Map,",
            "    final_spatial: SpatialIndex,",
        );
        let doc = registry(&[
            ("map", "authoritative/serialized"),
            ("final_spatial", "derived/rebuildable"),
            ("old_cache", "derived/rebuildable"),
        ]);

        let report = analyze_source_files_with_registry(&files, Some(&doc));

        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("registry field `old_cache` is not a current")));
    }

    #[test]
    fn module_level_mutable_state_fails() {
        let report = analyze_source_files(&[source(
            "services/pathing.rs",
            "static CACHE: Mutex<u32> = Mutex::new(0);\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/pathing.rs:1: module-level mutable simulation state is not allowed; store durable state under GameState, rebuildable state under DerivedState, or keep room/session/test-only state outside rts-sim".to_string()
            ]
        );
    }

    #[test]
    fn module_level_mutable_state_fails_inside_inline_modules() {
        let report = analyze_source_files(&[source(
            "services/pathing.rs",
            "mod hidden {\n    static CACHE: Mutex<u32> = Mutex::new(0);\n}\n",
        )]);

        assert_eq!(
            report.failures,
            vec![
                "services/pathing.rs:2: module-level mutable simulation state is not allowed; store durable state under GameState, rebuildable state under DerivedState, or keep room/session/test-only state outside rts-sim".to_string()
            ]
        );
    }

    #[test]
    fn module_level_mutable_state_ignores_test_files() {
        let report = analyze_source_files(&[source(
            "tests/pathing_cache_tests.rs",
            "static CACHE: Mutex<u32> = Mutex::new(0);\n",
        )]);

        assert!(report.failures.is_empty());
    }

    #[test]
    fn module_level_mutable_state_ignores_cfg_test_items() {
        let report = analyze_source_files(&[source(
            "services/pathing.rs",
            "#[cfg(test)]\n#[allow(dead_code)]\nstatic CACHE: Mutex<u32> = Mutex::new(0);\n",
        )]);

        assert!(report.failures.is_empty());
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
            vec![
                "services/commands.rs:1: service module commands must not import services::death without updating the architecture allowlist".to_string(),
                "services/commands.rs:1: service module commands must not import services::death: command adapter -> tick system edges are forbidden by the service role matrix; route orchestration through systems.rs or introduce facts/plans plus a narrow executor".to_string(),
            ]
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
    fn residual_command_adapter_tick_edges_need_named_role_exceptions() {
        let report = analyze_source_files(&[
            source(
                "services/commands.rs",
                "use crate::game::services::construction;\n",
            ),
            source("services/construction.rs", ""),
        ]);

        assert!(report.failures.is_empty());
        assert_eq!(
            report.metrics.service_edges,
            vec![ServiceEdge {
                source: "commands".to_string(),
                target: "construction".to_string(),
                path: "services/commands.rs".to_string(),
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
