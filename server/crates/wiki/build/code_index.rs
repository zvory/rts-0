//! Where every number comes from in the game's source code.
//!
//! * `definitions`: reads the struct literals that define units, buildings, weapons, abilities and
//!   research (`UnitDef { kind: EntityKind::Tank, stats: UnitStats { hp: 292, .. } }`,
//!   `AbilityCatalogEntry { kind: AbilityKind::Smoke, cooldown_ticks: balance::X, .. }`, ...) and
//!   records, per field, the file, line, expression and the constants it names. `..BASE` spreads
//!   are followed, so inherited fields point at the line that really sets them.
//! * `usage`: every place in the rules and simulation crates that reads a constant, with the
//!   enclosing function, so a number can be followed into the code that applies it.
//!
//! Only source text is read. Anything unrecognised is skipped, never guessed.

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Debug)]
struct Tok {
    text: String,
    line: usize,
    start: usize,
    end: usize,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut line) = (0usize, 1usize);
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                if b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
        } else if c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'"') | Some(b'#'))) {
            // string or raw string
            let (st, l0) = (i, line);
            if c == b'r' {
                let mut hashes = 0;
                i += 1;
                while b.get(i) == Some(&b'#') {
                    hashes += 1;
                    i += 1;
                }
                i += 1; // opening quote
                loop {
                    if i >= b.len() {
                        break;
                    }
                    if b[i] == b'\n' {
                        line += 1;
                    }
                    if b[i] == b'"' && (0..hashes).all(|h| b.get(i + 1 + h) == Some(&b'#')) {
                        i += 1 + hashes;
                        break;
                    }
                    i += 1;
                }
            } else {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    } else if b[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            out.push(Tok {
                text: "\"\"".into(),
                line: l0,
                start: st,
                end: i.min(b.len()),
            });
        } else if c == b'\'' {
            // char literal or lifetime
            if b.get(i + 2) == Some(&b'\'') || b.get(i + 1) == Some(&b'\\') {
                let st = i;
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                out.push(Tok {
                    text: "''".into(),
                    line,
                    start: st,
                    end: i.min(b.len()),
                });
            } else {
                i += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
            }
        } else if c.is_ascii_alphanumeric() || c == b'_' {
            let st = i;
            let numeric = c.is_ascii_digit();
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == b'_'
                    || (numeric && b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)))
            {
                i += 1;
            }
            out.push(Tok {
                text: src[st..i].to_string(),
                line,
                start: st,
                end: i,
            });
        } else {
            let two = src.get(i..i + 2).unwrap_or("");
            let len = if matches!(
                two,
                "::" | ".." | "->" | "=>" | "==" | "!=" | "<=" | ">=" | "&&" | "||"
            ) {
                2
            } else {
                1
            };
            out.push(Tok {
                text: src[i..i + len].to_string(),
                line,
                start: i,
                end: i + len,
            });
            i += len;
        }
    }
    out
}

fn is_ident(t: &str) -> bool {
    t.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && t.chars().all(|c| c.is_alphanumeric() || c == '_')
}

pub fn is_const_name(t: &str) -> bool {
    t.len() > 1
        && t.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && t.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && t.contains('_')
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    expr: String,
    line: usize,
    child: Option<Block>,
}

#[derive(Clone, Debug, Default)]
struct Block {
    type_name: String,
    fields: Vec<Field>,
    spreads: Vec<String>,
}

/// Parse `{ a: expr, b: Type { .. }, ..BASE }` starting at the `{` token. None if not a literal.
fn parse_block(t: &[Tok], src: &str, open: usize, type_name: &str) -> Option<(Block, usize)> {
    let mut block = Block {
        type_name: type_name.to_string(),
        ..Default::default()
    };
    let mut j = open + 1;
    loop {
        let tok = t.get(j)?;
        if tok.text == "}" {
            return Some((block, j));
        }
        if tok.text == ".." {
            let (end, _) = expr_end(t, j + 1)?;
            block.spreads.push(
                t[j + 1..end]
                    .iter()
                    .map(|x| x.text.as_str())
                    .collect::<String>(),
            );
            j = end;
        } else if is_ident(&tok.text) && t.get(j + 1)?.text == ":" {
            let start = j + 2;
            let (end, _) = expr_end(t, start)?;
            if end == start {
                return None;
            }
            // a nested struct literal: `Type { .. }` or `path::Type { .. }`
            let mut child = None;
            if t[end - 1].text == "}" {
                if let Some(k) = (start..end).find(|&k| t[k].text == "{") {
                    if k > start && t[k - 1].text.chars().next().is_some_and(char::is_uppercase) {
                        if let Some((b, close)) = parse_block(t, src, k, &t[k - 1].text) {
                            if close == end - 1 {
                                child = Some(b);
                            }
                        }
                    }
                }
            }
            block.fields.push(Field {
                name: tok.text.clone(),
                expr: src[t[start].start..t[end - 1].end]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                line: tok.line,
                child,
            });
            j = end;
        } else {
            return None;
        }
        match t.get(j)?.text.as_str() {
            "," => j += 1,
            "}" => {}
            _ => return None,
        }
    }
}

/// Index just past an expression: stops at `,` or `}` at depth 0.
fn expr_end(t: &[Tok], from: usize) -> Option<(usize, ())> {
    let mut depth = 0i32;
    let mut k = from;
    while k < t.len() {
        match t[k].text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return Some((k, ()));
                }
                depth -= 1;
            }
            "," if depth == 0 => return Some((k, ())),
            _ => {}
        }
        k += 1;
    }
    None
}

struct Parsed {
    /// (enum, variant, type name) -> block
    keyed: Vec<(String, String, Block, String)>,
    /// const NAME = Type { .. }
    named: HashMap<String, (Block, String)>,
}

fn parse_file(path: &str, src: &str, into: &mut Parsed) {
    let t = tokenize(src);
    for i in 1..t.len() {
        if t[i].text != "{" {
            continue;
        }
        let ty = &t[i - 1].text;
        if !ty.chars().next().is_some_and(char::is_uppercase) || !is_ident(ty) {
            continue;
        }
        if i >= 2
            && matches!(
                t[i - 2].text.as_str(),
                "struct"
                    | "enum"
                    | "impl"
                    | "trait"
                    | "union"
                    | "for"
                    | "mod"
                    | "fn"
                    | "match"
                    | "if"
                    | "while"
            )
        {
            continue;
        }
        let Some((block, _)) = parse_block(&t, src, i, ty) else {
            continue;
        };
        // const NAME: Type = Type {
        if i >= 6 && t[i - 2].text == "=" {
            if let Some(k) = (i.saturating_sub(8)..i - 2)
                .rev()
                .find(|&k| matches!(t[k].text.as_str(), "const" | "static"))
            {
                if let Some(name) = t.get(k + 1) {
                    into.named
                        .insert(name.text.clone(), (block.clone(), path.to_string()));
                }
            }
        }
        for f in &block.fields {
            if f.name == "kind" || f.name == "id" {
                let parts: Vec<&str> = f.expr.split("::").collect();
                if parts.len() >= 2 {
                    let (en, var) = (parts[parts.len() - 2], parts[parts.len() - 1]);
                    if matches!(
                        en,
                        "EntityKind" | "WeaponKind" | "AbilityKind" | "UpgradeKind"
                    ) && is_ident(var)
                    {
                        into.keyed.push((
                            en.to_string(),
                            var.to_string(),
                            block.clone(),
                            path.to_string(),
                        ));
                    }
                }
            }
        }
    }
}

/// field path -> (file, line, expr), following `..BASE` / `..BASE.sub` spreads.
fn flatten(
    block: &Block,
    file: &str,
    named: &HashMap<String, (Block, String)>,
    prefix: &str,
    out: &mut Vec<(String, String, usize, String)>,
    depth: usize,
) {
    if depth > 6 {
        return;
    }
    for f in &block.fields {
        let path = format!("{prefix}{}", f.name);
        match &f.child {
            Some(c) => flatten(c, file, named, &format!("{path}."), out, depth + 1),
            None => out.push((path, file.to_string(), f.line, f.expr.clone())),
        }
    }
    for spread in &block.spreads {
        let mut parts = spread.splitn(2, '.');
        let base = parts.next().unwrap_or("");
        let sub = parts.next().unwrap_or("");
        let Some((b, bfile)) = named.get(base) else {
            continue;
        };
        let mut inherited = Vec::new();
        flatten(b, bfile, named, "", &mut inherited, depth + 1);
        for (p, f, l, e) in inherited {
            let rel = if sub.is_empty() {
                Some(p.as_str())
            } else {
                p.strip_prefix(&format!("{sub}."))
            };
            if let Some(rel) = rel {
                let full = format!("{prefix}{rel}");
                if !out.iter().any(|(q, ..)| *q == full) {
                    out.push((full, f, l, e));
                }
            }
        }
    }
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if name != "tests" && name != "bin" {
                rust_files(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(p);
        }
    }
}

fn repo_path(src_root: &Path, p: &Path) -> String {
    p.strip_prefix(src_root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// `definitions`: { "EntityKind:Tank": { "stats.hp": {path, line, expr, consts} }, ... } — keyed by
/// the enum and variant named in the definition; the crate maps them to page ids at run time.
pub fn definitions_raw(src_root: &Path, consts: &HashSet<String>) -> Value {
    let wanted: HashMap<&str, &[&str]> = HashMap::from([
        ("EntityKind", &["UnitDef", "BuildingDef", "NodeDef"][..]),
        ("WeaponKind", &["WeaponProfile"][..]),
        ("AbilityKind", &["AbilityCatalogEntry"][..]),
        ("UpgradeKind", &["UpgradeCatalogEntry"][..]),
    ]);
    let mut files = Vec::new();
    rust_files(&src_root.join("server/crates/rules/src"), &mut files);
    files.sort();
    let mut parsed = Parsed {
        keyed: Vec::new(),
        named: HashMap::new(),
    };
    for f in &files {
        if let Ok(text) = std::fs::read_to_string(f) {
            parse_file(&repo_path(src_root, f), &text, &mut parsed);
        }
    }
    let mut out = Map::new();
    for (en, var, block, file) in &parsed.keyed {
        if !wanted
            .get(en.as_str())
            .is_some_and(|types| types.contains(&block.type_name.as_str()))
        {
            continue;
        }
        let key = format!("{en}:{var}");
        if out.contains_key(&key) {
            continue; // first definition wins (abilities appear once per catalog)
        }
        let mut fields = Vec::new();
        flatten(block, file, &parsed.named, "", &mut fields, 0);
        let mut m = Map::new();
        for (path, f, line, expr) in fields {
            let named: Vec<String> = tokenize(&expr)
                .into_iter()
                .map(|t| t.text)
                .filter(|t| consts.contains(t))
                .collect::<Vec<_>>();
            let mut uniq = Vec::new();
            for n in named {
                if !uniq.contains(&n) {
                    uniq.push(n);
                }
            }
            m.insert(
                path,
                json!({ "path": f, "line": line, "expr": expr, "consts": uniq }),
            );
        }
        out.insert(key, Value::Object(m));
    }
    Value::Object(out)
}

/// `usage`: { CONST: [ {path, line, fn} ] } across the rules and simulation crates (tests skipped).
pub fn usage(src_root: &Path, consts: &HashSet<String>, per_const: usize) -> Value {
    let mut files = Vec::new();
    for krate in ["rules", "sim"] {
        rust_files(
            &src_root.join(format!("server/crates/{krate}/src")),
            &mut files,
        );
    }
    files.sort();
    let mut out: HashMap<String, Vec<Value>> = HashMap::new();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let path = repo_path(src_root, f);
        let t = tokenize(&text);
        // brace depth, the functions we are inside (name, depth of their body), and test modules
        let mut depth = 0usize;
        let mut fns: Vec<(String, usize)> = Vec::new();
        let mut pending_fn: Option<String> = None;
        let mut skip_until: Option<usize> = None;
        let mut pending_test_mod = false;
        let mut seen_here: HashSet<(String, usize)> = HashSet::new();
        for (i, tok) in t.iter().enumerate() {
            match tok.text.as_str() {
                "{" => {
                    depth += 1;
                    if pending_test_mod && skip_until.is_none() {
                        skip_until = Some(depth - 1);
                    }
                    pending_test_mod = false;
                    if let Some(name) = pending_fn.take() {
                        fns.push((name, depth));
                    }
                    continue;
                }
                "}" => {
                    if fns.last().is_some_and(|(_, d)| *d == depth) {
                        fns.pop();
                    }
                    depth = depth.saturating_sub(1);
                    if skip_until == Some(depth) {
                        skip_until = None;
                    }
                    continue;
                }
                ";" => {
                    pending_fn = None; // a declaration without a body
                    pending_test_mod = false;
                    continue;
                }
                "fn" => {
                    pending_fn = t.get(i + 1).map(|n| n.text.clone());
                    continue;
                }
                "mod"
                    if t.get(i + 1)
                        .is_some_and(|n| n.text == "tests" || n.text == "test") =>
                {
                    pending_test_mod = true;
                    continue;
                }
                _ => {}
            }
            if skip_until.is_some() || !consts.contains(&tok.text) {
                continue;
            }
            let current_fn = fns.last().map(|(n, _)| n.clone()).unwrap_or_default();
            // skip the definition itself: `const NAME:`
            if i >= 1 && t[i - 1].text == "const" {
                continue;
            }
            if !seen_here.insert((tok.text.clone(), tok.line)) {
                continue;
            }
            let list = out.entry(tok.text.clone()).or_default();
            if list.len() < per_const {
                list.push(json!({ "path": path, "line": tok.line, "fn": current_fn }));
            } else if list.len() == per_const {
                list.push(json!({ "more": true }));
            }
        }
    }
    let mut m = Map::new();
    let mut keys: Vec<_> = out.keys().cloned().collect();
    keys.sort();
    for k in keys {
        m.insert(k.clone(), Value::Array(out.remove(&k).unwrap()));
    }
    Value::Object(m)
}
