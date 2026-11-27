//! Small helpers: JSON access, number/time formatting, HTML escaping.

use serde_json::Value;

pub static NULL: Value = Value::Null;

pub fn get<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or(&NULL)
}

/// Dotted path lookup: "attack.dmg".
pub fn path<'a>(v: &'a Value, dotted: &str) -> &'a Value {
    dotted.split('.').fold(v, |cur, k| get(cur, k))
}

pub fn s(v: &Value, key: &str) -> String {
    match get(v, key) {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => scalar(other),
    }
}

pub fn n(v: &Value, key: &str) -> Option<f64> {
    get(v, key).as_f64()
}

pub fn nz(v: &Value, key: &str) -> f64 {
    n(v, key).unwrap_or(0.0)
}

pub fn b(v: &Value, key: &str) -> bool {
    get(v, key).as_bool().unwrap_or(false)
}

pub fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    get(v, key).as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

pub fn strs(v: &Value, key: &str) -> Vec<String> {
    arr(v, key)
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

/// Text form of any scalar JSON value.
pub fn scalar(v: &Value) -> String {
    match v {
        Value::Null => "—".into(),
        Value::Bool(x) => {
            if *x {
                "yes".into()
            } else {
                "no".into()
            }
        }
        Value::Number(x) => x.as_f64().map(fmt).unwrap_or_else(|| x.to_string()),
        Value::String(x) => x.clone(),
        Value::Array(a) => a.iter().map(scalar).collect::<Vec<_>>().join(", "),
        Value::Object(_) => v.to_string(),
    }
}

pub fn fmt(x: f64) -> String {
    if !x.is_finite() {
        return "—".into();
    }
    if (x - x.round()).abs() < 1e-9 {
        return format!("{}", x.round() as i64);
    }
    let t = format!("{x:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn fmt2(x: f64) -> String {
    if (x - x.round()).abs() < 1e-9 {
        return format!("{}", x.round() as i64);
    }
    let t = format!("{x:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub struct Units {
    pub tick_hz: f64,
    pub tile: f64,
}

impl Units {
    pub fn secs(&self, ticks: f64) -> String {
        format!("{} s", fmt2(ticks / self.tick_hz))
    }
    pub fn secs_raw(&self, ticks: f64) -> f64 {
        ticks / self.tick_hz
    }
    /// px per tick -> tiles per second
    pub fn speed(&self, px_per_tick: f64) -> f64 {
        px_per_tick * self.tick_hz / self.tile
    }
}

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// "evolveWind" / "dao_wind" -> "EVOLVE_WIND" / "DAO_WIND"
pub fn upper_snake(id: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in id.chars() {
        if c.is_ascii_uppercase() && prev_lower {
            out.push('_');
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c.to_ascii_uppercase());
        }
    }
    out
}

pub fn title_case(id: &str) -> String {
    id.split(['_', '-'])
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// "2026-11-26T11:49:30-05:00" -> "2026-11-26"
pub fn day(iso: &str) -> String {
    iso.chars().take(10).collect()
}
