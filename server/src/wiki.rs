use std::path::{Component, Path, PathBuf};

use axum::extract::Path as AxumPath;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use pulldown_cmark::{html, CowStr, Event, Options, Parser};

const WIKI_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs");
const INDEX_DOC: &str = "context/README.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WikiPathError {
    Traversal,
    Missing,
}

pub async fn wiki_index_handler() -> Response {
    wiki_response_for(INDEX_DOC)
}

pub async fn wiki_page_handler(AxumPath(path): AxumPath<String>) -> Response {
    wiki_response_for(&path)
}

fn wiki_response_for(route_path: &str) -> Response {
    match resolve_doc_path(route_path) {
        Ok(doc_path) => match std::fs::read_to_string(&doc_path) {
            Ok(markdown) => wiki_html(route_path, &markdown).into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "wiki page not found").into_response(),
        },
        Err(WikiPathError::Traversal) => {
            (StatusCode::BAD_REQUEST, "invalid wiki path").into_response()
        }
        Err(WikiPathError::Missing) => {
            (StatusCode::NOT_FOUND, "wiki page not found").into_response()
        }
    }
}

fn wiki_html(route_path: &str, markdown: &str) -> impl IntoResponse {
    let title = page_title(route_path, markdown);
    let body = render_markdown(markdown);
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} - Bewegungskrieg Wiki</title>
<style>
body {{ max-width: 960px; margin: 0 auto; padding: 32px 20px; font: 16px/1.55 system-ui, sans-serif; color: #1c1f23; background: #f8f7f3; }}
main {{ background: #fff; border: 1px solid #ddd7cc; padding: 24px; }}
a {{ color: #0b5e86; }}
code, pre {{ background: #f1eee6; }}
pre {{ padding: 12px; overflow-x: auto; }}
table {{ border-collapse: collapse; }}
th, td {{ border: 1px solid #d8d2c7; padding: 4px 8px; }}
</style>
</head>
<body>
<main>
{body}
</main>
</body>
</html>"#
    );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Html(html),
    )
}

fn resolve_doc_path(route_path: &str) -> Result<PathBuf, WikiPathError> {
    let clean = route_path.trim_start_matches('/');
    if clean.is_empty() {
        return resolve_doc_path(INDEX_DOC);
    }
    let relative = Path::new(clean);
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(WikiPathError::Traversal);
            }
        }
    }
    if normalized
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        return Err(WikiPathError::Missing);
    }

    let first = normalized
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .ok_or(WikiPathError::Missing)?;
    if !matches!(first, "context" | "design") {
        return Err(WikiPathError::Missing);
    }

    Ok(Path::new(WIKI_ROOT).join(normalized))
}

fn render_markdown(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all()).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => {
            Event::Text(CowStr::Boxed(raw.into_string().into_boxed_str()))
        }
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn page_title(route_path: &str, markdown: &str) -> String {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(escape_text)
        .unwrap_or_else(|| escape_text(route_path))
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn wiki_resolver_allows_context_doc() {
        let path = resolve_doc_path("context/README.md").expect("context readme should resolve");
        assert!(path.ends_with("docs/context/README.md"));
    }

    #[test]
    fn wiki_resolver_blocks_traversal() {
        assert_eq!(
            resolve_doc_path("../server/Cargo.toml"),
            Err(WikiPathError::Traversal)
        );
        assert_eq!(
            resolve_doc_path("context/../../server/Cargo.toml"),
            Err(WikiPathError::Traversal)
        );
    }

    #[test]
    fn wiki_renderer_escapes_inline_html() {
        let rendered = render_markdown("# Hello\n\n<script>alert(1)</script>\n");
        assert!(rendered.contains("<h1>Hello</h1>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!rendered.contains("<script>"));
    }

    #[tokio::test]
    async fn wiki_index_route_renders_docs_readme() {
        let response = wiki_router()
            .oneshot(Request::builder().uri("/wiki").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("<title>Context capsules - Bewegungskrieg Wiki</title>"));
        assert!(body.contains("<main>"));
    }

    #[tokio::test]
    async fn wiki_doc_route_renders_allowlisted_page() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/server-sim.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("Capsule: server simulation"));
    }

    #[tokio::test]
    async fn wiki_missing_doc_is_not_found() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/missing.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wiki_traversal_route_is_bad_request() {
        let response = wiki_router()
            .oneshot(
                Request::builder()
                    .uri("/wiki/context/%2e%2e/%2e%2e/server/Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    fn wiki_router() -> Router {
        Router::new()
            .route("/wiki", get(wiki_index_handler))
            .route("/wiki/{*path}", get(wiki_page_handler))
    }
}
