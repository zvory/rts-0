//! Reference pages: weapons, comparison tables, constants, changelog, notes pages, search, maintenance.

use super::*;

// ---------------------------------------------------------------- weapons

pub(super) fn weapons_page(w: &Wiki) {
    let file = "weapons.html";
    let mut h = String::new();
    let facing = get(w.cur(), "facing");
    let _ = write!(
        h,
        "<ul><li><b>Armored</b> targets take 25% damage from weapons without armor penetration (AP). AP restores part of it.</li><li><i>Facing</i> weapons deal ×{} / ×{} / ×{} to a Tank's front / side / rear.</li><li>Machine guns fire bursts, so their damage is a range.</li></ul><p>Full rules: <a href=\"mechanics-combat.html\">Combat</a>.</p>",
        fmt(nz(facing, "front")),
        fmt(nz(facing, "side")),
        fmt(nz(facing, "rear"))
    );
    h.push_str(&h2("profiles", "Weapon profiles"));
    h.push_str("<table class=\"wikitable sortable\"><thead><tr><th>Weapon</th><th>Used by</th><th>Damage</th><th>Range</th><th>Cooldown</th><th>DPS</th><th>Class</th><th>Armor pen.</th><th>Special</th></tr></thead><tbody>");
    for wv in w.list("weapons") {
        let id = s(wv, "id");
        let users: Vec<String> = strs(wv, "usedBy")
            .into_iter()
            .filter(|u| playable(w, u))
            .collect();
        let mut special = Vec::new();
        if b(wv, "facingArmor") {
            special.push("facing".to_string());
        }
        if b(wv, "triggersTankArmorReaction") {
            special.push("Tanks turn to face it".into());
        }
        let pol = s(wv, "infantryPolicy");
        if pol != "None" {
            special.push(human(&pol));
        }
        // a unit's own version of a shared weapon takes its numbers from that unit's stats
        let variant = s(wv, "variantOf");
        let (key, f_dmg, f_range, f_cd) = if variant.is_empty() {
            (format!("weapon:{id}"), "dmg", "range_tiles", "cooldown")
        } else {
            (
                format!("unit:{variant}"),
                "stats.dmg",
                "stats.range_tiles",
                "stats.cooldown",
            )
        };
        let dmg = if get(wv, "area").is_null() {
            w.coded(&key, f_dmg, weapon_dmg_text(wv))
        } else {
            w.coded_const("MORTAR_INNER_DAMAGE", weapon_dmg_text(wv))
        };
        let _ = write!(
            h,
            "<tr id=\"w-{id}\"><td><b>{}</b></td><td>{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\">{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}%</td><td>{}</td></tr>",
            esc(&w.weapon_label(&id)),
            w.links(&users),
            weapon_hit(wv).1,
            dmg,
            w.coded(&key, f_range, weapon_range_text(wv)),
            nz(wv, "cooldownTicks"),
            w.coded(&key, f_cd, weapon_cooldown_text(w, wv)),
            weapon_dps(w, wv),
            esc(&human(&s(wv, "weaponClass"))),
            w.coded(&format!("weapon:{id}"), "armor_penetration", fmt(nz(wv, "armorPenetration") * 100.0)),
            esc(&special.join("; ")),
        );
    }
    h.push_str("</tbody></table>");

    // matrix: rows = targets, columns = weapons that deal damage
    let cols: Vec<&Value> = w
        .list("weapons")
        .iter()
        .filter(|wv| nz(wv, "dmg") > 0.0 && strs(wv, "usedBy").iter().any(|u| playable(w, u)))
        .collect();
    h.push_str(&h2("matrix", "Damage matrix"));
    h.push_str("<p class=\"muted\">Damage per hit after armor. Tank: front / side / rear. Shaded: reduced by armor or a weapon rule. Hover a header to see who uses it.</p><div class=\"matrix-wrap\"><table class=\"wikitable matrix\"><thead><tr><th>Target</th><th>HP</th>");
    for wv in &cols {
        let users: Vec<String> = strs(wv, "usedBy")
            .iter()
            .filter(|u| playable(w, u))
            .map(|u| w.name_of(u))
            .collect();
        let _ = write!(
            h,
            "<th title=\"{}\"><a href=\"#w-{}\">{}</a></th>",
            esc(&users.join(", ")),
            s(wv, "id"),
            esc(&w.weapon_label(&s(wv, "id")))
        );
    }
    h.push_str("</tr></thead><tbody>");
    let targets: Vec<String> = w
        .list("units")
        .iter()
        .chain(w.list("buildings").iter())
        .map(|v| s(v, "id"))
        .filter(|t| playable(w, t))
        .collect();
    for t in &targets {
        let _ = write!(
            h,
            "<tr><td>{}</td><td class=\"num\">{}</td>",
            w.link(t),
            fmt(target_hp(w, t))
        );
        for wv in &cols {
            let row = arr(wv, "vs").iter().find(|r| s(r, "target") == *t);
            let cell = match row {
                Some(r) if r.get("rear").is_some() => format!(
                    "{}/{}/{}",
                    fmt(nz(r, "front")),
                    fmt(nz(r, "side")),
                    fmt(nz(r, "rear"))
                ),
                Some(r) => {
                    let (lo, hi) = row_hit(r);
                    if lo == hi {
                        fmt(hi)
                    } else {
                        format!("{}–{}", fmt(lo), fmt(hi))
                    }
                }
                None => "—".into(),
            };
            let low = row.map(|r| nz(r, "dmg") < nz(wv, "dmg")).unwrap_or(false);
            let _ = write!(
                h,
                "<td class=\"num{}\">{}</td>",
                if low { " reduced" } else { "" },
                cell
            );
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table></div>");
    write(w, file, "Weapons & damage", &[], &h);
}

// ---------------------------------------------------------------- comparison tables

pub(super) fn faction_filter(w: &Wiki) -> String {
    let mut h = String::from("<div class=\"filters\" data-filter-table=\"cmp\"><button type=\"button\" class=\"on\" data-faction=\"\">All</button>");
    for f in w.list("factions") {
        let id = s(f, "id");
        let _ = write!(
            h,
            "<button type=\"button\" data-faction=\"{id}\">{}</button>",
            esc(&w.faction_name(&id))
        );
    }
    h.push_str("<input type=\"search\" placeholder=\"Filter rows…\" data-filter-text></div>");
    h
}

pub(super) fn unit_table(w: &Wiki) {
    let mut h = String::from("<p class=\"muted\">Click a header to sort. Time in seconds, speed in tiles/s, range and sight in tiles. DPS assumes every shot hits.</p>");
    h.push_str(&faction_filter(w));
    h.push_str("<div class=\"matrix-wrap\"><table class=\"wikitable sortable\" id=\"cmp\"><thead><tr><th>Unit</th><th>Faction</th><th>Steel</th><th>Oil</th><th>Supply</th><th>Time</th><th>HP</th><th>Armor</th><th>Damage</th><th>Range</th><th>Cooldown</th><th>DPS</th><th>Speed</th><th>Sight</th><th>Trained at</th></tr></thead><tbody>");
    for u in w.list("units") {
        let id = s(u, "id");
        let facs = w.factions_of("unit", &id);
        if facs.is_empty() {
            continue;
        }
        let at = get(u, "attack");
        let _ = write!(
            h,
            "<tr data-faction=\"{}\"><td>{} {}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
            facs.join(" "),
            w.badge(&id, &s(u, "icon"), "xs"),
            w.link(&id),
            esc(&facs.iter().map(|f| w.faction_name(f)).collect::<Vec<_>>().join(", ")),
            fmt(nz(u, "steel")),
            fmt(nz(u, "oil")),
            fmt(nz(u, "supply")),
            fmt2(w.u.secs_raw(nz(u, "buildTicks"))),
            fmt(nz(u, "hp")),
            esc(&s(u, "armor")),
            fmt(nz(at, "dmg")),
            fmt(nz(at, "rangeTiles")),
            fmt2(w.u.secs_raw(nz(at, "cooldownTicks"))),
            w.weapon(&s(u, "weapon")).map(|wv| weapon_dps(w, wv)).unwrap_or_else(|| dps(w, nz(at, "dmg"), nz(at, "cooldownTicks"))),
            fmt2(w.u.speed(nz(u, "speedPxPerTick"))),
            fmt(nz(u, "sightTiles")),
            if s(u, "trainedAt").is_empty() { "—".into() } else { w.link(&s(u, "trainedAt")) },
        );
    }
    h.push_str("</tbody></table></div>");
    write(w, "table-units.html", "Unit comparison", &[], &h);
}

pub(super) fn building_table(w: &Wiki) {
    let mut h = String::from("<p class=\"muted\">Click a header to sort. Time in seconds, footprint and sight in tiles.</p>");
    h.push_str(&faction_filter(w));
    h.push_str("<div class=\"matrix-wrap\"><table class=\"wikitable sortable\" id=\"cmp\"><thead><tr><th>Building</th><th>Faction</th><th>Steel</th><th>Oil</th><th>Time</th><th>HP</th><th>Armor</th><th>Footprint</th><th>Sight</th><th>Requires</th><th>Trains / researches</th></tr></thead><tbody>");
    for bv in w.list("buildings") {
        let id = s(bv, "id");
        let facs = w.factions_of("building", &id);
        if facs.is_empty() {
            continue;
        }
        let mut makes: Vec<String> = strs(bv, "trains").iter().map(|t| w.link(t)).collect();
        makes.extend(
            strs(bv, "researches")
                .iter()
                .filter(|r| !w.factions_of("research", r).is_empty())
                .map(|r| w.link_kind("research", r)),
        );
        let _ = write!(
            h,
            "<tr data-faction=\"{}\"><td>{} {}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td data-v=\"{}\">{}×{}</td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
            facs.join(" "),
            w.badge(&id, &s(bv, "icon"), "xs"),
            w.link(&id),
            esc(&facs.iter().map(|f| w.faction_name(f)).collect::<Vec<_>>().join(", ")),
            fmt(nz(bv, "steel")),
            fmt(nz(bv, "oil")),
            fmt2(w.u.secs_raw(nz(bv, "buildTicks"))),
            fmt(nz(bv, "hp")),
            esc(&s(bv, "armor")),
            nz(bv, "footW") * nz(bv, "footH"),
            s(bv, "footW"),
            s(bv, "footH"),
            fmt(nz(bv, "sightTiles")),
            w.links(&strs(bv, "requires")),
            if makes.is_empty() { "—".into() } else { makes.join(", ") },
        );
    }
    h.push_str("</tbody></table></div>");
    write(w, "table-buildings.html", "Building comparison", &[], &h);
}

// ---------------------------------------------------------------- constants

pub(super) fn constants_page(w: &Wiki) {
    let mut h = format!(
        "<p>All {} balance constants at <code>{}</code>: every public value in the rules code, plus a few combat values from the simulation (marked <i>sim</i>). <code>_TICKS</code> values also show seconds. Descriptions are the code's own comments.</p><div class=\"filters\"><input type=\"search\" placeholder=\"Filter constants…\" data-filter-consts autofocus></div>",
        w.list("constants").len(),
        esc(&s(w.game(), "short"))
    );
    let mut by_file: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for c in w.list("constants") {
        by_file.entry(s(c, "file")).or_default().push(c);
    }
    for (file, list) in by_file {
        let _ = write!(
            h,
            "<section class=\"const-group\"><h2 id=\"f-{}\"><code>{}</code></h2>",
            slug(&file),
            esc(&file)
        );
        h.push_str(&constants_table(w, &list));
        h.push_str("</section>");
    }
    write(w, "constants.html", "Balance constants", &[], &h);
}

// ---------------------------------------------------------------- changes

/// One block of a version's notes: everything that changed for one unit, ability, page, ...
pub(super) struct ChangeGroup {
    pub(super) kind: String,
    pub(super) id: String,
    /// name as of that version (for things no longer in the game)
    pub(super) name: String,
    pub(super) factions: Vec<String>,
    pub(super) lines: Vec<String>,
}

/// A constant change written like the page shows it: "HE splash damage: 25 → 30 TANK_HE_SPLASH_DAMAGE".
pub(super) fn const_change_line(
    w: &Wiki,
    c: &Change,
    owner: Option<&(&'static str, String)>,
) -> String {
    let prefixes = owner.map(|(k, i)| w.prefixes_of(k, i)).unwrap_or_default();
    // an added or removed constant is recorded whole ({ name, value }); a changed value as the
    // bare old and new values
    let val = |v: &Option<Value>| {
        v.as_ref().map(|v| {
            let raw = if v.is_object() { get(v, "value") } else { v };
            prop_label_value(w, &c.id, raw, &prefixes)
        })
    };
    let (label, old) = val(&c.old).unwrap_or_default();
    let (label_new, new) = val(&c.new).unwrap_or_default();
    let label = if label.is_empty() { label_new } else { label };
    let var = format!(
        "<a class=\"varname\" href=\"constants.html#c-{0}\">{0}</a>",
        esc(&c.id)
    );
    match (&c.old, &c.new) {
        (None, Some(_)) => format!(
            "<span class=\"tag add\">new</span> {}: <span class=\"new\">{}</span> {var}",
            esc(&label),
            new
        ),
        (Some(_), None) => format!(
            "<span class=\"tag del\">removed</span> {} {var}",
            esc(&label)
        ),
        _ => format!(
            "{}: <span class=\"old\">{}</span> → <span class=\"new\">{}</span> {var}",
            esc(&label),
            old,
            new
        ),
    }
}

/// " #1869" — the PR that made a change, when it could be pinned to exactly one commit.
pub(super) fn pr_for(w: &Wiki, _i: usize, c: &Change) -> String {
    match &c.pr {
        Some(v) => format!(" {}", pr_link(&esc(&s(w.game(), "repo")), v)),
        None => String::new(),
    }
}

/// "#1869" link for a commit subject "Merge pull request #1869 from …", else the short sha.
pub(super) fn pr_link(repo_url: &str, v: &Value) -> String {
    let subject = s(v, "subject");
    let sha = s(v, "sha");
    match subject
        .strip_prefix("Merge pull request #")
        .and_then(|r| r.split_once(' '))
    {
        Some((num, _)) => format!(
            "<a class=\"pr\" href=\"{repo_url}/pull/{num}\" title=\"{}\">#{num}</a>",
            esc(&subject)
        ),
        None => format!(
            "<a class=\"pr\" href=\"{repo_url}/commit/{sha}\" title=\"{}\">{}</a>",
            esc(&subject),
            esc(&sha.chars().take(7).collect::<String>())
        ),
    }
}

pub(super) fn change_groups(w: &Wiki, i: usize) -> Vec<ChangeGroup> {
    let order = |k: &str| match k {
        "faction" => 0,
        "unit" => 1,
        "building" => 2,
        "research" => 3,
        "ability" => 4,
        "weapon" => 5,
        "page" => 6,
        "node" => 7,
        _ => 8,
    };
    let mut groups: Vec<ChangeGroup> = Vec::new();
    for c in dedupe(w.diffs[i].iter()) {
        // values in code are listed under the page that shows them
        let (kind, id, mut line) = if c.kind == "constant" {
            match w.const_owner.get(&c.id) {
                Some(o) => (
                    o.0.to_string(),
                    o.1.clone(),
                    const_change_line(w, c, Some(o)),
                ),
                None => (
                    "constant".to_string(),
                    String::new(),
                    const_change_line(w, c, None),
                ),
            }
        } else {
            (c.kind.to_string(), c.id.clone(), change_line(w, c))
        };
        line.push_str(&pr_for(w, i, c));
        match groups.iter_mut().find(|g| g.kind == kind && g.id == id) {
            Some(g) => g.lines.push(line),
            None => groups.push(ChangeGroup {
                factions: w.factions_of(&kind, &id),
                kind,
                id,
                name: c.name.clone(),
                lines: vec![line],
            }),
        }
    }
    groups.sort_by_key(|g| (order(&g.kind), w.entity_name(&g.kind, &g.id)));
    groups
}

pub(super) fn group_html(w: &Wiki, g: &ChangeGroup) -> String {
    let exists = match g.kind.as_str() {
        "unit" => w.unit(&g.id).is_some(),
        "building" => w.building(&g.id).is_some(),
        "research" => w.research(&g.id).is_some(),
        "ability" => w.ability(&g.id).is_some(),
        "weapon" => w.weapon(&g.id).is_some(),
        "faction" => w.faction(&g.id).is_some(),
        _ => true,
    };
    let title = match g.kind.as_str() {
        "constant" => "Other values in code".to_string(),
        _ if !exists => format!(
            "{} <span class=\"muted\">(not in the current version)</span>",
            esc(&g.name)
        ),
        "unit" | "building" => format!("{} {}", w.badge(&g.id, "", "xs"), w.link(&g.id)),
        kind => w.link_kind(kind, &g.id),
    };
    let kind_label = match g.kind.as_str() {
        "page" => "mechanics",
        "constant" => "values",
        k => k,
    };
    format!(
        "<div class=\"cg\" data-kind=\"{}\" data-faction=\"{}\"><div class=\"cg-head\"><span class=\"kind\">{}</span> {title}</div><ul>{}</ul></div>",
        esc(&g.kind),
        esc(&g.factions.join(" ")),
        esc(kind_label),
        g.lines.iter().map(|l| format!("<li>{l}</li>")).collect::<String>()
    )
}

pub(super) fn changes_page(w: &Wiki) {
    let mut h = String::from("<p>Every version of the game's rules the wiki has read, newest first: the changes merged, and every value that changed, grouped by the page it belongs to.</p>");
    h.push_str("<div class=\"filters\" data-filter-changes><button type=\"button\" class=\"on\" data-kind=\"\">All</button>");
    for (k, label) in [
        ("unit", "Units"),
        ("building", "Buildings"),
        ("research", "Research"),
        ("ability", "Abilities"),
        ("weapon", "Weapons"),
        ("page", "Mechanics"),
    ] {
        let _ = write!(
            h,
            "<button type=\"button\" data-kind=\"{k}\">{label}</button>"
        );
    }
    h.push_str("<span class=\"sep\"></span>");
    for f in w.list("factions") {
        let id = s(f, "id");
        let _ = write!(
            h,
            "<button type=\"button\" data-faction=\"{id}\">{}</button>",
            esc(&w.faction_name(&id))
        );
    }
    h.push_str("<input type=\"search\" placeholder=\"Filter changes…\" data-filter-text></div>");
    let first = get(&w.snaps[0], "game");
    let _ = write!(
        h,
        "<p class=\"muted small\">{} versions tracked, from {} to {}.</p>",
        w.snaps.len(),
        esc(&day(&s(first, "date"))),
        esc(&day(&s(w.game(), "date")))
    );
    for i in (1..w.snaps.len()).rev() {
        let groups = change_groups(w, i);
        let g = get(&w.snaps[i], "game");
        let count = |k: &str| groups.iter().filter(|x| x.kind == k).count();
        let mut chips = Vec::new();
        for (k, one, many) in [
            ("unit", "unit", "units"),
            ("building", "building", "buildings"),
            ("research", "research", "research"),
            ("ability", "ability", "abilities"),
            ("weapon", "weapon", "weapons"),
        ] {
            match count(k) {
                0 => {}
                1 => chips.push(format!("1 {one}")),
                n => chips.push(format!("{n} {many}")),
            }
        }
        let values = w.diffs[i].iter().filter(|c| c.kind == "constant").count();
        if values > 0 {
            chips.push(format!("{values} values"));
        }
        let _ = write!(
            h,
            "<section class=\"version\" id=\"v-{short}\"><div class=\"version-head\"><h2>{date}</h2><a class=\"version-chip\" href=\"{repo}/commit/{commit}\"><code>{short}</code></a><span class=\"chips\">{chips}</span></div>",
            short = esc(&s(g, "short")),
            date = esc(&long_date(&s(g, "date"))),
            repo = esc(&s(g, "repo")),
            commit = esc(&s(g, "commit")),
            chips = chips.iter().map(|c| format!("<span>{c}</span>")).collect::<String>(),
        );
        h.push_str(&merged_list(w, i));
        if groups.is_empty() {
            h.push_str("<p class=\"muted\">No rule changes in this version.</p>");
        } else {
            h.push_str("<div class=\"cgs\">");
            for group in &groups {
                h.push_str(&group_html(w, group));
            }
            h.push_str("</div>");
        }
        h.push_str("</section>");
    }
    let _ = write!(
        h,
        "<section class=\"version\"><div class=\"version-head\"><h2>{}</h2><a class=\"version-chip\" href=\"{}/commit/{}\"><code>{}</code></a></div><p class=\"muted\">First version read. Changes are tracked from here.</p>{}</section>",
        esc(&long_date(&s(first, "date"))),
        esc(&s(first, "repo")),
        esc(&s(first, "commit")),
        esc(&s(first, "short")),
        merged_list(w, 0)
    );
    write(w, "changes.html", "Changelog", &[], &h);
}

/// "2026-11-26T11:49:30-05:00" -> "26 November 2026"
pub(super) fn long_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let d = day(iso);
    let parts: Vec<&str> = d.split('-').collect();
    match parts.as_slice() {
        [y, m, dd] => match m.parse::<usize>() {
            Ok(mi) if (1..=12).contains(&mi) => {
                format!("{} {} {}", dd.trim_start_matches('0'), MONTHS[mi - 1], y)
            }
            _ => d.clone(),
        },
        _ => d.clone(),
    }
}

/// The game changes in a version: pull requests ("#1994 mg fire rate") and other commits.
pub(super) fn merged_list(w: &Wiki, i: usize) -> String {
    let commits = arr(&w.snaps[i], "commits");
    if commits.is_empty() {
        return String::new();
    }
    let repo = s(get(&w.snaps[i], "game"), "repo");
    let mut items = Vec::new();
    for c in commits {
        let subject = s(c, "subject");
        let sha = s(c, "sha");
        let item = match subject
            .strip_prefix("Merge pull request #")
            .and_then(|r| r.split_once(" from "))
        {
            Some((num, branch)) => {
                let name = branch
                    .rsplit('/')
                    .next()
                    .unwrap_or(branch)
                    .replace(['-', '_'], " ");
                format!(
                    "<a href=\"{}/pull/{}\">#{}</a> {}",
                    esc(&repo),
                    esc(num),
                    esc(num),
                    esc(&name)
                )
            }
            None => format!(
                "<a href=\"{}/commit/{}\"><code>{}</code></a> {}",
                esc(&repo),
                esc(&sha),
                esc(&sha.chars().take(7).collect::<String>()),
                esc(&subject)
            ),
        };
        items.push(format!("<li>{item}</li>"));
    }
    let label = if i == 0 { "Earlier changes" } else { "Merged" };
    let open = if i > 0 && items.len() <= 6 {
        " open"
    } else {
        ""
    };
    format!(
        "<details class=\"merged\"{open}><summary>{label} ({})</summary><ul>{}</ul></details>",
        items.len(),
        items.join("")
    )
}

// ---------------------------------------------------------------- content pages

pub(super) fn content_page(w: &Wiki, key: &str, doc: &Doc) {
    let file = doc_file(key);
    let title = doc_title(key, doc);
    let mut body = format!("<div class=\"prose\">{}</div>", w.md(&doc.body, &file));
    // a `constants:` line gives the page an automatic table of those values, and their history
    if !w.constants_of("page", key).is_empty() {
        let table = properties(w, "page", key);
        let _ = write!(
            body,
            "{}<p class=\"muted small\">Every value in the code under this topic, read from the game's source. New ones appear here automatically.</p>{table}",
            h2("values", "Values in code")
        );
        body.push_str(&changelog(w, "page", key));
    }
    write(w, &file, &title, &[], &body);
}

// ---------------------------------------------------------------- search

pub(super) fn search_index(w: &Wiki) {
    let mut items: Vec<Value> = Vec::new();
    let mut push = |t: String, u: String, k: &str, x: String| {
        items.push(serde_json::json!({ "t": t, "u": u, "k": k, "x": x }));
    };
    for f in w.list("factions") {
        let id = s(f, "id");
        push(
            w.faction_name(&id),
            format!("faction-{id}.html"),
            "Faction",
            id,
        );
    }
    for u in w.list("units") {
        let id = s(u, "id");
        if playable(w, &id) {
            push(
                s(u, "name"),
                format!("unit-{id}.html"),
                "Unit",
                format!("{id} {}", s(u, "icon")),
            );
        }
    }
    for bv in w.list("buildings") {
        let id = s(bv, "id");
        if playable(w, &id) {
            push(
                s(bv, "name"),
                format!("building-{id}.html"),
                "Building",
                format!("{id} {}", s(bv, "icon")),
            );
        }
    }
    for r in w.list("upgrades") {
        let id = s(r, "id");
        if !w.factions_of("research", &id).is_empty() {
            push(s(r, "name"), format!("research-{id}.html"), "Research", id);
        }
    }
    for a in w.list("abilities") {
        if !arr(a, "factions").is_empty() {
            let id = s(a, "id");
            push(
                s(a, "label"),
                format!("abilities.html#{id}"),
                "Ability",
                format!("{id} {}", s(a, "title")),
            );
        }
    }
    for wv in w.list("weapons") {
        let id = s(wv, "id");
        push(
            w.weapon_label(&id),
            format!("weapons.html#w-{id}"),
            "Weapon",
            id,
        );
    }
    for (key, doc) in &w.content {
        if is_page_doc(key) {
            push(
                doc_title(key, doc),
                doc_file(key),
                "Page",
                key.replace('/', " "),
            );
        }
    }
    for c in w.list("constants") {
        let name = s(c, "name");
        push(
            name.clone(),
            format!("constants.html#c-{name}"),
            "Constant",
            format!("{} {}", scalar(get(c, "value")), s(c, "doc")),
        );
    }
    // "</" would end an inline <script> early
    let js = format!(
        "window.WIKI_INDEX={};",
        serde_json::to_string(&items).unwrap().replace("</", "<\\/")
    );

    let _ = SEARCH_INDEX.set(js);
}

// ---------------------------------------------------------------- maintenance

pub(super) fn maintenance_page(w: &Wiki) {
    let mut h = String::from("<p>Things to look at after a rules change or a note edit. Nothing here stops the wiki from building.</p>");
    // the last accuracy check, recorded with the version history (docs/wiki/history.json)
    h.push_str(&h2("accuracy", "Accuracy checks"));
    match Some(&w.verification).filter(|v| v.is_object()) {
        Some(last) => {
            let total: usize = arr(last, "checks").iter().map(|c| arr(c, "problems").len()).sum();
            let _ = write!(
                h,
                "<p>Last run {} on game version <code>{}</code>, covering {} versions: <b class=\"{}\">{}</b>.</p><table class=\"wikitable\"><thead><tr><th>Check</th><th>Items checked</th><th>Problems</th></tr></thead><tbody>",
                esc(&day(&s(last, "at"))),
                esc(&s(last, "game")),
                s(last, "versions"),
                if total == 0 { "ok" } else { "missing" },
                if total == 0 { "no problems found".to_string() } else { format!("{total} problem(s)") }
            );
            for c in arr(last, "checks") {
                let problems = arr(c, "problems");
                let _ = write!(h, "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>", esc(&s(c, "name")), s(c, "checked"), problems.len());
            }
            h.push_str("</tbody></table>");
            for c in arr(last, "checks") {
                for p in arr(c, "problems").iter().take(30) {
                    let _ = write!(h, "<p class=\"missing\">{}</p>", esc(&scalar(p)));
                }
            }
            h.push_str("<p class=\"muted small\">The checks: (1) every version's numbers against the game's own catalog dump and client display values; (2) every infobox, ability card and changelog section against the data; (3) every code link against the code at that commit; (4) every changelog entry against the git diff between versions, and every changed value reported; (5) the current version against the live game server's stats page. They run in the local wiki tool, which records the result here with the history.</p>");
        }
        None => h.push_str("<p class=\"muted\">No check recorded yet: the local wiki tool records one with the history.</p>"),
    }
    // broken template references
    let missing = w.missing.borrow().clone();
    h.push_str(&h2("missing", "Broken references in notes"));
    if missing.is_empty() {
        h.push_str(
            "<p class=\"ok\">None — every <code>{{…}}</code> in the content files resolved.</p>",
        );
    } else {
        h.push_str("<p>These <code>{{…}}</code> references did not match anything in the current rules (renamed or removed constant, unit or field). Fix them in the content file.</p><table class=\"wikitable\"><thead><tr><th>Page</th><th>Reference</th></tr></thead><tbody>");
        for (page, r) in &missing {
            let _ = write!(
                h,
                "<tr><td><a href=\"{0}\">{0}</a></td><td><code>{{{{{1}}}}}</code></td></tr>",
                esc(page),
                esc(r)
            );
        }
        h.push_str("</tbody></table>");
    }
    // new in the latest pull: the first place a new ability, unit or variable shows up
    if let Some(last) = w.diffs.last() {
        let added: Vec<&Change> = last
            .iter()
            .filter(|c| c.field.is_empty() && c.new.is_some())
            .collect();
        if !added.is_empty() {
            h.push_str(&h2("new", "New in the latest recorded version"));
            h.push_str("<p>Added to the game in the latest version recorded in the history. Their pages and properties are generated automatically; check that each new variable landed on the right page.</p><ul>");
            for c in added {
                let place = match c.kind {
                    "constant" => match w.const_owner.get(&c.id) {
                        Some((k, i)) => format!(" → shown on {}", w.link_kind(k, i)),
                        None => " → <b>not shown on any page</b>".into(),
                    },
                    _ => String::new(),
                };
                let _ = write!(
                    h,
                    "<li><span class=\"kind\">{}</span> {}{place}</li>",
                    c.kind,
                    entity_ref(w, c)
                );
            }
            h.push_str("</ul>");
        }
    }

    // coverage: every value in the code should be reachable from a page
    let mut named_in_definitions: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    if let Some(Value::Object(src)) = w.cur().get("sources") {
        for fields in src.values() {
            if let Value::Object(fields) = fields {
                for f in fields.values() {
                    named_in_definitions.extend(strs(f, "consts"));
                }
            }
        }
    }
    let mut unplaced: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in w.list("constants") {
        let name = s(c, "name");
        if is_value(c)
            && !w.const_owner.contains_key(&name)
            && !named_in_definitions.contains(&name)
        {
            unplaced.entry(s(c, "file")).or_default().push(name);
        }
    }
    let total_unplaced: usize = unplaced.values().map(Vec::len).sum();
    h.push_str(&h2("coverage", "Values not shown on any page"));
    let _ = write!(
        h,
        "<p>{total_unplaced} of {} constants appear only on <a href=\"constants.html\">Balance constants</a>. A value lands on a page when its name starts with (or contains) that unit's or ability's id, when the page's code definition uses it, or when the page's notes file has a <code>constants: PREFIX_</code> line. Add one of those for any value below that belongs somewhere.</p>",
        w.list("constants").len()
    );
    h.push_str("<details><summary>Show them</summary><table class=\"wikitable\"><thead><tr><th>File</th><th>Constants</th></tr></thead><tbody>");
    for (file, names) in &unplaced {
        let links: Vec<String> = names
            .iter()
            .map(|n| {
                format!(
                    "<a href=\"constants.html#c-{0}\"><code>{0}</code></a>",
                    esc(n)
                )
            })
            .collect();
        let _ = write!(
            h,
            "<tr><td><code>{}</code></td><td>{}</td></tr>",
            esc(file),
            links.join(" ")
        );
    }
    h.push_str("</tbody></table></details>");

    // every page's numbers should be traced to their line in the code
    let mut untraced = Vec::new();
    for (kind, coll) in [
        ("unit", "units"),
        ("building", "buildings"),
        ("research", "upgrades"),
        ("ability", "abilities"),
    ] {
        for v in w.list(coll) {
            let id = s(v, "id");
            if w.factions_of(kind, &id).is_empty() {
                continue;
            }
            if w.cur()
                .get("sources")
                .and_then(|x| x.get(format!("{kind}:{id}")))
                .is_none()
            {
                untraced.push(w.link_kind(kind, &id));
            }
        }
    }
    h.push_str(&h2(
        "untraced",
        "Pages whose numbers could not be traced to code",
    ));
    if untraced.is_empty() {
        h.push_str("<p class=\"ok\">None — every unit, building, research and ability definition was found in the code, field by field.</p>");
    } else {
        let _ = write!(h, "<p>The code definition for these was not found (the game may define them in a new way the code index in <code>server/crates/wiki/build</code> doesn't read yet). Their numbers are still correct, but not linked to a line: {}</p>", untraced.join(", "));
    }

    // content files with no matching entity
    let mut orphans = Vec::new();
    for key in w.content.keys() {
        let (dir, id) = key.split_once('/').unwrap_or(("", ""));
        let found = match dir {
            "units" => w.unit(id).is_some(),
            "buildings" => w.building(id).is_some(),
            "research" => w.research(id).is_some(),
            "abilities" => w.ability(id).is_some(),
            "factions" => w.faction(id).is_some(),
            _ => true,
        };
        if !found {
            orphans.push(key.clone());
        }
    }
    h.push_str(&h2("orphans", "Notes for hidden or missing entries"));
    if orphans.is_empty() {
        h.push_str("<p class=\"ok\">None.</p>");
    } else {
        let _ = write!(
            h,
            "<ul>{}</ul>",
            orphans
                .iter()
                .map(|o| format!("<li><code>content/{}.md</code></li>", esc(o)))
                .collect::<String>()
        );
    }
    // attack profile vs unit definition
    h.push_str(&h2("mismatch", "Unit definition vs. weapon in use"));
    h.push_str("<p>The infobox shows the attack the simulation uses. These units list different numbers in their unit definition (the game's own <code>/wiki/stats</code> page shows the definition).</p>");
    let mut rows = String::new();
    for u in w.list("units") {
        let id = s(u, "id");
        if !playable(w, &id) {
            continue;
        }
        let at = get(u, "attack");
        let pairs = [
            ("Damage", "dmg", "dmg"),
            ("Range", "rangeTiles", "rangeTiles"),
            ("Cooldown ticks", "cooldownTicks", "cooldownTicks"),
        ];
        for (label, dk, ak) in pairs {
            if n(u, dk) != n(at, ak) {
                let _ = write!(rows, "<tr><td>{}</td><td>{label}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>", w.link(&id), scalar(get(u, dk)), scalar(get(at, ak)));
            }
        }
    }
    if rows.is_empty() {
        h.push_str("<p class=\"ok\">All match.</p>");
    } else {
        let _ = write!(h, "<table class=\"wikitable\"><thead><tr><th>Unit</th><th>Stat</th><th>Definition</th><th>Weapon in use</th></tr></thead><tbody>{rows}</tbody></table>");
    }
    // research in no faction
    let unused: Vec<String> = w
        .list("upgrades")
        .iter()
        .filter(|r| w.factions_of("research", &s(r, "id")).is_empty())
        .map(|r| s(r, "id"))
        .collect();
    if !unused.is_empty() {
        h.push_str(&h2("unused", "Research defined but in no faction"));
        let _ = write!(
            h,
            "<p>Not shown in the wiki: {}.</p>",
            unused
                .iter()
                .map(|x| format!("<code>{}</code>", esc(x)))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    // stubs
    h.push_str(&h2("stubs", "Pages without notes"));
    let mut stubs: Vec<String> = Vec::new();
    for (kind, coll, dir) in [
        ("unit", "units", "units"),
        ("building", "buildings", "buildings"),
        ("research", "upgrades", "research"),
    ] {
        for v in w.list(coll) {
            let id = s(v, "id");
            if w.factions_of(kind, &id).is_empty() {
                continue;
            }
            if w.note(dir, &id).is_none_or(|d| d.body.trim().is_empty()) {
                stubs.push(w.link_kind(kind, &id));
            }
        }
    }
    let _ = write!(
        h,
        "<p>{} pages have no hand-written notes yet: {}</p>",
        stubs.len(),
        stubs.join(", ")
    );
    write(w, "maintenance.html", "Maintenance", &[], &h);
}
