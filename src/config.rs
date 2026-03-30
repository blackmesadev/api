use std::env;

use std::io::{Error, ErrorKind, Result};

/// Runtime settings loaded from environment variables.
#[derive(Clone, Debug)]
pub struct Settings {
    pub api_host: String,
    pub api_port: u16,
    pub database_url: String,
    pub redis_uri: String,
    pub redis_prefix: String,
    pub bot_redis_prefix: String,
    pub otlp_endpoint: String,
    pub otlp_auth: Option<String>,
    pub otlp_organization: Option<String>,
    pub discord_bot_token: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub discord_redirect_uri: String,
    pub jwt_secret: String,
}

impl Settings {
    /// Loads service settings from `.env` and process environment.
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let api_port = env::var("API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8080);

        Ok(Self {
            api_host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            api_port,
            database_url: required("DATABASE_URL")?,
            redis_uri: required("REDIS_URI")?,
            redis_prefix: env::var("REDIS_PREFIX").unwrap_or_else(|_| "bm-api".to_string()),
            bot_redis_prefix: env::var("BOT_REDIS_PREFIX")
                .unwrap_or_else(|_| "black-mesa".to_string()),
            otlp_endpoint: env::var("OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            otlp_auth: env::var("OTLP_AUTH").ok(),
            otlp_organization: env::var("OTLP_ORGANIZATION").ok(),
            discord_bot_token: required("DISCORD_BOT_TOKEN")?,
            discord_client_id: required("DISCORD_CLIENT_ID")?,
            discord_client_secret: required("DISCORD_CLIENT_SECRET")?,
            discord_redirect_uri: required("DISCORD_REDIRECT_URI")?,
            jwt_secret: required("JWT_SECRET")?,
        })
    }
}

fn required(key: &str) -> Result<String> {
    env::var(key).map_err(|_| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("Missing required environment variable: {key}"),
        )
    })
}
