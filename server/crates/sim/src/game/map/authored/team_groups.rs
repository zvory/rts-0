use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AuthoredStartLocation {
    pub(super) x: u32,
    pub(super) y: u32,
    #[serde(default)]
    team_group: Option<u32>,
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
