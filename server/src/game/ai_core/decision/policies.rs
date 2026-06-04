use super::*;

pub(super) fn active_tech_transition(
    observation: &AiObservation,
    profile: &AiProfile,
) -> Option<TechTransitionPolicy> {
    profile
        .tech_transition
        .filter(|transition| observation.economy.supply_used >= transition.supply_used_threshold)
}

pub(super) fn active_recovery(
    profile: &AiProfile,
    recovery_active: bool,
) -> Option<RecoveryTransitionPolicy> {
    if recovery_active {
        profile.recovery_transition
    } else {
        None
    }
}

pub(super) fn active_required_tech_path(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> &'static [EntityKind] {
    active_tech_transition(observation, profile)
        .map(|transition| transition.required_tech_path)
        .or_else(|| {
            active_recovery(profile, recovery_active).map(|recovery| recovery.required_tech_path)
        })
        .unwrap_or(profile.buildings.required_tech_path)
}

pub(super) fn active_production_policy(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> ProductionPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.production)
        .or_else(|| active_recovery(profile, recovery_active).map(|recovery| recovery.production))
        .unwrap_or(profile.production)
}

pub(super) fn active_attack_policy(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> AttackPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.attack)
        .or_else(|| active_recovery(profile, recovery_active).map(|recovery| recovery.attack))
        .unwrap_or(profile.attack)
}

pub(super) fn active_worker_policy(profile: &AiProfile, recovery_active: bool) -> WorkerPolicy {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.workers)
        .unwrap_or(profile.workers)
}

pub(super) fn active_resource_policy(profile: &AiProfile, recovery_active: bool) -> ResourcePolicy {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.resources)
        .unwrap_or(profile.resources)
}

pub(super) fn active_barracks_curve(profile: &AiProfile, recovery_active: bool) -> BarracksCurve {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.barracks_curve)
        .unwrap_or(profile.buildings.barracks_curve)
}

pub(super) fn active_expansion_policy(
    profile: &AiProfile,
    recovery_active: bool,
) -> Option<ExpansionPolicy> {
    active_recovery(profile, recovery_active)
        .and_then(|recovery| recovery.expansion)
        .or(profile.expansion)
}

pub(super) fn recovery_delay_ticks(policy: RecoveryTransitionPolicy) -> Option<u32> {
    let build_ticks = config::unit_stats(policy.delay_unit)?.build_ticks;
    // The fast proxy should not stay all-in forever. Wait long enough for the proxy to have
    // produced a meaningful early rifle stream, then recover into economy and support tech.
    Some(build_ticks.saturating_mul(policy.delay_unit_build_count))
}
