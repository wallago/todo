//! Bearer token on `/api` and `/mcp`.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use super::api_error::ApiError;
use super::oauth::Issuer;
use crate::prelude::*;

/// The token every protected request must send.
#[derive(Clone)]
pub(super) struct Token(Arc<str>);

impl Token {
    /// Reads the token from `path`, without surrounding whitespace.
    pub(super) fn read(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        let token = raw.trim();
        if token.is_empty() {
            return Err(Error::Config(format!("{}: token is empty", path.display())));
        }
        Ok(Self(token.into()))
    }

    /// A token from a string, for tests.
    #[cfg(test)]
    pub(super) fn from_static(token: &str) -> Self {
        Self(token.into())
    }

    /// The raw token bytes, to derive keys from.
    pub(super) fn secret(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// `true` if `sent` is the token. Takes the same time wherever they differ.
    pub(super) fn matches(&self, sent: &str) -> bool {
        self.0.as_bytes().ct_eq(sent.as_bytes()).into()
    }
}

// By hand, so the token never ends up in logs.
impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

/// What protected routes accept: the static token, and OAuth access tokens
/// when OAuth is on.
#[derive(Clone)]
pub(super) struct Guard {
    /// The static token.
    pub(super) token: Token,
    /// Issues and checks OAuth access tokens.
    pub(super) oauth: Option<Issuer>,
}

/// Middleware: 401 unless the request sends `Authorization: Bearer <token>`,
/// with the static token or an OAuth access token.
pub(super) async fn require(State(guard): State<Guard>, request: Request, next: Next) -> Response {
    let sent = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, sent)| sent.trim());
    let accepted = sent.is_some_and(|sent| {
        guard.token.matches(sent)
            || guard
                .oauth
                .as_ref()
                .is_some_and(|oauth| oauth.accepts(sent))
    });
    if accepted {
        return next.run(request).await;
    }
    // With OAuth on, point clients such as claude.ai to the login (RFC 9728).
    let challenge = match &guard.oauth {
        Some(oauth) => format!(
            "Bearer resource_metadata=\"{}\"",
            oauth.resource_metadata_url()
        ),
        None => "Bearer".to_owned(),
    };
    (
        [(header::WWW_AUTHENTICATE, challenge)],
        ApiError::unauthorized(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_router;

    /// A router whose token is `secret`.
    fn app() -> (tempfile::TempDir, axum::Router) {
        test_router("- [ ] a ^aaaaaa\n", Some(Token::from_static("secret")))
    }

    /// GETs `uri` with this `Authorization` header, if any.
    async fn get(app: &axum::Router, uri: &str, auth: Option<&str>) -> Response {
        let mut request = Request::get(uri).header(header::HOST, "localhost");
        if let Some(auth) = auth {
            request = request.header(header::AUTHORIZATION, auth);
        }
        let request = request.body(Body::empty()).expect("valid request");
        app.clone().oneshot(request).await.expect("infallible")
    }

    #[test]
    fn read_trims_and_refuses_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("token");
        std::fs::write(&path, "secret\n").expect("write");
        assert!(Token::read(&path).expect("token").matches("secret"));
        std::fs::write(&path, " \n").expect("write");
        assert!(Token::read(&path).is_err());
        assert!(Token::read(&dir.path().join("missing")).is_err());
    }

    #[tokio::test]
    async fn api_needs_the_token() {
        let (_dir, app) = app();
        for auth in [
            None,
            Some("Bearer wrong"),
            Some("Basic secret"),
            Some("secret"),
        ] {
            let response = get(&app, "/api/todos", auth).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{auth:?}");
            assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");
            let bytes = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            assert!(String::from_utf8_lossy(&bytes).contains("bearer token"));
        }
        for auth in ["Bearer secret", "bearer secret"] {
            let response = get(&app, "/api/todos", Some(auth)).await;
            assert_eq!(response.status(), StatusCode::OK, "{auth}");
        }
    }

    #[tokio::test]
    async fn mcp_needs_the_token_and_health_does_not() {
        let (_dir, app) = app();
        let response = get(&app, "/mcp", None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        // Past the token, the MCP service answers (here: a GET without a session).
        let response = get(&app, "/mcp", Some("Bearer secret")).await;
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
        let response = get(&app, "/health", None).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn client_sends_the_token() {
        let (_dir, app) = app();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let base = format!("http://{addr}");

        let todos = todo_client::Client::new(&base)
            .with_token("secret")
            .list()
            .await;
        assert_eq!(todos.expect("todos").len(), 1);
        match todo_client::Client::new(&base).list().await {
            Err(todo_client::Error::Api { status, .. }) => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected a 401, got {other:?}"),
        }
    }
}
