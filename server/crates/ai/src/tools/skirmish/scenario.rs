//! Skirmish scenario catalog and squad placement geometry.
//!
//! A scenario places the candidate and opponent squads on the line between the two starting
//! bases: the squads start `gap_tiles` apart around a point `center_t` of the way from the
//! candidate base to the opponent base, and the opponent squad is shifted `lateral_tiles`
//! sideways to vary the approach angle. Seeds add a few pixels of deterministic jitter so mirror
//! fights are not decided by exact symmetry.

use std::collections::BTreeSet;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

const FORMATION_SPACING_TILES: f32 = 1.5;
const CLUMP_SPACING_TILES: f32 = 1.0;
/// Spawn jitter around a tile centre; small enough that a Rifleman stays inside its tile.
const JITTER_PX: f32 = 6.0;
/// How far a spawn point may slide to find a free tile.
const MAX_SLIDE_TILES: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Formation {
    /// Shoulder to shoulder, facing the enemy.
    Line,
    /// Single file toward the enemy.
    Column,
    /// A tight block.
    Clump,
}

impl Formation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Column => "column",
            Self::Clump => "clump",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Scenario {
    pub(crate) id: String,
    pub(crate) gap_tiles: f32,
    pub(crate) center_t: f32,
    pub(crate) candidate_formation: Formation,
    pub(crate) opponent_formation: Formation,
    pub(crate) lateral_tiles: f32,
}

impl Scenario {
    pub(crate) fn new(
        gap_tiles: f32,
        center_t: f32,
        candidate_formation: Formation,
        opponent_formation: Formation,
        lateral_tiles: f32,
    ) -> Self {
        let lateral = lateral_tiles.round() as i32;
        let id = format!(
            "g{}_{}_v_{}_l{}{}_c{}",
            gap_tiles.round() as i32,
            candidate_formation.as_str(),
            opponent_formation.as_str(),
            if lateral < 0 { "m" } else { "" },
            lateral.abs(),
            (center_t * 100.0).round() as i32,
        );
        Self {
            id,
            gap_tiles,
            center_t,
            candidate_formation,
            opponent_formation,
            lateral_tiles,
        }
    }
}

/// Named scenario sets. `train` is what a sweep optimizes; `holdout` uses different distances,
/// formations, angles, and fight location, so a tuned parameter set is checked on fights it was
/// not selected on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Suite {
    Smoke,
    Train,
    Holdout,
    All,
}

impl Suite {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "train" => Ok(Self::Train),
            "holdout" => Ok(Self::Holdout),
            "all" => Ok(Self::All),
            other => Err(format!(
                "unknown suite {other:?}; expected smoke, train, holdout, or all"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Train => "train",
            Self::Holdout => "holdout",
            Self::All => "all",
        }
    }

    pub(crate) fn scenarios(self) -> Vec<Scenario> {
        match self {
            Self::Smoke => vec![Scenario::new(
                12.0,
                0.5,
                Formation::Line,
                Formation::Line,
                0.0,
            )],
            Self::Train => grid(
                &[8.0, 12.0, 16.0],
                0.5,
                &[
                    (Formation::Line, Formation::Line),
                    (Formation::Line, Formation::Column),
                    (Formation::Column, Formation::Line),
                    (Formation::Clump, Formation::Line),
                ],
                &[0.0, 4.0],
            ),
            Self::Holdout => grid(
                &[10.0, 14.0],
                0.42,
                &[
                    (Formation::Clump, Formation::Clump),
                    (Formation::Column, Formation::Column),
                    (Formation::Line, Formation::Clump),
                ],
                &[-3.0, 6.0],
            ),
            Self::All => {
                let mut all = Self::Train.scenarios();
                all.extend(Self::Holdout.scenarios());
                all
            }
        }
    }
}

fn grid(
    gaps: &[f32],
    center_t: f32,
    formations: &[(Formation, Formation)],
    laterals: &[f32],
) -> Vec<Scenario> {
    let mut scenarios = Vec::new();
    for &gap in gaps {
        for &(candidate, opponent) in formations {
            for &lateral in laterals {
                scenarios.push(Scenario::new(gap, center_t, candidate, opponent, lateral));
            }
        }
    }
    scenarios
}

/// World-pixel spawn points for both squads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placement {
    pub(crate) candidate: Vec<(f32, f32)>,
    pub(crate) opponent: Vec<(f32, f32)>,
}

pub(crate) struct PlacementInput {
    pub(crate) squad_size: usize,
    pub(crate) candidate_base: (f32, f32),
    pub(crate) opponent_base: (f32, f32),
    pub(crate) tile_px: f32,
    pub(crate) world_px: (f32, f32),
    pub(crate) seed: u32,
}

pub(crate) fn place_squads(scenario: &Scenario, input: &PlacementInput) -> Placement {
    let (cx, cy) = input.candidate_base;
    let (ox, oy) = input.opponent_base;
    let axis_len = (ox - cx).hypot(oy - cy).max(1.0);
    let axis = ((ox - cx) / axis_len, (oy - cy) / axis_len);
    let side = (-axis.1, axis.0);
    let tile = input.tile_px;
    let center = (
        cx + axis.0 * axis_len * scenario.center_t,
        cy + axis.1 * axis_len * scenario.center_t,
    );
    let half_gap = scenario.gap_tiles * tile * 0.5;
    let lateral = scenario.lateral_tiles * tile;
    let candidate_center = (center.0 - axis.0 * half_gap, center.1 - axis.1 * half_gap);
    let opponent_center = (
        center.0 + axis.0 * half_gap + side.0 * lateral,
        center.1 + axis.1 * half_gap + side.1 * lateral,
    );
    let mut rng = SmallRng::seed_from_u64(u64::from(input.seed) ^ fnv(scenario.id.as_bytes()));
    let tiles_wide = (input.world_px.0 / tile).floor().max(1.0) as i32;
    let tiles_high = (input.world_px.1 / tile).floor().max(1.0) as i32;
    let mut taken: BTreeSet<(i32, i32)> = BTreeSet::new();
    // Lab spawns hold one unit per tile, so each point snaps to the nearest free tile centre.
    let mut squad = |center: (f32, f32), formation: Formation| -> Vec<(f32, f32)> {
        formation_offsets(formation, input.squad_size, tile)
            .into_iter()
            .map(|(along, across)| {
                let x = center.0 + axis.0 * along + side.0 * across;
                let y = center.1 + axis.1 * along + side.1 * across;
                let (tx, ty) = free_tile((x, y), tile, (tiles_wide, tiles_high), &taken);
                taken.insert((tx, ty));
                let jx = rng.gen_range(-JITTER_PX..=JITTER_PX);
                let jy = rng.gen_range(-JITTER_PX..=JITTER_PX);
                ((tx as f32 + 0.5) * tile + jx, (ty as f32 + 0.5) * tile + jy)
            })
            .collect()
    };
    let candidate = squad(candidate_center, scenario.candidate_formation);
    let opponent = squad(opponent_center, scenario.opponent_formation);
    Placement {
        candidate,
        opponent,
    }
}

/// The unclaimed in-bounds tile whose centre is nearest `point` (ties by tile order).
fn free_tile(
    point: (f32, f32),
    tile: f32,
    tiles: (i32, i32),
    taken: &BTreeSet<(i32, i32)>,
) -> (i32, i32) {
    let clamp =
        |value: f32, count: i32| ((value / tile).floor() as i32).clamp(1, (count - 2).max(1));
    let home = (clamp(point.0, tiles.0), clamp(point.1, tiles.1));
    let mut best = home;
    let mut best_distance = f32::INFINITY;
    for dy in -MAX_SLIDE_TILES..=MAX_SLIDE_TILES {
        for dx in -MAX_SLIDE_TILES..=MAX_SLIDE_TILES {
            let candidate = (home.0 + dx, home.1 + dy);
            if taken.contains(&candidate)
                || candidate.0 < 1
                || candidate.1 < 1
                || candidate.0 > tiles.0 - 2
                || candidate.1 > tiles.1 - 2
            {
                continue;
            }
            let cx = (candidate.0 as f32 + 0.5) * tile;
            let cy = (candidate.1 as f32 + 0.5) * tile;
            let distance = (cx - point.0).hypot(cy - point.1);
            if distance < best_distance {
                best = candidate;
                best_distance = distance;
            }
        }
    }
    best
}

/// Offsets `(along the base axis, across it)` centred on the squad position.
fn formation_offsets(formation: Formation, count: usize, tile: f32) -> Vec<(f32, f32)> {
    let centred = |index: usize, count: usize, spacing: f32| {
        (index as f32 - (count.saturating_sub(1)) as f32 * 0.5) * spacing
    };
    match formation {
        Formation::Line => (0..count)
            .map(|i| (0.0, centred(i, count, FORMATION_SPACING_TILES * tile)))
            .collect(),
        Formation::Column => (0..count)
            .map(|i| (centred(i, count, FORMATION_SPACING_TILES * tile), 0.0))
            .collect(),
        Formation::Clump => {
            let columns = (count as f32).sqrt().ceil().max(1.0) as usize;
            let rows = count.div_ceil(columns);
            (0..count)
                .map(|i| {
                    let spacing = CLUMP_SPACING_TILES * tile;
                    (
                        centred(i / columns, rows, spacing),
                        centred(i % columns, columns, spacing),
                    )
                })
                .collect()
        }
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
