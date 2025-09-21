use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Discord API error: {0}")]
    Discord(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Database(#[from] bm_lib::db::MongoError),

    #[error("Cache error: {0}")]
    Cache(#[from] bm_lib::cache::RedisCacheError),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::Auth(_) => HttpResponse::Unauthorized().json(self.to_string()),
            ApiError::BadRequest(_) => HttpResponse::BadRequest().json(self.to_string()),
            ApiError::NotFound(_) => HttpResponse::NotFound().json(self.to_string()),
            ApiError::ParseError(_) => HttpResponse::BadRequest().json(self.to_string()),
            _ => HttpResponse::InternalServerError().json(self.to_string()),
        }
    }
}
