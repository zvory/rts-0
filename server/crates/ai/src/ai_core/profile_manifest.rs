use std::collections::BTreeSet;

use serde::Serialize;

#[cfg(test)]
use super::profiles::required_profiles;
use super::profiles::{
    profile_by_id, AiProfile, AI_1_0_TECH_ID, AI_1_1_TANK_MG_ID, AI_1_2_WAVE_COHORTS_ID,
    AI_2_0_TANK_PRESSURE_ID, AI_TURTLE_CHOKES_ID,
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
        AI_TURTLE_CHOKES_ID => (
            "AI Turtle Chokes",
            "Support-weapon turtle profile that opens three Riflemen on a main steel-line screen, fast expands on the AI 2.0 economy cadence, queues Entrenchment before capped four-per-line Machine Gunner production, fast-techs Anti-Tank Guns, prioritizes enemy-facing chokes, and stages Machine Gunners plus guns on own-base choke lines.",
            vec![
                "full_steel_saturation",
                "early_expansion",
                "three_rifle_opening",
                "support_tech",
                "entrenchment_research",
                "entrenchment_before_machine_gunners",
                "one_barracks_machine_gunner_core",
                "four_machine_gunners_per_main_choke",
                "wider_machine_gunner_spacing",
                "fast_anti_tank_gun_tech",
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
    use crate::ai_core::profiles::AI_2_0_TANK_PRESSURE_ID;

    #[test]
    fn profile_identities_are_complete_and_valid() {
        let identities = required_profile_identities();

        assert_eq!(identities.len(), 5);
        for identity in identities {
            validate_profile_identity(&identity).expect("identity should validate");
            assert!(!identity.fingerprint.is_empty());
            assert!(!identity.modules.is_empty());
        }
    }

    #[test]
    fn ai_2_0_tank_pressure_has_specific_manifest_metadata() {
        let tank_pressure =
            profile_identity_by_id(AI_2_0_TANK_PRESSURE_ID).expect("tank pressure identity");

        assert_eq!(tank_pressure.label, "AI 2.0 Tank Pressure");
        assert!(tank_pressure
            .modules
            .contains(&"mixed_tank_pressure".to_string()));
    }
}
