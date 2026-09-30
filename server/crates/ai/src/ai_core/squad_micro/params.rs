//! Tunable parameters for the rifle squad planner, with a compact `key=value` text form used by
//! the `ai-skirmish` command line and its reports.

use std::fmt;

use serde::Serialize;

/// How riflemen pick targets once an enemy is in sight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FocusMode {
    /// Attack-move and let simulation auto-targeting pick the nearest enemy per rifleman.
    Nearest,
    /// Every rifleman attacks the visible enemy nearest the squad centre (AI 2.1 wave style).
    SquadNearest,
    /// Each rifleman attacks the lowest-HP enemy it can reach, preferring targets in range.
    Weakest,
}

impl FocusMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::SquadNearest => "squad_nearest",
            Self::Weakest => "weakest",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "nearest" => Ok(Self::Nearest),
            "squad_nearest" => Ok(Self::SquadNearest),
            "weakest" => Ok(Self::Weakest),
            other => Err(format!(
                "unknown focus mode {other:?}; expected nearest, squad_nearest, or weakest"
            )),
        }
    }
}

/// Parameters for [`super::plan_rifle_squad`]. Distances are in tiles, durations in ticks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RifleSquadParams {
    pub(crate) focus: FocusMode,
    /// Stop assigning shooters to a target once their next shots already cover its HP.
    pub(crate) overkill_guard: bool,
    /// A rifleman at or below this HP that a visible enemy is targeting steps back. 0 disables.
    pub(crate) retreat_hp: u32,
    pub(crate) retreat_tiles: f32,
    pub(crate) retreat_ticks: u32,
    /// Hold position until an enemy comes within weapon reach plus `contact_margin_tiles` of any
    /// rifleman, then fight with the focus rule.
    pub(crate) hold: bool,
    pub(crate) contact_margin_tiles: f32,
    /// While advancing, regroup on the squad centre when a rifleman strays farther than this.
    /// 0 disables.
    pub(crate) regroup_tiles: f32,
    /// Extra distance beyond the simulation's weapon reach at which a target still counts as in
    /// range when choosing targets (it moves between decisions).
    pub(crate) range_margin_tiles: f32,
}

impl RifleSquadParams {
    /// Plain attack-move with simulation auto-targeting: the no-micro control.
    pub(crate) fn naive() -> Self {
        Self {
            focus: FocusMode::Nearest,
            overkill_guard: false,
            retreat_hp: 0,
            retreat_tiles: 2.0,
            retreat_ticks: 30,
            hold: false,
            contact_margin_tiles: 0.5,
            regroup_tiles: 0.0,
            range_margin_tiles: 0.0,
        }
    }

    /// Starting micro: focus the weakest reachable enemy without overkill, keep the squad together.
    pub(crate) fn micro() -> Self {
        Self {
            focus: FocusMode::Weakest,
            overkill_guard: true,
            regroup_tiles: 3.0,
            ..Self::naive()
        }
    }

    /// Parse `preset[,key=value...]` or `key=value,...` (keys override the `micro` preset).
    pub(crate) fn parse(spec: &str) -> Result<Self, String> {
        let mut parts = spec
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .peekable();
        let mut params = match parts.peek().copied() {
            Some("naive") => {
                parts.next();
                Self::naive()
            }
            Some("micro") | Some("default") => {
                parts.next();
                Self::micro()
            }
            _ => Self::micro(),
        };
        for part in parts {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| format!("expected key=value, got {part:?}"))?;
            params.set(key.trim(), value.trim())?;
        }
        params.validate()?;
        Ok(params)
    }

    pub(crate) fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "focus" => self.focus = FocusMode::parse(value)?,
            "overkill" => self.overkill_guard = parse_bool(key, value)?,
            "retreat_hp" => self.retreat_hp = parse_num(key, value)?,
            "retreat_tiles" => self.retreat_tiles = parse_num(key, value)?,
            "retreat_ticks" => self.retreat_ticks = parse_num(key, value)?,
            "hold" => self.hold = parse_bool(key, value)?,
            "contact_margin_tiles" => self.contact_margin_tiles = parse_num(key, value)?,
            "regroup_tiles" => self.regroup_tiles = parse_num(key, value)?,
            "range_margin_tiles" => self.range_margin_tiles = parse_num(key, value)?,
            other => {
                return Err(format!(
                    "unknown micro parameter {other:?}; expected one of {}",
                    Self::KEYS.join(", ")
                ))
            }
        }
        Ok(())
    }

    /// Copy one parameter, named as in [`Self::KEYS`], from `other`.
    pub(crate) fn copy_key_from(&mut self, key: &str, other: &Self) {
        match key {
            "focus" => self.focus = other.focus,
            "overkill" => self.overkill_guard = other.overkill_guard,
            "retreat_hp" => self.retreat_hp = other.retreat_hp,
            "retreat_tiles" => self.retreat_tiles = other.retreat_tiles,
            "retreat_ticks" => self.retreat_ticks = other.retreat_ticks,
            "hold" => self.hold = other.hold,
            "contact_margin_tiles" => self.contact_margin_tiles = other.contact_margin_tiles,
            "regroup_tiles" => self.regroup_tiles = other.regroup_tiles,
            "range_margin_tiles" => self.range_margin_tiles = other.range_margin_tiles,
            _ => {}
        }
    }

    pub(crate) const KEYS: [&'static str; 9] = [
        "focus",
        "overkill",
        "retreat_hp",
        "retreat_tiles",
        "retreat_ticks",
        "hold",
        "contact_margin_tiles",
        "regroup_tiles",
        "range_margin_tiles",
    ];

    fn validate(&self) -> Result<(), String> {
        for (key, value) in [
            ("retreat_tiles", self.retreat_tiles),
            ("contact_margin_tiles", self.contact_margin_tiles),
            ("regroup_tiles", self.regroup_tiles),
            ("range_margin_tiles", self.range_margin_tiles),
        ] {
            if !value.is_finite() || !(0.0..=64.0).contains(&value) {
                return Err(format!("{key} must be between 0 and 64 tiles, got {value}"));
            }
        }
        if self.retreat_ticks > 900 {
            return Err(format!(
                "retreat_ticks must be at most 900, got {}",
                self.retreat_ticks
            ));
        }
        Ok(())
    }
}

impl fmt::Display for RifleSquadParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "focus={},overkill={},retreat_hp={},retreat_tiles={},retreat_ticks={},hold={},contact_margin_tiles={},regroup_tiles={},range_margin_tiles={}",
            self.focus.as_str(),
            u8::from(self.overkill_guard),
            self.retreat_hp,
            self.retreat_tiles,
            self.retreat_ticks,
            u8::from(self.hold),
            self.contact_margin_tiles,
            self.regroup_tiles,
            self.range_margin_tiles,
        )
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool, String> {
    match value {
        "1" | "true" | "on" => Ok(true),
        "0" | "false" | "off" => Ok(false),
        other => Err(format!("{key} expects 0/1, got {other:?}")),
    }
}

fn parse_num<T: std::str::FromStr>(key: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{key} expects a number, got {value:?}"))
}
