use rts_phaserunner::{plan_from_env, render_dry_run, usage, CliAction};

fn main() {
    match plan_from_env() {
        Ok(CliAction::Help) => {
            print!("{}", usage());
        }
        Ok(CliAction::DryRun(plan)) => {
            print!("{}", render_dry_run(&plan));
        }
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}
