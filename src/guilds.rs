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
    pub permissions: Option<String>,
    pub owner: bool,
}

/// `GET /api/guilds` — list guilds the authenticated user can view config for.
#[get("/api/guilds")]
#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_guilds(
    state: web::Data<State>,
    user: AuthenticatedUser,
) -> Result<web::Json<Vec<UserGuild>>, ApiError> {
    let mut guilds = Vec::new();
    for guild_id in state.db.list_guild_ids().await? {
        let Some(config) = state.get_config(&guild_id).await? else {
            continue;
        };

        let guild = state.get_guild(&guild_id).await?;

        if state
            .check_permission(&config, guild.as_ref(), &user, Permission::ConfigView)
            .await?
        {
            let id = guild
                .as_ref()
                .map(|g| g.id.to_string())
                .unwrap_or_else(|| guild_id.to_string());
            let name = guild
                .as_ref()
                .map(|g| g.name.to_string())
                .unwrap_or_else(|| guild_id.to_string());
            let icon = guild.as_ref().and_then(|g| {
                g.icon.as_ref().map(|icon| {
                    format!(
                        "https://cdn.discordapp.com/icons/{}/{}.png",
                        g.id, icon
                    )
                })
            });
            let owner = guild
                .as_ref()
                .map(|g| g.owner_id == Some(user.user_id))
                .unwrap_or(false);

            guilds.push(UserGuild {
                id,
                name,
                icon,
                permissions: config.permission_groups.as_ref().map(|groups| {
                    groups
                        .iter()
                        .filter(|group| group.users.contains(&user.user_id))
                        .flat_map(|group| group.permissions.permissions())
                        .fold(String::new(), |acc, perm| acc + &perm.to_string() + ",")
                }),
                owner,
            });
        }
    }

    Ok(web::Json(guilds))
}
