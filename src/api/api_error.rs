use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use anyhow::Error;
use log::error;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Internal(Error),
}

impl From<Error> for ApiError {
    fn from(err: Error) -> Self {
        ApiError::Internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            ApiError::Internal(err) => {
                error!("Internal API error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erreur interne serveur".to_string(),
                )
                    .into_response()
            }
        }
    }
}