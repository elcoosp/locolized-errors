use loco_rs::prelude::*;

// Re-export the derive macro
pub use locolized_errors_macros::LocalizedApiError;
use serde::Serialize;

// Main error trait
pub trait LocalizedApiError: std::fmt::Display + Send + Sync + 'static + Serialize {
    fn error_kind(&self) -> ErrorKind;

    fn to_loco_error(&self) -> Error {
        let message = self.to_string();
        match self.error_kind() {
            ErrorKind::BadRequest => {
                let encoded = serde_json::json!({
                    "code": self,
                    "message": message
                });
                Error::BadRequest(serde_json::to_string(&encoded).unwrap())
            }
            ErrorKind::Unauthorized => Error::Unauthorized(message),
            ErrorKind::NotFound => Error::NotFound,
            ErrorKind::InternalServerError => Error::InternalServerError,
            ErrorKind::Custom(status, description) => Error::CustomError(
                status,
                loco_rs::controller::ErrorDetail::new(message, description),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    BadRequest,
    Unauthorized,
    NotFound,
    InternalServerError,
    Custom(axum::http::StatusCode, String),
}

// Create your error enums with the clean derive macro
