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

    let guild = state
        .get_guild(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_VIEW)
        .await?
    {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
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
    let update = config.into_inner();

    if id != update.id {
        return Err(ApiError::BadRequest(
            "ID in path does not match ID in body".to_string(),
        ));
    }

    let config = match state.get_config(&id).await? {
        Some(config) => config,
        None => return Err(ApiError::NotFound("Config not found".to_string())),
    };

    let guild = state
        .get_guild(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_EDIT)
        .await?
    {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let updated = state.update_config(&id, &update).await?;

    Ok(web::Json(updated))
}
