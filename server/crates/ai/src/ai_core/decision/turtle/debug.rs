use super::*;
use rts_protocol::{ObserverMapAnalysisLayer, ObserverMapAnalysisPrimitive};

const TURTLE_DEBUG_CHOKE_COLOR: &str = "#00d4ff";
const TURTLE_DEBUG_INACTIVE_CHOKE_COLOR: &str = "#5c7080";
const TURTLE_DEBUG_MG_SLOT_COLOR: &str = "#5eead4";
const TURTLE_DEBUG_AT_LINE_COLOR: &str = "#ffb000";
const TURTLE_DEBUG_AT_FACE_COLOR: &str = "#ff5d73";

pub(in crate::ai_core::decision) fn turtle_observer_debug_layers(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    policy: TurtleDefensePolicy,
) -> Vec<ObserverMapAnalysisLayer> {
    let Some(chokes) = prioritized_base_chokes(observation, Some(analysis), policy) else {
        return Vec::new();
    };
    let own_start_world = tile_center(observation.own_start_tile, observation.map.tile_size);
    let main_ready = observation.upgrades.contains(&UpgradeKind::Entrenchment)
        && count_units_near_choke(
            observation,
            chokes[0],
            &TURTLE_MACHINE_GUNNER_KIND,
            TurtleSlotZone::ChokeLine,
        ) >= policy.main_machine_gunner_target;
    let active_choke_count = if main_ready {
        chokes.len()
    } else {
        early_choke_count(observation, chokes.len())
    };
    let active_chokes = &chokes[..active_choke_count.min(chokes.len())];

    let mut choke_primitives = Vec::new();
    let mut anti_tank_primitives = Vec::new();
    let at_zone = TurtleSlotZone::AntiTankEmplacement {
        back_tiles: policy.anti_tank_back_tiles,
        own_start_world,
    };

    for (index, choke) in chokes.iter().copied().enumerate() {
        let active = index < active_chokes.len();
        let role = if index == 0 {
            "main"
        } else if active {
            "active secondary"
        } else {
            "candidate"
        };
        let color = if active {
            TURTLE_DEBUG_CHOKE_COLOR
        } else {
            TURTLE_DEBUG_INACTIVE_CHOKE_COLOR
        };
        let mg_count = count_units_near_choke(
            observation,
            choke,
            &TURTLE_MACHINE_GUNNER_KIND,
            TurtleSlotZone::ChokeLine,
        );
        let slot_bias = choke_route_slot_bias(observation, choke);
        let mg_capacity = slot_capacity(
            choke,
            observation.map.tile_size,
            policy.machine_gunner_slot_gap_tiles,
        )
        .max(policy.machine_gunners_per_choke);
        let mg_slots = preferred_machine_gunner_slots(
            choke,
            mg_capacity,
            observation.map,
            slot_bias,
            policy.machine_gunners_per_choke,
        );
        choke_primitives.push(debug_line(
            format!("p{}:choke:{}:line", observation.player_id, choke.id),
            choke.endpoint_a_world,
            choke.endpoint_b_world,
            color,
            if active { 0.96 } else { 0.38 },
            if active { 8.0 } else { 4.0 },
            Some(format!(
                "P{} K{} generated {}",
                observation.player_id, choke.id, role
            )),
            Some(format!(
                "AI Turtle P{} thinks K{} is the {role} choke line. This uses the static analyzer's full generated choke line for Machine Gunner slots. Width {} tiles, slot capacity {}, MGs currently near line {}/{}.",
                observation.player_id,
                choke.id,
                choke.width_tiles,
                mg_capacity,
                mg_count,
                policy.machine_gunners_per_choke
            )),
        ));
        let line_midpoint = (
            (choke.endpoint_a_world.0 + choke.endpoint_b_world.0) * 0.5,
            (choke.endpoint_a_world.1 + choke.endpoint_b_world.1) * 0.5,
        );
        choke_primitives.push(debug_marker(
            format!(
                "p{}:choke:{}:generated-line-mid",
                observation.player_id, choke.id
            ),
            line_midpoint,
            observation.map.tile_size as f32 * 0.26,
            "diamond",
            color,
            Some(format!("K{} generated", choke.id)),
            Some(format!(
                "Midpoint of AI Turtle P{} generated defended choke line for K{}. MG slots are distributed across this line.",
                observation.player_id, choke.id
            )),
        ));
        choke_primitives.extend(debug_line_strip_markers(
            format!(
                "p{}:choke:{}:generated-line-strip",
                observation.player_id, choke.id
            ),
            choke.endpoint_a_world,
            choke.endpoint_b_world,
            observation.map.tile_size,
            color,
            Some(format!(
                "Visible marker-strip fallback for AI Turtle P{} generated defended choke line K{}.",
                observation.player_id, choke.id
            )),
        ));
        for (suffix, point) in [
            ("a", choke.endpoint_a_world),
            ("b", choke.endpoint_b_world),
        ] {
            choke_primitives.push(debug_marker(
                format!(
                    "p{}:choke:{}:generated-line-end:{}",
                    observation.player_id, choke.id, suffix
                ),
                point,
                observation.map.tile_size as f32 * 0.18,
                "square",
                color,
                None,
                Some(format!(
                    "Endpoint {suffix} of AI Turtle P{} generated defended choke line for K{}.",
                    observation.player_id, choke.id
                )),
            ));
        }
        if let Some(point) = slot_bias {
            choke_primitives.push(debug_marker(
                format!("p{}:choke:{}:route-seed", observation.player_id, choke.id),
                point,
                observation.map.tile_size as f32 * 0.22,
                "diamond",
                TURTLE_DEBUG_MG_SLOT_COLOR,
                Some(format!("K{} seed", choke.id)),
                Some(format!(
                    "AI Turtle P{} seeds K{} slot ordering here: this is where the public enemy-start to own-start route crosses or comes closest to the choke line.",
                    observation.player_id, choke.id
                )),
            ));
        }
        for (slot_order, slot_index) in mg_slots.into_iter().enumerate() {
            let Some(point) = slot_world(
                choke,
                slot_index,
                mg_capacity,
                TurtleSlotZone::ChokeLine,
                observation.map,
            ) else {
                continue;
            };
            choke_primitives.push(debug_marker(
                format!(
                    "p{}:choke:{}:mg-slot:{}",
                    observation.player_id,
                    choke.id,
                    slot_order + 1
                ),
                point,
                observation.map.tile_size as f32 * 0.18,
                "circle",
                TURTLE_DEBUG_MG_SLOT_COLOR,
                Some(format!("MG{}", slot_order + 1)),
                Some(format!(
                    "Machine Gunner target slot {} for AI Turtle P{} on K{}. Slot index {} of {}; selected by coverage spacing along the defended choke line.",
                    slot_order + 1,
                    observation.player_id,
                    choke.id,
                    slot_index,
                    mg_capacity
                )),
            ));
        }
    }

    for choke in active_chokes.iter().copied() {
        let capacity = slot_capacity(choke, observation.map.tile_size, policy.slot_gap_tiles)
            .max(policy.machine_gunners_per_choke);
        let slots = coverage_preferred_slots(
            choke,
            capacity,
            at_zone,
            observation.map,
            choke_route_slot_bias(observation, choke),
            policy.machine_gunners_per_choke.max(1),
        );
        let at_count = count_units_near_choke(observation, choke, policy.anti_tank_kinds, at_zone);
        if let (Some(start), Some(end)) = (
            backed_choke_point(
                choke,
                choke.endpoint_a_world,
                policy.anti_tank_back_tiles,
                observation.map.tile_size,
                own_start_world,
            ),
            backed_choke_point(
                choke,
                choke.endpoint_b_world,
                policy.anti_tank_back_tiles,
                observation.map.tile_size,
                own_start_world,
            ),
        ) {
            anti_tank_primitives.push(debug_line(
                format!("p{}:choke:{}:at-line", observation.player_id, choke.id),
                start,
                end,
                TURTLE_DEBUG_AT_LINE_COLOR,
                0.9,
                3.0,
                Some(format!("AT line K{}", choke.id)),
                Some(format!(
                    "AI Turtle P{} thinks Anti-Tank Guns for K{} should set up on this backline, {:.1} tiles behind the choke on the own-base side. AT guns currently near line: {}; coverage slots shown: {}.",
                    observation.player_id,
                    choke.id,
                    policy.anti_tank_back_tiles,
                    at_count,
                    slots.len()
                )),
            ));
        }

        for (slot_order, slot_index) in slots.into_iter().enumerate() {
            let Some(line_point) = slot_world(
                choke,
                slot_index,
                capacity,
                TurtleSlotZone::ChokeLine,
                observation.map,
            ) else {
                continue;
            };
            let Some(emplacement) =
                slot_world(choke, slot_index, capacity, at_zone, observation.map)
            else {
                continue;
            };
            let face_toward = anti_tank_face_toward(
                choke,
                line_point,
                policy.anti_tank_back_tiles,
                observation.map,
                own_start_world,
            )
            .unwrap_or(line_point);
            anti_tank_primitives.push(debug_marker(
                format!(
                    "p{}:choke:{}:at-slot:{}",
                    observation.player_id,
                    choke.id,
                    slot_order + 1
                ),
                emplacement,
                observation.map.tile_size as f32 * 0.24,
                "square",
                TURTLE_DEBUG_AT_LINE_COLOR,
                Some(format!("AT{}", slot_order + 1)),
                Some(format!(
                    "Anti-Tank Gun emplacement slot {} for AI Turtle P{} on K{}. Slot index {} of {}; this is the setup position behind the choke.",
                    slot_order + 1,
                    observation.player_id,
                    choke.id,
                    slot_index,
                    capacity
                )),
            ));
            anti_tank_primitives.push(debug_line(
                format!(
                    "p{}:choke:{}:at-face:{}",
                    observation.player_id,
                    choke.id,
                    slot_order + 1
                ),
                emplacement,
                face_toward,
                TURTLE_DEBUG_AT_FACE_COLOR,
                0.55,
                1.5,
                None,
                Some(format!(
                    "Anti-Tank Gun slot {} for AI Turtle P{} on K{} sets up facing along this line toward the choke/approach lane.",
                    slot_order + 1,
                    observation.player_id,
                    choke.id
                )),
            ));
        }
    }

    vec![
        ObserverMapAnalysisLayer {
            id: format!("ai-turtle-chokes-p{}", observation.player_id),
            label: format!("AI Turtle P{} chokes", observation.player_id),
            default_visible: true,
            primitives: choke_primitives,
        },
        ObserverMapAnalysisLayer {
            id: format!("ai-turtle-at-p{}", observation.player_id),
            label: format!("AI Turtle P{} AT setup", observation.player_id),
            default_visible: true,
            primitives: anti_tank_primitives,
        },
    ]
}

fn debug_line(
    id: String,
    start: (f32, f32),
    end: (f32, f32),
    color: &str,
    alpha: f32,
    width: f32,
    label: Option<String>,
    tooltip: Option<String>,
) -> ObserverMapAnalysisPrimitive {
    ObserverMapAnalysisPrimitive::Line {
        id,
        x1: start.0,
        y1: start.1,
        x2: end.0,
        y2: end.1,
        color: color.to_string(),
        alpha,
        width,
        label,
        tooltip,
    }
}

fn debug_marker(
    id: String,
    point: (f32, f32),
    radius: f32,
    shape: &str,
    color: &str,
    label: Option<String>,
    tooltip: Option<String>,
) -> ObserverMapAnalysisPrimitive {
    ObserverMapAnalysisPrimitive::Marker {
        id,
        x: point.0,
        y: point.1,
        radius,
        shape: shape.to_string(),
        color: color.to_string(),
        label,
        tooltip,
    }
}

fn debug_line_strip_markers(
    id_prefix: String,
    start: (f32, f32),
    end: (f32, f32),
    tile_size: u32,
    color: &str,
    tooltip: Option<String>,
) -> Vec<ObserverMapAnalysisPrimitive> {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len <= f32::EPSILON {
        return Vec::new();
    }
    let tile = tile_size.max(1) as f32;
    let spacing = (tile * 0.38).max(8.0);
    let radius = (tile * 0.24).max(6.0);
    let count = ((len / spacing).ceil() as usize).clamp(2, 40);
    (0..count)
        .map(|index| {
            let t = if count <= 1 {
                0.5
            } else {
                index as f32 / (count - 1) as f32
            };
            ObserverMapAnalysisPrimitive::Marker {
                id: format!("{id_prefix}:{index}"),
                x: start.0 + dx * t,
                y: start.1 + dy * t,
                radius,
                shape: "square".to_string(),
                color: color.to_string(),
                label: None,
                tooltip: tooltip.clone(),
            }
        })
        .collect()
}
