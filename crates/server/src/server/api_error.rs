//! Errors answered as `{"error": "..."}`.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use todo_client::ErrorBody;

/// An error response: status and message.
#[derive(Debug)]
pub(super) struct ApiError(StatusCode, String);

impl ApiError {
    /// 400: the request can't be used as is.
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }

    /// 404: no todo with this id.
    pub(super) fn not_found(id: &str) -> Self {
        Self(StatusCode::NOT_FOUND, format!("no todo with id `{id}`"))
    }

    /// The message, without the status.
    pub(super) fn message(&self) -> &str {
        &self.1
    }
}

impl From<todo_store::Error> for ApiError {
    fn from(error: todo_store::Error) -> Self {
        use todo_md::Error as Md;
        use todo_store::Error as Store;
        match error {
            Store::Md(Md::NotFound(id)) => Self::not_found(&id.0),
            Store::Md(error) => Self(StatusCode::CONFLICT, error.to_string()),
            Store::Io { .. } => {
                tracing::error!("{error}");
                Self(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "could not read or write the vault".to_owned(),
                )
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorBody { error: self.1 })).into_response()
    }
}
