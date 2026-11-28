use super::*;
use std::collections::BTreeSet;

fn combat(kind: &str, beyond_midfield: bool) -> (StartPayload, Snapshot, Vec<AiBaseSite>, u32) {
    let (mut start, mut s, sites) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.doodads.clear();
    start.map.resources.clear();
    let specimen = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities
        .retain(|e| e.owner == 1 && e.kind == "resource_depot");
    let x = if beyond_midfield {
        110. * 32.
    } else {
        35. * 32.
    };
    let y = if beyond_midfield {
        100. * 32.
    } else {
        20. * 32.
    };
    let mut wounded = specimen.clone();
    wounded.id = 900;
    wounded.kind = kind.into();
    wounded.x = x;
    wounded.y = y;
    wounded.hp = 10;
    wounded.state = "attack".into();
    s.entities.push(wounded);
    let mut healthy = specimen.clone();
    healthy.id = 901;
    healthy.kind = kind.into();
    healthy.x = x - 32.;
    healthy.y = y;
    healthy.hp = 45;
    healthy.state = "idle".into();
    s.entities.push(healthy);
    let mut enemy = specimen;
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x = x + 64.;
    enemy.y = y;
    enemy.target_id = Some(900);
    enemy.hp = 45;
    enemy.state = "attack".into();
    s.entities.push(enemy);
    s.tick = 100;
    (start, s, sites, 900)
}
fn targets(request: &AiActionRequest, id: u32) -> bool {
    match request {
        AiActionRequest::Move { units, .. }
        | AiActionRequest::AttackMove { units, .. }
        | AiActionRequest::HoldPosition { units, .. } => units.contains(&id),
        _ => false,
    }
}
#[test]
fn wounded_combat_units_keep_retreat_ownership_then_resume_response_or_posts() {
    for kind in ["rifleman", "machine_gunner", "panzerfaust", "tank"] {
        let (start, mut s, sites, id) = combat(kind, false);
        let mut defense = tactics::DefenseAssignments::default();
        let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
        let f = frame(&start, &s, &sites);
        let owned = owned_sites(&f);
        let position = f.owned().iter().find(|e| e.id == id).unwrap().position;
        let mut a = AiActions::for_frame(&f);
        defense.step(&f, &mut a, &mut planner, &owned);
        let requests = a.into_requests();
        let issued: Vec<_> = requests.iter().filter(|r| targets(r, id)).collect();
        assert_eq!(
            issued.len(),
            1,
            "{kind} retreat must not be overwritten by response/guard orders"
        );
        assert!(matches!(issued[0],AiActionRequest::Move{x,y,..}
            if (*x-(position.0-3.*32.)).abs()<0.01 && (*y-position.1).abs()<0.01));
        s.tick = 109;
        s.entities
            .iter_mut()
            .find(|e| e.id == 999)
            .unwrap()
            .target_id = None;
        s.entities.iter_mut().find(|e| e.id == id).unwrap().state = "move".into();
        let f = frame(&start, &s, &sites);
        let mut a = AiActions::for_frame(&f);
        defense.step(&f, &mut a, &mut planner, &owned);
        assert!(
            a.into_requests().iter().all(|r| !targets(r, id)),
            "{kind} retreat ownership must persist on later decisions"
        );
        s.tick = 145;
        let f = frame(&start, &s, &sites);
        let mut a = AiActions::for_frame(&f);
        defense.step(&f, &mut a, &mut planner, &owned);
        assert!(
            a.into_requests().iter().any(|r| targets(r, id)),
            "{kind} must receive a fresh response or post route on release"
        );
    }
}
#[test]
fn new_unassigned_units_can_pull_back_without_becoming_defensive_anchors() {
    let (start, s, sites, id) = combat("machine_gunner", true);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    let mut defense = tactics::DefenseAssignments::default();
    defense.step(&f, &mut a, &mut DefensePlanner::new(), &owned_sites(&f));
    assert!(a
        .into_requests()
        .iter()
        .any(|r| matches!(r,AiActionRequest::Move{units,..} if units.contains(&id))));
    assert!(
        !defense.has_responders(),
        "an unassigned outbound unit must not define a new defended area"
    );
}
#[test]
fn low_health_without_visible_target_or_healthy_combat_cover_does_not_pull_back() {
    for no_cover in [false, true] {
        let (start, mut s, sites, id) = combat("machine_gunner", false);
        if no_cover {
            s.entities.iter_mut().find(|e| e.id == 901).unwrap().hp = 10;
        } else {
            s.entities
                .iter_mut()
                .find(|e| e.id == 999)
                .unwrap()
                .target_id = None;
        }
        let f = frame(&start, &s, &sites);
        let mut a = AiActions::for_frame(&f);
        tactics::DefenseAssignments::default().step(
            &f,
            &mut a,
            &mut DefensePlanner::new(),
            &owned_sites(&f),
        );
        assert!(!a.into_requests().iter().any(|r|matches!(r,AiActionRequest::Move{units,x,..} if units.contains(&id) && (*x-(35.*32.-96.)).abs()<0.01)));
    }
}
#[test]
fn retreating_infantry_no_longer_counts_as_the_expansion_screen() {
    let (start, mut s, sites) = fixture();
    let mut defense = tactics::DefenseAssignments::default();
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    defense.prepare_expansion(expansion(&f).unwrap().0);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    defense.step(&f, &mut AiActions::for_frame(&f), &mut planner, &owned);
    let id = *defense.screen_ids().iter().next().unwrap();
    let mut enemy = s.entities.iter().find(|e| e.id == id).unwrap().clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x += 64.;
    enemy.target_id = Some(id);
    enemy.hp = 45;
    s.entities.push(enemy);
    s.entities.iter_mut().find(|e| e.id == id).unwrap().hp = 10;
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert!(
        !defense.screen_ids().contains(&id),
        "a retreating unit cannot certify the Engineer's advance screen"
    );
}

#[test]
fn forward_owned_base_mobilizes_defenders_on_the_enemy_half() {
    let (mut start, mut s, sites, _) = combat("machine_gunner", true);
    start.map.resources = fixture().0.map.resources;
    s.entities.iter_mut().find(|e| e.id == 900).unwrap().hp = 45;
    building(
        &mut s,
        2000,
        "resource_depot",
        (110. * 32., 100. * 32.),
        None,
    );
    let f = frame(&start, &s, &sites);
    let mut defense = tactics::DefenseAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    defense.step(
        &f,
        &mut actions,
        &mut DefensePlanner::new(),
        &owned_sites(&f),
    );
    assert!(
        defense.has_responders(),
        "owned forward bases must be protected regardless of map bisector"
    );
    assert!(actions
        .into_requests()
        .iter()
        .any(|r| matches!(r,AiActionRequest::AttackMove{units,..} if units.contains(&900))));
}

#[test]
fn combat_borrows_home_posts_without_collapsing_the_four_base_plan() {
    let (mut start, mut s, sites) = super::tanks::staged(4);
    start.map.concealment_tiles.clear();
    for id in 1000..1060 {
        building(&mut s, id, "machine_gunner", (30. * 32., 20. * 32.), None);
    }
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut defense = tactics::DefenseAssignments::default();
    defense.step(&f, &mut AiActions::for_frame(&f), &mut planner, &owned);
    let posts = defense.assigned_posts().clone();
    assert!(posts.len() > 40);
    let anchor = *posts.values().next().unwrap();
    let template = s
        .entities
        .iter()
        .find(|e| e.kind == "rifleman" && e.owner == 1)
        .unwrap()
        .clone();
    for id in 9000..9012 {
        let mut enemy = template.clone();
        enemy.id = id;
        enemy.owner = 2;
        enemy.kind = "tank".into();
        enemy.x = anchor.0 + 3. * 32.;
        enemy.y = anchor.1;
        enemy.target_id = None;
        s.entities.push(enemy);
    }
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert!(defense.has_responders());
    assert_eq!(
        defense.assigned_posts(),
        &posts,
        "temporary combat ownership must not shrink the cut or discard return addresses"
    );
    s.entities.retain(|e| e.id < 9000);
    s.visible_tiles = vec![1; start.map.width as usize * start.map.height as usize];
    for e in s.entities.iter_mut().filter(|e| posts.contains_key(&e.id)) {
        e.state = "move".into();
    }
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert!(!defense.has_responders());
    assert_eq!(defense.assigned_posts(), &posts);
    let returns: BTreeSet<_> =
        a.into_requests()
            .iter()
            .flat_map(|r| match r {
                AiActionRequest::Move { units, .. }
                | AiActionRequest::HoldPosition { units, .. } => units.clone(),
                _ => vec![],
            })
            .collect();
    assert!(
        posts.keys().all(|id| returns.contains(id)),
        "release must replace stale retreat movement with home-post orders"
    );
}

#[test]
fn behind_line_presence_protects_even_exhausted_or_unfinished_buildings() {
    for (kind, progress) in [
        ("resource_depot", None),
        ("steel_mine", None),
        ("engineering_complex", Some(0.1)),
    ] {
        let (start, mut s, sites, _) = combat("machine_gunner", true);
        s.entities.iter_mut().find(|e| e.id == 900).unwrap().hp = 45;
        let enemy = s.entities.iter_mut().find(|e| e.id == 999).unwrap();
        enemy.target_id = None;
        enemy.state = "idle".into();
        building(&mut s, 2000, kind, (93. * 32., 100. * 32.), progress);
        let f = frame(&start, &s, &sites);
        let mut defense = tactics::DefenseAssignments::default();
        defense.step(
            &f,
            &mut AiActions::for_frame(&f),
            &mut DefensePlanner::new(),
            &owned_sites(&f),
        );
        assert!(defense.has_responders(),"an untargeted enemy19tiles from{kind}needs a response even with no remaining resources");
    }
}
