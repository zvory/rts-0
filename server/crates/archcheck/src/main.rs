use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    if command != "check-sim-architecture" {
        eprintln!("unknown command: {command}");
        print_usage();
        return ExitCode::from(2);
    }

    let mut bless = false;
    let mut reason = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bless" => bless = true,
            "--reason" => {
                let Some(value) = args.next() else {
                    eprintln!("--reason requires a value");
                    print_usage();
                    return ExitCode::from(2);
                };
                reason = Some(value);
            }
            _ => {
                eprintln!("unexpected argument: {arg}");
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    if reason.is_some() && !bless {
        eprintln!("--reason is only used with --bless");
        print_usage();
        return ExitCode::from(2);
    }

    let game_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sim/src/game")
        .components()
        .collect::<PathBuf>();
    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("baselines/sim-architecture.json")
        .components()
        .collect::<PathBuf>();

    if bless {
        let Some(reason) = reason else {
            eprintln!("--bless requires --reason \"short reason\"");
            print_usage();
            return ExitCode::from(2);
        };
        return match rts_archcheck::bless_sim_architecture_baseline(
            &game_root,
            &baseline_path,
            &reason,
        ) {
            Ok(summary) => {
                println!("updated sim architecture baseline:");
                for line in summary {
                    println!("  - {line}");
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("sim architecture baseline could not be updated: {error}");
                ExitCode::FAILURE
            }
        };
    }

    match rts_archcheck::check_sim_architecture_with_baseline(&game_root, &baseline_path) {
        Ok(report) if report.failures.is_empty() => {
            println!("sim architecture check passed");
            for note in report.ratchet_notes {
                println!("  - {note}");
            }
            ExitCode::SUCCESS
        }
        Ok(report) => {
            eprintln!("sim architecture check failed:");
            for failure in report.failures {
                eprintln!("  - {failure}");
            }
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("sim architecture check could not run: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p rts-archcheck -- check-sim-architecture [--bless --reason \"short reason\"]"
    );
}
