//! Every page of the site. Layout and the page list live here; the page builders are split
//! by kind: shared pieces (`parts`), game entities (`entities`) and reference pages (`reference`).

mod entities;
mod parts;
mod reference;

use entities::*;
use parts::*;
use reference::*;

use crate::render::content::Doc;
use crate::render::diff::Change;
use crate::render::util::{
    arr, b, day, esc, fmt, fmt2, get, n, nz, s, scalar, slug, strs, title_case, upper_snake,
};
use crate::render::Wiki;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

const CSS: &str = include_str!("../../../assets/wiki.css");
const JS: &str = include_str!("../../../assets/wiki.js");
/// Content folders that attach notes to generated pages; every other folder holds whole pages.
const ENTITY_DIRS: &[&str] = &[
    "units",
    "buildings",
    "research",
    "abilities",
    "factions",
    "images",
    "main",
];

/// The search index, built before any page so every page can carry its own copy.
static SEARCH_INDEX: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn render_all(w: &Wiki) {
    search_index(w);
    index(w);
    for f in w.list("factions") {
        faction_page(w, f);
    }
    for u in w.list("units") {
        if !w.factions_of("unit", &s(u, "id")).is_empty() {
            unit_page(w, u);
        }
    }
    for bld in w.list("buildings") {
        if !w.factions_of("building", &s(bld, "id")).is_empty() {
            building_page(w, bld);
        }
    }
    for r in w.list("upgrades") {
        if !w.factions_of("research", &s(r, "id")).is_empty() {
            research_page(w, r);
        }
    }
    research_index(w);
    abilities_page(w);
    weapons_page(w);
    unit_table(w);
    building_table(w);
    constants_page(w);
    changes_page(w);
    for (key, doc) in &w.content {
        if is_page_doc(key) {
            content_page(w, key, doc);
        }
    }
    maintenance_page(w); // last: collects problems found while rendering everything else
}

// ---------------------------------------------------------------- layout

pub fn is_page_doc(key: &str) -> bool {
    key.contains('/') && !ENTITY_DIRS.contains(&key.split('/').next().unwrap_or(""))
}

pub fn doc_file(key: &str) -> String {
    let (section, rest) = key.split_once('/').unwrap_or(("page", key));
    format!("{}-{}.html", slug(section), slug(rest))
}

pub fn doc_title(key: &str, doc: &Doc) -> String {
    doc.get("title")
        .map(str::to_string)
        .unwrap_or_else(|| title_case(key.rsplit('/').next().unwrap_or(key)))
}

fn page_docs(w: &Wiki) -> BTreeMap<String, Vec<(String, String, String)>> {
    // section -> [(order, title, file)]
    let mut out: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for (key, doc) in &w.content {
        if is_page_doc(key) {
            let section = key.split('/').next().unwrap().to_string();
            let order = doc.get("order").unwrap_or("50").to_string();
            out.entry(section).or_default().push((
                format!("{order:0>4}"),
                doc_title(key, doc),
                doc_file(key),
            ));
        }
    }
    for v in out.values_mut() {
        v.sort();
    }
    out
}

fn sidebar(w: &Wiki, active: &str) -> String {
    let mut h = String::new();
    let item = |h: &mut String, href: &str, label: &str| {
        let cls = if href == active {
            " class=\"active\""
        } else {
            ""
        };
        let _ = write!(h, "<li><a href=\"{href}\"{cls}>{}</a></li>", esc(label));
    };
    h.push_str("<div class=\"side-group\"><ul>");
    item(&mut h, "index.html", "Main page");
    item(&mut h, "changes.html", "Changelog");
    h.push_str("</ul></div><div class=\"side-group\"><h4>Factions</h4><ul>");
    for f in w.list("factions") {
        let id = s(f, "id");
        item(&mut h, &format!("faction-{id}.html"), &w.faction_name(&id));
    }
    h.push_str("</ul></div><div class=\"side-group\"><h4>Database</h4><ul>");
    item(&mut h, "table-units.html", "Units");
    item(&mut h, "table-buildings.html", "Buildings");
    item(&mut h, "research.html", "Research");
    item(&mut h, "abilities.html", "Abilities");
    item(&mut h, "weapons.html", "Weapons & damage");
    item(&mut h, "constants.html", "Balance constants");
    h.push_str("</ul></div>");
    for (section, docs) in page_docs(w) {
        let _ = write!(
            h,
            "<div class=\"side-group\"><h4>{}</h4><ul>",
            esc(&title_case(&section))
        );
        for (_, title, file) in docs {
            item(&mut h, &file, &title);
        }
        h.push_str("</ul></div>");
    }
    h.push_str("<div class=\"side-group\"><h4>Wiki</h4><ul>");
    item(&mut h, "maintenance.html", "Maintenance");
    item(&mut h, "stats", "Stats table");
    item(&mut h, "dev/", "Developer docs");
    h.push_str("</ul></div>");
    h
}

fn shell(w: &Wiki, file: &str, title: &str, crumbs: &[(String, String)], body: &str) -> String {
    let g = w.game();
    let mut crumb = String::from("<a href=\"index.html\">Main page</a>");
    for (href, label) in crumbs {
        let _ = write!(
            crumb,
            " <span>›</span> <a href=\"{href}\">{}</a>",
            esc(label)
        );
    }
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{t} — Bewegungskrieg Wiki</title>
<style>{css}</style>{icons}
<script>try{{var t=localStorage.getItem("bk-wiki-theme");if(t)document.documentElement.setAttribute("data-theme",t);if(localStorage.getItem("bk-wiki-code")==="1")document.documentElement.classList.add("show-code")}}catch(e){{}}</script>
</head>
<body>
<header class="topbar">
  <a class="brand" href="index.html"><span class="brand-mark">BK</span><span class="brand-text">Bewegungskrieg <b>Wiki</b></span></a>
  <div class="search"><input id="search" type="search" placeholder="Search units, buildings, research, constants…  ( / )" autocomplete="off" spellcheck="false"><div id="search-results" class="search-results" hidden></div></div>
  <a class="version" href="changes.html" title="The game build these pages were generated from — see the changelog">build <code>{short}</code>{date_sep}</a>
  <button class="code-toggle" id="code-toggle" type="button" title="Show the code variable behind every number, linked to its line">&lt;/&gt; Show code</button>
  <button class="theme-toggle" id="theme-toggle" type="button" title="Light / dark">◐</button>
</header>
<div class="layout">
<nav class="sidebar">{side}</nav>
<main class="page">
<div class="crumbs">{crumb}</div>
<h1 class="page-title">{t}</h1>
{body}
</main>
</div>
<footer class="footer">Stats from the running game build <a href="{repo}/commit/{commit}">{short}</a>, read from its own code. Notes: <code>docs/wiki/content/</code>. <a href="dev/">Developer docs</a>.</footer>
<script>{index_js}</script>
<script>{js}</script>
</body>
</html>
"##,
        t = esc(title),
        // inlined so a page shows correctly on its own (some viewers block linked files)
        css = CSS,
        js = JS,
        index_js = SEARCH_INDEX.get().map(String::as_str).unwrap_or(""),
        icons = w.icon_styles(body),
        side = sidebar(w, file),
        short = esc(&s(g, "short")),
        date_sep = if s(g, "date").is_empty() {
            String::new()
        } else {
            format!(" · {}", esc(&day(&s(g, "date"))))
        },
        repo = esc(&s(g, "repo")),
        commit = esc(&s(g, "commit")),
    )
}

fn write(w: &Wiki, file: &str, title: &str, crumbs: &[(String, String)], body: &str) {
    w.pages
        .borrow_mut()
        .insert(file.to_string(), shell(w, file, title, crumbs, body));
}
