use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

const MAX_PROMPT_CHARS: usize = 2_000;
const MAX_ATTEMPTS: usize = 80;

#[derive(Debug, Deserialize)]
pub struct UnitDesignRequest {
    pub prompt: String,
    #[serde(rename = "baseKind")]
    pub base_kind: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignAttempt {
    pub id: String,
    pub prompt: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub source: String,
    pub model: Option<String>,
    pub spec: UnitDesignSpec,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignSpec {
    pub name: String,
    pub role: String,
    pub silhouette: String,
    pub palette: Vec<String>,
    pub shapes: Vec<UnitDesignShape>,
    #[serde(rename = "animationNotes")]
    pub animation_notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignShape {
    pub kind: String,
    pub layer: i32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: f32,
    pub color: String,
    pub alpha: f32,
}

#[derive(Debug, Serialize)]
pub struct UnitDesignList {
    pub attempts: Vec<UnitDesignAttempt>,
}

#[derive(Debug, Serialize)]
pub struct UnitDesignGenerated {
    pub attempt: UnitDesignAttempt,
    pub warning: Option<String>,
}

pub async fn list_attempts() -> Result<UnitDesignList, String> {
    let mut attempts = read_attempts()?;
    attempts.sort_by(|a, b| b.id.cmp(&a.id));
    attempts.truncate(MAX_ATTEMPTS);
    Ok(UnitDesignList { attempts })
}

pub async fn generate_attempt(req: UnitDesignRequest) -> Result<UnitDesignGenerated, String> {
    let prompt = sanitize_prompt(&req.prompt)?;
    let base_kind = req.base_kind.as_deref().unwrap_or("new unit");
    let (mut spec, source, model, warning) = match generate_with_openai(&prompt, base_kind).await {
        Ok((spec, model)) => (spec, "openai".to_string(), Some(model), None),
        Err(err) => (
            fallback_spec(&prompt, base_kind),
            "offline".to_string(),
            None,
            Some(err),
        ),
    };
    normalize_spec(&mut spec);

    let attempt = UnitDesignAttempt {
        id: attempt_id(&prompt),
        prompt,
        created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        source,
        model,
        spec,
    };
    write_attempt(&attempt)?;
    Ok(UnitDesignGenerated { attempt, warning })
}

async fn generate_with_openai(
    prompt: &str,
    base_kind: &str,
) -> Result<(UnitDesignSpec, String), String> {
    let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        "OPENAI_API_KEY is not set; created an offline procedural draft".to_string()
    })?;
    let model = std::env::var("RTS_UNIT_LAB_MODEL").unwrap_or_else(|_| "gpt-5.4-mini".to_string());
    let system = "You create faction-agnostic RTS unit art specs for Bewegungskrieg. Return only JSON matching this shape: {\"name\":string,\"role\":string,\"silhouette\":string,\"palette\":[hex strings],\"shapes\":[{\"kind\":\"rect|ellipse|barrel|track|triangle\",\"layer\":integer,\"x\":number,\"y\":number,\"w\":number,\"h\":number,\"rotation\":number,\"color\":hex string,\"alpha\":number}],\"animationNotes\":[string]}. Use 6-18 simple hard-edged shapes, no flags, no national symbols, no text, no insignia.";
    let user = format!(
        "Base kind: {base_kind}\nPrompt: {prompt}\nCanvas coordinates are centered at 0,0. Keep the full unit inside roughly -70..70 x and -46..46 y. Facing points right. Use muted colors that will read over grass."
    );
    let body = json!({
        "model": model,
        "instructions": system,
        "input": user,
        "max_output_tokens": 1400,
        "text": {"format": {"type": "json_object"}}
    });
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            format!("OpenAI request failed; created an offline procedural draft: {err}")
        })?;
    let status = response.status();
    let response_text = response.text().await.map_err(|err| {
        format!("OpenAI response read failed; created an offline procedural draft: {err}")
    })?;
    if !status.is_success() {
        return Err(format!(
            "OpenAI returned {status}; created an offline procedural draft"
        ));
    }
    let envelope: serde_json::Value = serde_json::from_str(&response_text).map_err(|err| {
        format!("OpenAI response was not JSON; created an offline procedural draft: {err}")
    })?;
    let text = extract_response_text(&envelope).ok_or_else(|| {
        "OpenAI response did not contain output text; created an offline procedural draft"
            .to_string()
    })?;
    let spec: UnitDesignSpec = serde_json::from_str(&text).map_err(|err| {
        format!("OpenAI unit spec was invalid; created an offline procedural draft: {err}")
    })?;
    Ok((spec, model))
}

fn extract_response_text(envelope: &serde_json::Value) -> Option<String> {
    if let Some(text) = envelope.get("output_text").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    let output = envelope.get("output")?.as_array()?;
    for item in output {
        for content in item.get("content")?.as_array()? {
            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn sanitize_prompt(raw: &str) -> Result<String, String> {
    let prompt = raw.trim();
    if prompt.is_empty() {
        return Err("prompt is required".to_string());
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(format!(
            "prompt is too long; keep it under {MAX_PROMPT_CHARS} characters"
        ));
    }
    Ok(prompt.to_string())
}

fn fallback_spec(prompt: &str, base_kind: &str) -> UnitDesignSpec {
    let lower = prompt.to_ascii_lowercase();
    let tracked = lower.contains("tank") || lower.contains("tracked") || lower.contains("armor");
    let gun = lower.contains("gun") || lower.contains("cannon") || lower.contains("rifle");
    let car = lower.contains("car") || lower.contains("truck") || lower.contains("scout");
    let name = if tracked {
        "Tracked Field Prototype"
    } else if car {
        "Recon Field Prototype"
    } else if gun {
        "Support Weapon Prototype"
    } else {
        "Infantry Prototype"
    };
    let hull = if tracked { "#526246" } else { "#5d6047" };
    let accent = if lower.contains("fast") || lower.contains("scout") {
        "#8d9f72"
    } else {
        "#9b8d63"
    };
    let mut shapes = vec![
        shape("ellipse", 0, (0.0, 8.0, 88.0, 46.0, 0.0), "#151512", 0.38),
        shape("rect", 2, (-3.0, 0.0, 58.0, 28.0, 0.0), hull, 1.0),
        shape("rect", 3, (8.0, -1.0, 32.0, 18.0, 0.0), "#6f795c", 1.0),
        shape("triangle", 4, (37.0, 0.0, 18.0, 18.0, 0.0), accent, 0.95),
    ];
    if tracked {
        shapes.push(shape(
            "track",
            1,
            (-3.0, -18.0, 66.0, 9.0, 0.0),
            "#23231e",
            1.0,
        ));
        shapes.push(shape(
            "track",
            1,
            (-3.0, 18.0, 66.0, 9.0, 0.0),
            "#23231e",
            1.0,
        ));
    } else {
        shapes.push(shape(
            "ellipse",
            1,
            (-22.0, -17.0, 13.0, 13.0, 0.0),
            "#191915",
            1.0,
        ));
        shapes.push(shape(
            "ellipse",
            1,
            (22.0, -17.0, 13.0, 13.0, 0.0),
            "#191915",
            1.0,
        ));
        shapes.push(shape(
            "ellipse",
            1,
            (-22.0, 17.0, 13.0, 13.0, 0.0),
            "#191915",
            1.0,
        ));
        shapes.push(shape(
            "ellipse",
            1,
            (22.0, 17.0, 13.0, 13.0, 0.0),
            "#191915",
            1.0,
        ));
    }
    if gun {
        shapes.push(shape(
            "barrel",
            5,
            (47.0, 0.0, 44.0, 6.0, 0.0),
            "#20261f",
            1.0,
        ));
    }
    UnitDesignSpec {
        name: format!("{name} ({base_kind})"),
        role: "Local draft generated without the AI provider; refine with another prompt or set OPENAI_API_KEY.".to_string(),
        silhouette: "Low-profile hard-edged field unit with a clear forward read.".to_string(),
        palette: vec![hull.to_string(), accent.to_string(), "#23231e".to_string()],
        shapes,
        animation_notes: vec![
            "Idle: subtle one-pixel body vibration.".to_string(),
            "Move: alternate track or wheel phase from travel distance.".to_string(),
            "Fire: recoil the forward barrel/body accent backward for 120 ms.".to_string(),
        ],
    }
}

fn shape(
    kind: &str,
    layer: i32,
    geom: (f32, f32, f32, f32, f32),
    color: &str,
    alpha: f32,
) -> UnitDesignShape {
    let (x, y, w, h, rotation) = geom;
    UnitDesignShape {
        kind: kind.to_string(),
        layer,
        x,
        y,
        w,
        h,
        rotation,
        color: color.to_string(),
        alpha,
    }
}

fn normalize_spec(spec: &mut UnitDesignSpec) {
    spec.name = clamp_string(&spec.name, 64, "Unit Prototype");
    spec.role = clamp_string(&spec.role, 240, "");
    spec.silhouette = clamp_string(&spec.silhouette, 240, "");
    spec.palette.truncate(8);
    spec.palette.retain(|color| is_hex_color(color));
    spec.animation_notes.truncate(6);
    for note in &mut spec.animation_notes {
        *note = clamp_string(note, 140, "");
    }
    spec.shapes.truncate(24);
    for shape in &mut spec.shapes {
        if !matches!(
            shape.kind.as_str(),
            "rect" | "ellipse" | "barrel" | "track" | "triangle"
        ) {
            shape.kind = "rect".to_string();
        }
        shape.x = shape.x.clamp(-80.0, 80.0);
        shape.y = shape.y.clamp(-56.0, 56.0);
        shape.w = shape.w.clamp(2.0, 150.0);
        shape.h = shape.h.clamp(2.0, 110.0);
        shape.rotation = shape
            .rotation
            .clamp(-std::f32::consts::PI, std::f32::consts::PI);
        shape.alpha = shape.alpha.clamp(0.12, 1.0);
        if !is_hex_color(&shape.color) {
            shape.color = "#697256".to_string();
        }
    }
    spec.shapes.sort_by_key(|shape| shape.layer);
    if spec.shapes.is_empty() {
        *spec = fallback_spec("simple field unit", "new unit");
    }
}

fn clamp_string(value: &str, max_chars: usize, fallback: &str) -> String {
    let trimmed = value.trim();
    let source = if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    };
    source.chars().take(max_chars).collect()
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

fn attempt_id(prompt: &str) -> String {
    let ts = Utc::now().format("%Y%m%d%H%M%S");
    format!("{ts}-{}", slug(prompt))
}

fn slug(prompt: &str) -> String {
    let mut out = String::new();
    for ch in prompt.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 36 {
            break;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "unit".to_string()
    } else {
        trimmed.to_string()
    }
}

fn storage_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("unit-design-lab")
}

fn attempt_path(id: &str) -> PathBuf {
    storage_dir().join(format!("{id}.json"))
}

fn write_attempt(attempt: &UnitDesignAttempt) -> Result<(), String> {
    let dir = storage_dir();
    fs::create_dir_all(&dir)
        .map_err(|err| format!("failed to create unit design storage: {err}"))?;
    let payload = serde_json::to_string_pretty(attempt)
        .map_err(|err| format!("failed to serialize unit design: {err}"))?;
    fs::write(attempt_path(&attempt.id), payload)
        .map_err(|err| format!("failed to write unit design attempt: {err}"))
}

fn read_attempts() -> Result<Vec<UnitDesignAttempt>, String> {
    let dir = storage_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut attempts = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("failed to read unit designs: {err}"))? {
        let entry = entry.map_err(|err| format!("failed to read unit design entry: {err}"))?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let payload = fs::read_to_string(entry.path())
            .map_err(|err| format!("failed to read unit design file: {err}"))?;
        if let Ok(attempt) = serde_json::from_str::<UnitDesignAttempt>(&payload) {
            attempts.push(attempt);
        }
    }
    Ok(attempts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_spec_normalizes_to_drawable_shapes() {
        let mut spec = fallback_spec("fast tracked cannon unit", "tank");
        normalize_spec(&mut spec);
        assert!(!spec.shapes.is_empty());
        assert!(spec.shapes.iter().any(|shape| shape.kind == "track"));
        assert!(spec.shapes.iter().all(|shape| is_hex_color(&shape.color)));
    }

    #[test]
    fn prompt_slug_is_stable_and_bounded() {
        assert_eq!(slug("  Heavy Gun!!  "), "heavy-gun");
        assert!(slug("abcdefghijklmnopqrstuvwxyz0123456789-more").len() <= 36);
    }
}
