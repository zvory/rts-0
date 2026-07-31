use std::path::Path;

const RTS_GOOGLE_ANALYTICS_ID_ENV: &str = "RTS_GOOGLE_ANALYTICS_ID";
const ANALYTICS_META_PLACEHOLDER: &str = "<meta name=\"rts-google-analytics-id\" content=\"\" />";

/// Read `index.html`, inject optional production analytics configuration, add a versioned import
/// map for `/src/*.js`, and cache-bust top-level assets.
pub(crate) fn build(client_dir: &str, version: &str) -> String {
    let path = format!("{client_dir}/index.html");
    let html = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        rts_server::log_error!(%path, %err, "failed to read index.html");
        String::new()
    });
    build_from_html(
        client_dir,
        version,
        html,
        configured_google_analytics_id().as_deref(),
    )
}

fn build_from_html(
    client_dir: &str,
    version: &str,
    html: String,
    google_analytics_id: Option<&str>,
) -> String {
    let html = inject_google_analytics_id(html, google_analytics_id);

    let src_dir = format!("{client_dir}/src");
    let mut entries = String::new();
    let mut names = Vec::new();
    collect_js_modules(Path::new(&src_dir), Path::new(""), &mut names);
    names.sort();
    for name in names {
        entries.push_str(&format!(
            "    \"/src/{name}\": \"/src/{name}?v={version}\",\n"
        ));
    }
    if entries.ends_with(",\n") {
        entries.truncate(entries.len() - 2);
        entries.push('\n');
    }
    let import_map = format!(
        "<script type=\"importmap\">\n{{\n  \"imports\": {{\n{entries}  }}\n}}\n</script>\n  "
    );
    let html = html.replace(
        "<script type=\"module\"",
        &format!("{import_map}<script type=\"module\""),
    );

    html.replace("./src/main.js\"", &format!("./src/main.js?v={version}\""))
        .replace(".css\"", &format!(".css?v={version}\""))
        .replace(
            "/manifest.webmanifest\"",
            &format!("/manifest.webmanifest?v={version}\""),
        )
}

fn configured_google_analytics_id() -> Option<String> {
    let value = std::env::var(RTS_GOOGLE_ANALYTICS_ID_ENV).ok()?;
    let value = value.trim();
    if valid_google_analytics_id(value) {
        Some(value.to_string())
    } else {
        rts_server::log_warn!(
            env = RTS_GOOGLE_ANALYTICS_ID_ENV,
            "ignoring invalid Google Analytics measurement id"
        );
        None
    }
}

fn inject_google_analytics_id(html: String, measurement_id: Option<&str>) -> String {
    match measurement_id.filter(|value| valid_google_analytics_id(value)) {
        Some(measurement_id) => html.replace(
            ANALYTICS_META_PLACEHOLDER,
            &format!("<meta name=\"rts-google-analytics-id\" content=\"{measurement_id}\" />"),
        ),
        None => html,
    }
}

fn valid_google_analytics_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("G-") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.len() <= 32
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn collect_js_modules(dir: &Path, prefix: &Path, out: &mut Vec<String>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let next_prefix = prefix.join(entry.file_name());
        if path.is_dir() {
            collect_js_modules(&path, &next_prefix, out);
        } else if path.extension().is_some_and(|ext| ext == "js") {
            if let Some(name) = next_prefix.to_str() {
                out.push(name.replace('\\', "/"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_dir() -> &'static str {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../client")
    }

    fn source_html() -> String {
        std::fs::read_to_string(format!("{}/index.html", client_dir())).unwrap()
    }

    #[test]
    fn versioned_index_cache_busts_nested_js_modules() {
        let html = build_from_html(client_dir(), "test-version", source_html(), None);
        assert!(html.contains("\"/src/main.js\": \"/src/main.js?v=test-version\""));
        assert!(html
            .contains("\"/src/renderer/terrain.js\": \"/src/renderer/terrain.js?v=test-version\""));
        assert!(html.contains("./src/main.js?v=test-version\""));
        assert!(html.contains("./live_pause.css?v=test-version\""));
        assert!(html.contains("/manifest.webmanifest?v=test-version\""));
        assert!(html.contains("name=\"rts-google-analytics-id\" content=\"\""));
    }

    #[test]
    fn versioned_index_injects_only_valid_google_analytics_measurement_ids() {
        let html = build_from_html(
            client_dir(),
            "test-version",
            source_html(),
            Some("G-06WVK0QHVR"),
        );
        assert!(html.contains("content=\"G-06WVK0QHVR\""));

        let invalid = inject_google_analytics_id(source_html(), Some("G-bad\" />"));
        assert!(invalid.contains("content=\"\""));
        assert!(!invalid.contains("G-bad"));
    }

    #[test]
    fn google_analytics_measurement_id_validation_is_strict() {
        assert!(valid_google_analytics_id("G-06WVK0QHVR"));
        for value in ["", "UA-1234", "G-", "G-lowercase", "G-BAD_VALUE"] {
            assert!(!valid_google_analytics_id(value));
        }
    }
}
