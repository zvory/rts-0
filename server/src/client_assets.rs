use axum::http::{header, HeaderValue, Request};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::services::ServeDir;

const SIM_WASM_GLUE_PATH: &str = "/vendor/sim-wasm/rts_sim_wasm.js";
const MISSING_SIM_WASM_GLUE_MODULE: &str = r#"export default async function init() {
  throw new Error("prediction WASM glue is not available; run scripts/build-sim-wasm.sh");
}
"#;

pub(crate) fn fallback(path: &str) -> Option<Response> {
    if path != SIM_WASM_GLUE_PATH {
        return None;
    }
    Some(
        (
            [
                (
                    header::CONTENT_TYPE,
                    "application/javascript; charset=utf-8",
                ),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            MISSING_SIM_WASM_GLUE_MODULE,
        )
            .into_response(),
    )
}

pub(crate) fn service(client_dir: &str, state: super::AppState) -> Router {
    Router::new()
        .fallback_service(
            ServeDir::new(client_dir)
                .fallback(get(super::client_spa_fallback_handler).with_state(state)),
        )
        .layer(middleware::from_fn(revalidate_assets))
}

/// Module workers do not inherit the document import map, so their stable child-module URLs must
/// revalidate before use to prevent a browser from combining code from different deploys.
async fn revalidate_assets(request: Request<axum::body::Body>, next: Next) -> Response {
    let requires_revalidation = is_unversioned_javascript_module(request.uri());
    let mut response = next.run(request).await;
    if requires_revalidation {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    response
}

fn is_unversioned_javascript_module(uri: &axum::http::Uri) -> bool {
    uri.path().ends_with(".js")
        && !uri.query().is_some_and(|query| {
            query.split('&').any(|part| {
                part.split_once('=')
                    .is_some_and(|(key, value)| key == "v" && !value.is_empty())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[test]
    fn missing_sim_wasm_glue_gets_javascript_stub() {
        let response = fallback(SIM_WASM_GLUE_PATH)
            .expect("missing optional sim wasm glue should get a fallback module");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );
        assert!(
            fallback("/vendor/sim-wasm/rts_sim_wasm_bg.wasm").is_none(),
            "only the optional JS glue gets a fallback; ordinary missing assets must still 404"
        );
    }

    #[tokio::test]
    async fn assets_require_cache_revalidation_for_worker_module_coherency() {
        let app = Router::new()
            .route(
                "/src/renderer/rigs/animation.js",
                get(|| async {
                    (
                        [(header::CACHE_CONTROL, "public, max-age=86400")],
                        "export function rigContainerScale() {}",
                    )
                }),
            )
            .layer(middleware::from_fn(revalidate_assets));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/src/renderer/rigs/animation.js")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-cache")),
            "worker child modules must revalidate so a deploy cannot mix module versions"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/src/renderer/rigs/animation.js?v=current-build")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("public, max-age=86400")),
            "versioned document modules should preserve their existing cache policy"
        );
    }
}
