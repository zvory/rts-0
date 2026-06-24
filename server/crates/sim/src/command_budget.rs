pub(crate) const BASE_COMMAND_SUPPLY_CAP: u32 = 24;
pub(crate) const COMMAND_CAR_SUPPLY_CAP_BONUS: u32 = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_budget_constants_match_current_policy_scalars() {
        assert_eq!(BASE_COMMAND_SUPPLY_CAP, 24);
        assert_eq!(COMMAND_CAR_SUPPLY_CAP_BONUS, 20);
    }
}
