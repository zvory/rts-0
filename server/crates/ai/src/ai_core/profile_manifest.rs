use std::collections::BTreeSet;

use serde::Serialize;

use super::profiles::{
    profile_by_id, profile_spec_by_id, AiProfile, AiProfileSpec, AI_1_0_TECH_ID,
    AI_1_1_TANK_MG_ID, AI_1_2_WAVE_COHORTS_ID, AI_2_0_RIFLE_TANK_ID,
    AI_2_0_TANK_PRESSURE_ID,
};
#[cfg(test)]
use super::profiles::required_profiles;

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
    if let Some(spec) = profile_spec_by_id(profile.id) {
        return identity_from_spec(profile, spec);
    }

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
        fingerprint: profile_fingerprint(
            profile,
            label,
            None,
            summary,
            modules.as_slice(),
            &[],
        ),
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

fn identity_from_spec(profile: &AiProfile, spec: &'static AiProfileSpec) -> AiProfileIdentity {
    let overlays = spec
        .overlays
        .iter()
        .map(|overlay| AiOverlayIdentity {
            id: overlay.id.to_string(),
            summary: overlay.summary.to_string(),
        })
        .collect::<Vec<_>>();
    AiProfileIdentity {
        profile_id: spec.id.to_string(),
        label: spec.label.to_string(),
        base_profile_id: Some(spec.base_profile_id.to_string()),
        summary: spec.summary.to_string(),
        modules: spec
            .modules
            .iter()
            .map(|module| module.to_string())
            .collect(),
        fingerprint: profile_fingerprint(
            profile,
            spec.label,
            Some(spec.base_profile_id),
            spec.summary,
            spec.modules,
            &overlays,
        ),
        overlays,
    }
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
        AI_2_0_RIFLE_TANK_ID => (
            "AI 2.0 Rifle Tank",
            "AI 2.0 suite member with Rifleman pressure, earlier two-base economy, defensive MGs, and Tank-led mixed waves.",
            vec![
                "full_steel_saturation",
                "early_expansion",
                "rifle_pressure",
                "defensive_machine_gunners",
                "earlier_factory_tank_unlock",
                "tank_led_mixed_waves",
                "second_factory",
            ],
        ),
        AI_2_0_TANK_PRESSURE_ID => (
            "AI 2.0 Tank Pressure",
            "AI 2.0 suite member with earlier Factory unlock, two Factories, defensive MGs, and faster mixed Tank pressure.",
            vec![
                "full_steel_saturation",
                "early_expansion",
                "defensive_machine_gunners",
                "earlier_factory_tank_unlock",
                "mixed_tank_pressure",
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
    use crate::ai_core::profiles::{
        AI_2_0_AGENT_RUSH_ID, AI_2_0_RIFLE_TANK_ID, AI_2_0_TANK_PRESSURE_ID,
    };

    #[test]
    fn profile_identities_are_complete_and_valid() {
        let identities = required_profile_identities();

        assert!(identities
            .iter()
            .any(|identity| identity.profile_id == AI_2_0_AGENT_RUSH_ID));
        for identity in identities {
            validate_profile_identity(&identity).expect("identity should validate");
            assert!(!identity.fingerprint.is_empty());
            assert!(!identity.modules.is_empty());
        }
    }

    #[test]
    fn ai_2_0_identity_records_base_and_overlays() {
        let identity = profile_identity_by_id(AI_2_0_AGENT_RUSH_ID).expect("AI 2.0 identity");

        assert_eq!(
            identity.base_profile_id,
            Some("rifle_flood_full_saturation".to_string())
        );
        assert!(identity.modules.contains(&"bounded_decision_trace".to_string()));
        assert_eq!(
            identity
                .overlays
                .iter()
                .map(|overlay| overlay.id.as_str())
                .collect::<Vec<_>>(),
            vec!["agent_rifle_tank_cohorts"]
        );
    }

    #[test]
    fn ai_2_0_suite_members_have_specific_manifest_metadata() {
        let rifle_tank =
            profile_identity_by_id(AI_2_0_RIFLE_TANK_ID).expect("rifle tank identity");
        let tank_pressure =
            profile_identity_by_id(AI_2_0_TANK_PRESSURE_ID).expect("tank pressure identity");

        assert_eq!(rifle_tank.label, "AI 2.0 Rifle Tank");
        assert!(rifle_tank
            .modules
            .contains(&"tank_led_mixed_waves".to_string()));
        assert_eq!(tank_pressure.label, "AI 2.0 Tank Pressure");
        assert!(tank_pressure
            .modules
            .contains(&"mixed_tank_pressure".to_string()));
    }
}
