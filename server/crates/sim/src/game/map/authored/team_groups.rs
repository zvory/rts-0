use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AuthoredStartLocation {
    pub(super) x: u32,
    pub(super) y: u32,
    #[serde(default, deserialize_with = "deserialize_team_group")]
    team_group: Option<u32>,
}

fn deserialize_team_group<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error> {
    // Only omission means automatic assignment; explicit null is not a group.
    u32::deserialize(deserializer).map(Some)
}

pub(super) fn parse_team_groups(starts: &[AuthoredStartLocation]) -> Result<Vec<u32>, String> {
    if starts.iter().all(|start| start.team_group.is_none()) {
        return Ok(Vec::new());
    }
    starts
        .iter()
        .map(|start| match start.team_group {
            Some(group @ 1..=4) => Ok(group),
            _ => Err(
                "when using teamGroup, every start must have an integer teamGroup from 1 to 4"
                    .to_string(),
            ),
        })
        .collect()
}

pub(super) fn parse_start_locations(
    width: u32,
    height: u32,
    starts: &[AuthoredStartLocation],
) -> Result<Vec<(u32, u32)>, String> {
    let locations: Vec<_> = starts
        .iter()
        .map(|start| super::AuthoredLocation {
            x: start.x,
            y: start.y,
        })
        .collect();
    super::parse_locations(width, height, &locations, "startLocations")
}
