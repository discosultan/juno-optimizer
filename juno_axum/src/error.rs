use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub struct Error(anyhow::Error);

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let body = Json(json!({
            "message": self.0.to_string(),
        }));

        (status, body).into_response()
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self(err)
    }
}
