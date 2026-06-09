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
                        "unknown".to_string()
                    } else {
                        text
                    }
                }
                _ => "unknown".to_string(),
            }
        });

    println!("cargo:rustc-env=COMMIT_HASH={hash}");
}
