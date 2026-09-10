use super::*;
use crate::game::entity::{ProdItem, RallyIntent, RallyKind};

impl Game {
    pub fn new_warrior_portal_duel_scenario(
        scenario_case: Option<&str>,
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Warrior || unit_count != 1 {
            return Err(format!(
                "unsupported Warrior Portal duel launch {unit} x{unit_count}"
            ));
        }
        if scenario_case == Some("entrenched_riflemen") {
            return Self::new_warrior_entrenched_riflemen_scenario(seed);
        }
        if scenario_case.is_some() {
            return Err(format!(
                "unsupported Warrior combat scenario case {}",
                scenario_case.unwrap_or_default()
            ));
        }

        let mut map = flat_dev_map(2);
        let center = (map.width / 2, map.height / 2);
        let portal_tile = (center.0 - 5, center.1);
        let enemy_tile = (center.0 + 2, center.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = portal_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = enemy_tile;
        }

        let portal_pos = map.tile_center(portal_tile.0, portal_tile.1);
        let enemy_pos = map.tile_center(enemy_tile.0, enemy_tile.1);
        let rally = enemy_pos;
        let mut entities = EntityStore::new();
        let portal = entities
            .spawn_building(1, EntityKind::Portal, portal_pos.0, portal_pos.1, true)
            .ok_or_else(|| "failed to spawn Warrior Portal".to_string())?;
        let enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
            .ok_or_else(|| "failed to spawn Warrior duel Rifleman".to_string())?;

        let build_ticks = crate::rules::defs::unit_def(EntityKind::Warrior)
            .map(|definition| definition.stats.build_ticks)
            .ok_or_else(|| "missing Warrior unit definition".to_string())?;
        let producer = entities
            .get_mut(portal)
            .ok_or_else(|| "spawned Warrior Portal is missing".to_string())?;
        if !producer.push_production(ProdItem {
            unit: EntityKind::Warrior,
            progress: 0,
            total: build_ticks,
            paid: true,
        }) {
            return Err("failed to seed Warrior production".to_string());
        }
        producer.set_rally_point(Some(RallyIntent::new(
            RallyKind::AttackMove,
            rally.0,
            rally.1,
        )));

        if let Some(rifleman) = entities.get_mut(enemy) {
            rifleman.hold_position();
            rifleman.set_facing(std::f32::consts::PI);
        }

        let player_id = 1;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            portal_tile,
            seed,
            "dev:warrior_portal_duel",
        );
        if let Some(player) = game.state.players.iter_mut().find(|player| player.id == 1) {
            player.faction_id = crate::rules::faction::CULTIVATORS_FACTION_ID.to_string();
            player.set_supply_used(crate::rules::economy::supply_cost(EntityKind::Warrior));
        }

        DevScenarioSetup {
            game,
            player_id,
            units: Vec::new(),
            goal: rally,
            issue_after_ticks: u32::MAX,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:warrior_portal_duel")
    }

    fn new_warrior_entrenched_riflemen_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
        let mut map = flat_dev_map(2);
        let center = (map.width / 2, map.height / 2);
        let warrior_tile = (center.0 - 5, center.1);
        let rifleman_tile = (center.0 + 2, center.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = warrior_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = rifleman_tile;
        }

        let ts = config::TILE_SIZE as f32;
        let warrior_pos = map.tile_center(warrior_tile.0, warrior_tile.1);
        let rifleman_center = map.tile_center(rifleman_tile.0, rifleman_tile.1);
        let goal = (rifleman_center.0 + ts * 3.0, rifleman_center.1);
        let mut entities = EntityStore::new();
        let warrior = entities
            .spawn_unit(1, EntityKind::Warrior, warrior_pos.0, warrior_pos.1)
            .ok_or_else(|| "failed to spawn attack-moving Warrior".to_string())?;
        for offset_y in [-ts * 0.65, ts * 0.65] {
            let rifleman = entities
                .spawn_unit(
                    2,
                    EntityKind::Rifleman,
                    rifleman_center.0,
                    rifleman_center.1 + offset_y,
                )
                .ok_or_else(|| "failed to spawn entrenched Rifleman".to_string())?;
            if let Some(entity) = entities.get_mut(rifleman) {
                entity.hold_position();
                entity.set_facing(std::f32::consts::PI);
            }
        }
        if let Some(entity) = entities.get_mut(warrior) {
            entity.set_facing(0.0);
        }

        let player_id = 1;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            warrior_tile,
            seed,
            "dev:warrior_portal_duel:entrenched_riflemen",
        );
        if let Some(player) = game.state.players.iter_mut().find(|player| player.id == 1) {
            player.faction_id = crate::rules::faction::CULTIVATORS_FACTION_ID.to_string();
            player.set_supply_used(crate::rules::economy::supply_cost(EntityKind::Warrior));
        }
        if let Some(player) = game.state.players.iter_mut().find(|player| player.id == 2) {
            player.upgrades.insert(upgrade::UpgradeKind::Entrenchment);
        }

        DevScenarioSetup {
            game,
            player_id,
            units: vec![warrior],
            goal,
            issue_after_ticks: config::TICK_HZ * 6,
            order: DevScenarioOrder::AttackMove,
        }
        .checkpoint_backed("dev:warrior_portal_duel:entrenched_riflemen")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_produces_a_warrior_that_kills_the_rifleman() {
        let mut setup = Game::new_warrior_portal_duel_scenario(None, EntityKind::Warrior, 1, 7)
            .expect("Warrior Portal duel should build");
        for _ in 0..config::TICK_HZ * 30 {
            setup.game.tick();
        }
        assert!(setup
            .game
            .state
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::Warrior));
        assert!(!setup
            .game
            .state
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::Rifleman));
    }

    #[test]
    fn entrenched_riflemen_case_digs_in_before_the_warrior_attack_moves() {
        let mut setup = Game::new_warrior_portal_duel_scenario(
            Some("entrenched_riflemen"),
            EntityKind::Warrior,
            1,
            8,
        )
        .expect("entrenched Riflemen case should build");
        assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 6);
        assert!(matches!(setup.order, DevScenarioOrder::AttackMove));

        let warrior = setup
            .game
            .state
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Warrior)
            .expect("Warrior should exist");
        let rifleman_stats = crate::rules::defs::unit_def(EntityKind::Rifleman)
            .expect("Rifleman definition")
            .stats;
        let conservative_entrenched_range_px = (rifleman_stats.range_tiles
            + config::ENTRENCHMENT_RANGE_BONUS_TILES as f32)
            * config::TILE_SIZE as f32
            + rifleman_stats.radius
            + warrior.radius();
        assert!(setup
            .game
            .state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Rifleman)
            .all(|rifleman| {
                let dx = warrior.pos_x - rifleman.pos_x;
                let dy = warrior.pos_y - rifleman.pos_y;
                dx * dx + dy * dy
                    > conservative_entrenched_range_px * conservative_entrenched_range_px
            }));

        for _ in 0..config::ENTRENCHMENT_DIG_IN_TICKS {
            setup.game.tick();
        }
        assert_eq!(setup.game.state.trenches.all().len(), 2);
        assert!(setup
            .game
            .state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Rifleman)
            .all(|rifleman| {
                rifleman
                    .movement
                    .as_ref()
                    .and_then(|movement| movement.occupied_trench_id)
                    .is_some()
            }));
    }
}
