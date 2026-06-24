#[path = "../command_budget.rs"]
mod command_budget;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandBudgetConfig {
    base_command_supply_cap: u32,
    command_car_supply_cap_bonus: u32,
}

fn main() {
    let config = CommandBudgetConfig {
        base_command_supply_cap: command_budget::BASE_COMMAND_SUPPLY_CAP,
        command_car_supply_cap_bonus: command_budget::COMMAND_CAR_SUPPLY_CAP_BONUS,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&config).expect("command budget config serializes")
    );
}
