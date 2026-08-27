use super::*;

pub(super) fn active_tech_transition(
    observation: &AiObservation,
    profile: &AiProfile,
) -> Option<TechTransitionPolicy> {
    profile.tech_transition.filter(|transition| {
        resource_float_met(observation, *transition)
            || transition_tech_path_started(observation, profile, *transition)
    })
}

fn resource_float_met(observation: &AiObservation, transition: TechTransitionPolicy) -> bool {
    observation.economy.steel >= transition.resource_float.steel
        && observation.economy.oil >= transition.resource_float.oil
}

fn transition_tech_path_started(
    observation: &AiObservation,
    profile: &AiProfile,
    transition: TechTransitionPolicy,
) -> bool {
    transition
        .required_tech_path
        .iter()
        .copied()
        .filter(|kind| !profile.buildings.required_tech_path.contains(kind))
        .any(|kind| {
            observation.owned.iter().any(|entity| entity.kind == kind)
                || observation
                    .pending_builds
                    .iter()
                    .any(|build| build.kind == kind)
        })
}

pub(super) fn active_required_tech_path(
    observation: &AiObservation,
    profile: &AiProfile,
) -> &'static [EntityKind] {
    active_tech_transition(observation, profile)
        .map(|transition| transition.required_tech_path)
        .unwrap_or(profile.buildings.required_tech_path)
}

pub(super) fn active_production_policy(
    observation: &AiObservation,
    profile: &AiProfile,
) -> ProductionPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.production)
        .unwrap_or(profile.production)
}

pub(super) fn active_attack_policy(
    observation: &AiObservation,
    profile: &AiProfile,
) -> AttackPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.attack)
        .unwrap_or(profile.attack)
}

pub(super) fn active_worker_policy(profile: &AiProfile) -> WorkerPolicy {
    profile.workers
}

pub(super) fn active_resource_policy(profile: &AiProfile) -> ResourcePolicy {
    profile.resources
}

pub(super) fn active_barracks_curve(profile: &AiProfile) -> BarracksCurve {
    profile.buildings.barracks_curve
}

pub(super) fn active_expansion_policy(profile: &AiProfile) -> Option<ExpansionPolicy> {
    profile.expansion
}
