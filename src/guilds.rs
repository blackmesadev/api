use actix_web::{get, web};
use serde::Serialize;
use tracing::instrument;

use bm_lib::discord::{Channel, Id, Role};
use bm_lib::permissions::Permission;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

#[derive(Debug, Serialize)]
pub struct UserGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: Permission,
    pub owner: bool,
    /// Highest Discord role name for this user (by position), if available.
    pub highest_role: Option<String>,
    /// Black Mesa permission group names the user belongs to.
    pub permission_groups: Vec<String>,
    /// Approximate member count from the cached Guild object.
    pub member_count: Option<u32>,
    /// Number of infractions in this guild.
    pub infraction_count: u64,
    /// Number of active infractions in this guild.
    pub active_infraction_count: u64,
    /// Whether automod is enabled.
    pub automod_enabled: bool,
    /// Whether moderation module is enabled.
    pub moderation_enabled: bool,
    /// Whether music module is enabled.
    pub music_enabled: bool,
}

/// `GET /api/guilds` - list guilds the authenticated user can view config for.
///
/// Strategy (no full guild iteration, no keyspace scan):
/// 1. Read `member_guilds:{user_id}` - O(1) Redis GET - to get the exact set
///  of guild IDs the bot knows the user is in.
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

        // Resolve the user's Discord roles in this guild for role name lookups.
        let member_roles = state.get_member_roles(guild_id, &user.user_id).await?;

        // Find highest Discord role by position.
        let highest_role = member_roles.as_ref().and_then(|role_ids| {
            guild
                .roles
                .iter()
                .filter(|r| role_ids.contains(&r.id))
                .max_by_key(|r| r.position)
                .map(|r| r.name.to_string())
        });

        // Find BM permission groups the user belongs to.
        let permission_groups = config
            .permission_groups
            .as_ref()
            .map(|groups| {
                groups
                    .iter()
                    .filter(|g| {
                        g.users.contains(&user.user_id)
                            || member_roles
                                .as_ref()
                                .map(|roles| g.roles.iter().any(|r| roles.contains(r)))
                                .unwrap_or(false)
                    })
                    .map(|g| g.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Fetch infraction counts from the database.
        let all_infractions = state.db.get_infractions(guild_id, None, None, None).await;
        let (infraction_count, active_infraction_count) = match all_infractions {
            Ok(ref infs) => (
                infs.len() as u64,
                infs.iter().filter(|i| i.active).count() as u64,
            ),
            Err(_) => (0, 0),
        };

        // Guild owner always has access
        if guild.owner_id == Some(user.user_id) {
            guilds.push(UserGuild {
                id: guild_id.to_string(),
                name: guild.name.to_string(),
                icon: guild.icon.map(|s| s.to_string()),
                permissions: Permission::all(),
                owner: true,
                highest_role,
                permission_groups,
                member_count: guild.member_count.or(guild.approximate_member_count),
                infraction_count,
                active_infraction_count,
                automod_enabled: config.automod_enabled,
                moderation_enabled: config.moderation_enabled,
                music_enabled: config.music_enabled,
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
            highest_role,
            permission_groups,
            member_count: guild.member_count.or(guild.approximate_member_count),
            infraction_count,
            active_infraction_count,
            automod_enabled: config.automod_enabled,
            moderation_enabled: config.moderation_enabled,
            music_enabled: config.music_enabled,
        });
    }

    Ok(web::Json(guilds))
}

/// `GET /api/guilds/{id}/channels` - list channels for a guild via Discord REST API.
#[get("/api/guilds/{id}/channels")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_guild_channels(
    state: web::Data<State>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<web::Json<Vec<Channel>>, ApiError> {
    let guild_id = Id::from_str(&path.into_inner())
        .map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;
    state
        .require_guild_permission(&user, &guild_id, Permission::CONFIG_VIEW)
        .await?;

    // Try cache first, fallback to Discord API if needed
    let channels = match state.get_channels(&guild_id).await? {
        Some(cached) => cached,
        None => {
            // Cache miss - fetch from Discord and update cache
            let fetched = state
                .bot
                .get_guild_channels(&guild_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Discord API error: {}", e)))?;

            // Store in cache for future requests
            state.set_channels(&guild_id, &fetched).await?;

            fetched
        }
    };

    Ok(web::Json(channels))
}

/// `GET /api/guilds/{id}/roles` - list roles for a guild from bot cache.
#[get("/api/guilds/{id}/roles")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_guild_roles(
    state: web::Data<State>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<web::Json<Vec<Role>>, ApiError> {
    let guild_id = Id::from_str(&path.into_inner())
        .map_err(|_| ApiError::ParseError("Invalid guild ID".into()))?;
    state
        .require_guild_permission(&user, &guild_id, Permission::CONFIG_VIEW)
        .await?;

    let guild = state
        .get_guild(&guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;

    let mut roles = guild.roles;

    roles.sort_by(|a, b| b.position.cmp(&a.position));

    Ok(web::Json(roles))
}
