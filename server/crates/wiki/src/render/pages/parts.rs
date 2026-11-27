//! Pieces shared by many pages: costs, infoboxes, properties, damage tables, changelog lines, navboxes.

use super::*;

// ---------------------------------------------------------------- shared bits

pub(super) fn cost(steel: f64, oil: f64) -> String {
    if steel == 0.0 && oil == 0.0 {
        return "Free".into();
    }
    let mut parts = Vec::new();
    if steel > 0.0 {
        parts.push(format!(
            "<span class=\"res steel\" title=\"Steel\">{}</span>",
            fmt(steel)
        ));
    }
    if oil > 0.0 {
        parts.push(format!(
            "<span class=\"res oil\" title=\"Oil\">{}</span>",
            fmt(oil)
        ));
    }
    parts.join(" ")
}

pub(super) fn cost_of(v: &Value) -> String {
    cost(nz(v, "steel"), nz(v, "oil"))
}

/// The constant that sets a research cost or time: `TANK_PLUS_COST_OIL`, or `DAO_COST_OIL` for Daos.
pub(super) fn research_const(w: &Wiki, id: &str, suffix: &str) -> Option<String> {
    let own = format!("{}_{suffix}", crate::render::util::upper_snake(id));
    if w.const_entry(&own).is_some() {
        return Some(own);
    }
    let dao = format!("DAO_{suffix}");
    (id.starts_with("dao_") && w.const_entry(&dao).is_some()).then_some(dao)
}

pub(super) fn coded_opt(w: &Wiki, name: Option<&str>, display: String) -> String {
    match name {
        Some(n) => w.coded_const(n, display),
        None => display,
    }
}

/// Cost with each number tied to the field that sets it.
pub(super) fn cost_coded(
    w: &Wiki,
    key: &str,
    steel_field: &str,
    oil_field: &str,
    steel: f64,
    oil: f64,
) -> String {
    if steel == 0.0 && oil == 0.0 {
        return w.coded(key, steel_field, "Free".into());
    }
    let mut parts = Vec::new();
    if steel > 0.0 {
        parts.push(w.coded(
            key,
            steel_field,
            format!(
                "<span class=\"res steel\" title=\"Steel\">{}</span>",
                fmt(steel)
            ),
        ));
    }
    if oil > 0.0 {
        parts.push(w.coded(
            key,
            oil_field,
            format!("<span class=\"res oil\" title=\"Oil\">{}</span>", fmt(oil)),
        ));
    }
    parts.join(" ")
}

pub(super) enum Row {
    Section(String),
    Field(String, String),
}

pub(super) fn field(label: &str, value: String) -> Row {
    Row::Field(label.to_string(), value)
}

pub(super) fn infobox(class: &str, title: &str, picture: &str, sub: &str, rows: &[Row]) -> String {
    let mut h = format!(
        "<aside class=\"infobox {class}\"><div class=\"ib-title\">{}</div><div class=\"ib-picture\">{picture}</div><div class=\"ib-sub\">{sub}</div><table>",
        esc(title)
    );
    for r in rows {
        match r {
            Row::Section(t) => {
                let _ = write!(
                    h,
                    "<tr class=\"ib-section\"><th colspan=\"2\">{}</th></tr>",
                    esc(t)
                );
            }
            Row::Field(k, v) if !v.is_empty() => {
                let _ = write!(h, "<tr><th>{}</th><td>{v}</td></tr>", esc(k));
            }
            _ => {}
        }
    }
    h.push_str("</table></aside>");
    h
}

pub(super) fn card(w: &Wiki, kind: &str, id: &str, v: &Value) -> String {
    format!(
        "<a class=\"card {fc}\" href=\"{href}\">{badge}<span class=\"card-name\">{name}</span><span class=\"card-cost\">{cost}</span></a>",
        fc = w.faction_class(kind, id),
        href = w.file_for(kind, id),
        badge = w.badge(id, &s(v, "icon"), "md"),
        name = esc(&w.entity_name(kind, id)),
        cost = cost_of(v),
    )
}

pub(super) fn dps(w: &Wiki, dmg: f64, cooldown: f64) -> String {
    if dmg <= 0.0 || cooldown <= 0.0 {
        return "—".into();
    }
    fmt2(dmg / w.u.secs_raw(cooldown))
}

pub(super) fn tiles(x: f64) -> String {
    if x == 1.0 {
        "1 tile".into()
    } else {
        format!("{} tiles", fmt(x))
    }
}

/// Ids and other text constants (`SMOKE_ABILITY = "smoke"`) are wiring, not game values.
pub(super) fn is_value(c: &Value) -> bool {
    !get(c, "value").is_string()
}

/// Rust enum text -> reader text: "AntiArmorThreat" -> "Anti armor threat".
pub fn human(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("AntiTankGun {") {
        let m = rest
            .split(':')
            .nth(1)
            .and_then(|v| v.trim().trim_end_matches('}').trim().parse::<f64>().ok());
        return m
            .map(|m| format!("{}% damage to infantry", fmt(m * 100.0)))
            .unwrap_or_else(|| raw.to_string());
    }
    match raw {
        "AntiTank" => return "Anti-tank".into(),
        "None" | "" => return "—".into(),
        _ => {}
    }
    let mut out = String::new();
    for (i, ch) in raw.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Damage of one attack against a target row as (worst, best): machine-gun bursts range from
/// the aimed ray alone to every ray hitting.
pub(super) fn row_hit(row: &Value) -> (f64, f64) {
    match (n(row, "burstMin"), n(row, "burstMax")) {
        (Some(a), Some(b)) => (a, b),
        _ => (nz(row, "dmg"), nz(row, "dmg")),
    }
}

pub(super) fn weapon_hit(wv: &Value) -> (f64, f64) {
    let burst = get(wv, "burst");
    match (n(burst, "min"), n(burst, "max")) {
        (Some(a), Some(b)) => (a, b),
        _ => (nz(wv, "dmg"), nz(wv, "dmg")),
    }
}

pub(super) fn weapon_dmg_text(wv: &Value) -> String {
    let burst = get(wv, "burst");
    if burst.is_null() {
        return fmt(nz(wv, "dmg"));
    }
    format!(
        "{}–{} <span class=\"muted\">per burst ({} bullets × {}% of {})</span>",
        fmt(nz(burst, "min")),
        fmt(nz(burst, "max")),
        fmt(nz(burst, "rays")),
        fmt(nz(burst, "rayMultiplier") * 100.0),
        fmt(nz(wv, "dmg"))
    )
}

pub(super) fn weapon_dps(w: &Wiki, wv: &Value) -> String {
    let (lo, hi) = weapon_hit(wv);
    let cd = nz(wv, "cooldownTicks");
    if lo == hi {
        dps(w, hi, cd)
    } else {
        format!("{}–{}", dps(w, lo, cd), dps(w, hi, cd))
    }
}

pub(super) fn notes_html(w: &Wiki, dir: &str, id: &str, file: &str) -> String {
    match w.note(dir, id) {
        Some(doc) if !doc.body.trim().is_empty() => {
            format!("<div class=\"prose\">{}</div>", w.md(&doc.body, file))
        }
        _ => format!(
            "<p class=\"stub\">No notes yet — add them in <code>content/{dir}/{id}.md</code>.</p>"
        ),
    }
}

pub(super) fn description(v: &Value) -> String {
    let d = s(v, "description");
    if d.is_empty() {
        String::new()
    } else {
        format!("<blockquote class=\"tooltip\"><span class=\"tooltip-label\">Tooltip</span>{}</blockquote>", esc(&d))
    }
}

/// "A, B and C"
pub(super) fn and_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

pub(super) fn article(next_word: &str) -> &'static str {
    if next_word
        .chars()
        .next()
        .is_some_and(|c| "AEIOUaeiou".contains(c))
    {
        "an"
    } else {
        "a"
    }
}

/// One-sentence lead built from the data; a `lead:` line in the content file replaces it.
pub(super) fn lead(w: &Wiki, dir: &str, id: &str, generated: String) -> String {
    let text = match w.note(dir, id).and_then(|d| d.get("lead")) {
        Some(custom) => w.expand(custom, ""),
        None => generated,
    };
    format!("<p class=\"lead\">{text}</p>")
}

pub(super) fn faction_names(w: &Wiki, kind: &str, id: &str) -> String {
    let names: Vec<String> = w
        .factions_of(kind, id)
        .iter()
        .map(|f| w.faction_name(f))
        .collect();
    names.join(" / ")
}

pub(super) fn unit_lead(w: &Wiki, u: &Value) -> String {
    let id = s(u, "id");
    let armored = s(u, "armor") == "Armored";
    let fac = faction_names(w, "unit", &id);
    let next = if armored { "armored" } else { fac.as_str() };
    let mut t = format!(
        "The <b>{}</b> is {} {}{} unit",
        esc(&s(u, "name")),
        article(next),
        if armored { "armored " } else { "" },
        esc(&fac)
    );
    let at = s(u, "trainedAt");
    if !at.is_empty() {
        let _ = write!(t, " trained at the {}", w.link(&at));
    }
    t.push('.');
    let upg = s(u, "upgradeRequires");
    if !upg.is_empty() {
        let _ = write!(t, " Requires {}.", w.link_kind("research", &upg));
    }
    t
}

pub(super) fn building_lead(w: &Wiki, bv: &Value) -> String {
    let id = s(bv, "id");
    let fac = faction_names(w, "building", &id);
    let mut t = format!(
        "The <b>{}</b> is {} {} building.",
        esc(&s(bv, "name")),
        article(&fac),
        esc(&fac)
    );
    let trains: Vec<String> = strs(bv, "trains").iter().map(|x| w.link(x)).collect();
    if !trains.is_empty() {
        let _ = write!(t, " Trains {}.", and_list(&trains));
    }
    let research: Vec<String> = strs(bv, "researches")
        .iter()
        .filter(|r| !w.factions_of("research", r).is_empty())
        .map(|r| w.link_kind("research", r))
        .collect();
    if !research.is_empty() {
        let _ = write!(t, " Researches {}.", and_list(&research));
    }
    let extracts = s(bv, "extracts");
    if !extracts.is_empty() {
        let _ = write!(
            t,
            " Extracts {}.",
            esc(&w.name_of(&extracts).to_lowercase())
        );
    }
    t
}

pub(super) fn research_lead(w: &Wiki, r: &Value) -> String {
    let id = s(r, "id");
    let fac = faction_names(w, "research", &id);
    let mut t = format!(
        "<b>{}</b> is {} {} research at the {}.",
        esc(&s(r, "name")),
        article(&fac),
        esc(&fac),
        w.link(&s(r, "researchedAt"))
    );
    let req = s(r, "requiresUpgrade");
    if !req.is_empty() {
        let _ = write!(t, " Requires {}.", w.link_kind("research", &req));
    }
    t
}

pub(super) fn h2(id: &str, title: &str) -> String {
    format!("<h2 id=\"{id}\">{}</h2>", esc(title))
}

pub(super) fn related_constants(w: &Wiki, kind: &str, id: &str) -> String {
    let table = properties(w, kind, id);
    if table.is_empty() {
        return String::new();
    }
    format!(
        "{}<p class=\"muted small\">Every value the code defines for this {kind}, read from the game's source. The variable column links to the line that sets it; turn on <b>Show code</b> (top right) to also see where each one is used.</p>{table}",
        h2("properties", "Properties")
    )
}

/// Constants that belong to an entity or that its code definition names, in code order, each
/// with a readable label, its value, the variable (linked to its line) and where it is used.
pub(super) fn properties(w: &Wiki, kind: &str, id: &str) -> String {
    let key = format!("{kind}:{id}");
    let mut names: Vec<String> = w
        .constants_of(kind, id)
        .iter()
        .map(|c| s(c, "name"))
        .collect();
    for c in w.referenced_constants(&key) {
        if !names.contains(&c) {
            names.push(c);
        }
    }
    let mut entries: Vec<&Value> = names
        .iter()
        .filter_map(|n| w.const_entry(n))
        .filter(|c| is_value(c))
        .collect();
    if entries.is_empty() {
        return String::new();
    }
    entries.sort_by_key(|c| (Wiki::const_path(c), nz(c, "line") as i64));
    let prefixes = w.prefixes_of(kind, id);
    let mut h = String::from("<table class=\"props\"><thead><tr><th>Property</th><th>Value</th><th>Variable</th><th class=\"code-only\">Used in</th></tr></thead><tbody>");
    for c in entries {
        let name = s(c, "name");
        let (label, value) = prop_label_value(w, &name, get(c, "value"), &prefixes);
        let _ = write!(
            h,
            "<tr><th>{}</th><td class=\"num\">{}</td><td><a class=\"varname\" href=\"{}\" title=\"{}\">{}</a></td><td class=\"code-only used\">{}</td></tr>",
            esc(&label),
            value,
            esc(&w.gh(&Wiki::const_path(c), nz(c, "line"))),
            esc(&s(c, "doc")),
            esc(&name),
            usage_links(w, &name, 3)
        );
    }
    h.push_str("</tbody></table>");
    h
}

/// "Duration", "6 s" from BREAKTHROUGH_DURATION_TICKS = 180 (owner prefix BREAKTHROUGH).
pub(super) fn prop_label_value(
    w: &Wiki,
    name: &str,
    v: &Value,
    prefixes: &[String],
) -> (String, String) {
    let mut sorted: Vec<&String> = prefixes.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let mut rest = name.to_string();
    for p in sorted {
        if let Some(r) = name.strip_prefix(&format!("{p}_")) {
            rest = r.to_string();
            break;
        }
        if let Some(pos) = name.find(&format!("_{p}_")) {
            rest = name[pos + p.len() + 2..].to_string();
            break;
        }
    }
    let mut words: Vec<String> = rest
        .split('_')
        .filter(|x| !x.is_empty())
        .map(|x| x.to_lowercase())
        .collect();
    let ends = |words: &[String], tail: &[&str]| {
        words.len() >= tail.len()
            && words[words.len() - tail.len()..]
                .iter()
                .zip(tail)
                .all(|(a, b)| a == b)
    };
    let num = v.as_f64();
    let mut drop = 0;
    let value = match num {
        Some(x) if ends(&words, &["rad", "per", "tick"]) => {
            drop = 3;
            words.push("(turn rate)".into());
            format!("{}°/s", fmt2((x * w.u.tick_hz).to_degrees()))
        }
        Some(x) if ends(&words, &["px", "per", "tick"]) => {
            drop = 3;
            format!("{} tiles/s", fmt2(w.u.speed(x)))
        }
        Some(x) if ends(&words, &["ticks"]) => {
            drop = 1;
            w.u.secs(x)
        }
        Some(x) if ends(&words, &["tiles"]) => {
            drop = 1;
            tiles(x)
        }
        Some(x) if ends(&words, &["rad"]) => {
            drop = 1;
            format!("{}°", fmt(x.to_degrees()))
        }
        Some(x) if ends(&words, &["px"]) => {
            drop = 1;
            format!("{} px", fmt(x))
        }
        Some(x) if ends(&words, &["multiplier"]) => {
            drop = 1;
            format!("×{}", fmt(x))
        }
        Some(x) if ends(&words, &["cost", "steel"]) => {
            drop = 2;
            words.push("steel cost".into());
            cost(x, 0.0)
        }
        Some(x) if ends(&words, &["cost", "oil"]) => {
            drop = 2;
            words.push("oil cost".into());
            cost(0.0, x)
        }
        Some(x)
            if (0.0..=1.0).contains(&x)
                && words.last().is_some_and(|l| {
                    matches!(l.as_str(), "reduction" | "penetration" | "chance")
                }) =>
        {
            format!("{}%", fmt(x * 100.0))
        }
        _ => esc(&scalar(v)),
    };
    // keep a word we appended ("steel cost", "(turn rate)"), drop the unit words
    let pushed = if matches!(
        words.last().map(String::as_str),
        Some("steel cost" | "oil cost" | "(turn rate)")
    ) {
        words.pop()
    } else {
        None
    };
    let keep = words.len().saturating_sub(drop);
    words.truncate(keep);
    if let Some(p) = pushed {
        if p != "(turn rate)" || words.is_empty() {
            words.push(p.trim_matches(['(', ')']).to_string());
        }
    }
    for word in words.iter_mut() {
        if matches!(
            word.as_str(),
            "he" | "ap" | "mg" | "hp" | "atg" | "aoe" | "xp"
        ) {
            *word = word.to_uppercase();
        }
    }
    let mut label = words.join(" ");
    if label.is_empty() {
        label = title_case(&name.to_lowercase());
    }
    let mut chars = label.chars();
    let label = chars
        .next()
        .map(|f| f.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    (label, value)
}

/// "advance_moving_units · waypoints.rs:160" links for where a constant is read.
pub(super) fn usage_links(w: &Wiki, name: &str, max: usize) -> String {
    let uses = w.usage(name);
    let mut parts = Vec::new();
    for u in uses.iter().filter(|u| u.get("path").is_some()).take(max) {
        let path = s(u, "path");
        let file = path.rsplit('/').next().unwrap_or(&path).to_string();
        let func = s(u, "fn");
        let label = if func.is_empty() {
            format!("{file}:{}", s(u, "line"))
        } else {
            format!("{func} · {file}")
        };
        parts.push(format!(
            "<a href=\"{}\">{}</a>",
            esc(&w.gh(&path, nz(u, "line"))),
            esc(&label)
        ));
    }
    let total = uses.iter().filter(|u| u.get("path").is_some()).count()
        + uses.iter().filter(|u| u.get("more").is_some()).count();
    if total > max {
        parts.push(format!(
            "<span class=\"muted\">+{} more</span>",
            total - max
        ));
    }
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join("<br>")
    }
}

pub(super) fn constants_table(w: &Wiki, list: &[&Value]) -> String {
    let g = w.game();
    let mut h = String::from("<table class=\"wikitable sortable consts\"><thead><tr><th>Constant</th><th>Value</th><th>Meaning</th><th>Defined at</th><th>Used in</th></tr></thead><tbody>");
    for c in list {
        let name = s(c, "name");
        let v = get(c, "value");
        let mut value = esc(&scalar(v));
        if name.ends_with("_TICKS") {
            if let Some(x) = v.as_f64() {
                let _ = write!(value, " <span class=\"muted\">({})</span>", w.u.secs(x));
            }
        }
        let file_path = match s(c, "path") {
            p if p.is_empty() => format!("server/crates/rules/src/{}", s(c, "file")),
            p => p,
        };
        let src = format!(
            "{}/blob/{}/{}#L{}",
            s(g, "repo"),
            s(g, "commit"),
            file_path,
            s(c, "line")
        );
        let _ = write!(
            h,
            "<tr id=\"c-{n}\"><td><code>{n}</code></td><td class=\"num\" data-v=\"{raw}\">{value}</td><td>{doc}</td><td><a href=\"{src}\" class=\"src\">{file}:{line}</a></td><td class=\"used\">{used}</td></tr>",
            used = usage_links(w, &name, 3),
            n = esc(&name),
            raw = esc(&scalar(v)),
            doc = esc(&s(c, "doc")),
            file = esc(&s(c, "file")),
            line = esc(&s(c, "line")),
        );
    }
    h.push_str("</tbody></table>");
    h
}

/// "Tank" / "Tank (front 60, side 90, rear 102)"
pub(super) fn hits(hp: f64, dmg: f64) -> Option<f64> {
    (dmg > 0.0 && hp > 0.0).then(|| (hp / dmg).ceil())
}

pub(super) fn hits_cell(w: &Wiki, hp: f64, dmg: f64, cooldown: f64) -> (String, f64) {
    match hits(hp, dmg) {
        None => ("—".into(), f64::INFINITY),
        Some(k) => {
            let t = if cooldown > 0.0 {
                format!(
                    " <span class=\"muted\">({})</span>",
                    w.u.secs((k - 1.0) * cooldown)
                )
            } else {
                String::new()
            };
            (format!("{}{}", fmt(k), t), k)
        }
    }
}

pub(super) fn target_hp(w: &Wiki, id: &str) -> f64 {
    w.unit(id)
        .or_else(|| w.building(id))
        .map(|v| nz(v, "hp"))
        .unwrap_or(0.0)
}

pub(super) fn target_armor(w: &Wiki, id: &str) -> String {
    w.unit(id)
        .or_else(|| w.building(id))
        .map(|v| s(v, "armor"))
        .unwrap_or_default()
}

/// Targets that belong to a playable faction (skips internal kinds such as the Shop's flying item).
pub(super) fn playable(w: &Wiki, id: &str) -> bool {
    !w.factions_of(w.kind_of(id), id).is_empty()
}

pub(super) fn weapon_damage_table(w: &Wiki, weapon: &Value) -> String {
    let cd = nz(weapon, "cooldownTicks");
    let mut h = String::from(
        "<table class=\"wikitable sortable\"><thead><tr><th>Target</th><th>Faction</th><th>Armor</th><th>HP</th><th>Damage per hit</th><th>Hits to kill (time)</th></tr></thead><tbody>",
    );
    for row in arr(weapon, "vs") {
        let t = s(row, "target");
        if !playable(w, &t) {
            continue;
        }
        let hp = target_hp(w, &t);
        let dmg = nz(row, "dmg");
        let (dmg_cell, hits_html, sort) = if row.get("rear").is_some() {
            let (f, si, r) = (nz(row, "front"), nz(row, "side"), nz(row, "rear"));
            let kk = |x: f64| hits(hp, x).map(fmt).unwrap_or("—".into());
            (
                format!(
                    "{} / {} / {} <span class=\"muted\">front / side / rear</span>",
                    fmt(f),
                    fmt(si),
                    fmt(r)
                ),
                format!("{} / {} / {}", kk(f), kk(si), kk(r)),
                hits(hp, f).unwrap_or(f64::INFINITY),
            )
        } else {
            let (lo, hi) = row_hit(row);
            let (mut cell, k) = hits_cell(w, hp, hi, cd);
            let mut d = if lo == hi {
                fmt(hi)
            } else {
                format!("{}–{}", fmt(lo), fmt(hi))
            };
            if lo != hi {
                if let Some(kl) = hits(hp, lo) {
                    let _ = write!(cell, " <span class=\"muted\">to {}</span>", fmt(kl));
                }
            }
            if let Some(e) = n(row, "entrenched") {
                let _ = write!(d, " <span class=\"muted\">(entrenched {})</span>", fmt(e));
            }
            (d, cell, k)
        };
        let fac = w
            .factions_of(w.kind_of(&t), &t)
            .iter()
            .map(|f| w.faction_name(f))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            h,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\" data-v=\"{}\">{}</td></tr>",
            w.link(&t),
            esc(&fac),
            esc(&target_armor(w, &t)),
            fmt(hp),
            dmg,
            dmg_cell,
            if sort.is_finite() { fmt(sort) } else { "99999".into() },
            hits_html
        );
    }
    h.push_str("</tbody></table>");
    h
}

pub(super) fn damage_taken_table(w: &Wiki, target: &str) -> String {
    let hp = target_hp(w, target);
    let mut rows: Vec<(f64, String)> = Vec::new();
    for weapon in w.list("weapons") {
        let users: Vec<String> = strs(weapon, "usedBy")
            .into_iter()
            .filter(|u| playable(w, u))
            .collect();
        if users.is_empty() {
            continue;
        }
        let Some(row) = arr(weapon, "vs").iter().find(|r| s(r, "target") == target) else {
            continue;
        };
        let cd = nz(weapon, "cooldownTicks");
        let (dmg_cell, hits_html, sort) = if row.get("rear").is_some() {
            let (f, si, r) = (nz(row, "front"), nz(row, "side"), nz(row, "rear"));
            let kk = |x: f64| hits(hp, x).map(fmt).unwrap_or("—".into());
            (
                format!(
                    "{} / {} / {} <span class=\"muted\">front / side / rear</span>",
                    fmt(f),
                    fmt(si),
                    fmt(r)
                ),
                format!("{} / {} / {}", kk(f), kk(si), kk(r)),
                hits(hp, f).unwrap_or(f64::INFINITY),
            )
        } else {
            let (lo, hi) = row_hit(row);
            let (cell, k) = hits_cell(w, hp, hi, cd);
            (
                if lo == hi {
                    fmt(hi)
                } else {
                    format!("{}–{} <span class=\"muted\">burst</span>", fmt(lo), fmt(hi))
                },
                cell,
                k,
            )
        };
        let id = s(weapon, "id");
        rows.push((
            sort,
            format!(
                "<tr><td><a href=\"weapons.html#w-{id}\">{}</a></td><td>{}</td><td class=\"num\" data-v=\"{}\">{}</td><td class=\"num\" data-v=\"{}\">{}</td></tr>",
                esc(&weapon_name(&id)),
                w.links(&users),
                nz(row, "dmg"),
                dmg_cell,
                if sort.is_finite() { fmt(sort) } else { "99999".into() },
                hits_html
            ),
        ));
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut h = String::from("<table class=\"wikitable sortable\"><thead><tr><th>Weapon</th><th>Used by</th><th>Damage per hit</th><th>Hits to kill (time)</th></tr></thead><tbody>");
    for (_, r) in rows {
        h.push_str(&r);
    }
    h.push_str("</tbody></table>");
    h
}

pub(super) fn ability_card(w: &Wiki, a: &Value, file: &str) -> String {
    let id = s(a, "id");
    let mut rows: Vec<(String, String)> = Vec::new();
    rows.push(("Used by".into(), w.links(&strs(a, "carriers"))));
    let target = match s(a, "targetMode").as_str() {
        "self" => "None (instant)".to_string(),
        "worldPoint" => "Ground".to_string(),
        "friendlyUnit" => "Friendly unit".to_string(),
        other => title_case(&crate::render::util::upper_snake(other).to_lowercase()),
    };
    rows.push(("Target".into(), esc(&target)));
    let key = format!("ability:{id}");
    let mut range = n(a, "rangeTiles")
        .map(|r| w.coded(&key, "range_tiles", tiles(r)))
        .unwrap_or_default();
    if let Some(Value::Object(per)) = a.get("carrierRanges") {
        for (carrier, r) in per {
            let _ = write!(
                range,
                " ({}: {})",
                esc(&w.name_of(carrier)),
                r.as_f64().map(tiles).unwrap_or("—".into())
            );
        }
    }
    rows.push(("Range".into(), range));
    rows.push((
        "Minimum range".into(),
        n(a, "minRangeTiles")
            .filter(|x| *x > 0.0)
            .map(|x| w.coded(&key, "min_range_tiles", tiles(x)))
            .unwrap_or_default(),
    ));
    rows.push((
        "Cooldown".into(),
        n(a, "cooldownTicks")
            .filter(|x| *x > 0.0)
            .map(|x| w.coded(&key, "cooldown_ticks", w.u.secs(x)))
            .unwrap_or_default(),
    ));
    if let Some(c) = n(a, "charges") {
        let mut t = w.coded(&key, "charges", fmt(c));
        if let Some(r) = n(a, "chargeRechargeTicks") {
            let _ = write!(
                t,
                ", one back every {}",
                w.coded(&key, "charge_recharge_ticks", w.u.secs(r))
            );
        }
        rows.push(("Charges".into(), t));
    }
    let mut c = w.coded(&key, "cost", cost_of(a));
    if let Some(Value::Object(per)) = a.get("carrierCosts") {
        for (carrier, cc) in per {
            let _ = write!(c, " ({}: {})", esc(&w.name_of(carrier)), cost_of(cc));
        }
    }
    rows.push(("Cost".into(), c));
    let mut req = Vec::new();
    let tech = s(a, "techRequirement");
    if !tech.is_empty() {
        req.push(w.link(&tech));
    }
    let upg = s(a, "upgradeRequirement");
    if !upg.is_empty() {
        req.push(w.link_kind("research", &upg));
    }
    rows.push(("Requires".into(), req.join(", ")));
    rows.push((
        "Autocast".into(),
        if b(a, "autocast") {
            "yes".into()
        } else {
            String::new()
        },
    ));
    // every other value the code defines for this ability (new variables appear automatically)
    rows.extend(extra_properties(w, "ability", &id));
    let mut h = format!(
        "<div class=\"ability {fc}\" id=\"{id}\"><div class=\"ability-head\">{badge}<span class=\"ability-name\">{label}</span>{hotkey}<a class=\"anchor\" href=\"abilities.html#{id}\">#</a></div>",
        fc = w.faction_class("ability", &id),
        badge = w.badge(&format!("ability-{id}"), &s(a, "icon"), "sm"),
        label = esc(&s(a, "label")),
        hotkey = if s(a, "hotkey").is_empty() { String::new() } else { format!("<kbd title=\"Hotkey\">{}</kbd>", esc(&s(a, "hotkey"))) },
    );
    let title = s(a, "title");
    if !title.is_empty() {
        let _ = write!(h, "<p class=\"ability-text\">{}</p>", esc(&title));
    }
    h.push_str("<table class=\"kv\">");
    for (k, v) in rows.iter().filter(|(_, v)| !v.is_empty()) {
        let _ = write!(h, "<tr><th>{}</th><td>{v}</td></tr>", esc(k));
    }
    h.push_str("</table>");
    if let Some(doc) = w.note("abilities", &id) {
        let _ = write!(h, "<div class=\"prose\">{}</div>", w.md(&doc.body, file));
    }
    h.push_str("</div>");
    h
}

/// Rows for constants that belong to an entity but are not already shown from its definition.
pub(super) fn extra_properties(w: &Wiki, kind: &str, id: &str) -> Vec<(String, String)> {
    let shown = w.referenced_constants(&format!("{kind}:{id}"));
    let mut entries: Vec<&Value> = w
        .constants_of(kind, id)
        .into_iter()
        .filter(|c| is_value(c) && !shown.contains(&s(c, "name")))
        .collect();
    entries.sort_by_key(|c| (Wiki::const_path(c), nz(c, "line") as i64));
    let prefixes = w.prefixes_of(kind, id);
    entries
        .into_iter()
        .map(|c| {
            let name = s(c, "name");
            let (label, value) = prop_label_value(w, &name, get(c, "value"), &prefixes);
            (label, w.coded_const(&name, value))
        })
        .collect()
}

// ---------------------------------------------------------------- change log formatting

pub(super) fn field_label(field: &str) -> String {
    let known: &[(&str, &str)] = &[
        ("hp", "HP"),
        ("dmg", "Damage"),
        ("rangeTiles", "Range"),
        ("cooldownTicks", "Cooldown"),
        ("speedPxPerTick", "Speed"),
        ("sightTiles", "Sight"),
        ("steel", "Steel cost"),
        ("oil", "Oil cost"),
        ("supply", "Supply"),
        ("buildTicks", "Build time"),
        ("researchTicks", "Research time"),
        ("radiusPx", "Body radius"),
        ("armor", "Armor"),
        ("description", "Tooltip"),
        ("title", "Tooltip"),
        ("name", "Name"),
        ("label", "Name"),
        ("attack.dmg", "Damage"),
        ("attack.rangeTiles", "Range"),
        ("attack.cooldownTicks", "Cooldown"),
        ("hotkey", "Hotkey"),
        ("icon", "Icon"),
        ("carriers", "Used by"),
        ("commandCard", "On command card"),
        ("autocast", "Autocast"),
        ("factions", "Factions"),
        ("abilities", "Abilities"),
        ("buildables", "Can build"),
        ("buildings", "Buildings"),
        ("units", "Units"),
        ("upgrades", "Research"),
        ("productionAnchors", "Production anchors"),
        ("builders", "Builders"),
        ("gatherers", "Gatherers"),
        ("researches", "Researches"),
        ("upgradeRequires", "Requires research"),
        ("requiresUpgrade", "Requires research"),
        ("researchedAt", "Researched at"),
        ("weapon", "Weapon"),
        ("usedBy", "Used by"),
        ("armorPenetration", "Armor penetration"),
        ("trainedAt", "Trained at"),
        ("trainRequirement.kinds", "Requires"),
        ("requires", "Requires"),
        ("trains", "Trains"),
        ("footW", "Footprint width"),
        ("footH", "Footprint height"),
        ("chargeRechargeTicks", "Charge recharge"),
        ("minRangeTiles", "Minimum range"),
        ("loadout.steel", "Starting steel"),
        ("loadout.oil", "Starting oil"),
        ("value", "Value"),
    ];
    known
        .iter()
        .find(|(k, _)| *k == field)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| field.to_string())
}

pub(super) fn change_value(w: &Wiki, c: &Change, v: &Option<Value>) -> String {
    let Some(v) = v else { return "—".into() };
    let ticks = c.field.ends_with("Ticks") || (c.kind == "constant" && c.id.ends_with("_TICKS"));
    if let Some(x) = v.as_f64() {
        if ticks {
            return w.u.secs(x);
        }
        if c.field == "speedPxPerTick" {
            return format!("{} tiles/s", fmt2(w.u.speed(x)));
        }
        if c.field.ends_with("rangeTiles") || c.field == "sightTiles" {
            return tiles(x);
        }
        return fmt(x);
    }
    if let Value::Array(items) = v {
        let names: Vec<String> = items.iter().map(|i| any_name(w, i)).collect();
        return if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        };
    }
    match v {
        Value::String(x) if x.len() > 60 => format!("“{x}”"),
        Value::String(_) => any_name(w, v),
        other => scalar(other),
    }
}

/// Display name for an id of any kind (unit, building, research, ability, faction).
pub(super) fn any_name(w: &Wiki, v: &Value) -> String {
    let Value::String(id) = v else {
        return scalar(v);
    };
    if w.unit(id).is_some() || w.building(id).is_some() {
        w.name_of(id)
    } else if w.research(id).is_some() {
        w.entity_name("research", id)
    } else if w.ability(id).is_some() {
        w.entity_name("ability", id)
    } else if w.faction(id).is_some() {
        w.faction_name(id)
    } else {
        id.clone()
    }
}

pub(super) fn change_line(w: &Wiki, c: &Change) -> String {
    // lists: show what was added and removed instead of both whole lists
    if let (Some(Value::Array(old)), Some(Value::Array(new))) = (&c.old, &c.new) {
        let added: Vec<String> = new
            .iter()
            .filter(|x| !old.contains(x))
            .map(|x| any_name(w, x))
            .collect();
        let removed: Vec<String> = old
            .iter()
            .filter(|x| !new.contains(x))
            .map(|x| any_name(w, x))
            .collect();
        if !added.is_empty() || !removed.is_empty() {
            let mut parts = Vec::new();
            if !added.is_empty() {
                parts.push(format!(
                    "<span class=\"new\">+ {}</span>",
                    esc(&added.join(", "))
                ));
            }
            if !removed.is_empty() {
                parts.push(format!(
                    "<span class=\"old\">− {}</span>",
                    esc(&removed.join(", "))
                ));
            }
            return format!("{}: {}", esc(&field_label(&c.field)), parts.join(" "));
        }
        return format!("{}: order changed", esc(&field_label(&c.field)));
    }
    // long text (tooltips): show the new wording, old wording on hover
    if let (Some(Value::String(old)), Some(Value::String(new))) = (&c.old, &c.new) {
        if old.len() > 60 || new.len() > 60 {
            return format!(
                "{} changed: <span class=\"new-text\" title=\"Was: {}\">“{}”</span>",
                esc(&field_label(&c.field)),
                esc(old),
                esc(new)
            );
        }
    }
    let what = if c.field.is_empty() {
        if c.new.is_some() {
            "<span class=\"tag add\">new</span>".to_string()
        } else {
            "<span class=\"tag del\">removed</span>".to_string()
        }
    } else {
        format!(
            "{}: <span class=\"old\">{}</span> → <span class=\"new\">{}</span>",
            esc(&field_label(&c.field)),
            esc(&change_value(w, c, &c.old)),
            esc(&change_value(w, c, &c.new))
        )
    };
    what
}

/// Drop repeats: a unit's definition and its attack profile often change together.
pub(super) fn dedupe<'a>(changes: impl Iterator<Item = &'a Change>) -> Vec<&'a Change> {
    let mut seen = std::collections::HashSet::new();
    changes
        .filter(|c| {
            seen.insert((
                c.kind,
                c.id.clone(),
                field_label(&c.field),
                format!("{:?}{:?}", c.old, c.new),
            ))
        })
        .collect()
}

pub(super) fn entity_ref(w: &Wiki, c: &Change) -> String {
    match c.kind {
        "constant" => format!(
            "<a href=\"constants.html#c-{0}\"><code>{0}</code></a>",
            esc(&c.id)
        ),
        "weapon" => format!(
            "<a href=\"weapons.html#w-{}\">{}</a>",
            esc(&c.id),
            esc(&weapon_name(&c.id))
        ),
        "faction" => format!(
            "<a href=\"faction-{}.html\">{}</a>",
            esc(&c.id),
            esc(&w.faction_name(&c.id))
        ),
        "unit" | "building" | "research" | "ability" if c.new.is_some() || c.field.is_empty() => {
            format!(
                "<a href=\"{}\">{}</a>",
                w.file_for(c.kind, &c.id),
                esc(&c.name)
            )
        }
        _ => esc(&c.name),
    }
}

pub(super) fn pull_heading(w: &Wiki, i: usize) -> String {
    let g = get(&w.snaps[i], "game");
    format!(
        "{} · game <a href=\"{}/commit/{}\"><code>{}</code></a>",
        esc(&day(&s(g, "date"))),
        esc(&s(g, "repo")),
        esc(&s(g, "commit")),
        esc(&s(g, "short"))
    )
}

/// Changes to one entity across all pulls, including its related constants.
pub(super) fn changelog(w: &Wiki, kind: &str, id: &str) -> String {
    let mut h = String::new();
    for i in (1..w.snaps.len()).rev() {
        let lines: Vec<String> = dedupe(w.diffs[i].iter())
            .into_iter()
            .filter(|c| {
                (c.kind == kind && c.id == id)
                    || (c.kind == "constant"
                        && w.const_owner
                            .get(&c.id)
                            .is_some_and(|(k, e)| *k == kind && e == id))
            })
            .map(|c| {
                if c.kind == "constant" {
                    format!(
                        "<li>{}{}</li>",
                        const_change_line(w, c, w.const_owner.get(&c.id)),
                        pr_for(w, i, c)
                    )
                } else {
                    format!("<li>{}{}</li>", change_line(w, c), pr_for(w, i, c))
                }
            })
            .collect();
        if !lines.is_empty() {
            let _ = write!(
                h,
                "<div class=\"pull\"><div class=\"pull-head\">{}</div><ul>{}</ul></div>",
                pull_heading(w, i),
                lines.join("")
            );
        }
    }
    let first = get(&w.snaps[0], "game");
    if h.is_empty() {
        format!(
            "{}<p class=\"muted\">No changes since tracking began ({}).</p>",
            h2("changelog", "Changelog"),
            esc(&day(&s(first, "date")))
        )
    } else {
        format!(
            "{}{h}<p class=\"muted\">Tracked since {}.</p>",
            h2("changelog", "Changelog"),
            esc(&day(&s(first, "date")))
        )
    }
}

// ---------------------------------------------------------------- navbox

pub(super) fn navbox(w: &Wiki, faction: &str) -> String {
    let Some(f) = w.faction(faction) else {
        return String::new();
    };
    let mut h = format!(
        "<div class=\"navbox fac-{faction}\"><div class=\"navbox-title\"><a href=\"faction-{faction}.html\">{}</a></div><table>",
        esc(&w.faction_name(faction))
    );
    for t in arr(f, "trainables") {
        let units: Vec<String> = strs(t, "units")
            .into_iter()
            .filter(|u| w.unit(u).is_some())
            .collect();
        if units.is_empty() {
            continue;
        }
        let _ = write!(
            h,
            "<tr><th>{}</th><td>{}</td></tr>",
            w.link(&s(t, "building")),
            units
                .iter()
                .map(|u| w.link(u))
                .collect::<Vec<_>>()
                .join(" · ")
        );
    }
    let blds: Vec<String> = strs(f, "buildings").iter().map(|x| w.link(x)).collect();
    let _ = write!(
        h,
        "<tr><th>Buildings</th><td>{}</td></tr>",
        blds.join(" · ")
    );
    let rs: Vec<String> = arr(f, "upgrades")
        .iter()
        .map(|u| w.link_kind("research", &s(u, "id")))
        .collect();
    if !rs.is_empty() {
        let _ = write!(h, "<tr><th>Research</th><td>{}</td></tr>", rs.join(" · "));
    }
    h.push_str("</table></div>");
    h
}
