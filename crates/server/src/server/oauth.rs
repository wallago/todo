//! Minimal OAuth 2.1 so claude.ai can add `/mcp` as a custom connector.
//!
//! One user, no database. The login page asks for the server token, and every
//! code and token it hands out is signed with a key derived from that token.
//! Nothing is stored, so a restart logs nobody out, and changing the token
//! file logs every client out.
//!
//! Endpoints:
//! - `/.well-known/oauth-protected-resource[/mcp]` (RFC 9728)
//! - `/.well-known/oauth-authorization-server` (RFC 8414)
//! - `/oauth/register` (RFC 7591, accepts known redirect URIs only)
//! - `/oauth/authorize` (login form, S256 PKCE required)
//! - `/oauth/token` (`authorization_code` and `refresh_token`)

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::auth::Token;

/// How long an access token lives, in seconds.
const ACCESS_TTL: u64 = 60 * 60;
/// How long a refresh token lives, in seconds.
const REFRESH_TTL: u64 = 90 * 24 * 60 * 60;
/// How long an authorization code lives, in seconds.
const CODE_TTL: u64 = 5 * 60;
/// The one client id handed out by `/oauth/register`.
const CLIENT_ID: &str = "todo-mcp";

/// Signs and checks codes and tokens.
#[derive(Clone)]
pub(super) struct Issuer {
    /// Public base URL, without trailing slash, e.g. `https://todo.example.com`.
    base: Arc<str>,
    /// The server token: the login page's password.
    token: Token,
    /// HMAC key derived from the token.
    key: Arc<[u8]>,
}

/// What a signed code or token carries.
#[derive(Serialize, Deserialize)]
struct Claims {
    /// `c` code, `a` access token, `r` refresh token.
    k: char,
    /// Expiry, Unix seconds.
    exp: u64,
    /// Code only: the PKCE challenge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cc: Option<String>,
    /// Code only: the redirect URI it was issued for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ru: Option<String>,
}

/// An HMAC-SHA256 keyed with `key`.
fn hmac(key: &[u8]) -> Hmac<Sha256> {
    // HMAC takes keys of any length, so this never fails.
    <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap_or_else(|_| unreachable!())
}

/// Seconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Issuer {
    /// An issuer at `public_url` whose login password is `token`.
    pub(super) fn new(public_url: &str, token: Token) -> Self {
        let key = hmac(token.secret()).chain_update(b"todo-server oauth v1");
        let key = key.finalize().into_bytes().to_vec();
        Self {
            base: public_url.trim_end_matches('/').into(),
            token,
            key: key.into(),
        }
    }

    /// Where `/mcp` clients find the protected resource metadata.
    pub(super) fn resource_metadata_url(&self) -> String {
        format!("{}/.well-known/oauth-protected-resource", self.base)
    }

    /// HMAC of `payload`.
    fn mac(&self, payload: &[u8]) -> Hmac<Sha256> {
        hmac(&self.key).chain_update(payload)
    }

    /// `base64(claims).base64(hmac)`.
    fn sign(&self, claims: &Claims) -> String {
        // A struct of strings and numbers always serializes.
        let payload = B64.encode(serde_json::to_vec(claims).unwrap_or_default());
        let sig = B64.encode(self.mac(payload.as_bytes()).finalize().into_bytes());
        format!("{payload}.{sig}")
    }

    /// The claims of `signed` if its signature is ours, its kind is `kind`,
    /// and it has not expired.
    fn verify(&self, signed: &str, kind: char) -> Option<Claims> {
        let (payload, sig) = signed.split_once('.')?;
        let sig = B64.decode(sig).ok()?;
        self.mac(payload.as_bytes()).verify_slice(&sig).ok()?;
        let claims: Claims = serde_json::from_slice(&B64.decode(payload).ok()?).ok()?;
        (claims.k == kind && claims.exp > now()).then_some(claims)
    }

    /// `true` if `sent` is a live access token from this issuer.
    pub(super) fn accepts(&self, sent: &str) -> bool {
        self.verify(sent, 'a').is_some()
    }

    /// A fresh access and refresh token pair, as the token endpoint answers it.
    fn token_pair(&self) -> Response {
        let token = |k, ttl| {
            self.sign(&Claims {
                k,
                exp: now() + ttl,
                cc: None,
                ru: None,
            })
        };
        (
            [(header::CACHE_CONTROL, "no-store")],
            Json(json!({
                "access_token": token('a', ACCESS_TTL),
                "token_type": "Bearer",
                "expires_in": ACCESS_TTL,
                "refresh_token": token('r', REFRESH_TTL),
            })),
        )
            .into_response()
    }
}

/// `true` for redirect URIs Claude uses: the claude.ai callback, or a loopback
/// URL on any port (Claude Code).
fn redirect_allowed(uri: &str) -> bool {
    if matches!(
        uri,
        "https://claude.ai/api/mcp/auth_callback" | "https://claude.com/api/mcp/auth_callback"
    ) {
        return true;
    }
    let Some(rest) = uri.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let host = if authority.starts_with('[') {
        authority.split_inclusive(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// The OAuth routes, served at the root of the server.
pub(super) fn routes<S: Clone + Send + Sync + 'static>(issuer: Issuer) -> Router<S> {
    Router::new()
        .route("/.well-known/oauth-protected-resource", get(resource))
        .route("/.well-known/oauth-protected-resource/mcp", get(resource))
        .route("/.well-known/oauth-authorization-server", get(server))
        .route("/oauth/register", post(register))
        .route("/oauth/authorize", get(authorize_form).post(authorize))
        .route("/oauth/token", post(token))
        .with_state(issuer)
}

/// RFC 9728: `/mcp` is guarded by this server.
async fn resource(State(issuer): State<Issuer>) -> Json<serde_json::Value> {
    Json(json!({
        "resource": format!("{}/mcp", issuer.base),
        "authorization_servers": [issuer.base.as_ref()],
        "bearer_methods_supported": ["header"],
    }))
}

/// RFC 8414: where to log in and get tokens.
async fn server(State(issuer): State<Issuer>) -> Json<serde_json::Value> {
    let base = issuer.base.as_ref();
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "client_id_metadata_document_supported": true,
    }))
}

/// An OAuth error body, `{"error": "...", "error_description": "..."}`.
fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(json!({ "error": error, "error_description": description })),
    )
        .into_response()
}

/// What `/oauth/register` reads.
#[derive(Deserialize)]
struct Registration {
    /// Where the client wants codes sent.
    #[serde(default)]
    redirect_uris: Vec<String>,
    /// Display name, echoed back.
    client_name: Option<String>,
}

/// RFC 7591: every client gets the same public client id, as long as its
/// redirect URIs are Claude's.
async fn register(Json(registration): Json<Registration>) -> Response {
    if registration.redirect_uris.is_empty()
        || !registration
            .redirect_uris
            .iter()
            .all(|uri| redirect_allowed(uri))
    {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "only Claude's redirect URIs are accepted",
        );
    }
    (
        StatusCode::CREATED,
        Json(json!({
            "client_id": CLIENT_ID,
            "client_id_issued_at": now(),
            "client_name": registration.client_name,
            "redirect_uris": registration.redirect_uris,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        })),
    )
        .into_response()
}

/// The authorization request, from the query (GET) or the form (POST).
#[derive(Deserialize)]
struct AuthorizeParams {
    /// Must be `code`.
    #[serde(default)]
    response_type: String,
    /// Any client id: the redirect URI is what is checked.
    #[serde(default)]
    client_id: String,
    /// Where to send the code.
    #[serde(default)]
    redirect_uri: String,
    /// PKCE challenge.
    #[serde(default)]
    code_challenge: String,
    /// Must be `S256`.
    #[serde(default)]
    code_challenge_method: String,
    /// Echoed back to the client.
    state: Option<String>,
    /// Echoed into the form, unused.
    scope: Option<String>,
    /// Echoed into the form, unused.
    resource: Option<String>,
    /// POST only: the server token.
    password: Option<String>,
}

impl AuthorizeParams {
    /// The reason this request can't go on, if any.
    fn problem(&self) -> Option<&'static str> {
        if !redirect_allowed(&self.redirect_uri) {
            Some("this redirect URI is not one of Claude's")
        } else if self.response_type != "code" {
            Some("response_type must be `code`")
        } else if self.code_challenge.is_empty() || self.code_challenge_method != "S256" {
            Some("an S256 PKCE code challenge is required")
        } else {
            None
        }
    }
}

/// Escapes text for HTML attributes and content.
fn escape(text: &str) -> String {
    text.chars().fold(String::new(), |mut out, c| {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
        out
    })
}

/// A plain page with `body` inside.
fn page(status: StatusCode, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Todo login</title><style>body{{font-family:system-ui,sans-serif;\
         max-width:24rem;margin:4rem auto;padding:0 1rem}}input,button{{font:inherit;\
         width:100%;padding:.5rem;margin:.25rem 0;box-sizing:border-box}}\
         .error{{color:#b00020}}</style></head><body>{body}</body></html>"
    );
    (status, [(header::CACHE_CONTROL, "no-store")], Html(html)).into_response()
}

/// The login form, carrying the request along as hidden fields.
fn login_form(params: &AuthorizeParams, error: Option<&str>) -> Response {
    let mut hidden = String::new();
    for (name, value) in [
        ("response_type", Some(params.response_type.as_str())),
        ("client_id", Some(params.client_id.as_str())),
        ("redirect_uri", Some(params.redirect_uri.as_str())),
        ("code_challenge", Some(params.code_challenge.as_str())),
        (
            "code_challenge_method",
            Some(params.code_challenge_method.as_str()),
        ),
        ("state", params.state.as_deref()),
        ("scope", params.scope.as_deref()),
        ("resource", params.resource.as_deref()),
    ] {
        if let Some(value) = value {
            let _ = write!(
                hidden,
                "<input type=\"hidden\" name=\"{name}\" value=\"{}\">",
                escape(value)
            );
        }
    }
    let status = if error.is_some() {
        StatusCode::UNAUTHORIZED
    } else {
        StatusCode::OK
    };
    let error = error
        .map(|error| format!("<p class=\"error\">{}</p>", escape(error)))
        .unwrap_or_default();
    page(
        status,
        &format!(
            "<h1>Todo</h1><p>Let Claude read and change your todos.</p>{error}\
             <form method=\"post\" action=\"/oauth/authorize\">{hidden}\
             <label for=\"password\">Server token</label>\
             <input id=\"password\" name=\"password\" type=\"password\" \
             autocomplete=\"current-password\" required autofocus>\
             <button type=\"submit\">Allow</button></form>"
        ),
    )
}

/// GET: show the login form.
async fn authorize_form(Query(params): Query<AuthorizeParams>) -> Response {
    match params.problem() {
        Some(problem) => page(
            StatusCode::BAD_REQUEST,
            &format!("<h1>Todo</h1><p class=\"error\">{problem}</p>"),
        ),
        None => login_form(&params, None),
    }
}

/// POST: check the token, then send a code to the client.
async fn authorize(State(issuer): State<Issuer>, Form(params): Form<AuthorizeParams>) -> Response {
    if let Some(problem) = params.problem() {
        return page(
            StatusCode::BAD_REQUEST,
            &format!("<h1>Todo</h1><p class=\"error\">{problem}</p>"),
        );
    }
    let password = params.password.as_deref().unwrap_or_default().trim();
    if !issuer.token.matches(password) {
        tracing::warn!("OAuth login with a wrong token");
        return login_form(&params, Some("Wrong token."));
    }
    let code = issuer.sign(&Claims {
        k: 'c',
        exp: now() + CODE_TTL,
        cc: Some(params.code_challenge.clone()),
        ru: Some(params.redirect_uri.clone()),
    });
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.append_pair("code", &code);
    if let Some(state) = &params.state {
        query.append_pair("state", state);
    }
    let separator = if params.redirect_uri.contains('?') {
        '&'
    } else {
        '?'
    };
    tracing::info!("OAuth login for {}", params.redirect_uri);
    Redirect::to(&format!(
        "{}{separator}{}",
        params.redirect_uri,
        query.finish()
    ))
    .into_response()
}

/// What `/oauth/token` reads.
#[derive(Deserialize)]
struct TokenRequest {
    /// `authorization_code` or `refresh_token`.
    grant_type: String,
    /// `authorization_code` only.
    code: Option<String>,
    /// `authorization_code` only: must match the one the code was issued for.
    redirect_uri: Option<String>,
    /// `authorization_code` only: the PKCE verifier.
    code_verifier: Option<String>,
    /// `refresh_token` only.
    refresh_token: Option<String>,
}

/// Exchange a code or a refresh token for a new token pair.
async fn token(State(issuer): State<Issuer>, Form(request): Form<TokenRequest>) -> Response {
    let invalid = |description| oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", description);
    match request.grant_type.as_str() {
        "authorization_code" => {
            let Some(claims) = request
                .code
                .as_deref()
                .and_then(|code| issuer.verify(code, 'c'))
            else {
                return invalid("unknown or expired code");
            };
            if let Some(sent) = &request.redirect_uri
                && Some(sent) != claims.ru.as_ref()
            {
                return invalid("redirect_uri does not match");
            }
            let verifier = request.code_verifier.as_deref().unwrap_or_default();
            let challenge = B64.encode(Sha256::digest(verifier.as_bytes()));
            if claims.cc.as_deref() != Some(challenge.as_str()) {
                return invalid("PKCE verification failed");
            }
            issuer.token_pair()
        }
        "refresh_token" => {
            match request
                .refresh_token
                .as_deref()
                .and_then(|token| issuer.verify(token, 'r'))
            {
                Some(_) => issuer.token_pair(),
                None => invalid("unknown or expired refresh token"),
            }
        }
        _ => oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "use authorization_code or refresh_token",
        ),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_router_with;

    const BASE: &str = "https://todo.example.com";
    const CALLBACK: &str = "https://claude.ai/api/mcp/auth_callback";

    fn app() -> (tempfile::TempDir, Router) {
        test_router_with(
            "- [ ] a ^aaaaaa\n",
            Some(Token::from_static("secret")),
            Some(BASE),
        )
    }

    async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Response) {
        let response = app.clone().oneshot(request).await.expect("infallible");
        (response.status(), response)
    }

    async fn body(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        String::from_utf8_lossy(&bytes).into_owned()
    }

    async fn json_of(response: Response) -> serde_json::Value {
        serde_json::from_str(&body(response).await).expect("json body")
    }

    fn form(uri: &str, pairs: &[(&str, &str)]) -> Request<Body> {
        let mut body = form_urlencoded::Serializer::new(String::new());
        for (key, value) in pairs {
            body.append_pair(key, value);
        }
        Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body.finish()))
            .expect("valid request")
    }

    fn get(uri: &str, auth: Option<&str>) -> Request<Body> {
        let mut request = Request::get(uri).header(header::HOST, "localhost");
        if let Some(auth) = auth {
            request = request.header(header::AUTHORIZATION, format!("Bearer {auth}"));
        }
        request.body(Body::empty()).expect("valid request")
    }

    #[test]
    fn redirects() {
        for ok in [
            CALLBACK,
            "http://localhost:3118/callback",
            "http://127.0.0.1:50000/callback",
            "http://localhost/callback",
            "http://[::1]:8080/cb",
        ] {
            assert!(redirect_allowed(ok), "{ok}");
        }
        for bad in [
            "https://evil.example/api/mcp/auth_callback",
            "https://claude.ai.evil.example/api/mcp/auth_callback",
            "http://localhost.evil.example/callback",
            "http://localhost@evil.example/callback",
            "https://localhost/callback",
            "",
        ] {
            assert!(!redirect_allowed(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn unauthorized_mcp_points_to_metadata() {
        let (_dir, app) = app();
        let (status, response) = send(&app, get("/mcp", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            "Bearer resource_metadata=\"https://todo.example.com/.well-known/oauth-protected-resource\""
        );
        let (status, response) =
            send(&app, get("/.well-known/oauth-protected-resource", None)).await;
        assert_eq!(status, StatusCode::OK);
        let metadata = json_of(response).await;
        assert_eq!(metadata["resource"], "https://todo.example.com/mcp");
        assert_eq!(metadata["authorization_servers"][0], BASE);
        let (_, response) = send(&app, get("/.well-known/oauth-authorization-server", None)).await;
        let metadata = json_of(response).await;
        assert_eq!(metadata["issuer"], BASE);
        assert_eq!(metadata["code_challenge_methods_supported"][0], "S256");
    }

    #[tokio::test]
    async fn register_accepts_claude_only() {
        let (_dir, app) = app();
        let register = |uris: serde_json::Value| {
            Request::post("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "redirect_uris": uris }).to_string()))
                .expect("valid request")
        };
        let (status, response) = send(&app, register(json!([CALLBACK]))).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json_of(response).await["client_id"], CLIENT_ID);
        let (status, _) = send(&app, register(json!(["https://evil.example/cb"]))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn full_login_flow() {
        let (_dir, app) = app();
        let verifier = "a-long-enough-pkce-verifier-for-the-test-0123456789";
        let challenge = B64.encode(Sha256::digest(verifier.as_bytes()));
        let request = [
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", CALLBACK),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", "xyz"),
        ];

        // The form shows, and carries the request along.
        let query = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(request)
            .finish();
        let (status, response) = send(&app, get(&format!("/oauth/authorize?{query}"), None)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body(response)
                .await
                .contains("name=\"state\" value=\"xyz\"")
        );

        // A wrong token shows the form again.
        let mut wrong = request.to_vec();
        wrong.push(("password", "nope"));
        let (status, _) = send(&app, form("/oauth/authorize", &wrong)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // The right token redirects with a code and the state.
        let mut right = request.to_vec();
        right.push(("password", "secret"));
        let (status, response) = send(&app, form("/oauth/authorize", &right)).await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let location = response.headers()[header::LOCATION]
            .to_str()
            .expect("ascii")
            .to_owned();
        let query = location
            .strip_prefix(&format!("{CALLBACK}?"))
            .expect("callback");
        let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(pairs[1], ("state".to_owned(), "xyz".to_owned()));
        let code = pairs[0].1.clone();

        // A wrong verifier fails, the right one gets tokens.
        let exchange = |verifier: &str| {
            form(
                "/oauth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", CALLBACK),
                    ("code_verifier", verifier),
                    ("client_id", CLIENT_ID),
                ],
            )
        };
        let (status, response) = send(&app, exchange("wrong")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json_of(response).await["error"], "invalid_grant");
        let (status, response) = send(&app, exchange(verifier)).await;
        assert_eq!(status, StatusCode::OK);
        let tokens = json_of(response).await;
        let access = tokens["access_token"].as_str().expect("access token");
        let refresh = tokens["refresh_token"].as_str().expect("refresh token");

        // The access token opens /mcp and /api; the refresh token does not.
        let (status, _) = send(&app, get("/api/todos", Some(access))).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(&app, get("/mcp", Some(access))).await;
        assert_ne!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = send(&app, get("/api/todos", Some(refresh))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // A code is not an access token either.
        let (status, _) = send(&app, get("/api/todos", Some(&code))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Refreshing gives a new pair; the static token still works.
        let (status, response) = send(
            &app,
            form(
                "/oauth/token",
                &[("grant_type", "refresh_token"), ("refresh_token", refresh)],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(json_of(response).await["access_token"].is_string());
        let (status, _) = send(&app, get("/api/todos", Some("secret"))).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn tokens_from_another_key_are_refused() {
        let ours = Issuer::new(BASE, Token::from_static("secret"));
        let theirs = Issuer::new(BASE, Token::from_static("other"));
        let token = theirs.sign(&Claims {
            k: 'a',
            exp: now() + 60,
            cc: None,
            ru: None,
        });
        assert!(theirs.accepts(&token));
        assert!(!ours.accepts(&token));
        let expired = ours.sign(&Claims {
            k: 'a',
            exp: now() - 1,
            cc: None,
            ru: None,
        });
        assert!(!ours.accepts(&expired));
    }
}
