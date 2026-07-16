//! Offline SVG renderer for AI static map-analysis diagnostics.
//!
//! This is deliberately a developer tool: it loads the authored map through the same simulation
//! map loader used by live games, runs the current AI map analysis, then draws the observer
//! diagnostics primitives over terrain so route-analysis bugs are visible without a browser lobby.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process;

use rts_protocol::{ObserverMapAnalysisLayer, ObserverMapAnalysisPrimitive};
use rts_sim::game::map::{Map, CURRENT_MAP_VERSION};
use rts_sim::game::{Game, MapMetadata, PlayerInit};
use rts_sim::protocol::{MapInfo, StartPayload};

use crate::ai_core::map_analysis::{AiMapAnalysis, AiMapAnalysisDebugSnapshot};

const DEFAULT_MAP: &str = "Chokes";
const DEFAULT_PLAYERS: u32 = 2;
const DEFAULT_SEED: u32 = 0x1234_5678;
const DEFAULT_TILE_PX: u32 = 7;
const HEADER_PX: u32 = 34;
const PASSABLE_FILL: &str = "#223629";
const BLOCKED_FILL: &str = "#11151b";
const GRID_STROKE: &str = "#26323a";
const TEXT_FILL: &str = "#f4ead2";
const TEXT_HALO: &str = "#080a0d";

#[derive(Clone, Debug)]
struct CliConfig {
    map_name: String,
    players: u32,
    seed: u32,
    out: PathBuf,
    tile_px: u32,
    layers: LayerSelection,
    show_grid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LayerSelection {
    All,
    Only(BTreeSet<String>),
}

pub fn run_from_env() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };

    match render_map_analysis_svg(&config) {
        Ok(report) => {
            println!(
                "AI map analysis: map={} players={} seed={} chokes={}",
                report.map_name, report.players, report.seed, report.debug.choke_count
            );
            println!("svg: {}", report.out.display());
        }
        Err(err) => {
            eprintln!("ai-map-analysis-debug failed: {err}");
            process::exit(1);
        }
    }
}

fn parse_args_or_exit() -> Option<CliConfig> {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            print_usage();
            process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let mut map_name = DEFAULT_MAP.to_string();
    let mut players = DEFAULT_PLAYERS;
    let mut seed = DEFAULT_SEED;
    let mut out: Option<PathBuf> = None;
    let mut tile_px = DEFAULT_TILE_PX;
    let mut layers = LayerSelection::All;
    let mut show_grid = true;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(None);
            }
            "--map" => {
                map_name = required_value(&arg, &mut args)?;
            }
            "--players" => {
                players = parse_u32_flag(&arg, &mut args)?;
            }
            "--seed" => {
                seed = parse_seed_flag(&arg, &mut args)?;
            }
            "--out" => {
                out = Some(PathBuf::from(required_value(&arg, &mut args)?));
            }
            "--tile-px" => {
                tile_px = parse_u32_flag(&arg, &mut args)?;
            }
            "--layers" => {
                layers = parse_layers(&required_value(&arg, &mut args)?)?;
            }
            "--no-grid" => {
                show_grid = false;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if players == 0 {
        return Err("--players must be greater than zero".to_string());
    }
    if tile_px == 0 {
        return Err("--tile-px must be greater than zero".to_string());
    }

    let out = out.unwrap_or_else(|| default_out_path(&map_name, seed, &layers));
    Ok(Some(CliConfig {
        map_name,
        players,
        seed,
        out,
        tile_px,
        layers,
        show_grid,
    }))
}

fn print_usage() {
    eprintln!(
        "usage: cargo run --manifest-path server/Cargo.toml -p rts-ai --bin ai-map-analysis-debug -- [flags]\n\
         \n\
         flags:\n\
           --map <name>        map name understood by Map::load_for_players (default: Chokes)\n\
           --players <n>       active player count used for slot selection (default: 2)\n\
           --seed <n|0xhex>    deterministic map seed (default: 0x12345678)\n\
           --out <path>        SVG output path (default: /tmp/rts-map-analysis/...svg)\n\
           --tile-px <n>       rendered pixels per map tile (default: 7)\n\
           --layers <list>     all, or comma-list of chokes,bases,resources\n\
           --no-grid           omit tile grid lines"
    );
}

#[derive(Debug)]
struct RenderReport {
    map_name: String,
    players: u32,
    seed: u32,
    out: PathBuf,
    debug: AiMapAnalysisDebugSnapshot,
}

fn render_map_analysis_svg(config: &CliConfig) -> Result<RenderReport, String> {
    let start = start_payload_for_map(&config.map_name, config.players, config.seed)?;
    let analysis = AiMapAnalysis::analyze(&start);
    let debug = analysis.debug_snapshot();
    let overlay = analysis.debug_overlay();
    let svg = render_svg(config, &start, &debug, &overlay.layers)?;

    if let Some(parent) = config
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(&config.out, svg).map_err(|err| err.to_string())?;

    Ok(RenderReport {
        map_name: config.map_name.clone(),
        players: config.players,
        seed: config.seed,
        out: config.out.clone(),
        debug,
    })
}

fn start_payload_for_map(map_name: &str, players: u32, seed: u32) -> Result<StartPayload, String> {
    let player_inits = player_inits(players);
    let slots: Vec<_> = player_inits
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players(map_name, &slots, seed)?;
    let metadata = Map::metadata_for_name(map_name).unwrap_or_else(|_| MapMetadata {
        name: map_name.to_string(),
        schema_version: CURRENT_MAP_VERSION,
        content_hash: "debug".to_string(),
    });
    let game =
        Game::new_with_random_ai_profiles_and_map_metadata(&player_inits, seed, map, metadata);
    Ok(game.start_payload())
}

fn player_inits(players: u32) -> Vec<PlayerInit> {
    (1..=players)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("P{id}"),
            color: player_color(id),
            is_ai: true,
        })
        .collect()
}

fn render_svg(
    config: &CliConfig,
    start: &StartPayload,
    debug: &AiMapAnalysisDebugSnapshot,
    layers: &[ObserverMapAnalysisLayer],
) -> Result<String, String> {
    let map = &start.map;
    let tile_px = config.tile_px;
    let map_w = map
        .width
        .checked_mul(tile_px)
        .ok_or_else(|| "render width overflow".to_string())?;
    let map_h = map
        .height
        .checked_mul(tile_px)
        .ok_or_else(|| "render height overflow".to_string())?;
    let svg_h = map_h
        .checked_add(HEADER_PX)
        .ok_or_else(|| "render height overflow".to_string())?;

    let mut out = String::new();
    writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {map_w} {svg_h}" width="{map_w}" height="{svg_h}" shape-rendering="crispEdges">"#
    )
    .unwrap();
    writeln!(
        out,
        r##"<rect width="100%" height="100%" fill="#0b0d10"/>"##
    )
    .unwrap();
    writeln!(
        out,
        r#"<text x="8" y="22" fill="{TEXT_FILL}" font-family="monospace" font-size="16" font-weight="700">{} players={} seed={} chokes={} passable={} blocked={}</text>"#,
        escape_xml(&config.map_name),
        config.players,
        config.seed,
        debug.choke_count,
        debug.passable_tiles,
        debug.blocked_tiles
    )
    .unwrap();
    writeln!(out, r#"<g transform="translate(0 {HEADER_PX})">"#).unwrap();
    render_terrain(&mut out, map, tile_px, config.show_grid);
    for layer in layers {
        if !config.layers.includes(&layer.id) {
            continue;
        }
        render_layer(&mut out, layer, map.tile_size, tile_px);
    }
    writeln!(out, "</g>").unwrap();
    writeln!(out, "</svg>").unwrap();
    Ok(out)
}

fn render_terrain(out: &mut String, map: &MapInfo, tile_px: u32, show_grid: bool) {
    let width_px = map.width.saturating_mul(tile_px);
    let height_px = map.height.saturating_mul(tile_px);
    writeln!(
        out,
        r#"<rect x="0" y="0" width="{width_px}" height="{height_px}" fill="{PASSABLE_FILL}"/>"#
    )
    .unwrap();

    for y in 0..map.height {
        for x in 0..map.width {
            let idx = (y as usize)
                .saturating_mul(map.width as usize)
                .saturating_add(x as usize);
            if map
                .terrain
                .get(idx)
                .copied()
                .is_some_and(rts_rules::terrain::is_passable_map_code)
            {
                continue;
            }
            writeln!(
                out,
                r#"<rect x="{}" y="{}" width="{tile_px}" height="{tile_px}" fill="{BLOCKED_FILL}"/>"#,
                x.saturating_mul(tile_px),
                y.saturating_mul(tile_px)
            )
            .unwrap();
        }
    }

    if show_grid {
        writeln!(
            out,
            r#"<g stroke="{GRID_STROKE}" stroke-width="0.35" opacity="0.26">"#
        )
        .unwrap();
        for x in 0..=map.width {
            let px = x.saturating_mul(tile_px);
            writeln!(
                out,
                r#"<line x1="{px}" y1="0" x2="{px}" y2="{height_px}"/>"#
            )
            .unwrap();
        }
        for y in 0..=map.height {
            let py = y.saturating_mul(tile_px);
            writeln!(out, r#"<line x1="0" y1="{py}" x2="{width_px}" y2="{py}"/>"#).unwrap();
        }
        writeln!(out, "</g>").unwrap();
    }
}

fn render_layer(
    out: &mut String,
    layer: &ObserverMapAnalysisLayer,
    world_tile_size: u32,
    tile_px: u32,
) {
    writeln!(out, r#"<g id="layer-{}">"#, escape_xml(&layer.id)).unwrap();
    for primitive in &layer.primitives {
        match primitive {
            ObserverMapAnalysisPrimitive::TileRect {
                id,
                tile_x,
                tile_y,
                tile_w,
                tile_h,
                fill,
                stroke,
                alpha,
                label,
                tooltip: _,
            } => {
                let x = tile_x.saturating_mul(tile_px);
                let y = tile_y.saturating_mul(tile_px);
                let w = tile_w.saturating_mul(tile_px);
                let h = tile_h.saturating_mul(tile_px);
                let stroke_opacity = 0.92;
                let stroke_width = 1.4;
                writeln!(
                    out,
                    r#"<rect id="{}" x="{x}" y="{y}" width="{w}" height="{h}" fill="{}" fill-opacity="{:.3}" stroke="{}" stroke-opacity="{stroke_opacity:.2}" stroke-width="{stroke_width:.1}"/>"#,
                    escape_xml(id),
                    escape_xml(fill),
                    alpha.clamp(0.0, 1.0),
                    escape_xml(stroke)
                )
                .unwrap();
                if let Some(label) = label {
                    render_text_label(
                        out,
                        label,
                        x as f32 + w as f32 * 0.5,
                        y as f32 + h as f32 * 0.5,
                        12,
                    );
                }
            }
            ObserverMapAnalysisPrimitive::Marker {
                id,
                x,
                y,
                radius,
                shape,
                color,
                label,
                tooltip: _,
            } => {
                let px = world_to_render_px(*x, world_tile_size, tile_px);
                let py = world_to_render_px(*y, world_tile_size, tile_px);
                let r = world_to_render_px(*radius, world_tile_size, tile_px).max(3.0);
                render_marker(out, id, px, py, r, shape, color);
                if let Some(label) = label {
                    render_text_label(out, label, px, py - r - 3.0, 11);
                }
            }
            ObserverMapAnalysisPrimitive::Line {
                id,
                x1,
                y1,
                x2,
                y2,
                color,
                alpha,
                width,
                label,
                tooltip: _,
            } => {
                let px1 = world_to_render_px(*x1, world_tile_size, tile_px);
                let py1 = world_to_render_px(*y1, world_tile_size, tile_px);
                let px2 = world_to_render_px(*x2, world_tile_size, tile_px);
                let py2 = world_to_render_px(*y2, world_tile_size, tile_px);
                writeln!(
                    out,
                    r#"<line id="{}" x1="{px1:.2}" y1="{py1:.2}" x2="{px2:.2}" y2="{py2:.2}" stroke="{}" stroke-opacity="{:.3}" stroke-width="{:.2}" stroke-linecap="round"/>"#,
                    escape_xml(id),
                    escape_xml(color),
                    alpha.clamp(0.0, 1.0),
                    width.max(1.0)
                )
                .unwrap();
                if let Some(label) = label {
                    render_text_label(out, label, (px1 + px2) * 0.5, (py1 + py2) * 0.5, 11);
                }
            }
        }
    }
    writeln!(out, "</g>").unwrap();
}

fn render_marker(
    out: &mut String,
    id: &str,
    x: f32,
    y: f32,
    radius: f32,
    shape: &str,
    color: &str,
) {
    let color = escape_xml(color);
    let id = escape_xml(id);
    match shape {
        "diamond" => {
            writeln!(
                out,
                r#"<polygon id="{id}" points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{color}" fill-opacity="0.9" stroke="{TEXT_HALO}" stroke-width="1.4"/>"#,
                x,
                y - radius,
                x + radius,
                y,
                x,
                y + radius,
                x - radius,
                y
            )
            .unwrap();
        }
        "square" => {
            writeln!(
                out,
                r#"<rect id="{id}" x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{color}" fill-opacity="0.9" stroke="{TEXT_HALO}" stroke-width="1.4"/>"#,
                x - radius,
                y - radius,
                radius * 2.0,
                radius * 2.0
            )
            .unwrap();
        }
        _ => {
            writeln!(
                out,
                r#"<circle id="{id}" cx="{x:.2}" cy="{y:.2}" r="{radius:.2}" fill="{color}" fill-opacity="0.9" stroke="{TEXT_HALO}" stroke-width="1.4"/>"#
            )
            .unwrap();
        }
    }
}

fn render_text_label(out: &mut String, text: &str, x: f32, y: f32, font_size: u32) {
    let text = escape_xml(text);
    writeln!(
        out,
        r#"<text x="{x:.2}" y="{y:.2}" text-anchor="middle" dominant-baseline="middle" font-family="monospace" font-size="{font_size}" font-weight="800" fill="{TEXT_FILL}" stroke="{TEXT_HALO}" stroke-width="3" paint-order="stroke">{text}</text>"#
    )
    .unwrap();
}

fn world_to_render_px(value: f32, world_tile_size: u32, tile_px: u32) -> f32 {
    if world_tile_size == 0 {
        return 0.0;
    }
    value / world_tile_size as f32 * tile_px as f32
}

impl LayerSelection {
    fn includes(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(ids) => ids.contains(id),
        }
    }

    fn suffix(&self) -> String {
        match self {
            Self::All => "all".to_string(),
            Self::Only(ids) => ids.iter().cloned().collect::<Vec<_>>().join("-"),
        }
    }
}

fn parse_layers(value: &str) -> Result<LayerSelection, String> {
    if value == "all" {
        return Ok(LayerSelection::All);
    }
    let mut ids = BTreeSet::new();
    for raw in value.split(',') {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        if !matches!(id, "chokes" | "bases" | "resources") {
            return Err(format!("unknown map-analysis layer: {id}"));
        }
        ids.insert(id.to_string());
    }
    if ids.is_empty() {
        return Err("--layers must be all or a non-empty comma list".to_string());
    }
    Ok(LayerSelection::Only(ids))
}

fn required_value(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_u32_flag(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<u32, String> {
    let value = required_value(flag, args)?;
    value
        .parse::<u32>()
        .map_err(|_| format!("{flag} must be an unsigned integer"))
}

fn parse_seed_flag(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<u32, String> {
    let value = required_value(flag, args)?;
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).map_err(|_| format!("{flag} has invalid hex seed"));
    }
    value
        .parse::<u32>()
        .map_err(|_| format!("{flag} must be an unsigned integer or 0x-prefixed hex"))
}

fn default_out_path(map_name: &str, seed: u32, layers: &LayerSelection) -> PathBuf {
    Path::new("/tmp/rts-map-analysis").join(format!(
        "{}-seed-{}-{}.svg",
        slug(map_name),
        seed,
        layers.suffix()
    ))
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn player_color(id: u32) -> String {
    const COLORS: [&str; 8] = [
        "#45a3ff", "#ff6b6b", "#74c476", "#f7d774", "#c77dff", "#f2a541", "#06d6a0", "#8fb8d0",
    ];
    COLORS
        .get((id.saturating_sub(1) as usize) % COLORS.len())
        .copied()
        .unwrap_or("#e7dfc5")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_layer_filter_and_hex_seed() {
        let config = parse_args([
            "--map".to_string(),
            "Chokes".to_string(),
            "--seed".to_string(),
            "0x10".to_string(),
            "--layers".to_string(),
            "chokes,bases".to_string(),
        ])
        .expect("args should parse")
        .expect("help should not be requested");

        assert_eq!(config.map_name, "Chokes");
        assert_eq!(config.seed, 16);
        assert!(config.layers.includes("chokes"));
        assert!(config.layers.includes("bases"));
        assert!(!config.layers.includes("resources"));
    }

    #[test]
    fn renders_default_map_svg_smoke() {
        let config = CliConfig {
            map_name: "Chokes".to_string(),
            players: 2,
            seed: DEFAULT_SEED,
            out: PathBuf::from("/tmp/not-written.svg"),
            tile_px: 2,
            layers: parse_layers("chokes").unwrap(),
            show_grid: false,
        };
        let start = start_payload_for_map(&config.map_name, config.players, config.seed)
            .expect("default map should load");
        let analysis = AiMapAnalysis::analyze(&start);
        let debug = analysis.debug_snapshot();
        let overlay = analysis.debug_overlay();
        let svg = render_svg(&config, &start, &debug, &overlay.layers).expect("svg renders");

        assert!(svg.contains("Chokes"));
        assert!(svg.contains("layer-chokes"));
        assert!(svg.contains("K0"));
    }
}
