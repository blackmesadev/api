use chrono::{Duration, Utc};
use jsonwebtoken::{encode, errors::Error, EncodingKey, Header};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub discord_token: String,
    pub discord_refresh: String,
    pub discord_token_type: String,
    pub discord_scope: String,
    pub discord_expires_in: u64,
    pub exp: i64,
}

pub fn create_token(
    user_id: &str,
    discord_token: &str,
    discord_refresh: &str,
    discord_token_type: &str,
    discord_scope: &str,
    expires_in: u64,
    jwt_secret: &str,
) -> Result<String, Error> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::seconds(expires_in as i64))
        .unwrap_or_else(Utc::now)
        .timestamp();

    let claims = Claims {
        sub: user_id.to_string(),
        discord_token: discord_token.to_string(),
        discord_refresh: discord_refresh.to_string(),
        discord_token_type: discord_token_type.to_string(),
        discord_scope: discord_scope.to_string(),
        discord_expires_in: expires_in,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
}
