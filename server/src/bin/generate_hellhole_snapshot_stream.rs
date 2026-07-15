use std::path::PathBuf;

use rts_server::tools::hellhole_snapshot_stream::{
    write_hellhole_snapshot_stream, DEFAULT_FRAME_COUNT,
};

const DEFAULT_OUTPUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../client/assets/snapshot-streams/supply-300-hellhole.rtsstream"
);

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    let frame_count = std::env::args()
        .nth(2)
        .map(|raw| raw.parse::<u32>())
        .transpose()
        .unwrap_or_else(|err| {
            eprintln!("invalid frame count: {err}");
            std::process::exit(2);
        })
        .unwrap_or(DEFAULT_FRAME_COUNT);

    match write_hellhole_snapshot_stream(&output, frame_count) {
        Ok(summary) => println!(
            "wrote {}: {} frames, ticks {}..={}, {} initial entities, {} deaths, {} respawns, minimum {} entities, {} bytes",
            output.display(),
            summary.frame_count,
            summary.first_tick,
            summary.last_tick,
            summary.initial_entity_count,
            summary.death_events,
            summary.respawned_units,
            summary.minimum_entity_count,
            summary.byte_len,
        ),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
