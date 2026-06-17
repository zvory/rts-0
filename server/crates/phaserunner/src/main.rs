fn main() {
    if std::env::args().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return;
    }

    print_help();
}

fn print_help() {
    println!(
        "\
rts-phaserunner

Rust behavior model for the RTS phase runner.

This Phase 1 binary is intentionally side-effect free. The active operator
entrypoint remains scripts/phase-runner.sh until later migration phases.

Usage:
  rts-phaserunner --help
"
    );
}
