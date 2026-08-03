use std::io::Read;
use std::process::ExitCode;

use serde::Serialize;

use rts_sim::game::map::{analyze_authored_json, check_authored_json};

const OUTPUT_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorOutput<'a> {
    schema_version: u32,
    valid: bool,
    error: &'a str,
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("{message}\n");
            print_usage();
            ExitCode::from(2)
        }
        Err(CliError::Map(message)) => {
            print_json(&ErrorOutput {
                schema_version: OUTPUT_SCHEMA_VERSION,
                valid: false,
                error: &message,
            });
            ExitCode::from(1)
        }
    }
}

enum CliError {
    Usage(String),
    Map(String),
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), CliError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(CliError::Usage("missing command".to_string()));
    };
    if matches!(command.as_str(), "-h" | "--help") {
        print_usage();
        return Ok(());
    }
    if !matches!(command.as_str(), "check" | "report") {
        return Err(CliError::Usage(format!("unknown command: {command}")));
    }
    let Some(input) = args.next() else {
        return Err(CliError::Usage(
            "missing map path (use - for stdin)".to_string(),
        ));
    };
    if let Some(extra) = args.next() {
        return Err(CliError::Usage(format!("unexpected argument: {extra}")));
    }
    let json = read_input(&input).map_err(CliError::Map)?;
    match command.as_str() {
        "check" => print_json(&check_authored_json(&json).map_err(CliError::Map)?),
        "report" => print_json(&analyze_authored_json(&json).map_err(CliError::Map)?),
        _ => unreachable!(),
    }
    Ok(())
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|error| format!("cannot read map JSON from stdin: {error}"))?;
        return Ok(input);
    }
    std::fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))
}

fn print_json(value: &impl Serialize) {
    serde_json::to_writer(std::io::stdout(), value)
        .expect("serializing CLI output should not fail");
    println!();
}

fn print_usage() {
    eprintln!(
        "usage: authored-map <check|report> <map.json|->\n\
         \n\
         check   validate with the live authored-map materializer and print a JSON summary\n\
         report  additionally report rifleman/scout-car routes between every base pair\n\
         \n\
         Use - as the path to read authored map JSON from stdin. Output is stable JSON; map\n\
         validation failures also use JSON and exit with status 1."
    );
}
