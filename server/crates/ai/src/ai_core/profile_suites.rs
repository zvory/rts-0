use super::profiles::{
    profile_by_id, AI_1_0_TECH_ID, AI_1_1_TANK_MG_ID, AI_1_2_WAVE_COHORTS_ID,
    AI_2_0_TANK_PRESSURE_ID, AI_2_1_ECONOMY_MANAGER_ID, AI_TURTLE_CHOKES_ID,
};

pub(crate) const AI_1_0_SUITE_ID: &str = "ai_1_0";
pub(crate) const AI_1_1_SUITE_ID: &str = "ai_1_1";
pub(crate) const AI_1_2_SUITE_ID: &str = "ai_1_2";
pub(crate) const AI_2_0_SUITE_ID: &str = "ai_2_0";
pub(crate) const AI_2_1_SUITE_ID: &str = "ai_2_1";
pub(crate) const AI_TURTLE_SUITE_ID: &str = "ai_turtle";

const AI_1_0_MEMBERS: [&str; 1] = [AI_1_0_TECH_ID];
const AI_1_1_MEMBERS: [&str; 1] = [AI_1_1_TANK_MG_ID];
const AI_1_2_MEMBERS: [&str; 1] = [AI_1_2_WAVE_COHORTS_ID];
const AI_2_0_MEMBERS: [&str; 1] = [AI_2_0_TANK_PRESSURE_ID];
const AI_2_1_MEMBERS: [&str; 1] = [AI_2_1_ECONOMY_MANAGER_ID];
const AI_TURTLE_MEMBERS: [&str; 1] = [AI_TURTLE_CHOKES_ID];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiProfileSuite {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) members: &'static [&'static str],
}

pub(crate) const AI_PROFILE_SUITES: [AiProfileSuite; 6] = [
    AiProfileSuite {
        id: AI_1_0_SUITE_ID,
        label: "AI 1.0",
        summary: "Stable AI 1.0 suite; currently a single technology profile.",
        members: &AI_1_0_MEMBERS,
    },
    AiProfileSuite {
        id: AI_1_1_SUITE_ID,
        label: "AI 1.1",
        summary: "Stable AI 1.1 suite; currently a single Tank/MG profile.",
        members: &AI_1_1_MEMBERS,
    },
    AiProfileSuite {
        id: AI_1_2_SUITE_ID,
        label: "AI 1.2",
        summary: "Stable AI 1.2 suite; currently a single wave-cohort profile.",
        members: &AI_1_2_MEMBERS,
    },
    AiProfileSuite {
        id: AI_2_0_SUITE_ID,
        label: "AI 2.0",
        summary: "AI 2.0 suite currently pinned to the promoted tank-pressure profile.",
        members: &AI_2_0_MEMBERS,
    },
    AiProfileSuite {
        id: AI_2_1_SUITE_ID,
        label: "AI 2.1",
        summary: "AI 2.1 suite currently pins AI 2.0 strategy values behind the economy manager.",
        members: &AI_2_1_MEMBERS,
    },
    AiProfileSuite {
        id: AI_TURTLE_SUITE_ID,
        label: "AI Turtle",
        summary: "Turtle suite currently pinned to the choke-line support weapon profile.",
        members: &AI_TURTLE_MEMBERS,
    },
];

pub(crate) fn profile_suite_by_id(id: &str) -> Option<&'static AiProfileSuite> {
    AI_PROFILE_SUITES.iter().find(|suite| suite.id == id)
}

pub(crate) fn canonical_profile_request_id(input: &str) -> Option<&'static str> {
    match input {
        "ai1" | "ai_1_0" => Some(AI_1_0_SUITE_ID),
        "ai11" | "ai_1_1" => Some(AI_1_1_SUITE_ID),
        "ai12" | "ai_1_2" => Some(AI_1_2_SUITE_ID),
        "ai20" | "ai_2_0" => Some(AI_2_0_SUITE_ID),
        "ai21" | "ai_2_1" => Some(AI_2_1_SUITE_ID),
        "turtle" | "ai_turtle" => Some(AI_TURTLE_SUITE_ID),
        id => profile_by_id(id)
            .map(|profile| profile.id)
            .or_else(|| profile_suite_by_id(id).map(|suite| suite.id)),
    }
}

pub(crate) fn available_profile_request_ids() -> Vec<&'static str> {
    let mut ids = AI_PROFILE_SUITES
        .iter()
        .map(|suite| suite.id)
        .collect::<Vec<_>>();
    for profile in super::profiles::required_profiles() {
        if !ids.contains(&profile.id) {
            ids.push(profile.id);
        }
    }
    ids
}

pub(crate) fn resolve_profile_request_id(
    request_id: &str,
    seed: u32,
    selector: u64,
) -> Option<&'static str> {
    if let Some(profile) = profile_by_id(request_id) {
        return Some(profile.id);
    }
    let suite = profile_suite_by_id(request_id)?;
    let index = suite_member_index(seed, selector, suite.members.len());
    suite.members.get(index).copied()
}

fn suite_member_index(seed: u32, selector: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (u64::from(seed).wrapping_add(selector) % len as u64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_version_aliases_resolve_to_suite_requests() {
        assert_eq!(canonical_profile_request_id("ai1"), Some(AI_1_0_SUITE_ID));
        assert_eq!(
            canonical_profile_request_id("ai_1_1"),
            Some(AI_1_1_SUITE_ID)
        );
        assert_eq!(canonical_profile_request_id("ai12"), Some(AI_1_2_SUITE_ID));
        assert_eq!(
            canonical_profile_request_id("ai_2_0"),
            Some(AI_2_0_SUITE_ID)
        );
        assert_eq!(canonical_profile_request_id("ai21"), Some(AI_2_1_SUITE_ID));
        assert_eq!(
            canonical_profile_request_id("turtle"),
            Some(AI_TURTLE_SUITE_ID)
        );
        assert_eq!(canonical_profile_request_id("ai_2_0_rifle_tank"), None);
        assert_eq!(canonical_profile_request_id("ai_2_0_agent_rush"), None);
        assert_eq!(canonical_profile_request_id("missing"), None);
    }

    #[test]
    fn suite_resolution_is_seeded_and_keeps_exact_profiles_exact() {
        assert_eq!(
            resolve_profile_request_id(AI_2_0_SUITE_ID, 0, 0),
            Some(AI_2_0_TANK_PRESSURE_ID)
        );
        assert_eq!(
            resolve_profile_request_id(AI_2_0_SUITE_ID, 1, 0),
            Some(AI_2_0_TANK_PRESSURE_ID)
        );
        assert_eq!(
            resolve_profile_request_id(AI_2_0_TANK_PRESSURE_ID, 1, 0),
            Some(AI_2_0_TANK_PRESSURE_ID)
        );
        assert_eq!(
            resolve_profile_request_id(AI_2_1_SUITE_ID, 1, 0),
            Some(AI_2_1_ECONOMY_MANAGER_ID)
        );
        assert_eq!(
            resolve_profile_request_id(AI_TURTLE_SUITE_ID, 1, 0),
            Some(AI_TURTLE_CHOKES_ID)
        );
    }

    #[test]
    fn all_suite_members_are_registered_profiles() {
        for suite in AI_PROFILE_SUITES {
            assert!(!suite.members.is_empty(), "{} has no members", suite.id);
            for member in suite.members {
                assert!(
                    profile_by_id(member).is_some(),
                    "{} is not registered",
                    member
                );
            }
        }
    }
}
