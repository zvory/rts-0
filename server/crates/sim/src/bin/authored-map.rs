use std::fs::File;
use std::io::Read;
use std::process::ExitCode;

use serde::Serialize;

use rts_sim::game::process_authored_json;

const OUTPUT_SCHEMA_VERSION: u32 = 2;
const MAX_AUTHORED_MAP_BYTES: usize = 512 * 1024;

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
    let include_route_report = command == "report";
    print_json(&process_authored_json(&json, include_route_report).map_err(CliError::Map)?);
    Ok(())
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        return read_bounded(std::io::stdin().lock(), "stdin");
    }
    let file = File::open(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    if file
        .metadata()
        .map_err(|error| format!("cannot inspect {path}: {error}"))?
        .len()
        > MAX_AUTHORED_MAP_BYTES as u64
    {
        return Err(format!(
            "map JSON exceeds the {MAX_AUTHORED_MAP_BYTES}-byte input limit"
        ));
    }
    read_bounded(file, path)
}

fn read_bounded(reader: impl Read, source: &str) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(MAX_AUTHORED_MAP_BYTES.min(64 * 1024));
    reader
        .take((MAX_AUTHORED_MAP_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read map JSON from {source}: {error}"))?;
    if bytes.len() > MAX_AUTHORED_MAP_BYTES {
        return Err(format!(
            "map JSON exceeds the {MAX_AUTHORED_MAP_BYTES}-byte input limit"
        ));
    }
    String::from_utf8(bytes).map_err(|_| "map JSON must be UTF-8".to_string())
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn bounded_reader_accepts_limit_and_rejects_limit_plus_one() {
        let accepted = vec![b' '; MAX_AUTHORED_MAP_BYTES];
        assert_eq!(
            read_bounded(Cursor::new(accepted), "fixture")
                .expect("limit-sized input should pass")
                .len(),
            MAX_AUTHORED_MAP_BYTES
        );

        let oversized = vec![b' '; MAX_AUTHORED_MAP_BYTES + 1];
        let error = read_bounded(Cursor::new(oversized), "fixture")
            .expect_err("limit plus one should fail");
        assert!(error.contains("input limit"), "{error}");
    }
}
