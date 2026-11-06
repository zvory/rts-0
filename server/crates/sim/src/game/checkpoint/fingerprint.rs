//! Versioned end-state projection. Never serialize a HashMap in its native iteration order.
use std::io::{self, BufWriter, Write};

use rand::RngCore;
use serde::{ser::SerializeMap, Serialize, Serializer};
use sha2::{Digest, Sha256};

use super::*;

pub(in crate::game) const END_STATE_HASH_ALGORITHM: &str = "rts-end-state-json-sha256-v1";

impl Game {
    /// Fingerprint committed simulation state, excluding unapplied input and historical commands.
    /// Ordered JSON is streamed through a small buffer; no full checkpoint or command-log clone.
    pub fn replay_end_state_hash(&self) -> Result<String, String> {
        let mut writer = BufWriter::with_capacity(64 * 1024, HashWriter(Sha256::new()));
        writer
            .write_all(END_STATE_HASH_ALGORITHM.as_bytes())
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut writer, &EndState(&self.state)).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(format!("{:x}", writer.get_ref().0.clone().finalize()))
    }
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct EndState<'a>(&'a GameState);
impl Serialize for EndState<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let state = self.0;
        let mut out = serializer.serialize_map(None)?;
        macro_rules! field {
            ($name:literal, $value:expr) => {
                out.serialize_entry($name, &$value)?;
            };
        }
        field!("simSchemaVersion", SIM_SCHEMA_VERSION);
        field!("map", MapBindingV1::from_state(state));
        field!("seed", state.seed);
        field!("tick", state.tick);
        field!("lastWorldCombatTick", state.last_world_combat_tick);
        field!("lastWorldCombatPosition", state.last_world_combat_position);
        field!(
            "worldCombatActiveThroughTick",
            state.world_combat_active_through_tick
        );
        field!("worldCombatPosition", state.world_combat_position);
        field!("rng", RngDescriptorV1::from_rng(&state.rng));
        // SmallRng is platform-dependent. Probe its future output without consuming live draws,
        // so equal seed/draw counters alone cannot mask different generator implementations.
        let mut rng = state.rng.clone();
        let probe: Vec<u32> = (0..8).map(|_| rng.next_u32()).collect();
        field!("rngProbe", probe);
        field!("players", state.players);
        field!("startingLoadouts", state.starting_loadouts);
        field!("startingLoadout", state.starting_loadout);
        field!("nextEntityId", state.entities.checkpoint_next_id());
        let entities: BTreeMap<_, _> = state.entities.iter().map(|e| (e.id, e)).collect();
        field!("entities", entities);
        field!("fog", FogStateV1::from_fog(&state.fog));
        field!(
            "buildingMemory",
            BuildingMemoryV1::from_memory(&state.building_memory)
        );
        field!(
            "antiTankGunMemory",
            AntiTankGunMemoryV1::from_memory(&state.anti_tank_gun_memory)
        );
        field!("lingeringSight", state.lingering_sight);
        field!("firingReveals", state.firing_reveals);
        field!("smokes", state.smokes);
        field!("trenches", state.trenches);
        field!("groundDecals", state.ground_decals);
        field!("abilityRuntime", state.ability_runtime);
        field!("mortarShells", state.mortar_shells);
        field!("artilleryShells", state.artillery_shells);
        field!("panzerfaustShots", state.panzerfaust_shots);
        field!("activeConstructionSites", state.active_construction_sites);
        field!("labGodModePlayers", state.lab_god_mode_players);
        out.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::ReplayStartComposition;

    fn game() -> Game {
        Game::new(
            &[super::super::super::PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".into(),
                name: "Hash".into(),
                color: "#fff".into(),
                is_ai: false,
            }],
            42,
        )
    }

    #[test]
    fn end_state_hash_covers_hidden_state_and_ignores_unapplied_input_and_history() {
        let mut game = game();
        let baseline = game.replay_end_state_hash().unwrap();
        let mut copy = game.clone_for_replay_keyframe();
        assert_eq!(baseline, copy.replay_end_state_hash().unwrap());
        copy.state.players[0].oil += 1;
        assert_ne!(baseline, copy.replay_end_state_hash().unwrap());
        let mut copy = game.clone_for_replay_keyframe();
        let id = copy.state.entities.ids()[0];
        let entity = copy.state.entities.get_mut(id).unwrap();
        entity.pos_x = f32::from_bits(entity.pos_x.to_bits() + 1);
        assert_ne!(
            baseline,
            copy.replay_end_state_hash().unwrap(),
            "one float ULP must not be rounded away"
        );
        let _ = game.state.rng.next_u32();
        assert_ne!(baseline, game.replay_end_state_hash().unwrap());
        let after_rng = game.replay_end_state_hash().unwrap();
        game.enqueue(1, crate::game::command::SimCommand::Stop { units: vec![1] });
        game.state.command_log.push(CommandLogEntry {
            tick: 1,
            player_id: 1,
            command: crate::protocol::Command::Stop { units: vec![1] },
        });
        assert_eq!(after_rng, game.replay_end_state_hash().unwrap());
    }

    #[test]
    fn end_state_hash_ignores_hashmap_insertion_order_and_checkpoint_roundtrip() {
        let mut game = game();
        for _ in 0..3 {
            game.tick();
        }
        let baseline = game.replay_end_state_hash().unwrap();
        let mut entities = game.state.entities.checkpoint_entities();
        entities.reverse();
        game.state.entities = EntityStore::from_checkpoint_entities(
            game.state.entities.checkpoint_next_id(),
            entities,
        );
        assert_eq!(baseline, game.replay_end_state_hash().unwrap());
        let restored = Game::restore_checkpoint_payload_text(
            &game.checkpoint_payload_text().unwrap(),
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        assert_eq!(baseline, restored.replay_end_state_hash().unwrap());
    }

    #[test]
    fn end_state_hash_detects_divergence_and_handles_missing_or_unknown_hashes() {
        let game = game();
        let mut artifact = ReplayStartComposition::capture(&game, "test")
            .unwrap()
            .finalize(&game, None, game.scores());
        assert_eq!(artifact.verify_end_state(&game).unwrap(), Some(true));
        artifact.end_state_hash.as_mut().unwrap().digest = "0".repeat(64);
        assert_eq!(artifact.verify_end_state(&game).unwrap(), Some(false));
        artifact.end_state_hash.as_mut().unwrap().algorithm = "future".into();
        assert!(artifact.verify_end_state(&game).is_err());
        artifact.end_state_hash = None;
        assert_eq!(artifact.verify_end_state(&game).unwrap(), None);
    }

    #[test]
    fn sha256_stream_has_standard_known_answer() {
        let mut writer = HashWriter(Sha256::new());
        writer.write_all(b"abc").unwrap();
        assert_eq!(
            format!("{:x}", writer.0.finalize()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    #[ignore = "release-only end-state cost measurement; run with --release --ignored --nocapture"]
    fn end_state_hash_cost() {
        let players: Vec<_> = (1..=4)
            .map(|id| super::super::super::PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".into(),
                name: format!("Player {id}"),
                color: "#fff".into(),
                is_ai: false,
            })
            .collect();
        let map =
            Map::load_for_players("4 Player Map", &[(1, 1), (2, 2), (3, 3), (4, 4)], 42).unwrap();
        let metadata = Map::metadata_for_name("4 Player Map").unwrap();
        let mut game =
            Game::new_with_random_ai_profiles_and_map_metadata(&players, 42, map, metadata);
        let original_count = game.state.entities.iter().count();
        for i in original_count..2_000 {
            game.state.entities.spawn_unit(
                1,
                crate::game::EntityKind::Rifleman,
                200.0 + (i % 100) as f32,
                200.0 + (i / 100) as f32,
            );
        }
        let entry = CommandLogEntry {
            tick: 1,
            player_id: 1,
            command: crate::protocol::Command::Stop { units: vec![1] },
        };
        let without_history = game.replay_end_state_hash().unwrap();
        game.state.command_log = vec![entry; 200_000];
        let expected = game.replay_end_state_hash().unwrap();
        assert_eq!(without_history, expected);
        let mut samples = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            assert_eq!(game.replay_end_state_hash().unwrap(), expected);
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        eprintln!("END_STATE_HASH_BENCH entities={} historical_commands={} rounds=100 median_us={} p95_us={} max_us={}",
            game.state.entities.iter().count(), game.state.command_log.len(), samples[50], samples[95], samples[99]);
    }
}
