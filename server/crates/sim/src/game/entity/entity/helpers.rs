use super::{BUILDING_START_HP_DENOMINATOR, BUILDING_START_HP_NUMERATOR};

pub(super) fn construction_hp_for_progress(max_hp: u32, progress: u32, total: u32) -> u32 {
    if max_hp == 0 {
        return 0;
    }
    if total == 0 || progress >= total {
        return max_hp;
    }
    let start_hp = max_hp
        .saturating_mul(BUILDING_START_HP_NUMERATOR)
        .div_ceil(BUILDING_START_HP_DENOMINATOR)
        .clamp(1, max_hp);
    let remaining_hp = max_hp.saturating_sub(start_hp);
    let gained_hp = (remaining_hp as u64)
        .saturating_mul(progress as u64)
        .checked_div(total as u64)
        .unwrap_or(remaining_hp as u64) as u32;
    start_hp.saturating_add(gained_hp).min(max_hp)
}
