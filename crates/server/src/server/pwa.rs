//! `/app`: the offline web app (PWA), built into the binary.
//!
//! Public on purpose: the files hold no data, and the app asks for the
//! token itself, sending it as a header to `/api` like any other client.

use axum::{
    Router,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};

use crate::app::AppState;

/// Every file of the app: name, content type, content.
const FILES: [(&str, &str, &str); 6] = [
    (
        "index.html",
        "text/html; charset=utf-8",
        include_str!("../../pwa/index.html"),
    ),
    (
        "app.js",
        "text/javascript; charset=utf-8",
        include_str!("../../pwa/app.js"),
    ),
    (
        "sw.js",
        "text/javascript; charset=utf-8",
        include_str!("../../pwa/sw.js"),
    ),
    (
        "style.css",
        "text/css; charset=utf-8",
        include_str!("../../pwa/style.css"),
    ),
    (
        "manifest.webmanifest",
        "application/manifest+json",
        include_str!("../../pwa/manifest.webmanifest"),
    ),
    (
        "icon.svg",
        "image/svg+xml",
        include_str!("../../pwa/icon.svg"),
    ),
];

/// Scripts, styles and calls from this server only: the token in local
/// storage stays out of reach of anything injected.
const CSP: &str = "default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// `/` and `/app` lead to `/app/`, which serves the app.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/app/") }))
        .route("/app", get(|| async { Redirect::permanent("/app/") }))
        .route("/app/", get(|| async { file("index.html") }))
        .route(
            "/app/{name}",
            get(|Path(name): Path<String>| async move { file(&name) }),
        )
}

/// One file of the app, or 404. Always revalidated, so a new version shows
/// at once; the service worker keeps a copy for offline.
fn file(name: &str) -> Response {
    match FILES.iter().find(|(file, _, _)| *file == name) {
        Some((_, kind, body)) => (
            [
                (header::CONTENT_TYPE, *kind),
                (header::CACHE_CONTROL, "no-cache"),
                (header::CONTENT_SECURITY_POLICY, CSP),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            *body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    use super::super::test_app;
    use super::*;

    async fn get(path: &str) -> Response {
        let (_, app) = test_app("");
        app.oneshot(Request::get(path).body(Body::empty()).expect("request"))
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn serves_the_app_without_a_token() {
        let response = get("/app/").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert!(
            response
                .headers()
                .contains_key(header::CONTENT_SECURITY_POLICY)
        );
        assert_eq!(get("/app/sw.js").await.status(), StatusCode::OK);
        assert_eq!(
            get("/app/manifest.webmanifest").await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn root_and_bare_app_lead_to_it() {
        for path in ["/", "/app"] {
            let response = get(path).await;
            assert!(response.status().is_redirection(), "{path}");
            assert_eq!(response.headers()[header::LOCATION], "/app/");
        }
    }

    #[tokio::test]
    async fn unknown_files_are_404() {
        assert_eq!(
            get("/app/../Cargo.toml").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(get("/app/nope.js").await.status(), StatusCode::NOT_FOUND);
    }
}
