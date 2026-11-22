//! Correlation ID middleware (technical-specs.md §11).
//!
//! Every request gets a UUID v4, carried through its tracing span so debug
//! logs can be grepped by it. On a 5xx response, the id goes to the client
//! too (header + JSON body) so a user can hand it to an admin to locate the
//! matching log lines. 4xx responses are untouched — those already carry an
//! actionable, user-facing message and don't need a correlation id.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

const HEADER_NAME: &str = "x-correlation-id";

pub async fn correlation_id(req: Request, next: Next) -> Response {
    let id = Uuid::new_v4();
    let span = tracing::info_span!("request", correlation_id = %id);

    async move {
        let response = next.run(req).await;
        attach_correlation_id(response, id)
    }
    .instrument(span)
    .await
}

fn attach_correlation_id(mut response: Response, id: Uuid) -> Response {
    let header_value = HeaderValue::from_str(&id.to_string())
        .expect("a UUID's string form is always a valid header value");
    response
        .headers_mut()
        .insert(HeaderName::from_static(HEADER_NAME), header_value.clone());

    if response.status().is_server_error() {
        let body = serde_json::json!({
            "error": "technical_error",
            "correlation_id": id.to_string(),
        })
        .to_string();
        let (mut parts, _discarded_body) = response.into_parts();
        parts.headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response = Response::from_parts(parts, Body::from(body));
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn ok_handler() -> StatusCode {
        StatusCode::OK
    }

    async fn error_handler() -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn app() -> Router {
        Router::new()
            .route("/ok", get(ok_handler))
            .route("/error", get(error_handler))
            .layer(axum::middleware::from_fn(correlation_id))
    }

    #[tokio::test]
    async fn every_response_gets_the_header() {
        let response = app()
            .oneshot(Request::get("/ok").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key(HEADER_NAME));
    }

    #[tokio::test]
    async fn a_5xx_response_body_carries_the_same_id_as_its_header() {
        let response = app()
            .oneshot(Request::get("/error").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let header_id = response
            .headers()
            .get(HEADER_NAME)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["correlation_id"].as_str().unwrap(), header_id);
    }
}
