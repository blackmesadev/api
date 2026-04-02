use actix_web::{get, web};
use serde::Serialize;
use tracing::instrument;

use bm_lib::permissions::Permission;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

#[derive(Debug, Serialize)]
pub struct UserGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: Permission,
    pub owner: bool,
}

/// `GET /api/guilds` — list guilds the authenticated user can view config for.
///
/// Strategy (no full guild iteration, no keyspace scan):
/// 1. Read `member_guilds:{user_id}` — O(1) Redis GET — to get the exact set
///    of guild IDs the bot knows the user is in.
/// 2. For each guild, fetch guild + config from cache/DB (O(m) where m = guild count).
/// 3. Resolve Discord + DB permissions for each guild using permission inheritance.
/// 4. Return only guilds where user has CONFIG_VIEW permission (includes Discord admins).
#[get("/api/guilds")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_guilds(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<UserGuild>>, ApiError> {
    // O(1): single Redis GET for the user's guild membership reverse index.
    let member_guild_ids = state.get_member_guilds(&user.user_id).await?;

    let mut member_guild_ids_log: Vec<String> =
        member_guild_ids.iter().map(|id| id.to_string()).collect();
    member_guild_ids_log.sort();
    tracing::info!("user in {}", member_guild_ids_log.join(", "));

    let mut guilds = Vec::new();

    // Check permissions for each guild the user is a member of.
    // This respects both Discord permissions (admin, etc.) and Black Mesa permission groups.
    for guild_id in &member_guild_ids {
        let Some(guild) = state.get_guild(guild_id).await? else {
            continue;
        };
        let Some(config) = state.get_config(guild_id).await? else {
            continue;
        };

        // Guild owner always has access
        if guild.owner_id == Some(user.user_id) {
            guilds.push(UserGuild {
                id: guild_id.to_string(),
                name: guild.name.to_string(),
                icon: guild.icon.map(|s| s.to_string()),
                permissions: Permission::all(),
                owner: true,
            });
            continue;
        }

        // Resolve full permissions (Discord + Black Mesa permission groups)
        let perms = match state
            .resolve_member_permissions(&config, &guild, &user.user_id)
            .await
        {
            Ok(perms) => perms,
            Err(e) => {
                tracing::warn!(guild_id = %guild_id, error = ?e, "Failed to resolve permissions");
                continue;
            }
        };

        // Only include guilds where user has CONFIG_VIEW permission
        if !perms.has_permission(Permission::CONFIG_VIEW) {
            continue;
        }

        guilds.push(UserGuild {
            id: guild_id.to_string(),
            name: guild.name.to_string(),
            icon: guild.icon.map(|s| s.to_string()),
            permissions: perms,
            owner: false,
        });
    }

    Ok(web::Json(guilds))
}
