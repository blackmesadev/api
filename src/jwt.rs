use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub discord_token: String,
    pub discord_refresh: String,
    pub exp: i64,
}

pub fn create_token(
    user_id: &str,
    discord_token: &str,
    discord_refresh: &str,
    expires_in: u64,
) -> String {
    let expiration = Utc::now()
        .checked_add_signed(Duration::seconds(expires_in as i64))
        .unwrap()
        .timestamp();

    let claims = Claims {
        sub: user_id.to_string(),
        discord_token: discord_token.to_string(),
        discord_refresh: discord_refresh.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(
            std::env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set")
                .as_bytes(),
        ),
    )
    .unwrap()
}
