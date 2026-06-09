use std::process::Command;

fn main() {
    let hash = std::env::var("COMMIT_HASH")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            match Command::new("git")
                .current_dir(&manifest_dir)
                .args(["rev-parse", "--short=4", "HEAD"])
                .output()
            {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if text.is_empty() {
                        println!(
                            "cargo:warning=git rev-parse returned empty; version will be 'unknown'"
                        );
                        "unknown".to_string()
                    } else {
                        text
                    }
                }
                Ok(_) => {
                    println!("cargo:warning=git rev-parse failed; version will be 'unknown'");
                    "unknown".to_string()
                }
                Err(_) => {
                    println!("cargo:warning=failed to run git; version will be 'unknown'");
                    "unknown".to_string()
                }
            }
        });

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let tag = Command::new("git")
        .current_dir(&manifest_dir)
        .args(["tag", "--points-at", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let first = text.lines().next().map(str::trim).unwrap_or("").to_string();
            if first.is_empty() {
                None
            } else {
                Some(first)
            }
        });

    let version = match tag {
        Some(t) => format!("{hash} [{t}]"),
        None => hash,
    };

    println!("cargo:rustc-env=COMMIT_HASH={version}");
}
