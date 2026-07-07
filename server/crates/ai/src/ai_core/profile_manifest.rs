use std::collections::BTreeSet;

use serde::Serialize;

#[cfg(test)]
use super::profiles::required_profiles;
use super::profiles::{
    profile_by_id, AiProfile, AI_1_0_TECH_ID, AI_1_1_TANK_MG_ID, AI_1_2_WAVE_COHORTS_ID,
};

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
        AI_1_0_TECH_ID => (
            "AI 1.0 Tech",
            "Legacy technology profile with economy, expansion, and tank transition goals.",
            vec!["economy", "expansion", "tech_transition", "frontal_attack"],
        ),
        AI_1_1_TANK_MG_ID => (
            "AI 1.1 Tank MG",
            "AI 1.0 fork with Tank-only transition pressure and defensive Machine Gunners.",
            vec![
                "economy",
                "expansion",
                "defensive_machine_gunners",
                "tank_transition",
                "frontal_attack",
            ],
        ),
        AI_1_2_WAVE_COHORTS_ID => (
            "AI 1.2 Wave Cohorts",
            "AI 1.1 fork with launched-unit exclusion, line staging, and a second Factory.",
            vec![
                "economy",
                "expansion",
                "defensive_machine_gunners",
                "tank_transition",
                "frontal_wave_cohorts",
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
    fn ai_1_2_identity_records_wave_cohort_modules() {
        let identity = profile_identity_by_id(AI_1_2_WAVE_COHORTS_ID).expect("AI 1.2 identity");

        assert_eq!(identity.base_profile_id, None);
        assert!(identity
            .modules
            .contains(&"frontal_wave_cohorts".to_string()));
        assert!(identity.modules.contains(&"second_factory".to_string()));
        assert!(identity.overlays.is_empty());
    }
}
