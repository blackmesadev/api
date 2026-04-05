use crate::error::ApiError;
use actix_web::{dev::Payload, get, web, FromRequest, HttpRequest};
use bm_lib::discord::Id;
use futures::Future;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::instrument;

use crate::{
    discord::DiscordUser,
    jwt::{self, Claims},
    State,
};

#[derive(Debug, Deserialize)]
struct OAuthParams {
    pub code: String,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    pub token: String,
}

#[derive(Debug)]
pub struct AuthenticatedUser {
    pub user_id: Id,
    pub discord_token: String,
    pub discord_refresh: String,
    pub discord_token_type: String,
    pub discord_scope: String,
    pub discord_expires_in: u64,
}

impl FromRequest for AuthenticatedUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let auth_header = req
                .headers()
                .get("Authorization")
                .ok_or_else(|| ApiError::Auth("Missing Authorization header".to_string()))?
                .to_str()
                .map_err(|_| ApiError::Auth("Invalid Authorization header".to_string()))?;

            if !auth_header.starts_with("Bearer ") {
                return Err(ApiError::Auth("Invalid Authorization header".to_string()));
            }

            let token = auth_header.trim_start_matches("Bearer ");
            let state = req
                .app_data::<web::Data<State>>()
                .ok_or_else(|| ApiError::Internal("App state not configured".to_string()))?;

            let claims = decode::<Claims>(
                token,
                &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
                &Validation::default(),
            )
            .map_err(|_| ApiError::Auth("Invalid token".to_string()))?;

            let user_id = claims
                .claims
                .sub
                .parse()
                .map_err(|_| ApiError::Auth("Invalid user ID".to_string()))?;

            Ok(AuthenticatedUser {
                user_id,
                discord_token: claims.claims.discord_token,
                discord_refresh: claims.claims.discord_refresh,
                discord_token_type: claims.claims.discord_token_type,
                discord_scope: claims.claims.discord_scope,
                discord_expires_in: claims.claims.discord_expires_in,
            })
        })
    }
}

#[get("/api/oauth/discord")]
#[instrument(skip(state, params))]
pub async fn oauth_discord(
    state: web::Data<State>,
    params: web::Query<OAuthParams>,
) -> Result<web::Json<AuthResponse>, ApiError> {
    let oauth_response = state
        .rest
        .oauth_token(params.code.clone(), params.redirect_uri.clone())
        .await?;

    let me = state.rest.get_self(&oauth_response.access_token).await?;

    let id: Id = me
        .id
        .parse()
        .map_err(|_| ApiError::Internal("Invalid user ID from Discord".into()))?;

    // Cache the user profile so /api/me doesn't need to hit Discord.
    state.set_user(&id, &me).await?;

    let token = jwt::create_token(
        &id.to_string(),
        &oauth_response.access_token,
        &oauth_response.refresh_token,
        &oauth_response.token_type,
        &oauth_response.scope,
        oauth_response.expires_in,
        &state.jwt_secret,
    )
    .map_err(|e| ApiError::Internal(format!("Failed to create token: {e}")))?;

    Ok(web::Json(AuthResponse { token }))
}

/// `GET /api/me` - return the authenticated user's profile.
/// Served from cache (TTL 10 min); falls back to Discord REST on cache miss.
#[get("/api/me")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_me(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<DiscordUser>, ApiError> {
    if let Some(cached) = state.get_user(&user.user_id).await? {
        return Ok(web::Json(cached));
    }

    let me = state
        .rest
        .get_self(&user.discord_token)
        .await
        .map_err(ApiError::Discord)?;

    state.set_user(&user.user_id, &me).await?;
    Ok(web::Json(me))
}

#[get("/api/oauth/refresh")]
#[instrument(skip(state), fields(user_id = %user.user_id))]
pub async fn refresh_token(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<AuthResponse>, ApiError> {
    // Local JWT refresh: no Discord token refresh or bot API calls required.
    let token = jwt::create_token(
        &user.user_id.to_string(),
        &user.discord_token,
        &user.discord_refresh,
        &user.discord_token_type,
        &user.discord_scope,
        user.discord_expires_in,
        &state.jwt_secret,
    )
    .map_err(|e| ApiError::Internal(format!("Failed to create token: {e}")))?;

    Ok(web::Json(AuthResponse { token }))
}
