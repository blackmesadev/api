use crate::error::ApiError;
use actix_web::{dev::Payload, get, web, FromRequest, HttpRequest};
use bm_lib::discord::Id;
use futures::Future;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::instrument;

use crate::{
    jwt::{self, Claims},
    State,
};

#[derive(Debug, Deserialize)]
struct OAuthParams {
    pub code: String,
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
            let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

            let claims = decode::<Claims>(
                token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
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
            })
        })
    }
}

#[get("/oauth/discord")]
#[instrument(skip(state, params))]
pub async fn oauth_discord(
    state: web::Data<State>,
    params: web::Query<OAuthParams>,
) -> Result<web::Json<AuthResponse>, ApiError> {
    let discord_response = state.rest.oauth_token(params.code.clone()).await?;

    let me = state.rest.get_self(&discord_response.access_token).await?;

    let id = me["id"]
        .as_str()
        .ok_or_else(|| ApiError::Internal("ID not found in response".to_string()))?;

    let token = jwt::create_token(
        id,
        &discord_response.access_token,
        &discord_response.refresh_token,
        discord_response.expires_in,
    );

    Ok(web::Json(AuthResponse { token }))
}

#[get("/oauth/refresh")]
#[instrument(skip(state), fields(user_id = %user.user_id))]
pub async fn refresh_token(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<AuthResponse>, actix_web::Error> {
    let refresh_response = state
        .rest
        .refresh_token(&user.discord_refresh)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let me = state
        .rest
        .get_self(&refresh_response.access_token)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let id = me["id"]
        .as_str()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("ID not found in response"))?;

    let token = jwt::create_token(
        id,
        &refresh_response.access_token,
        &refresh_response.refresh_token,
        refresh_response.expires_in,
    );

    Ok(web::Json(AuthResponse { token }))
}
