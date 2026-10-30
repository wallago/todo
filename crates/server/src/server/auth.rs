//! Bearer token on `/api` and `/mcp`, read-only token on `/api/agenda`.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use axum::{
    extract::{Query, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use subtle::ConstantTimeEq;

use super::api_error::ApiError;
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

    /// `true` if `sent` is the token. Takes the same time wherever they differ.
    fn matches(&self, sent: &str) -> bool {
        self.0.as_bytes().ct_eq(sent.as_bytes()).into()
    }
}

// By hand, so the token never ends up in logs.
impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

/// The main token, and the optional read-only one.
#[derive(Clone, Debug)]
pub(super) struct Tokens {
    /// Everything, sent as a header only.
    pub(super) main: Token,
    /// `GET /api/agenda` only, as a header or `?token=`, for widgets that
    /// can't set headers (KWGT). A leak can't change anything.
    pub(super) read: Option<Token>,
}

/// `?token=` on `/api/agenda`.
#[derive(Deserialize)]
struct TokenQuery {
    /// The read-only token.
    token: Option<String>,
}

/// The token in `Authorization: Bearer <token>`, if any.
fn bearer(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, sent)| sent.trim())
}

/// The 401 answer.
fn unauthorized() -> Response {
    (
        [(header::WWW_AUTHENTICATE, "Bearer")],
        ApiError::unauthorized(),
    )
        .into_response()
}

/// Middleware: 401 unless the request sends `Authorization: Bearer <main token>`.
pub(super) async fn require(
    State(tokens): State<Tokens>,
    request: Request,
    next: Next,
) -> Response {
    if bearer(&request).is_some_and(|sent| tokens.main.matches(sent)) {
        return next.run(request).await;
    }
    unauthorized()
}

/// `true` if `request` sends the main or read-only token as a header, or the
/// read-only one as `?token=`.
fn allows_read(tokens: &Tokens, request: &Request) -> bool {
    let read = tokens.read.as_ref();
    let header_ok = bearer(request).is_some_and(|sent| {
        tokens.main.matches(sent) || read.is_some_and(|read| read.matches(sent))
    });
    header_ok
        || Query::<TokenQuery>::try_from_uri(request.uri())
            .ok()
            .and_then(|Query(query)| query.token)
            .is_some_and(|sent| read.is_some_and(|read| read.matches(&sent)))
}

/// Middleware: like [`require`], but also takes the read-only token, as a
/// header or `?token=`. The main token stays header-only, out of URLs and logs.
pub(super) async fn require_read(
    State(tokens): State<Tokens>,
    request: Request,
    next: Next,
) -> Response {
    if allows_read(&tokens, &request) {
        return next.run(request).await;
    }
    unauthorized()
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_router;

    /// A router whose token is `secret` and read-only token `peek`.
    fn app() -> (tempfile::TempDir, axum::Router) {
        let tokens = Tokens {
            main: Token("secret".into()),
            read: Some(Token("peek".into())),
        };
        test_router("- [ ] a ^aaaaaa\n", Some(tokens))
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
    async fn read_token_only_opens_the_agenda() {
        let (_dir, app) = app();
        for (uri, auth) in [
            ("/api/agenda", Some("Bearer secret")),
            ("/api/agenda", Some("Bearer peek")),
            ("/api/agenda?token=peek", None),
            ("/api/agenda?format=text&token=peek", None),
        ] {
            let response = get(&app, uri, auth).await;
            assert_eq!(response.status(), StatusCode::OK, "{uri} {auth:?}");
        }
        for (uri, auth) in [
            ("/api/agenda", None),
            ("/api/agenda?token=wrong", None),
            // The main token never goes in a URL.
            ("/api/agenda?token=secret", None),
            ("/api/todos?token=peek", None),
            ("/api/todos", Some("Bearer peek")),
            ("/mcp", Some("Bearer peek")),
        ] {
            let response = get(&app, uri, auth).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{uri} {auth:?}"
            );
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
