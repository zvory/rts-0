use rts_phaserunner::{
    execute_plan, plan_from_env, render_dry_run, usage, CliAction, SystemCommandRunner,
};

fn main() {
    match plan_from_env() {
        Ok(CliAction::Help) => {
            print!("{}", usage());
        }
        Ok(CliAction::DryRun(plan)) => {
            print!("{}", render_dry_run(&plan));
        }
        Ok(CliAction::Execute(plan)) => {
            let mut runner = SystemCommandRunner;
            if let Err(err) = execute_plan(&plan, &mut runner) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}
