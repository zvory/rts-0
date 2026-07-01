use std::path::PathBuf;

use rts_server::lab_scenarios::{
    bundled_lab_scenario_asset_build_sha, convert_lab_scenario_catalog_assets_to_checkpoints,
};

const DEFAULT_LAB_SCENARIO_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lab-scenarios");

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LAB_SCENARIO_DIR));
    if let Some(extra) = args.next() {
        eprintln!("unexpected extra argument: {extra}");
        std::process::exit(2);
    }

    match convert_lab_scenario_catalog_assets_to_checkpoints(
        &root,
        bundled_lab_scenario_asset_build_sha(),
    ) {
        Ok(converted) => {
            println!(
                "converted {converted} lab scenario asset(s) under {}",
                root.display()
            );
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
