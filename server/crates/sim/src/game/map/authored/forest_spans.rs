use std::collections::HashSet;

pub(super) fn parse_forest_spans(
    width: u32,
    height: u32,
    spans: &[[u32; 3]],
) -> Result<Vec<(u32, u32)>, String> {
    let mut locations = Vec::new();
    let mut seen = HashSet::new();
    for (index, &[y, x_start, x_end]) in spans.iter().enumerate() {
        if y >= height || x_start > x_end || x_end >= width {
            return Err(format!(
                "forestSpans[{index}] = [{y},{x_start},{x_end}] is outside the {width}x{height} map or has reversed x bounds"
            ));
        }
        for x in x_start..=x_end {
            if !seen.insert((x, y)) {
                return Err(format!(
                    "forestSpans[{index}] overlaps an earlier span at ({x},{y})"
                ));
            }
            locations.push((x, y));
        }
    }
    locations.sort_unstable();
    Ok(locations)
}

pub(super) fn merge_overlay_locations(
    explicit: Vec<(u32, u32)>,
    forest: &[(u32, u32)],
) -> Vec<(u32, u32)> {
    let mut merged: HashSet<_> = explicit.into_iter().collect();
    merged.extend(forest.iter().copied());
    let mut locations: Vec<_> = merged.into_iter().collect();
    locations.sort_unstable();
    locations
}
