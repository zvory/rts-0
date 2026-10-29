use super::*;

#[test]
fn industrial_center_ai_upgrades_only_eligible_depots_near_supply_cap() {
    for (used, reserved, expected) in [
        (50, 0, None),
        (80, 0, Some(10)),
        (180, 1, Some(11)),
        (280, 2, None),
    ] {
        let mut depots: Vec<_> = (10..13)
            .map(|id| building(id, EntityKind::ResourceDepot, Some(0)))
            .collect();
        for depot in depots.iter_mut().take(reserved) {
            depot.industrial_center_slot_reserved = true;
        }
        let observation = observation(
            AiEconomy {
                steel: 800,
                oil: 300,
                supply_used: used,
                supply_cap: 100 + reserved as u32 * 100,
            },
            depots,
        );
        let facts = AiFacts::from_observation(&observation);
        let budget = SpendBudget::new(800, 300, used, observation.economy.supply_cap);
        let mut actions = AiActionContext::new(&facts, budget);
        let mut intents = Vec::new();
        queue_industrial_center(&mut actions, &observation, &facts, &mut intents);
        let actual = actions
            .into_commands()
            .into_iter()
            .find_map(|command| match command {
                Command::Research {
                    building,
                    upgrade: UpgradeKind::IndustrialCenter,
                } => Some(building),
                _ => None,
            });
        assert_eq!(actual, expected);
    }
}
