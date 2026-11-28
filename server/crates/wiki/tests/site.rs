//! Builds the wiki from this checkout's docs/wiki data, as the server does, and checks that it
//! covers the rules and that no hand-written note points at something that no longer exists.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
}

fn site() -> &'static BTreeMap<String, String> {
    static SITE: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    SITE.get_or_init(|| {
        let read = |p: &str| std::fs::read_to_string(repo().join(p));
        let client_rules_js = read("client/src/config/rules_mirror.js").expect("rules mirror");
        let icons = read("docs/wiki/icons.json").ok();
        let history = read("docs/wiki/history.json").ok();
        rts_wiki::build_site(&rts_wiki::Inputs {
            client_rules_js: &client_rules_js,
            content_dir: &repo().join("docs/wiki/content"),
            icons: icons.as_deref(),
            history: history.as_deref(),
            build_id: "0123456789ab",
        })
    })
}

#[test]
fn every_playable_unit_building_and_research_has_a_page() {
    let site = site();
    let mut missing = Vec::new();
    // test fixtures are not factions anyone plays
    for catalog in rts_rules::faction::CATALOGS
        .iter()
        .filter(|c| !c.units.is_empty() && !c.id.contains("fixture") && c.id != "ekat")
    {
        let units = catalog
            .units
            .iter()
            .map(|k| format!("unit-{}.html", k.stable_id()));
        let buildings = catalog
            .buildings
            .iter()
            .map(|k| format!("building-{}.html", k.stable_id()));
        let research = catalog
            .upgrades
            .iter()
            .map(|u| format!("research-{}.html", u.kind.stable_id()));
        let factions = std::iter::once(format!("faction-{}.html", catalog.id));
        for file in units.chain(buildings).chain(research).chain(factions) {
            if [
                "unit-artillery.html",
                "building-depot.html",
                "building-healing_seal.html",
                "research-artillery_unlock.html",
                "research-ballistic_tables.html",
            ]
            .contains(&file.as_str())
            {
                continue;
            }
            if !site.contains_key(&file) {
                missing.push(file);
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "no wiki page for: {missing:?}");
    for file in [
        "index.html",
        "changes.html",
        "constants.html",
        "maintenance.html",
        "weapons.html",
        "abilities.html",
    ] {
        assert!(site.contains_key(file), "missing {file}");
    }
}

#[test]
fn technique_library_lists_only_available_research() {
    let site = site();
    let library = &site["building-technique_library.html"];
    for technique in ["charge", "shield", "throw_sword"] {
        assert!(library.contains(&format!("research-technique_{technique}.html")));
    }
    assert!(!library.contains("research-technique_blink.html"));
    assert!(!site.contains_key("research-technique_blink.html"));
}

#[test]
fn notes_have_no_broken_references() {
    let broken: Vec<&String> = site()
        .iter()
        .filter(|(_, html)| html.contains("<span class=\"missing\""))
        .map(|(file, _)| file)
        .collect();
    assert!(
        broken.is_empty(),
        "notes in docs/wiki/content reference names the rules no longer have (see the Maintenance page): {broken:?}"
    );
}

#[test]
fn every_definition_is_traced_to_code() {
    // the code index (build.rs) must find each unit/building/research/ability definition, or its
    // page loses the "set in <file>:<line>" links
    let maintenance = &site()["maintenance.html"];
    assert!(
        maintenance.contains(
            "None — every unit, building, research and ability definition was found in the code"
        ),
        "some definitions were not found by the code index: see the Maintenance page"
    );
}

#[test]
fn pages_are_self_contained_and_name_the_build() {
    for (file, html) in site() {
        assert!(
            html.to_ascii_lowercase().starts_with("<!doctype html>"),
            "{file}"
        );
        assert!(
            !html.contains("<link rel=\"stylesheet\""),
            "{file} loads an external stylesheet"
        );
        assert!(
            html.contains("0123456789ab") || html.contains("012345678"),
            "{file} does not name the build"
        );
    }
}

#[test]
fn unit_pages_show_compiled_numbers() {
    let tank = rts_rules::defs::unit_def(rts_rules::EntityKind::Tank).expect("Tank def");
    let page = &site()["unit-tank.html"];
    assert!(
        page.contains(&format!(">{}<", tank.stats.hp)),
        "Tank HP {} not on its page",
        tank.stats.hp
    );
}

#[test]
fn current_wiki_hides_archived_content_and_links() {
    for file in [
        "faction-ekat.html",
        "unit-ekat.html",
        "unit-golem.html",
        "building-zamok.html",
        "unit-artillery.html",
        "research-artillery_unlock.html",
        "research-ballistic_tables.html",
        "building-healing_seal.html",
        "building-depot.html",
    ] {
        assert!(
            !site().contains_key(file),
            "archived page generated: {file}"
        );
        for (page, html) in site() {
            assert!(
                !html.contains(&format!("href=\"{file}")),
                "{page} links to {file}"
            );
            assert!(
                !html.contains(&format!("\"u\":\"{file}")),
                "{file} is searchable"
            );
        }
    }
    let abilities = &site()["abilities.html"];
    for id in [
        "healingSeal",
        "pointFire",
        "blanketFire",
        "ekatTeleport",
        "ekatLineShot",
        "ekatMagicAnchor",
        "ekatConsumeGolem",
    ] {
        assert!(
            !abilities.contains(&format!("id=\"{id}\"")),
            "archived ability: {id}"
        );
    }
    let weapons = &site()["weapons.html"];
    assert!(!weapons.contains("id=\"w-artillery_gun\""));
    assert!(site().contains_key("unit-rocket_launcher.html"));
    assert!(site().contains_key("faction-cultivators.html"));
}

#[test]
fn hidden_content_leaves_no_constants_or_history_behind() {
    // constants whose home is hidden content: not listed, not searchable, not in any changelog
    for (page, html) in site() {
        for prefix in ["EKAT_", "ARTILLERY_", "HEALING_SEAL_", "BALLISTIC_TABLES_"] {
            assert!(!html.contains(prefix), "{page} shows a {prefix}* constant");
        }
        // raw ids of hidden abilities used to leak into faction list changes
        for id in ["ekatTeleport", "ekatMagicAnchor", "healingSeal"] {
            assert!(!html.contains(id), "{page} shows the hidden id {id}");
        }
    }
    // ...while the constants that shown pages still read stay
    assert!(site()["constants.html"].contains("id=\"c-TANK_HE_SPLASH_DAMAGE\""));
    let economy = &site()["mechanics-economy.html"];
    assert!(!economy.contains("Golem") && !economy.contains("Zamok"));
}

#[test]
fn homepage_keeps_version_without_counts_or_explanation() {
    let page = &site()["index.html"];
    assert!(page.contains("Game version <a"));
    assert!(!page.contains("class=\"chips\""));
    assert!(!page.contains(": the build this server is running"));
}
