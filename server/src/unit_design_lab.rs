use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_ATTEMPTS: usize = 200;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignAttempt {
    pub id: Option<String>,
    pub prompt: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub spec: UnitDesignSpec,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignSpec {
    pub name: String,
    pub role: Option<String>,
    pub silhouette: Option<String>,
    pub palette: Option<Vec<String>>,
    pub shapes: Vec<UnitDesignShape>,
    #[serde(rename = "animationNotes")]
    pub animation_notes: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitDesignShape {
    pub kind: String,
    pub layer: Option<i32>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: Option<f32>,
    pub color: String,
    pub alpha: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct UnitDesignCatalog {
    pub root: String,
    pub tree: Vec<UnitDesignTreeNode>,
    pub attempts: Vec<UnitDesignFile>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UnitDesignTreeNode {
    pub name: String,
    pub path: String,
    pub kind: UnitDesignTreeNodeKind,
    pub children: Vec<UnitDesignTreeNode>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UnitDesignTreeNodeKind {
    Directory,
    File,
}

#[derive(Debug, Serialize, Clone)]
pub struct UnitDesignFile {
    pub path: String,
    pub name: String,
    pub attempt: UnitDesignAttempt,
}

pub async fn catalog() -> Result<UnitDesignCatalog, String> {
    let dir = storage_dir();
    let mut attempts = Vec::new();
    if dir.exists() {
        read_attempts_recursive(&dir, &dir, &mut attempts)?;
    }
    attempts.sort_by(|a, b| b.path.cmp(&a.path));
    attempts.truncate(MAX_ATTEMPTS);
    let tree = build_tree(&attempts);
    Ok(UnitDesignCatalog {
        root: "server/assets/unit-design-lab".to_string(),
        tree,
        attempts,
    })
}

fn storage_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("unit-design-lab")
}

fn read_attempts_recursive(
    root: &Path,
    dir: &Path,
    attempts: &mut Vec<UnitDesignFile>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|err| {
            format!(
                "failed to read unit design directory {}: {err}",
                dir.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to read unit design directory entry: {err}"))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect unit design file {}: {err}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            read_attempts_recursive(root, &path, attempts)?;
            continue;
        }
        if !file_type.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let rel_path = relative_json_path(root, &path)?;
        let payload = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read unit design file {}: {err}", path.display()))?;
        let attempt = parse_attempt(&payload)
            .map_err(|err| format!("failed to parse unit design file {rel_path}: {err}"))?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("design.json")
            .to_string();
        attempts.push(UnitDesignFile {
            path: rel_path,
            name,
            attempt,
        });
    }
    Ok(())
}

fn relative_json_path(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path.strip_prefix(root).map_err(|err| {
        format!(
            "failed to resolve unit design path {}: {err}",
            path.display()
        )
    })?;
    Ok(rel
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn parse_attempt(payload: &str) -> Result<UnitDesignAttempt, serde_json::Error> {
    if let Ok(attempt) = serde_json::from_str::<UnitDesignAttempt>(payload) {
        return Ok(attempt);
    }
    let spec = serde_json::from_str::<UnitDesignSpec>(payload)?;
    Ok(UnitDesignAttempt {
        id: None,
        prompt: None,
        created_at: None,
        source: Some("codex".to_string()),
        model: None,
        spec,
    })
}

fn build_tree(files: &[UnitDesignFile]) -> Vec<UnitDesignTreeNode> {
    let mut roots = Vec::new();
    for file in files {
        insert_path(&mut roots, &file.path);
    }
    roots.sort_by(compare_nodes);
    roots
}

fn insert_path(nodes: &mut Vec<UnitDesignTreeNode>, path: &str) {
    let parts = path.split('/').collect::<Vec<_>>();
    insert_parts(nodes, &parts, "");
}

fn insert_parts(nodes: &mut Vec<UnitDesignTreeNode>, parts: &[&str], parent: &str) {
    if parts.is_empty() {
        return;
    }
    let name = parts[0];
    let path = if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    };
    let kind = if parts.len() == 1 {
        UnitDesignTreeNodeKind::File
    } else {
        UnitDesignTreeNodeKind::Directory
    };
    let index = nodes
        .iter()
        .position(|node| node.name == name)
        .unwrap_or_else(|| {
            nodes.push(UnitDesignTreeNode {
                name: name.to_string(),
                path: path.clone(),
                kind,
                children: Vec::new(),
            });
            nodes.len() - 1
        });
    if parts.len() > 1 {
        insert_parts(&mut nodes[index].children, &parts[1..], &path);
        nodes[index].children.sort_by(compare_nodes);
    }
}

fn compare_nodes(a: &UnitDesignTreeNode, b: &UnitDesignTreeNode) -> std::cmp::Ordering {
    match (&a.kind, &b.kind) {
        (UnitDesignTreeNodeKind::Directory, UnitDesignTreeNodeKind::File) => {
            std::cmp::Ordering::Less
        }
        (UnitDesignTreeNodeKind::File, UnitDesignTreeNodeKind::Directory) => {
            std::cmp::Ordering::Greater
        }
        _ => a.name.cmp(&b.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_attempt_and_bare_spec() {
        let full = r##"{
          "id": "v1",
          "prompt": "tracked scout",
          "createdAt": "2026-06-15T00:00:00Z",
          "source": "codex",
          "model": "manual",
          "spec": {
            "name": "Scout",
            "role": "Recon",
            "silhouette": "Boxy",
            "palette": ["#526246"],
            "shapes": [{"kind":"rect","x":0,"y":0,"w":20,"h":10,"color":"#526246"}],
            "animationNotes": ["move"]
          }
        }"##;
        let bare = r##"{
          "name": "Bare",
          "shapes": [{"kind":"ellipse","x":0,"y":0,"w":12,"h":12,"color":"#697256"}]
        }"##;

        assert_eq!(parse_attempt(full).unwrap().id.as_deref(), Some("v1"));
        assert_eq!(parse_attempt(bare).unwrap().spec.name, "Bare");
    }

    #[test]
    fn builds_directory_first_tree() {
        let files = vec![
            file("tank/b.json"),
            file("a.json"),
            file("tank/a.json"),
            file("scout/v1.json"),
        ];
        let tree = build_tree(&files);
        assert_eq!(tree[0].name, "scout");
        assert_eq!(tree[1].name, "tank");
        assert_eq!(tree[2].name, "a.json");
        assert_eq!(tree[1].children[0].name, "a.json");
    }

    fn file(path: &str) -> UnitDesignFile {
        UnitDesignFile {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            attempt: UnitDesignAttempt {
                id: None,
                prompt: None,
                created_at: None,
                source: None,
                model: None,
                spec: UnitDesignSpec {
                    name: path.to_string(),
                    role: None,
                    silhouette: None,
                    palette: None,
                    shapes: Vec::new(),
                    animation_notes: None,
                },
            },
        }
    }
}
