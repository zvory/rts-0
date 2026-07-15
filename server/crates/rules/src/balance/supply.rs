//! Supply cap and provider constants.

/// Every active player has the full match supply allowance without requiring a building.
pub const INTRINSIC_SUPPLY_CAP: u32 = 200;
pub const SUPPLY_CAP_MAX: u32 = INTRINSIC_SUPPLY_CAP;

// Buildings no longer gate army capacity. Keep these definitions explicit for building-stat,
// replay, and fixture compatibility while their entities remain in the catalog.
pub const DEPOT_SUPPLY: u32 = 0;
pub const CITY_CENTRE_SUPPLY: u32 = 0;
