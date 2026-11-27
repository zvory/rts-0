//! Reads the player-facing text (labels, icons, tooltips) from the client's rules mirror
//! (`client/src/config/rules_mirror.js`). This is presentation data the Rust rules do not own.
//! The reader is deliberately forgiving: anything it cannot understand is simply left out.

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Entry {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub upgrade_requires: Option<String>,
    pub requires_text: Option<String>,
}

pub struct ClientText {
    pub kinds: HashMap<String, Entry>,
    pub upgrades: HashMap<String, Entry>,
}

pub fn read(js: &str, rust_consts: &HashMap<String, f64>) -> ClientText {
    let consts = js_consts(js, rust_consts);
    ClientText {
        kinds: entries(js, "export const STATS", "KIND.", &consts),
        upgrades: entries(js, "export const UPGRADES", "UPGRADE.", &consts),
    }
}

/// `export const NAME = <arithmetic>;` values, falling back to Rust constants of the same name.
fn js_consts(js: &str, rust: &HashMap<String, f64>) -> HashMap<String, f64> {
    let mut map = rust.clone();
    for line in js.lines() {
        let Some(rest) = line.trim().strip_prefix("export const ") else {
            continue;
        };
        let Some((name, expr)) = rest.split_once('=') else {
            continue;
        };
        let Some(expr) = expr.trim().strip_suffix(';') else {
            continue;
        };
        if let Some(v) = eval(expr, &map) {
            map.insert(name.trim().to_string(), v);
        }
    }
    map
}

fn entries(
    js: &str,
    marker: &str,
    key_prefix: &str,
    consts: &HashMap<String, f64>,
) -> HashMap<String, Entry> {
    let mut out = HashMap::new();
    let Some(start) = js.find(marker) else {
        return out;
    };
    let bytes: Vec<char> = js[start..].chars().collect();
    let Some(open) = bytes.iter().position(|&c| c == '{') else {
        return out;
    };
    // Walk the top-level object; record `[KEY]: <value>` pairs at depth 1.
    let mut i = open + 1;
    let mut depth = 1;
    while i < bytes.len() && depth > 0 {
        let c = bytes[i];
        match c {
            '"' | '\'' | '`' => i = skip_string(&bytes, i),
            '/' if bytes.get(i + 1) == Some(&'/') => {
                while i < bytes.len() && bytes[i] != '\n' {
                    i += 1;
                }
            }
            '{' | '(' => {
                depth += 1;
                i += 1;
            }
            '}' | ')' => {
                depth -= 1;
                i += 1;
            }
            '[' if depth == 1 => {
                let close = (i..bytes.len())
                    .find(|&j| bytes[j] == ']')
                    .unwrap_or(bytes.len());
                let key: String = bytes[i + 1..close].iter().collect();
                // value runs to the next ',' or '}' at depth 1
                let mut j = close + 1;
                let mut d = 0;
                while j < bytes.len() {
                    match bytes[j] {
                        '"' | '\'' | '`' => {
                            j = skip_string(&bytes, j);
                            continue;
                        }
                        '{' | '(' | '[' => d += 1,
                        '}' | ')' | ']' => {
                            if d == 0 {
                                break;
                            }
                            d -= 1;
                        }
                        ',' if d == 0 => break,
                        _ => {}
                    }
                    j += 1;
                }
                let value: String = bytes[close + 1..j.min(bytes.len())].iter().collect();
                if let Some(id) = key.trim().strip_prefix(key_prefix) {
                    out.insert(id.to_ascii_lowercase(), entry(&value, consts));
                }
                i = j;
            }
            _ => i += 1,
        }
    }
    out
}

fn entry(value: &str, consts: &HashMap<String, f64>) -> Entry {
    Entry {
        label: field_string(value, "label", consts),
        icon: field_string(value, "icon", consts),
        description: field_string(value, "description", consts),
        upgrade_requires: field_raw(value, "upgradeRequires")
            .and_then(|raw| raw.strip_prefix("UPGRADE.").map(|s| s.to_ascii_lowercase())),
        requires_text: field_string(value, "upgradeRequiresText", consts)
            .or_else(|| field_string(value, "requiresText", consts)),
    }
}

/// Position just after `name:` when `name` is a whole field name.
fn field_pos(value: &str, name: &str) -> Option<usize> {
    let pat = format!("{name}:");
    let mut from = 0;
    while let Some(p) = value[from..].find(&pat) {
        let at = from + p;
        let before = value[..at].chars().last();
        if before.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
            return Some(at + pat.len());
        }
        from = at + pat.len();
    }
    None
}

fn field_raw(value: &str, name: &str) -> Option<String> {
    let start = field_pos(value, name)?;
    let rest = value[start..].trim_start();
    let end = rest.find([',', '}', '\n']).unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// A string field: literals and templates joined with `+`.
fn field_string(value: &str, name: &str, consts: &HashMap<String, f64>) -> Option<String> {
    let start = field_pos(value, name)?;
    let chars: Vec<char> = value[start..].chars().collect();
    let mut i = 0;
    let mut out = String::new();
    let mut any = false;
    loop {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let Some(&q) = chars.get(i) else { break };
        if !matches!(q, '"' | '\'' | '`') {
            break;
        }
        let end = skip_string(&chars, i);
        let body: String = chars[i + 1..end - 1].iter().collect();
        out.push_str(&unescape(&body, q == '`', consts));
        any = true;
        i = end;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if chars.get(i) == Some(&'+') {
            i += 1;
        } else {
            break;
        }
    }
    any.then_some(out)
}

fn skip_string(chars: &[char], start: usize) -> usize {
    let q = chars[start];
    let mut i = start + 1;
    let mut brace = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if q == '`' && c == '$' && chars.get(i + 1) == Some(&'{') {
            brace += 1;
            i += 2;
            continue;
        }
        if q == '`' && brace > 0 && c == '}' {
            brace -= 1;
        } else if c == q && brace == 0 {
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

fn unescape(body: &str, template: bool, consts: &HashMap<String, f64>) -> String {
    let mut out = String::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            out.push(match chars[i + 1] {
                'n' => '\n',
                't' => '\t',
                other => other,
            });
            i += 2;
        } else if template && c == '$' && chars.get(i + 1) == Some(&'{') {
            let mut j = i + 2;
            let mut depth = 1;
            while j < chars.len() {
                match chars[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let expr: String = chars[i + 2..j.min(chars.len())].iter().collect();
            out.push_str(&interpolate(expr.trim(), consts));
            i = j + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn interpolate(expr: &str, consts: &HashMap<String, f64>) -> String {
    if let Some((head, tail)) = expr.rsplit_once(".toFixed(") {
        let digits: usize = tail.trim_end_matches(')').trim().parse().unwrap_or(0);
        if let Some(v) = eval(head, consts) {
            return format!("{v:.digits$}");
        }
    }
    match eval(expr, consts) {
        Some(v) => fmt_num(v),
        None => expr.to_string(),
    }
}

pub fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.4}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Tiny arithmetic evaluator: numbers, known identifiers, + - * / and parentheses.
/// `Math.fround(x)` and `Math.round(x)` are understood; anything else fails.
pub fn eval(expr: &str, consts: &HashMap<String, f64>) -> Option<f64> {
    let tokens = tokenize(expr)?;
    let mut p = Parser {
        t: &tokens,
        i: 0,
        consts,
    };
    let v = p.expr()?;
    (p.i == tokens.len()).then_some(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(char),
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let st = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let lit: String = chars[st..i].iter().filter(|&&ch| ch != '_').collect();
            out.push(Tok::Num(lit.parse().ok()?));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            out.push(Tok::Id(chars[st..i].iter().collect()));
        } else if "+-*/()".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else {
            return None;
        }
    }
    Some(out)
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    consts: &'a HashMap<String, f64>,
}

impl Parser<'_> {
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.t.get(self.i).cloned() {
            self.i += 1;
            let r = self.term()?;
            v = if op == '+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.atom()?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = self.t.get(self.i).cloned() {
            self.i += 1;
            let r = self.atom()?;
            v = if op == '*' { v * r } else { v / r };
        }
        Some(v)
    }
    fn atom(&mut self) -> Option<f64> {
        match self.t.get(self.i).cloned()? {
            Tok::Num(n) => {
                self.i += 1;
                Some(n)
            }
            Tok::Op('-') => {
                self.i += 1;
                Some(-self.atom()?)
            }
            Tok::Op('(') => {
                self.i += 1;
                let v = self.expr()?;
                (self.t.get(self.i) == Some(&Tok::Op(')'))).then_some(())?;
                self.i += 1;
                Some(v)
            }
            Tok::Id(id) => {
                self.i += 1;
                if matches!(id.as_str(), "Math.fround" | "Math.round" | "Number") {
                    let v = self.atom()?;
                    return Some(if id == "Math.round" { v.round() } else { v });
                }
                self.consts.get(&id).copied()
            }
            _ => None,
        }
    }
}
