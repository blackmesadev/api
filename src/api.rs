use crate::error::ApiError;
use actix_web::{get, post, web};
use bm_lib::permissions::Permission;
use bm_lib::{discord::Id, model::Config};
use tracing::instrument;

use crate::{auth::AuthenticatedUser, State};

#[get("/api/config/{id}")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_config(
    state: web::Data<State>,
    id: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<web::Json<Config>, ApiError> {
    let id = Id::from_str(&id).map_err(|_| ApiError::ParseError("Invalid ID".to_string()))?;

    let config = match state.get_config(&id).await? {
        Some(config) => config,
        None => return Err(ApiError::NotFound("Config not found".to_string())),
    };

    if !state
        .check_permission(&config, &user, Permission::ConfigView)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".to_string()));
    }

    Ok(web::Json(config))
}

#[post("/api/config/{id}")]
#[instrument(skip(state, user, config), fields(user_id = %user.user_id))]
pub async fn post_config(
    state: web::Data<State>,
    id: web::Path<String>,
    config: web::Json<Config>,
    user: AuthenticatedUser,
) -> Result<web::Json<Config>, ApiError> {
    let id = Id::from_str(&id).map_err(|_| ApiError::ParseError("Invalid ID".to_string()))?;

    if id != config.id {
        return Err(ApiError::BadRequest(
            "ID in path does not match ID in body".to_string(),
        ));
    }

    if !state
        .check_permission(&config, &user, Permission::ConfigEdit)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".to_string()));
    }

    state.update_config(&id, &config).await?;

    Ok(config)
}
