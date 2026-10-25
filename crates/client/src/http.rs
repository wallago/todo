//! Request functions: one per server route.

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{ErrorBody, Id, NewTodo, Todo, TodoPatch};

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
#[derive(Clone, Debug)]
pub struct Client {
    /// Keeps connections open between calls.
    http: reqwest::Client,
    /// e.g. `http://127.0.0.1:3000`, without a trailing `/`.
    base: String,
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
        }
    }

    /// Every todo.
    pub async fn list(&self) -> Result<Vec<Todo>> {
        send(self.request(Method::GET, None)).await
    }

    /// One todo.
    pub async fn get(&self, id: &Id) -> Result<Todo> {
        send(self.request(Method::GET, Some(id))).await
    }

    /// Adds a todo to the inbox note. Returns it with its new id.
    pub async fn add(&self, new: &NewTodo) -> Result<Todo> {
        send(self.request(Method::POST, None).json(new)).await
    }

    /// Changes the fields set in `patch`.
    pub async fn update(&self, id: &Id, patch: &TodoPatch) -> Result<Todo> {
        send(self.request(Method::PATCH, Some(id)).json(patch)).await
    }

    /// Ticks a todo. Safe to retry.
    pub async fn complete(&self, id: &Id) -> Result<Todo> {
        let url = format!("{}/api/todos/{}/complete", self.base, id.0);
        send(self.http.post(url)).await
    }

    /// Removes a todo and its line.
    pub async fn remove(&self, id: &Id) -> Result<()> {
        check(self.request(Method::DELETE, Some(id)).send().await?).await?;
        Ok(())
    }

    /// A request to `/api/todos`, or `/api/todos/{id}`.
    fn request(&self, method: Method, id: Option<&Id>) -> RequestBuilder {
        let url = match id {
            Some(id) => format!("{}/api/todos/{}", self.base, id.0),
            None => format!("{}/api/todos", self.base),
        };
        self.http.request(method, url)
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
