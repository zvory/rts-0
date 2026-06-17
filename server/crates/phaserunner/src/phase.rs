use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseId {
    label: String,
    major: u32,
    decimal: Option<String>,
    suffix: Option<char>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseIdParseError {
    raw: String,
}

impl PhaseId {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, PhaseIdParseError> {
        let raw = raw.as_ref();
        let label = raw.strip_prefix("phase-").unwrap_or(raw);
        let mut chars = label.chars().peekable();

        let mut major_text = String::new();
        while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
            major_text.push(chars.next().expect("peeked digit"));
        }
        if major_text.is_empty() {
            return Err(PhaseIdParseError::new(raw));
        }

        let decimal = if matches!(chars.peek(), Some('.')) {
            chars.next();
            let mut decimal_text = String::new();
            while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
                decimal_text.push(chars.next().expect("peeked digit"));
            }
            if decimal_text.is_empty() {
                return Err(PhaseIdParseError::new(raw));
            }
            Some(decimal_text)
        } else {
            None
        };

        let suffix = match chars.next() {
            Some(ch) if ch.is_ascii_lowercase() => Some(ch),
            Some(_) => return Err(PhaseIdParseError::new(raw)),
            None => None,
        };
        if chars.next().is_some() {
            return Err(PhaseIdParseError::new(raw));
        }

        let major = major_text
            .parse()
            .map_err(|_| PhaseIdParseError::new(raw))?;
        Ok(Self {
            label: label.to_string(),
            major,
            decimal,
            suffix,
        })
    }

    pub fn file_name(&self) -> String {
        format!("{}.md", self)
    }
}

impl fmt::Display for PhaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "phase-{}", self.label)
    }
}

impl PhaseIdParseError {
    fn new(raw: &str) -> Self {
        Self {
            raw: raw.to_string(),
        }
    }
}

impl fmt::Display for PhaseIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid phase '{}'; use N, N.M, Na, phase-N, phase-N.M, or phase-Na",
            self.raw
        )
    }
}

impl std::error::Error for PhaseIdParseError {}

impl Ord for PhaseId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| compare_decimals(self.decimal.as_deref(), other.decimal.as_deref()))
            .then_with(|| self.suffix.is_some().cmp(&other.suffix.is_some()))
            .then_with(|| self.suffix.cmp(&other.suffix))
            .then_with(|| self.label.cmp(&other.label))
    }
}

impl PartialOrd for PhaseId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_decimals(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(right)) => compare_fraction_digits("0", right),
        (Some(left), None) => compare_fraction_digits(left, "0"),
        (Some(left), Some(right)) => compare_fraction_digits(left, right),
    }
}

fn compare_fraction_digits(left: &str, right: &str) -> Ordering {
    let width = left.len().max(right.len());
    let left = right_pad_digits(left, width);
    let right = right_pad_digits(right, width);
    left.cmp(&right)
}

fn right_pad_digits(value: &str, width: usize) -> String {
    let mut padded = value.to_string();
    while padded.len() < width {
        padded.push('0');
    }
    padded
}

#[derive(Debug)]
pub enum PhaseDiscoveryError {
    InvalidFrom(PhaseIdParseError),
    InvalidTo(PhaseIdParseError),
    UnorderedBounds { from: PhaseId, to: PhaseId },
    ReadDir(std::io::Error),
    Empty { from: PhaseId, to: PhaseId },
}

impl fmt::Display for PhaseDiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrom(err) => write!(f, "invalid --from: {err}"),
            Self::InvalidTo(err) => write!(f, "invalid --to: {err}"),
            Self::UnorderedBounds { from, to } => {
                write!(f, "--from must be before --to: {from} .. {to}")
            }
            Self::ReadDir(err) => write!(f, "failed to read plan directory: {err}"),
            Self::Empty { from, to } => {
                write!(f, "no phase files discovered after {from} through {to}")
            }
        }
    }
}

impl std::error::Error for PhaseDiscoveryError {}

pub fn discover_phases(
    plan_dir: &Path,
    from: impl AsRef<str>,
    to: impl AsRef<str>,
) -> Result<Vec<PhaseId>, PhaseDiscoveryError> {
    let from = PhaseId::parse(from).map_err(PhaseDiscoveryError::InvalidFrom)?;
    let to = PhaseId::parse(to).map_err(PhaseDiscoveryError::InvalidTo)?;
    if from >= to {
        return Err(PhaseDiscoveryError::UnorderedBounds { from, to });
    }

    let mut phases = Vec::new();
    for entry in fs::read_dir(plan_dir).map_err(PhaseDiscoveryError::ReadDir)? {
        let entry = entry.map_err(PhaseDiscoveryError::ReadDir)?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(label) = file_name.strip_suffix(".md") else {
            continue;
        };
        if !label.starts_with("phase-") {
            continue;
        }
        if let Ok(phase) = PhaseId::parse(label) {
            if phase > from && phase <= to {
                phases.push(phase);
            }
        }
    }

    phases.sort();
    if phases.is_empty() {
        return Err(PhaseDiscoveryError::Empty { from, to });
    }
    Ok(phases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn normalizes_supported_phase_ids() {
        for (raw, expected) in [
            ("1", "phase-1"),
            ("5.5", "phase-5.5"),
            ("3a", "phase-3a"),
            ("phase-9", "phase-9"),
            ("phase-10.25", "phase-10.25"),
            ("phase-2b", "phase-2b"),
        ] {
            assert_eq!(PhaseId::parse(raw).unwrap().to_string(), expected);
        }
    }

    #[test]
    fn rejects_invalid_phase_ids() {
        for raw in ["", "phase-", "a1", "1A", "1.", "1.2b3", "phase/1"] {
            assert!(PhaseId::parse(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn orders_numeric_decimal_and_suffixed_ids() {
        let mut ids = [
            "phase-3a",
            "phase-5.5",
            "phase-3",
            "phase-10",
            "phase-5",
            "phase-5.10",
            "phase-5.2",
        ]
        .into_iter()
        .map(|raw| PhaseId::parse(raw).unwrap())
        .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(
            ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [
                "phase-3",
                "phase-3a",
                "phase-5",
                "phase-5.10",
                "phase-5.2",
                "phase-5.5",
                "phase-10"
            ]
        );
    }

    #[test]
    fn discovery_excludes_from_and_includes_to() {
        let dir = temp_dir("phaserunner-discovery");
        for file in [
            "phase-1.md",
            "phase-2.md",
            "phase-2a.md",
            "phase-2.5.md",
            "phase-3.md",
            "phase-alpha.md",
            "plan.md",
        ] {
            File::create(dir.join(file)).unwrap();
        }

        let phases = discover_phases(&dir, "1", "3").unwrap();
        assert_eq!(
            phases.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["phase-2", "phase-2a", "phase-2.5", "phase-3"]
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn discovery_rejects_empty_and_reversed_ranges() {
        let dir = temp_dir("phaserunner-empty-discovery");
        File::create(dir.join("phase-1.md")).unwrap();
        assert!(matches!(
            discover_phases(&dir, "1", "1"),
            Err(PhaseDiscoveryError::UnorderedBounds { .. })
        ));
        assert!(matches!(
            discover_phases(&dir, "1", "2"),
            Err(PhaseDiscoveryError::Empty { .. })
        ));
        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
