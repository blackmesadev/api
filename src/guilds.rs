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
/// 2. For each of those guilds fetch `roles:{guild_id}:{user_id}` (pipelined,
///    O(m) where m = guild count, typically < 100).
/// 3. Single DB query bounded by `guild_id = ANY(known_guilds)`.
/// 4. Fetch guild/config and compute real permissions per result.
#[get("/api/guilds")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_guilds(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<UserGuild>>, ApiError> {
    // O(1): single Redis GET for the user's guild membership reverse index.
    let member_guild_ids = state.get_member_guilds(&user.user_id).await?;

    // O(m) pipelined: fetch the role IDs the user holds in each guild.
    let all_role_ids = state
        .get_all_member_roles(&user.user_id, &member_guild_ids)
        .await?;

    let guild_ids_vec: Vec<_> = member_guild_ids.iter().copied().collect();
    let role_ids_vec: Vec<_> = all_role_ids.into_iter().collect();

    // DB query bounded to the known guild set — no full permissions-table scan.
    let guild_ids = state
        .db
        .list_guilds_for_user(
            &user.user_id,
            &guild_ids_vec,
            &role_ids_vec,
            Permission::CONFIG_VIEW,
        )
        .await?;

    let mut guilds = Vec::with_capacity(guild_ids.len());
    for guild_id in &guild_ids {
        let Some(guild) = state.get_guild(guild_id).await? else {
            continue;
        };
        let Some(config) = state.get_config(guild_id).await? else {
            continue;
        };

        let perms = state
            .resolve_member_permissions(&config, &guild, &user.user_id)
            .await
            .unwrap_or(Permission::CONFIG_VIEW);

        guilds.push(UserGuild {
            id: guild_id.to_string(),
            name: guild.name.to_string(),
            icon: guild.icon.map(|s| s.to_string()),
            permissions: perms,
            owner: guild.owner_id == Some(user.user_id),
        });
    }

    Ok(web::Json(guilds))
}
