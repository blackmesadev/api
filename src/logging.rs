use actix_web::{delete, get, post, web};
use bm_lib::{
    discord::Id,
    model::logging::{LogConfig, LogEventType},
    permissions::Permission,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

/// `GET /api/logging/{guild_id}` - list all log configs for a guild.
#[get("/api/logging/{guild_id}")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_log_configs(
    state: web::Data<State>,
    guild_id: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<LogConfig>>, ApiError> {
    let guild_id =
        Id::from_str(&guild_id).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;

    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_VIEW)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".into()));
    }

    let configs = state.db.get_log_configs(&guild_id).await?;
    Ok(web::Json(configs))
}

#[derive(Debug, Deserialize)]
pub struct UpsertLogConfigRequest {
    pub event: String,
    pub enabled: bool,
    pub channel_id: Option<String>,
    pub embed: bool,
    pub text_content: Option<String>,
    pub embed_title: Option<String>,
    pub embed_body: Option<String>,
    pub embed_color: Option<u32>,
    pub embed_footer: Option<String>,
}

/// `POST /api/logging/{guild_id}` - create or update a log config for a guild.
#[post("/api/logging/{guild_id}")]
#[instrument(skip(state, user, body), fields(user_id = %user.user_id))]
pub async fn upsert_log_config(
    state: web::Data<State>,
    guild_id: web::Path<String>,
    body: web::Json<UpsertLogConfigRequest>,
    user: AuthenticatedUser,
) -> Result<web::Json<LogConfig>, ApiError> {
    let guild_id =
        Id::from_str(&guild_id).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;

    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_EDIT)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".into()));
    }

    // Validate event type
    if LogEventType::from_db_key(&body.event).is_none() {
        return Err(ApiError::BadRequest(format!(
            "Unknown log event type: {}",
            body.event
        )));
    }

    let channel_id = body
        .channel_id
        .as_deref()
        .map(Id::from_str)
        .transpose()
        .map_err(|_| ApiError::ParseError("Invalid channel_id".into()))?;

    let log_config = LogConfig {
        id: None,
        guild_id,
        event: body.event.clone(),
        enabled: body.enabled,
        channel_id,
        embed: body.embed,
        text_content: body.text_content.clone(),
        embed_title: body.embed_title.clone(),
        embed_body: body.embed_body.clone(),
        embed_color: body.embed_color,
        embed_footer: body.embed_footer.clone(),
    };

    let result = state.db.upsert_log_config(&log_config).await?;
    Ok(web::Json(result))
}

#[derive(Debug, Deserialize)]
pub struct BulkUpsertRequest {
    pub configs: Vec<UpsertLogConfigRequest>,
}

/// `POST /api/logging/{guild_id}/bulk` - create or update multiple log configs at once.
#[post("/api/logging/{guild_id}/bulk")]
#[instrument(skip(state, user, body), fields(user_id = %user.user_id))]
pub async fn bulk_upsert_log_configs(
    state: web::Data<State>,
    guild_id: web::Path<String>,
    body: web::Json<BulkUpsertRequest>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<LogConfig>>, ApiError> {
    let guild_id =
        Id::from_str(&guild_id).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;

    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_EDIT)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".into()));
    }

    let mut log_configs = Vec::with_capacity(body.configs.len());
    for req in &body.configs {
        if LogEventType::from_db_key(&req.event).is_none() {
            return Err(ApiError::BadRequest(format!(
                "Unknown log event type: {}",
                req.event
            )));
        }

        let channel_id = req
            .channel_id
            .as_deref()
            .map(Id::from_str)
            .transpose()
            .map_err(|_| ApiError::ParseError("Invalid channel_id".into()))?;

        log_configs.push(LogConfig {
            id: None,
            guild_id,
            event: req.event.clone(),
            enabled: req.enabled,
            channel_id,
            embed: req.embed,
            text_content: req.text_content.clone(),
            embed_title: req.embed_title.clone(),
            embed_body: req.embed_body.clone(),
            embed_color: req.embed_color,
            embed_footer: req.embed_footer.clone(),
        });
    }

    let results = state
        .db
        .bulk_upsert_log_configs(&guild_id, &log_configs)
        .await?;
    Ok(web::Json(results))
}

/// `DELETE /api/logging/{guild_id}/{event}` - delete a log config for a specific event.
#[delete("/api/logging/{guild_id}/{event}")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn delete_log_config(
    state: web::Data<State>,
    path: web::Path<(String, String)>,
    user: AuthenticatedUser,
) -> Result<web::Json<bool>, ApiError> {
    let (guild_id_str, event) = path.into_inner();
    let guild_id =
        Id::from_str(&guild_id_str).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;

    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::CONFIG_EDIT)
        .await?
    {
        return Err(ApiError::Auth("Insufficient permissions".into()));
    }

    let deleted = state.db.delete_log_config(&guild_id, &event).await?;
    Ok(web::Json(deleted))
}
