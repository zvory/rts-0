//! Supply cap and provider constants.

pub const DEPOT_SUPPLY: u32 = 8;
pub const CITY_CENTRE_SUPPLY: u32 = 10;
pub const SUPPLY_CAP_MAX: u32 = 300;

#[cfg(test)]
mod tests {
    use super::SUPPLY_CAP_MAX;

    #[test]
    fn hard_supply_cap_is_three_hundred() {
        assert_eq!(SUPPLY_CAP_MAX, 300);
    }
}
