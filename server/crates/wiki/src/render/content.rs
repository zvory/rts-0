//! Hand-written content: Markdown files with an optional front-matter block and `{{...}}`
//! templates that pull live values from the current rules snapshot.

use crate::render::util::{esc, fmt, path, scalar};
use crate::render::Wiki;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub meta: BTreeMap<String, String>,
    pub body: String,
}

impl Doc {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.meta
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }
}

/// All `*.md` files under `dir`, keyed by their relative path without extension ("units/tank").
pub fn load(dir: &Path) -> BTreeMap<String, Doc> {
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Doc>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            let key = p
                .strip_prefix(root)
                .unwrap()
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            out.insert(key, parse(&text));
        }
    }
}

pub fn parse(text: &str) -> Doc {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut doc = Doc::default();
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            for line in rest[..end].lines() {
                if let Some((k, v)) = line.split_once(':') {
                    doc.meta.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            doc.body = rest[end + 4..].trim_start_matches(['-', '\n']).to_string();
            return doc;
        }
    }
    doc.body = text;
    doc
}

impl Wiki {
    /// Markdown (with templates) -> HTML.
    pub fn md(&self, text: &str, page: &str) -> String {
        let expanded = self.expand(text, page);
        let opts = pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_FOOTNOTES;
        let parser = pulldown_cmark::Parser::new_ext(&expanded, opts);
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, parser);
        html
    }

    /// Replace every `{{...}}` with its value; unknown references are flagged on the page and
    /// listed on the Maintenance page.
    /// Code spans and fenced code blocks are left alone, so examples can show `{{...}}`.
    pub fn expand(&self, text: &str, page: &str) -> String {
        let mut out = String::new();
        let mut fenced = false;
        for line in text.split_inclusive('\n') {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                out.push_str(line);
                continue;
            }
            if fenced {
                out.push_str(line);
                continue;
            }
            // outside fences: expand only between inline code spans
            for (i, part) in line.split('`').enumerate() {
                if i > 0 {
                    out.push('`');
                }
                if i % 2 == 1 {
                    out.push_str(part);
                } else {
                    out.push_str(&self.expand_plain(part, page));
                }
            }
        }
        out
    }

    fn expand_plain(&self, text: &str, page: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                out.push_str(&rest[start..]);
                return out;
            };
            let inner = after[..end].trim();
            match self.resolve(inner) {
                Some(v) => out.push_str(&v),
                None => {
                    self.missing
                        .borrow_mut()
                        .push((page.to_string(), inner.to_string()));
                    out.push_str(&format!(
                        "<span class=\"missing\" title=\"Unknown reference — see Maintenance\">&#123;&#123;{}&#125;&#125;</span>",
                        esc(inner).replace('|', "&#124;")
                    ));
                }
            }
            rest = &after[end + 2..];
        }
        out.push_str(rest);
        out
    }

    /// Template forms:
    ///   {{CONST}}  {{CONST|s}}  {{CONST|pct}}  {{CONST|x}}  {{CONST|change}}  {{CONST|tiles}}
    ///   {{unit:tank}}  {{unit:tank|Text}}  {{unit:tank.hp}}  {{unit:tank.attack.dmg|s}}
    ///   (also building:, research:, ability:, faction:, weapon:, page:mechanics/combat)
    ///   {{dmg:tank_cannon>tank}}  {{dmg:tank_cannon>tank.rear}}
    fn resolve(&self, inner: &str) -> Option<String> {
        let (head, filter) = match inner.split_once('|') {
            Some((h, f)) => (h.trim(), Some(f.trim())),
            None => (inner, None),
        };
        if let Some((kind, rest)) = head.split_once(':') {
            let kind = kind.trim();
            let rest = rest.trim();
            if kind == "calc" {
                let v = self.calc(rest)?;
                return Some(self.filtered(&serde_json::json!(v), filter));
            }
            if kind == "rules" {
                let v = path(self.cur(), rest);
                return (!v.is_null()).then(|| self.filtered(v, filter));
            }
            if kind == "units" {
                // every playable unit whose field is true / non-empty: {{units:entrenchmentEligible}}
                let list: Vec<String> = self
                    .list("units")
                    .iter()
                    .filter(|u| {
                        !self
                            .factions_of("unit", &crate::render::util::s(u, "id"))
                            .is_empty()
                    })
                    .filter(|u| match rest.split_once('=') {
                        // {{units:threatRole=AntiArmorThreat}}
                        Some((field, want)) => scalar(path(u, field.trim())) == want.trim(),
                        None => match path(u, rest) {
                            Value::Bool(b) => *b,
                            Value::Null => false,
                            Value::String(s) => !s.is_empty(),
                            _ => true,
                        },
                    })
                    .map(|u| self.link(&crate::render::util::s(u, "id")))
                    .collect();
                return (!list.is_empty()).then(|| list.join(", "));
            }
            if kind == "dmg" {
                let (weapon, target) = rest.split_once('>')?;
                let (target, facing) = match target.split_once('.') {
                    Some((t, f)) => (t, f),
                    None => (target, "dmg"),
                };
                let w = self.weapon(weapon.trim())?;
                let row = crate::render::util::arr(w, "vs")
                    .iter()
                    .find(|r| crate::render::util::s(r, "target") == target.trim())?;
                return Some(scalar(row.get(facing)?));
            }
            if kind == "page" {
                let doc = self.content.get(rest)?;
                let title = filter
                    .map(str::to_string)
                    .unwrap_or_else(|| crate::render::pages::doc_title(rest, doc));
                return Some(format!(
                    "<a href=\"{}\">{}</a>",
                    crate::render::pages::doc_file(rest),
                    esc(&title)
                ));
            }
            let (id, field) = match rest.split_once('.') {
                Some((i, f)) => (i.trim(), Some(f.trim())),
                None => (rest, None),
            };
            let (v, file) = self.entity(kind, id)?;
            return match field {
                None => {
                    let label = filter
                        .map(str::to_string)
                        .unwrap_or_else(|| self.entity_name(kind, id));
                    Some(format!("<a href=\"{file}\">{}</a>", esc(&label)))
                }
                Some(f) => {
                    let value = path(v, f);
                    (!value.is_null()).then(|| self.filtered(value, filter))
                }
            };
        }
        let value = self.constant(head)?;
        Some(self.filtered(&value, filter))
    }

    fn filtered(&self, v: &Value, filter: Option<&str>) -> String {
        let Some(x) = v.as_f64() else {
            return esc(&scalar(v));
        };
        match filter {
            Some("s") => self.u.secs(x),
            Some("pct") => format!("{}%", fmt(x * 100.0)),
            Some("x") => format!("×{}", fmt(x)),
            Some("change") => {
                let d = (x - 1.0) * 100.0;
                format!("{}{}%", if d >= 0.0 { "+" } else { "−" }, fmt(d.abs()))
            }
            Some("tiles") => format!("{} tiles", fmt(x / self.u.tile)),
            Some("speed") => format!("{} tiles/s", crate::render::util::fmt2(self.u.speed(x))),
            Some("deg") => format!("{}°", fmt(x.to_degrees())),
            Some("round") => format!("{}", x.round() as i64),
            Some("2") => crate::render::util::fmt2(x),
            _ => fmt(x),
        }
    }

    /// `{{calc: STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}}` — + - * / and parentheses over
    /// numbers, constant names and `unit:tank.hp`-style references.
    fn calc(&self, expr: &str) -> Option<f64> {
        let toks = tokenize(expr)?;
        let mut i = 0;
        let v = self.sum(&toks, &mut i)?;
        (i == toks.len()).then_some(v)
    }

    fn sum(&self, t: &[Tok], i: &mut usize) -> Option<f64> {
        let mut v = self.product(t, i)?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = t.get(*i) {
            *i += 1;
            let r = self.product(t, i)?;
            v = if *op == '+' { v + r } else { v - r };
        }
        Some(v)
    }

    fn product(&self, t: &[Tok], i: &mut usize) -> Option<f64> {
        let mut v = self.atom(t, i)?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = t.get(*i) {
            *i += 1;
            let r = self.atom(t, i)?;
            v = if *op == '*' { v * r } else { v / r };
        }
        Some(v)
    }

    fn atom(&self, t: &[Tok], i: &mut usize) -> Option<f64> {
        let tok = t.get(*i)?.clone();
        *i += 1;
        match tok {
            Tok::Num(n) => Some(n),
            Tok::Op('-') => Some(-self.atom(t, i)?),
            Tok::Op('(') => {
                let v = self.sum(t, i)?;
                matches!(t.get(*i), Some(Tok::Op(')'))).then_some(())?;
                *i += 1;
                Some(v)
            }
            Tok::Name(name) => match name.split_once(':') {
                Some((kind, rest)) => {
                    let (id, field) = rest.split_once('.')?;
                    let (v, _) = self.entity(kind, id)?;
                    path(v, field).as_f64()
                }
                None => self.constant(&name)?.as_f64(),
            },
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
enum Tok {
    Num(f64),
    Name(String),
    Op(char),
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() {
            i += 1;
        } else if c[i].is_ascii_digit() || c[i] == '.' {
            let st = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            out.push(Tok::Num(c[st..i].iter().collect::<String>().parse().ok()?));
        } else if c[i].is_alphabetic() || c[i] == '_' {
            let st = i;
            while i < c.len() && (c[i].is_alphanumeric() || "_:.".contains(c[i])) {
                i += 1;
            }
            out.push(Tok::Name(c[st..i].iter().collect()));
        } else if "+-*/()".contains(c[i]) {
            out.push(Tok::Op(c[i]));
            i += 1;
        } else {
            return None;
        }
    }
    Some(out)
}
