use std::collections::BTreeSet;

use serde::Serialize;

#[cfg(test)]
use super::profiles::required_profiles;
use super::profiles::{profile_by_id, AiProfile, AI_2_1_ID, AI_TURTLE_ID, JEFFS_AI_ID};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiProfileIdentity {
    pub(crate) profile_id: String,
    pub(crate) label: String,
    pub(crate) base_profile_id: Option<String>,
    pub(crate) summary: String,
    pub(crate) modules: Vec<String>,
    pub(crate) overlays: Vec<AiOverlayIdentity>,
    pub(crate) fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiOverlayIdentity {
    pub(crate) id: String,
    pub(crate) summary: String,
}

pub(crate) fn profile_identity(profile: &AiProfile) -> AiProfileIdentity {
    let (label, summary, modules) = baseline_metadata(profile.id);
    let overlays = Vec::new();
    let base_profile_id = None;
    AiProfileIdentity {
        profile_id: profile.id.to_string(),
        label: label.to_string(),
        base_profile_id,
        summary: summary.to_string(),
        modules: modules.iter().map(|module| module.to_string()).collect(),
        overlays,
        fingerprint: profile_fingerprint(profile, label, None, summary, modules.as_slice(), &[]),
    }
}

pub(crate) fn profile_identity_by_id(id: &str) -> Option<AiProfileIdentity> {
    profile_by_id(id).map(profile_identity)
}

#[cfg(test)]
pub(crate) fn required_profile_identities() -> Vec<AiProfileIdentity> {
    required_profiles()
        .into_iter()
        .map(profile_identity)
        .collect()
}

pub(crate) fn validate_profile_identity(identity: &AiProfileIdentity) -> Result<(), String> {
    let Some(expected) = profile_identity_by_id(&identity.profile_id) else {
        return Err(format!("unknown AI profile {}", identity.profile_id));
    };
    if identity.fingerprint != expected.fingerprint {
        return Err(format!(
            "AI profile {} fingerprint mismatch: got {}, expected {}",
            identity.profile_id, identity.fingerprint, expected.fingerprint
        ));
    }
    if identity.modules.is_empty() {
        return Err(format!("AI profile {} has no modules", identity.profile_id));
    }
    let mut overlay_ids = BTreeSet::new();
    for overlay in &identity.overlays {
        if !overlay_ids.insert(overlay.id.as_str()) {
            return Err(format!(
                "AI profile {} repeats overlay {}",
                identity.profile_id, overlay.id
            ));
        }
    }
    Ok(())
}

fn baseline_metadata(profile_id: &str) -> (&'static str, &'static str, Vec<&'static str>) {
    match profile_id {
        AI_2_1_ID => (
            "AI 2.1",
            "Pressure profile with proposal-based economy management, defensive Machine Gunners, mixed Tank pressure, and a second Factory.",
            vec![
                "economy_manager",
                "full_steel_saturation",
                "early_expansion",
                "defensive_machine_gunners",
                "earlier_factory_tank_unlock",
                "mixed_tank_pressure",
                "second_factory",
            ],
        ),
        AI_TURTLE_ID => (
            "AI Turtle",
            "Support-weapon turtle profile with proposal-based economy management, Entrenchment, and Machine Gunner plus Anti-Tank Gun choke defense.",
            vec![
                "economy_manager",
                "full_steel_saturation",
                "early_expansion",
                "two_rifle_opening",
                "support_tech",
                "entrenchment_research",
                "entrenchment_before_machine_gunners",
                "floated_second_gun_works",
                "four_machine_gunners_per_main_choke",
                "wider_machine_gunner_spacing",
                "fast_anti_tank_gun_tech",
                "compact_gun_works_placement",
                "main_choke_first",
                "nearest_enemy_choke_priority",
                "base_route_sector_priority",
                "full_choke_line_coverage_slots",
                "anti_tank_coverage_emplacements",
                "close_spawn_two_chokes",
                "steel_line_rifle_screen",
                "machine_gunner_choke_line_staging",
                "anti_tank_emplacements",
            ],
        ),
        JEFFS_AI_ID => (
            "Jeff's AI",
            "Server-authoritative port of the locally evaluated V3 champion policy with a Machine Gunner screen, Entrenchment, armored scouting, and five-Tank pressure.",
            vec![
                "economy_manager",
                "local_v3_policy",
                "machine_gunner_screen",
                "entrenchment_research",
                "armored_scout",
                "five_tank_pressure",
                "second_factory",
            ],
        ),
        _ => (
            "AI Profile",
            "Developer AI profile resolved through the shared profile registry.",
            vec!["shared_decision_loop"],
        ),
    }
}

fn profile_fingerprint(
    profile: &AiProfile,
    label: &str,
    base_profile_id: Option<&str>,
    summary: &str,
    modules: &[&str],
    overlays: &[AiOverlayIdentity],
) -> String {
    let mut text = format!(
        "profile={profile:?}|label={label}|base={}|summary={summary}|modules={}",
        base_profile_id.unwrap_or("-"),
        modules.join(",")
    );
    for overlay in overlays {
        text.push_str("|overlay=");
        text.push_str(&overlay.id);
        text.push(':');
        text.push_str(&overlay.summary);
    }
    format!("fnv1a64:{:016x}", fnv1a64(text.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::profiles::{AI_2_1_ID, AI_TURTLE_ID};

    #[test]
    fn profile_identities_are_complete_and_valid() {
        let identities = required_profile_identities();

        assert_eq!(identities.len(), 3);
        for identity in identities {
            validate_profile_identity(&identity).expect("identity should validate");
            assert!(!identity.fingerprint.is_empty());
            assert!(!identity.modules.is_empty());
        }
    }

    #[test]
    fn canonical_profile_manifest_metadata_uses_short_names() {
        let identity = profile_identity_by_id(AI_2_1_ID).expect("AI 2.1 identity");

        assert_eq!(identity.label, "AI 2.1");
        assert!(identity.modules.contains(&"economy_manager".to_string()));

        let turtle = profile_identity_by_id(AI_TURTLE_ID).expect("AI Turtle identity");
        assert_eq!(turtle.label, "AI Turtle");
    }
}
