//! Pages for the game's own things: main page, factions, units, buildings, research, abilities.

use super::*;

// ---------------------------------------------------------------- main page

pub(super) fn index(w: &Wiki) {
    let file = "index.html";
    let g = w.game();
    let mut h = String::new();
    let intro = w
        .content
        .get("main")
        .map(|d| w.md(&d.body, file))
        .unwrap_or_default();
    let _ = write!(
        h,
        "<section class=\"banner\"><div class=\"prose\">{intro}</div><p class=\"muted\">Game version <a href=\"{}/commit/{}\"><code>{}</code></a>{}.</p></section>",
        esc(&s(g, "repo")),
        esc(&s(g, "commit")),
        esc(&s(g, "short")),
        match day(&s(g, "date")) {
            d if d.is_empty() => String::new(),
            d => format!(" ({})", esc(&d)),
        },
    );
    h.push_str("<div class=\"portal-grid\">");
    for f in w.list("factions") {
        let fid = s(f, "id");
        let _ = write!(
            h,
            "<section class=\"portal fac-{fid}\"><h2><a href=\"faction-{fid}.html\">{}</a></h2>",
            esc(&w.faction_name(&fid))
        );
        if let Some(sum) = w.note("factions", &fid).and_then(|d| d.get("summary")) {
            let _ = write!(h, "<p class=\"muted\">{}</p>", esc(sum));
        }
        h.push_str("<h3>Units</h3><div class=\"card-grid\">");
        for u in strs(f, "units") {
            if let Some(v) = w.unit(&u) {
                h.push_str(&card(w, "unit", &u, v));
            }
        }
        h.push_str("</div><h3>Buildings</h3><div class=\"card-grid small\">");
        for bid in strs(f, "buildings") {
            if let Some(v) = w.building(&bid) {
                h.push_str(&card(w, "building", &bid, v));
            }
        }
        h.push_str("</div>");
        let rs: Vec<String> = arr(f, "upgrades")
            .iter()
            .map(|u| w.link_kind("research", &s(u, "id")))
            .collect();
        if !rs.is_empty() {
            let _ = write!(
                h,
                "<h3>Research</h3><p class=\"inline-list\">{}</p>",
                rs.join(" · ")
            );
        }
        h.push_str("</section>");
    }
    h.push_str("</div><div class=\"portal-grid three\">");
    // latest changes
    h.push_str("<section class=\"portal\"><h2><a href=\"changes.html\">Latest changes</a></h2>");
    match (1..w.snaps.len()).rev().find(|i| !w.diffs[*i].is_empty()) {
        Some(i) => {
            let _ = write!(h, "<div class=\"muted\">{}</div>", pull_heading(w, i));
            for g in change_groups(w, i).iter().filter(|g| g.kind != "constant").take(5) {
                h.push_str(&group_html(w, g));
            }
            let _ = write!(h, "<p><a href=\"changes.html\">Full changelog →</a></p>");
        }
        None => h.push_str("<p class=\"muted\">No changes recorded yet. The changelog comes from <code>docs/wiki/history.json</code>; see <a href=\"help-editing.html\">Editing this wiki</a>.</p>"),
    }
    h.push_str("</section><section class=\"portal\"><h2>Mechanics &amp; guides</h2>");
    for (section, docs) in page_docs(w) {
        let _ = write!(h, "<h3>{}</h3><ul>", esc(&title_case(&section)));
        for (_, title, f) in docs {
            let _ = write!(h, "<li><a href=\"{f}\">{}</a></li>", esc(&title));
        }
        h.push_str("</ul>");
    }
    h.push_str(
        "</section><section class=\"portal\"><h2>Reference</h2><ul>\
        <li><a href=\"table-units.html\">Unit comparison table</a></li>\
        <li><a href=\"table-buildings.html\">Building comparison table</a></li>\
        <li><a href=\"research.html\">All research</a></li>\
        <li><a href=\"abilities.html\">All abilities</a></li>\
        <li><a href=\"weapons.html\">Weapons &amp; damage matrix</a></li>\
        <li><a href=\"constants.html\">Every balance constant</a></li>\
        <li><a href=\"changes.html\">Rule change history</a></li></ul></section></div>",
    );
    write(w, file, "Bewegungskrieg Wiki", &[], &h);
}

// ---------------------------------------------------------------- faction

pub(super) fn faction_page(w: &Wiki, f: &Value) {
    let fid = s(f, "id");
    let file = format!("faction-{fid}.html");
    let name = w.faction_name(&fid);
    let mut h = String::new();
    let lo = get(f, "loadout");
    let start: Vec<String> = arr(lo, "starting")
        .iter()
        .map(|g| format!("{} × {}", s(g, "count"), w.link(&s(g, "kind"))))
        .collect();
    let role = |key: &str| {
        let ids = strs(f, key);
        if ids.is_empty() {
            String::new()
        } else {
            w.links(&ids)
        }
    };
    let mut rows = vec![
        Row::Section("Start".into()),
        field("Resources", cost(nz(lo, "steel"), nz(lo, "oil"))),
        field("Starting units", start.join("<br>")),
        Row::Section("Workers".into()),
        field("Builds", role("builders")),
        field("Gathers", role("gatherers")),
    ];
    let opening = strs(lo, "openingUpgrades");
    if !opening.is_empty() {
        rows.push(field(
            "Starts with research",
            opening
                .iter()
                .map(|r| w.link_kind("research", r))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    let picture = w.badge(
        &format!("faction-{fid}"),
        &name.chars().take(2).collect::<String>().to_uppercase(),
        "lg",
    );
    h.push_str(&infobox(
        &format!("fac-{fid}"),
        &name,
        &picture,
        "Faction",
        &rows,
    ));
    h.push_str(&notes_html(w, "factions", &fid, &file));

    // tech tree tiers
    h.push_str(&h2("tech-tree", "Tech tree"));
    let blds = strs(f, "buildings");
    let mut tier: HashMap<String, usize> = HashMap::new();
    fn depth(
        w: &Wiki,
        id: &str,
        own: &[String],
        memo: &mut HashMap<String, usize>,
        guard: usize,
    ) -> usize {
        if let Some(d) = memo.get(id) {
            return *d;
        }
        let reqs: Vec<String> = w
            .building(id)
            .map(|b| strs(b, "requires"))
            .unwrap_or_default();
        let d = reqs
            .iter()
            .filter(|r| own.contains(r) && guard < 20)
            .map(|r| depth(w, r, own, memo, guard + 1) + 1)
            .max()
            .unwrap_or(0);
        memo.insert(id.to_string(), d);
        d
    }
    for bid in &blds {
        let d = depth(w, bid, &blds, &mut tier, 0);
        tier.insert(bid.clone(), d);
    }
    let max = tier.values().copied().max().unwrap_or(0);
    h.push_str("<div class=\"tech-tree\">");
    for t in 0..=max {
        let _ = write!(
            h,
            "<div class=\"tier\"><div class=\"tier-head\">Tier {}</div>",
            t + 1
        );
        for bid in blds.iter().filter(|b| tier.get(*b) == Some(&t)) {
            let Some(bv) = w.building(bid) else { continue };
            let reqs = strs(bv, "requires");
            let units: Vec<String> = arr(f, "trainables")
                .iter()
                .filter(|x| s(x, "building") == *bid)
                .flat_map(|x| strs(x, "units"))
                .filter(|u| w.unit(u).is_some())
                .collect();
            let research: Vec<String> = arr(f, "upgrades")
                .iter()
                .filter(|u| s(u, "researchedAt") == *bid)
                .map(|u| s(u, "id"))
                .collect();
            let _ = write!(
                h,
                "<div class=\"tree-node\">{}",
                card(w, "building", bid, bv)
            );
            if !reqs.is_empty() {
                let _ = write!(h, "<div class=\"tree-line\">needs {}</div>", w.links(&reqs));
            }
            if !units.is_empty() {
                let _ = write!(
                    h,
                    "<div class=\"tree-line\">trains {}</div>",
                    w.links(&units)
                );
            }
            if !research.is_empty() {
                let _ = write!(
                    h,
                    "<div class=\"tree-line\">researches {}</div>",
                    research
                        .iter()
                        .map(|r| w.link_kind("research", r))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            h.push_str("</div>");
        }
        h.push_str("</div>");
    }
    h.push_str("</div>");

    // units by producer
    h.push_str(&h2("units", "Units"));
    h.push_str("<table class=\"wikitable sortable\"><thead><tr><th>Unit</th><th>Trained at</th><th>Cost</th><th>Supply</th><th>Time</th><th>HP</th><th>Armor</th><th>Damage</th><th>Range</th><th>DPS</th></tr></thead><tbody>");
    for uid in strs(f, "units") {
        let Some(u) = w.unit(&uid) else { continue };
        let at = get(u, "attack");
        let _ = write!(
            h,
            "<tr><td>{} {}</td><td>{}</td><td data-v=\"{}\">{}</td><td class=\"num\">{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            w.badge(&uid, &s(u, "icon"), "xs"),
            w.link(&uid),
            if s(u, "trainedAt").is_empty() { "—".into() } else { w.link(&s(u, "trainedAt")) },
            nz(u, "steel") + nz(u, "oil"),
            cost_of(u),
            fmt(nz(u, "supply")),
            nz(u, "buildTicks"),
            w.u.secs(nz(u, "buildTicks")),
            fmt(nz(u, "hp")),
            esc(&s(u, "armor")),
            fmt(nz(at, "dmg")),
            fmt(nz(at, "rangeTiles")),
            w.weapon(&s(u, "weapon")).map(|wv| weapon_dps(w, wv)).unwrap_or_else(|| dps(w, nz(at, "dmg"), nz(at, "cooldownTicks"))),
        );
    }
    h.push_str("</tbody></table>");

    h.push_str(&h2("buildings", "Buildings"));
    h.push_str("<table class=\"wikitable sortable\"><thead><tr><th>Building</th><th>Cost</th><th>Build time</th><th>HP</th><th>Footprint</th><th>Requires</th></tr></thead><tbody>");
    for bid in &blds {
        let Some(bv) = w.building(bid) else { continue };
        let _ = write!(
            h,
            "<tr><td>{} {}</td><td data-v=\"{}\">{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\">{}</td><td>{}×{}</td><td>{}</td></tr>",
            w.badge(bid, &s(bv, "icon"), "xs"),
            w.link(bid),
            nz(bv, "steel") + nz(bv, "oil"),
            cost_of(bv),
            nz(bv, "buildTicks"),
            w.u.secs(nz(bv, "buildTicks")),
            fmt(nz(bv, "hp")),
            s(bv, "footW"),
            s(bv, "footH"),
            w.links(&strs(bv, "requires")),
        );
    }
    h.push_str("</tbody></table>");

    let ups: Vec<String> = arr(f, "upgrades").iter().map(|u| s(u, "id")).collect();
    if !ups.is_empty() {
        h.push_str(&h2("research", "Research"));
        h.push_str(&research_table(w, &ups));
    }
    let abil = faction_abilities(w, f);
    if !abil.is_empty() {
        h.push_str(&h2("abilities", "Abilities"));
        h.push_str("<div class=\"ability-list\">");
        for a in &abil {
            if let Some(av) = w.ability(a) {
                h.push_str(&ability_card(w, av, &file));
            }
        }
        h.push_str("</div>");
    }
    h.push_str(&related_constants(w, "faction", &fid));
    h.push_str(&changelog(w, "faction", &fid));
    write(w, &file, &name, &[], &h);
}

/// Abilities a unit or building of this faction can use (catalogs share a few entries).
pub(super) fn faction_abilities(w: &Wiki, f: &Value) -> Vec<String> {
    let own_units = strs(f, "units");
    let own_blds = strs(f, "buildings");
    strs(f, "abilities")
        .into_iter()
        .filter(|a| {
            w.ability(a).is_some_and(|av| {
                let c = strs(av, "carriers");
                c.is_empty()
                    || c.iter()
                        .any(|x| own_units.contains(x) || own_blds.contains(x))
            })
        })
        .collect()
}

pub(super) fn research_table(w: &Wiki, ids: &[String]) -> String {
    let mut h = String::from("<table class=\"wikitable sortable\"><thead><tr><th>Research</th><th>Researched at</th><th>Cost</th><th>Time</th><th>Requires</th><th>Effect (in-game text)</th></tr></thead><tbody>");
    for id in ids {
        let Some(r) = w.research(id) else { continue };
        let req = s(r, "requiresUpgrade");
        let _ = write!(
            h,
            "<tr><td>{} {}</td><td>{}</td><td data-v=\"{}\">{}</td><td class=\"num\" data-v=\"{}\">{}</td><td>{}</td><td>{}</td></tr>",
            w.badge(id, &s(r, "icon"), "xs"),
            w.link_kind("research", id),
            w.link(&s(r, "researchedAt")),
            nz(r, "steel") + nz(r, "oil"),
            cost_of(r),
            nz(r, "researchTicks"),
            n(r, "researchTicks").map(|t| w.u.secs(t)).unwrap_or("—".into()),
            if req.is_empty() { "—".into() } else { w.link_kind("research", &req) },
            esc(&s(r, "description")),
        );
    }
    h.push_str("</tbody></table>");
    h
}

// ---------------------------------------------------------------- unit

pub(super) fn unit_page(w: &Wiki, u: &Value) {
    let id = s(u, "id");
    let file = format!("unit-{id}.html");
    let name = s(u, "name");
    let factions = w.factions_of("unit", &id);
    let at = get(u, "attack");
    let mut req: Vec<String> = Vec::new();
    let tr = get(u, "trainRequirement");
    // the producer itself is already shown as "Trained at"
    let kinds: Vec<String> = strs(tr, "kinds")
        .into_iter()
        .filter(|k| *k != s(u, "trainedAt"))
        .collect();
    if !kinds.is_empty() {
        let sep = if s(tr, "mode") == "any" { " or " } else { ", " };
        req.push(
            kinds
                .iter()
                .map(|k| w.link(k))
                .collect::<Vec<_>>()
                .join(sep),
        );
    }
    let upg = s(u, "upgradeRequires");
    if !upg.is_empty() {
        req.push(format!("{} research", w.link_kind("research", &upg)));
    }
    let weapon = s(u, "weapon");
    let weapon_v = w.weapon(&weapon);
    let key = format!("unit:{id}");
    // the attack numbers come from the weapon profile, or from the unit itself for Cultivators
    let (akey, adm, arg, acd) = if b(u, "disciple") || weapon_v.is_none() {
        (
            key.clone(),
            "stats.dmg",
            "stats.range_tiles",
            "stats.cooldown",
        )
    } else {
        (format!("weapon:{weapon}"), "dmg", "range_tiles", "cooldown")
    };
    let mut rows = vec![
        Row::Section("Production".into()),
        field(
            "Cost",
            cost_coded(
                w,
                &key,
                "stats.cost_steel",
                "stats.cost_oil",
                nz(u, "steel"),
                nz(u, "oil"),
            ),
        ),
        field(
            "Supply",
            w.coded(&key, "stats.supply", fmt(nz(u, "supply"))),
        ),
        field(
            "Build time",
            w.coded(&key, "stats.build_ticks", w.u.secs(nz(u, "buildTicks"))),
        ),
        field(
            "Trained at",
            if s(u, "trainedAt").is_empty() {
                String::new()
            } else {
                w.link(&s(u, "trainedAt"))
            },
        ),
        field("Requires", req.join("<br>")),
        Row::Section("Defense".into()),
        field("HP", w.coded(&key, "stats.hp", fmt(nz(u, "hp")))),
        field("Armor", w.coded(&key, "armor_class", esc(&s(u, "armor")))),
    ];
    if nz(at, "dmg") > 0.0 || weapon_v.is_some() {
        rows.push(Row::Section("Attack".into()));
        let dmg_text = weapon_v
            .filter(|wv| !get(wv, "burst").is_null())
            .map(weapon_dmg_text)
            .unwrap_or_else(|| fmt(nz(at, "dmg")));
        rows.push(field("Damage", w.coded(&akey, adm, dmg_text)));
        let sr = get(u, "stationaryRange");
        match (n(u, "deployedRangeTiles"), n(sr, "maxTiles")) {
            (Some(d), _) => rows.push(field(
                "Range",
                format!(
                    "{} <span class=\"muted\">(set up)</span>",
                    w.coded_const("ANTI_TANK_GUN_DEPLOYED_RANGE_TILES", tiles(d))
                ),
            )),
            (None, Some(max)) => rows.push(field(
                "Range",
                format!(
                    "{} → {} <span class=\"muted\">(after {} still)</span>",
                    w.coded(&akey, arg, fmt(nz(at, "rangeTiles"))),
                    w.coded_const("TANK_STATIONARY_RANGE_MAX_TILES", tiles(max)),
                    w.coded_const(
                        "TANK_STATIONARY_RANGE_RAMP_TICKS",
                        w.u.secs(nz(sr, "rampTicks"))
                    )
                ),
            )),
            _ => rows.push(field(
                "Range",
                w.coded(&akey, arg, tiles(nz(at, "rangeTiles"))),
            )),
        }
        let eng = nz(u, "engagementRangeTiles");
        if eng > nz(at, "rangeTiles") {
            rows.push(field(
                "Acquisition range",
                w.coded_const("DISCIPLE_ENGAGEMENT_RANGE_TILES", tiles(eng)),
            ));
        }
        rows.push(field(
            "Cooldown",
            w.coded(&akey, acd, w.u.secs(nz(at, "cooldownTicks"))),
        ));
        rows.push(field(
            "DPS",
            weapon_v
                .map(|wv| weapon_dps(w, wv))
                .unwrap_or_else(|| dps(w, nz(at, "dmg"), nz(at, "cooldownTicks"))),
        ));
        if let Some(wv) = weapon_v {
            rows.push(field(
                "Weapon",
                format!(
                    "<a href=\"weapons.html#w-{weapon}\">{}</a><br><span class=\"muted\">{}, armor pen. {}%</span>",
                    esc(&weapon_name(&weapon)),
                    esc(&human(&s(wv, "weaponClass"))),
                    fmt(nz(wv, "armorPenetration") * 100.0)
                ),
            ));
        }
    }
    rows.push(Row::Section("Movement & vision".into()));
    rows.push(field(
        "Speed",
        w.coded(
            &key,
            "stats.speed",
            format!("{} tiles/s", fmt2(w.u.speed(nz(u, "speedPxPerTick")))),
        ),
    ));
    rows.push(field(
        "Sight",
        w.coded(&key, "stats.sight_tiles", tiles(nz(u, "sightTiles"))),
    ));
    let sub = factions
        .iter()
        .map(|f| {
            format!(
                "<a href=\"faction-{f}.html\">{}</a>",
                esc(&w.faction_name(f))
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
        + " unit";
    let mut h = infobox(
        &w.faction_class("unit", &id),
        &name,
        &w.badge(&id, &s(u, "icon"), "lg"),
        &sub,
        &rows,
    );
    h.push_str(&lead(w, "units", &id, unit_lead(w, u)));
    h.push_str(&description(u));
    h.push_str(&notes_html(w, "units", &id, &file));

    // weapons
    let mut weapon_ids: Vec<String> = Vec::new();
    if !weapon.is_empty() {
        weapon_ids.push(weapon.clone());
    }
    weapon_ids.extend(strs(u, "secondaryWeapons"));
    if !weapon_ids.is_empty() {
        h.push_str(&h2("weapons", "Weapons"));
        h.push_str("<table class=\"wikitable\"><thead><tr><th>Weapon</th><th>Damage</th><th>Range</th><th>Cooldown</th><th>DPS</th><th>Class</th><th>Armor pen.</th><th>Notes</th></tr></thead><tbody>");
        for wid in &weapon_ids {
            let Some(wv) = w.weapon(wid) else { continue };
            let mut notes = Vec::new();
            if wid != &weapon {
                notes.push("secondary".to_string());
            }
            if b(wv, "facingArmor") {
                notes.push("facing".into());
            }
            let pol = s(wv, "infantryPolicy");
            if pol != "None" && !pol.is_empty() {
                notes.push(human(&pol));
            }
            let _ = write!(
                h,
                "<tr><td><a href=\"weapons.html#w-{wid}\">{}</a></td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}%</td><td>{}</td></tr>",
                esc(&weapon_name(wid)),
                weapon_dmg_text(wv),
                fmt(nz(wv, "rangeTiles")),
                w.u.secs(nz(wv, "cooldownTicks")),
                weapon_dps(w, wv),
                esc(&human(&s(wv, "weaponClass"))),
                fmt(nz(wv, "armorPenetration") * 100.0),
                esc(&notes.join("; ")),
            );
        }
        h.push_str("</tbody></table>");
    }

    // abilities
    let abilities: Vec<&Value> = w
        .list("abilities")
        .iter()
        .filter(|a| strs(a, "carriers").contains(&id) && !arr(a, "factions").is_empty())
        .collect();
    if !abilities.is_empty() {
        h.push_str(&h2("abilities", "Abilities"));
        h.push_str("<div class=\"ability-list\">");
        for a in abilities {
            h.push_str(&ability_card(w, a, &file));
        }
        h.push_str("</div>");
    }

    // traits
    let mut traits = Vec::new();
    if b(u, "firesWhileMoving") {
        traits.push("Fires while moving.".to_string());
    }
    if let Some(t) = n(u, "setupTicks") {
        let mut line = format!("Set up: {}", w.u.secs(t));
        if let Some(td) = n(u, "teardownTicks") {
            let _ = write!(line, " · Pack up: {}", w.u.secs(td));
        }
        if b(u, "firesOnlyWhenSetUp") {
            line.push_str(" · No damage until set up");
        }
        traits.push(line);
    }
    if b(u, "manualEmplacement") {
        traits.push("Can be set up facing a chosen direction.".into());
    }
    if b(u, "entrenchmentEligible") {
        traits.push(
            "Can entrench (needs <a href=\"research-entrenchment.html\">Entrenchment</a>).".into(),
        );
    }
    if b(u, "rifleInfantry") {
        traits.push("Rifle infantry: boosted by <a href=\"research-methamphetamines.html\">Methamphetamines</a>.".into());
    }
    if b(u, "slowedByBarbedWire") {
        traits.push(
            "Slowed and damaged by <a href=\"building-barbed_wire.html\">Barbed Wire</a>.".into(),
        );
    }
    if b(u, "crushesBarbedWire") {
        traits.push("Crushes <a href=\"building-barbed_wire.html\">Barbed Wire</a>.".into());
    }
    if s(u, "threatRole") == "AntiArmorThreat" {
        traits.push("Enemy anti-tank weapons target it first.".into());
    }
    if !traits.is_empty() {
        h.push_str(&h2("traits", "Traits"));
        let _ = write!(
            h,
            "<ul class=\"traits\">{}</ul>",
            traits
                .iter()
                .map(|t| format!("<li>{t}</li>"))
                .collect::<String>()
        );
    }

    // damage dealt
    let dealing: Vec<&Value> = weapon_ids
        .iter()
        .filter_map(|x| w.weapon(x))
        .filter(|wv| nz(wv, "dmg") > 0.0)
        .collect();
    if !dealing.is_empty() {
        h.push_str(&h2("damage-dealt", "Damage dealt"));
        h.push_str("<p class=\"muted\">Damage per hit after armor, on open ground. Hits to kill assume every shot lands; time counts from the first shot.</p>");
        for (i, wv) in dealing.iter().enumerate() {
            let _ = write!(
                h,
                "<details{}><summary>{}</summary>{}</details>",
                if i == 0 { " open" } else { "" },
                esc(&weapon_name(&s(wv, "id"))),
                weapon_damage_table(w, wv)
            );
        }
    }
    h.push_str(&h2("damage-taken", "Damage taken"));
    let _ = write!(
        h,
        "<details><summary>Every weapon vs {} ({} HP)</summary>{}</details>",
        esc(&name),
        fmt(nz(u, "hp")),
        damage_taken_table(w, &id)
    );
    h.push_str(&related_constants(w, "unit", &id));
    h.push_str(&changelog(w, "unit", &id));
    for f in &factions {
        h.push_str(&navbox(w, f));
    }
    let crumbs: Vec<(String, String)> = factions
        .first()
        .map(|f| vec![(format!("faction-{f}.html"), w.faction_name(f))])
        .unwrap_or_default();
    write(w, &file, &name, &crumbs, &h);
}

// ---------------------------------------------------------------- building

pub(super) fn building_page(w: &Wiki, bv: &Value) {
    let id = s(bv, "id");
    let file = format!("building-{id}.html");
    let name = s(bv, "name");
    let factions = w.factions_of("building", &id);
    let key = format!("building:{id}");
    let mut rows = vec![
        Row::Section("Construction".into()),
        field(
            "Cost",
            cost_coded(
                w,
                &key,
                "stats.cost_steel",
                "stats.cost_oil",
                nz(bv, "steel"),
                nz(bv, "oil"),
            ),
        ),
        field(
            "Build time",
            w.coded(&key, "stats.build_ticks", w.u.secs(nz(bv, "buildTicks"))),
        ),
        field(
            "Requires",
            if strs(bv, "requires").is_empty() {
                String::new()
            } else {
                w.links(&strs(bv, "requires"))
            },
        ),
        field("Built by", {
            let builders: Vec<String> = factions
                .iter()
                .filter_map(|f| w.faction(f))
                .flat_map(|f| strs(f, "builders"))
                .collect();
            if factions
                .iter()
                .filter_map(|f| w.faction(f))
                .any(|f| strs(f, "buildables").contains(&id))
            {
                w.links(&builders)
            } else {
                String::new()
            }
        }),
        Row::Section("Structure".into()),
        field("HP", w.coded(&key, "stats.hp", fmt(nz(bv, "hp")))),
        field("Armor", w.coded(&key, "armor_class", esc(&s(bv, "armor")))),
        field(
            "Footprint",
            format!(
                "{} × {} tiles",
                w.coded(&key, "stats.foot_w", s(bv, "footW")),
                w.coded(&key, "stats.foot_h", s(bv, "footH"))
            ),
        ),
        field(
            "Sight",
            w.coded(&key, "stats.sight_tiles", tiles(nz(bv, "sightTiles"))),
        ),
    ];
    if nz(bv, "dmg") > 0.0 {
        rows.push(Row::Section("Attack".into()));
        rows.push(field("Damage", fmt(nz(bv, "dmg"))));
        rows.push(field("Range", tiles(nz(bv, "rangeTiles"))));
        rows.push(field("Cooldown", w.u.secs(nz(bv, "cooldownTicks"))));
    }
    let extracts = s(bv, "extracts");
    if !extracts.is_empty() {
        rows.push(field("Extracts", esc(&w.name_of(&extracts))));
    }
    let sub = factions
        .iter()
        .map(|f| {
            format!(
                "<a href=\"faction-{f}.html\">{}</a>",
                esc(&w.faction_name(f))
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
        + " building";
    let mut h = infobox(
        &w.faction_class("building", &id),
        &name,
        &w.badge(&id, &s(bv, "icon"), "lg"),
        &sub,
        &rows,
    );
    h.push_str(&lead(w, "buildings", &id, building_lead(w, bv)));
    h.push_str(&description(bv));
    h.push_str(&notes_html(w, "buildings", &id, &file));

    let trains: Vec<String> = strs(bv, "trains");
    if !trains.is_empty() {
        h.push_str(&h2("trains", "Trains"));
        h.push_str("<div class=\"card-grid\">");
        for t in &trains {
            if let Some(v) = w.unit(t).or_else(|| w.building(t)) {
                h.push_str(&card(w, w.kind_of(t), t, v));
            }
        }
        h.push_str("</div>");
    }
    let research: Vec<String> = strs(bv, "researches")
        .into_iter()
        .filter(|r| !w.factions_of("research", r).is_empty())
        .collect();
    if !research.is_empty() {
        h.push_str(&h2("research", "Research"));
        h.push_str(&research_table(w, &research));
    }
    let needed_by: Vec<String> = w
        .list("buildings")
        .iter()
        .filter(|o| strs(o, "requires").contains(&id) && playable(w, &s(o, "id")))
        .map(|o| s(o, "id"))
        .chain(
            w.list("units")
                .iter()
                .filter(|u| {
                    strs(get(u, "trainRequirement"), "kinds").contains(&id)
                        && playable(w, &s(u, "id"))
                })
                .map(|u| s(u, "id")),
        )
        .collect();
    if !needed_by.is_empty() {
        h.push_str(&h2("unlocks", "Required by"));
        let _ = write!(h, "<p>{}</p>", w.links(&needed_by));
    }
    h.push_str(&h2("damage-taken", "Damage taken"));
    let _ = write!(
        h,
        "<details><summary>Every weapon vs {} ({} HP)</summary>{}</details>",
        esc(&name),
        fmt(nz(bv, "hp")),
        damage_taken_table(w, &id)
    );
    h.push_str(&related_constants(w, "building", &id));
    h.push_str(&changelog(w, "building", &id));
    for f in &factions {
        h.push_str(&navbox(w, f));
    }
    let crumbs: Vec<(String, String)> = factions
        .first()
        .map(|f| vec![(format!("faction-{f}.html"), w.faction_name(f))])
        .unwrap_or_default();
    write(w, &file, &name, &crumbs, &h);
}

// ---------------------------------------------------------------- research

pub(super) fn research_page(w: &Wiki, r: &Value) {
    let id = s(r, "id");
    let file = format!("research-{id}.html");
    let name = s(r, "name");
    let factions = w.factions_of("research", &id);
    let req = s(r, "requiresUpgrade");
    let rows = vec![
        Row::Section("Research".into()),
        field("Cost", {
            let steel = research_const(w, &id, "COST_STEEL");
            let oil = research_const(w, &id, "COST_OIL");
            let (st, oi) = (nz(r, "steel"), nz(r, "oil"));
            if st == 0.0 && oi == 0.0 {
                "Free".to_string()
            } else {
                [
                    (st > 0.0).then(|| {
                        coded_opt(
                            w,
                            steel.as_deref(),
                            format!("<span class=\"res steel\">{}</span>", fmt(st)),
                        )
                    }),
                    (oi > 0.0).then(|| {
                        coded_opt(
                            w,
                            oil.as_deref(),
                            format!("<span class=\"res oil\">{}</span>", fmt(oi)),
                        )
                    }),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
            }
        }),
        field(
            "Time",
            n(r, "researchTicks")
                .map(|t| {
                    coded_opt(
                        w,
                        research_const(w, &id, "RESEARCH_TICKS").as_deref(),
                        w.u.secs(t),
                    )
                })
                .unwrap_or_default(),
        ),
        field("Researched at", w.link(&s(r, "researchedAt"))),
        field(
            "Requires",
            if req.is_empty() {
                String::new()
            } else {
                w.link_kind("research", &req)
            },
        ),
    ];
    let sub = factions
        .iter()
        .map(|f| {
            format!(
                "<a href=\"faction-{f}.html\">{}</a>",
                esc(&w.faction_name(f))
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
        + " research";
    let mut h = infobox(
        &w.faction_class("research", &id),
        &name,
        &w.badge(&id, &s(r, "icon"), "lg"),
        &sub,
        &rows,
    );
    h.push_str(&lead(w, "research", &id, research_lead(w, r)));
    h.push_str(&description(r));
    h.push_str(&notes_html(w, "research", &id, &file));
    let mut unlocks: Vec<String> = Vec::new();
    for u in w.list("units") {
        if s(u, "upgradeRequires") == id && playable(w, &s(u, "id")) {
            unlocks.push(format!("{} (unit)", w.link(&s(u, "id"))));
        }
    }
    for a in w.list("abilities") {
        if s(a, "upgradeRequirement") == id {
            unlocks.push(format!("{} (ability)", w.link_kind("ability", &s(a, "id"))));
        }
    }
    for o in w.list("upgrades") {
        if s(o, "requiresUpgrade") == id {
            unlocks.push(format!(
                "{} (research)",
                w.link_kind("research", &s(o, "id"))
            ));
        }
    }
    if !unlocks.is_empty() {
        h.push_str(&h2("unlocks", "Unlocks"));
        let _ = write!(
            h,
            "<ul>{}</ul>",
            unlocks
                .iter()
                .map(|x| format!("<li>{x}</li>"))
                .collect::<String>()
        );
    }
    h.push_str(&related_constants(w, "research", &id));
    h.push_str(&changelog(w, "research", &id));
    for f in &factions {
        h.push_str(&navbox(w, f));
    }
    write(
        w,
        &file,
        &name,
        &[("research.html".into(), "Research".into())],
        &h,
    );
}

pub(super) fn research_index(w: &Wiki) {
    let mut h = String::from("<p>All research, by faction.</p>");
    for f in w.list("factions") {
        let fid = s(f, "id");
        let ups: Vec<String> = arr(f, "upgrades").iter().map(|u| s(u, "id")).collect();
        if ups.is_empty() {
            continue;
        }
        h.push_str(&h2(&fid, &w.faction_name(&fid)));
        h.push_str(&research_table(w, &ups));
    }
    write(w, "research.html", "Research", &[], &h);
}

// ---------------------------------------------------------------- abilities

pub(super) fn abilities_page(w: &Wiki) {
    let file = "abilities.html";
    let mut h = String::from("<p>All abilities, by faction.</p>");
    for f in w.list("factions") {
        let fid = s(f, "id");
        let ids = faction_abilities(w, f);
        if ids.is_empty() {
            continue;
        }
        h.push_str(&h2(&format!("f-{fid}"), &w.faction_name(&fid)));
        h.push_str("<div class=\"ability-list\">");
        for a in ids {
            if let Some(av) = w.ability(&a) {
                h.push_str(&ability_card(w, av, file));
            }
        }
        h.push_str("</div>");
    }
    write(w, file, "Abilities", &[], &h);
}
