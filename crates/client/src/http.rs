//! Request functions: one per server route.

use std::fmt;

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{ErrorBody, Id, NewTodo, Todo, TodoPatch, TodoQuery};

/// What can go wrong when calling the server.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server couldn't be reached, or its answer couldn't be read.
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The server answered with an error.
    #[error("{status}: {message}")]
    Api {
        /// HTTP status, e.g. 404.
        status: StatusCode,
        /// The server's message.
        message: String,
    },
}

impl Error {
    /// `true` if the server has no todo with this id.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status == StatusCode::NOT_FOUND)
    }
}

/// Shorthand for the client's results.
pub type Result<T> = std::result::Result<T, Error>;

/// Talks to a todo server.
#[derive(Clone)]
pub struct Client {
    /// Keeps connections open between calls.
    http: reqwest::Client,
    /// e.g. `http://127.0.0.1:3000`, without a trailing `/`.
    base: String,
    /// Sent as `Authorization: Bearer`, if set.
    token: Option<String>,
}

// By hand, so the token never ends up in logs.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .finish_non_exhaustive()
    }
}

// Every call fails the same way: see `Error`.
#[allow(clippy::missing_errors_doc)]
impl Client {
    /// A client for the server at `base`, e.g. `http://127.0.0.1:3000`.
    #[must_use]
    pub fn new(base: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_owned(),
            token: None,
        }
    }

    /// Sends `token` on every call, for a server with a `token_file`.
    #[must_use]
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    /// Every todo.
    pub async fn list(&self) -> Result<Vec<Todo>> {
        send(self.request(Method::GET, None)).await
    }

    /// The todos of one note, like `travel/japan.md`.
    pub async fn list_note(&self, note: &str) -> Result<Vec<Todo>> {
        let query = TodoQuery {
            note: Some(note.to_owned()),
        };
        send(self.request(Method::GET, None).query(&query)).await
    }

    /// Every note's path, relative to the vault.
    pub async fn notes(&self) -> Result<Vec<String>> {
        send(self.call(Method::GET, "/api/notes")).await
    }

    /// One todo.
    pub async fn get(&self, id: &Id) -> Result<Todo> {
        send(self.request(Method::GET, Some(id))).await
    }

    /// Adds a todo to the inbox note, or to `new.note`. Returns it with its
    /// new id.
    pub async fn add(&self, new: &NewTodo) -> Result<Todo> {
        send(self.request(Method::POST, None).json(new)).await
    }

    /// Changes the fields set in `patch`.
    pub async fn update(&self, id: &Id, patch: &TodoPatch) -> Result<Todo> {
        send(self.request(Method::PATCH, Some(id)).json(patch)).await
    }

    /// Ticks a todo. Safe to retry.
    pub async fn complete(&self, id: &Id) -> Result<Todo> {
        send(self.call(Method::POST, &format!("/api/todos/{}/complete", id.0))).await
    }

    /// Removes a todo and its line.
    pub async fn remove(&self, id: &Id) -> Result<()> {
        check(self.request(Method::DELETE, Some(id)).send().await?).await?;
        Ok(())
    }

    /// A request to `/api/todos`, or `/api/todos/{id}`.
    fn request(&self, method: Method, id: Option<&Id>) -> RequestBuilder {
        match id {
            Some(id) => self.call(method, &format!("/api/todos/{}", id.0)),
            None => self.call(method, "/api/todos"),
        }
    }

    /// A request to `path`, with the token if there is one.
    fn call(&self, method: Method, path: &str) -> RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.base));
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }
}

/// Sends a request and reads the JSON answer.
async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T> {
    Ok(check(request.send().await?).await?.json().await?)
}

/// Turns an error status into [`Error::Api`], keeping the server's message.
async fn check(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = match response.json::<ErrorBody>().await {
        Ok(body) => body.error,
        Err(_) => status.canonical_reason().unwrap_or("error").to_owned(),
    };
    Err(Error::Api { status, message })
}
