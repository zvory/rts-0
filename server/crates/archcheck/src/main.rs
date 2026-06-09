use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: cargo run -p rts-archcheck -- check-sim-architecture");
        return ExitCode::from(2);
    };

    if command != "check-sim-architecture" {
        eprintln!("unknown command: {command}");
        eprintln!("usage: cargo run -p rts-archcheck -- check-sim-architecture");
        return ExitCode::from(2);
    }

    if let Some(extra) = args.next() {
        eprintln!("unexpected argument: {extra}");
        eprintln!("usage: cargo run -p rts-archcheck -- check-sim-architecture");
        return ExitCode::from(2);
    }

    let game_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sim/src/game")
        .components()
        .collect::<PathBuf>();

    match rts_archcheck::check_sim_architecture(&game_root) {
        Ok(report) if report.failures.is_empty() => {
            println!("sim architecture check passed");
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
