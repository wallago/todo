use std::path::Path;

use axum::{
    Router,
    http::{HeaderValue, header::CACHE_CONTROL},
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

/// Serves the built web app in `dir` at `/`. `assets/` file names carry a
/// hash of their content, so browsers keep them for good; everything else
/// (`index.html`, `sw.js`, the manifest) is checked again on each load, so a
/// new build reaches them.
pub(super) fn routes<S: Clone + Send + Sync + 'static>(dir: &Path) -> Router<S> {
    let assets = ServeDir::new(dir.join("assets"));
    let rest = ServeDir::new(dir);
    Router::new()
        .nest_service("/assets", assets)
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .fallback_service(rest)
        .layer(SetResponseHeaderLayer::if_not_present(
            CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
}
