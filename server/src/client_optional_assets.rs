use axum::http::header;
use axum::response::{IntoResponse, Response};

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

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
}
