//! Controlled authoritative progression, with ordinary income and no injected economy.
use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::lab::LabOp;
use rts_sim::game::{Game, PlayerInit};

#[test]
fn eco_classic_economy_reaches_four_bases_and_mixed_entrenched_defense() {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let map = rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        7,
        map,
        rts_sim::game::map::Map::metadata_for_name("Classic").unwrap(),
    );
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 2,
        enabled: true,
    })
    .unwrap();
    let mut controllers = [AiController::with_profile_id(1, "eco_ai")];
    let mut first_two = None;
    let mut first_three = None;
    let mut first_four = None;
    let mut meth_started_at_bases = None;
    let mut entrenched = false;
    let mut pf_seen = false;
    let mut mg_seen = false;
    for _ in 0..18000 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
        if game.current_tick() % 30 != 0 {
            continue;
        }
        let s = game.snapshot_for(1);
        let owned: Vec<_> = s.entities.iter().filter(|e| e.owner == 1).collect();
        let bases = owned
            .iter()
            .filter(|e| e.kind == "resource_depot" && e.build_progress.is_none())
            .count();
        let barracks = owned
            .iter()
            .filter(|e| e.kind == "barracks" && e.build_progress.is_none())
            .count();
        if bases >= 2 && first_two.is_none() {
            first_two = Some(s.tick);
            assert!(s.upgrades.iter().any(|u| u == "entrenchment"));
        }
        if bases >= 3 && first_three.is_none() {
            first_three = Some(s.tick);
        }
        if bases >= 4 && first_four.is_none() {
            first_four = Some(s.tick);
            assert!(
                barracks >= 4,
                "three-base stage must complete four Barracks before base four"
            );
        }
        if meth_started_at_bases.is_none()
            && owned
                .iter()
                .any(|e| e.prod_upgrade.as_deref() == Some("methamphetamines"))
        {
            meth_started_at_bases = Some(bases);
            assert_eq!(
                bases, 2,
                "second-base tech must not be deferred until later expansion"
            );
        }
        let mg_total = owned
            .iter()
            .filter(|e| {
                e.kind == "machine_gunner"
                    || (e.kind == "barracks" && e.prod_kind.as_deref() == Some("machine_gunner"))
            })
            .count();
        let pf_total = owned
            .iter()
            .filter(|e| {
                e.kind == "panzerfaust"
                    || (e.kind == "barracks" && e.prod_kind.as_deref() == Some("panzerfaust"))
            })
            .count();
        assert!(
            owned
                .iter()
                .filter(|e| e.kind == "barracks")
                .all(|e| e.prod_queue.unwrap_or(0) <= 1),
            "Eco must never create an unobservable second troop queue item"
        );
        mg_seen |= owned.iter().any(|e| e.kind == "machine_gunner");
        pf_seen |= owned.iter().any(|e| e.kind == "panzerfaust");
        entrenched |= owned
            .iter()
            .any(|e| e.kind == "machine_gunner" && e.occupied_trench_id.is_some());
        if s.tick % 1500 == 0 {
            eprintln!("Eco tick={} bases={bases} barracks={barracks} bank={}/{} supply={}/{} upgrades={:?} troops={}/{} trenches={entrenched}",s.tick,s.steel,s.oil,s.supply_used,s.supply_cap,s.upgrades,owned.iter().filter(|e|e.kind=="machine_gunner").count(),owned.iter().filter(|e|e.kind=="panzerfaust").count());
        }
        if bases >= 4
            && barracks >= 5
            && mg_seen
            && pf_seen
            && entrenched
            && mg_total.abs_diff(pf_total) <= 1
            && meth_started_at_bases == Some(2)
            && s.upgrades.iter().any(|u| u == "methamphetamines")
        {
            eprintln!("Eco milestones: two={first_two:?} three={first_three:?} four={first_four:?}, complete defense tick={}",s.tick);
            return;
        }
    }
    let s = game.snapshot_for(1);
    panic!("Eco incomplete: two={first_two:?} three={first_three:?} four={first_four:?} MG={mg_seen} PF={pf_seen} entrenched={entrenched} bank={}/{} upgrades={:?} own={:?}",s.steel,s.oil,s.upgrades,s.entities.iter().filter(|e|e.owner==1).map(|e|(&e.kind,&e.state,e.build_progress)).collect::<Vec<_>>());
}
