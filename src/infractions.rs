use actix_web::{get, post, web};
use bm_lib::{
    discord::Id,
    model::{Infraction, InfractionType, Uuid},
    permissions::Permission,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

#[derive(Debug, Deserialize)]
pub struct InfractionQuery {
    pub user_id: Option<String>,
    #[serde(rename = "type")]
    pub infraction_type: Option<String>,
    pub active: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInfractionRequest {
    pub guild_id: String,
    pub user_id: String,
    pub infraction_type: String,
    pub reason: Option<String>,
    pub expires_at: Option<u64>,
    pub mute_role_id: Option<String>,
}

/// `GET /api/infractions/{guild_id}` - list / search infractions for a guild.
///
/// Optional query parameters:
/// - `user_id`  - filter by user
/// - `type`     - filter by infraction type (warn/mute/kick/ban)
/// - `active`   - `"true"` or `"false"`
#[get("/api/infractions/{guild_id}")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_infractions(
    state: web::Data<State>,
    guild_id: web::Path<String>,
    query: web::Query<InfractionQuery>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<Infraction>>, ApiError> {
    let guild_id =
        Id::from_str(&guild_id).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;

    // Permission check - require infraction view access.
    let config = state.get_config(&guild_id).await?.ok_or_else(|| {
        ApiError::NotFound("Config not found - guild may not be set up yet".into())
    })?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::INFRACTION_VIEW)
        .await?
    {
        return Err(ApiError::Forbidden("Insufficient permissions".into()));
    }

    let user_id = query
        .user_id
        .as_deref()
        .map(Id::from_str)
        .transpose()
        .map_err(|_| ApiError::ParseError("Invalid user_id".into()))?;

    let infraction_type = match &query.infraction_type {
        Some(t) => Some(
            InfractionType::from_str(t)
                .ok_or_else(|| ApiError::ParseError(format!("Invalid infraction type: {t}")))?,
        ),
        None => None,
    };

    let active = match query.active.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };

    let infractions = state
        .db
        .get_infractions(&guild_id, user_id.as_ref(), infraction_type, active)
        .await?;

    Ok(web::Json(infractions))
}

/// `POST /api/infractions` - create a new infraction.
#[post("/api/infractions")]
#[instrument(skip(state, user, body), fields(user_id = %user.user_id))]
pub async fn create_infraction(
    state: web::Data<State>,
    body: web::Json<CreateInfractionRequest>,
    user: AuthenticatedUser,
) -> Result<web::Json<Infraction>, ApiError> {
    let guild_id = Id::from_str(&body.guild_id)
        .map_err(|_| ApiError::ParseError("Invalid guild_id".into()))?;
    let target_user_id =
        Id::from_str(&body.user_id).map_err(|_| ApiError::ParseError("Invalid user_id".into()))?;
    let moderator_id = user.user_id;

    let infraction_type = InfractionType::from_str(&body.infraction_type)
        .ok_or_else(|| ApiError::ParseError("Invalid infraction_type".into()))?;

    // Permission check - require infraction edit access.
    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::INFRACTION_EDIT)
        .await?
    {
        return Err(ApiError::Forbidden("Insufficient permissions".into()));
    }

    let mute_role_id = body
        .mute_role_id
        .as_deref()
        .map(Id::from_str)
        .transpose()
        .map_err(|_| ApiError::ParseError("Invalid mute_role_id".into()))?;

    let mut infraction = Infraction::new(
        guild_id,
        target_user_id,
        moderator_id,
        infraction_type,
        body.reason.clone(),
        body.expires_at,
        true,
    );
    infraction.mute_role_id = mute_role_id;

    state.db.create_infraction(&infraction).await?;

    Ok(web::Json(infraction))
}

/// `POST /api/infractions/{guild_id}/{id}/deactivate` - deactivate an infraction.
#[post("/api/infractions/{guild_id}/{id}/deactivate")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn deactivate_infraction(
    state: web::Data<State>,
    path: web::Path<(String, String)>,
    user: AuthenticatedUser,
) -> Result<web::Json<serde_json::Value>, ApiError> {
    let (guild_id_str, infraction_id_str) = path.into_inner();

    let guild_id =
        Id::from_str(&guild_id_str).map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;
    let infraction_id = Uuid::from_string(&infraction_id_str)
        .ok_or_else(|| ApiError::ParseError("Invalid infraction ID".into()))?;

    // Permission check
    let config = state
        .get_config(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    if !state
        .check_permission(&config, Some(&guild), &user, Permission::INFRACTION_EDIT)
        .await?
    {
        return Err(ApiError::Forbidden("Insufficient permissions".into()));
    }

    // Verify the infraction belongs to this guild before deactivating.
    let _infraction = state
        .db
        .get_infraction(&guild_id, &infraction_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Infraction not found".into()))?;

    let deactivated = state.db.deactivate_infraction(&infraction_id).await?;

    if !deactivated {
        return Err(ApiError::BadRequest(
            "Infraction is already inactive".into(),
        ));
    }

    Ok(web::Json(
        serde_json::json!({ "success": true, "id": infraction_id_str }),
    ))
}
