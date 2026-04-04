use bm_lib::{discord::Guild, model::Config, permissions::Permission};
use tracing::instrument;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

impl State {
    /// Compute the effective [`Permission`] for a user in a guild.
    ///
    /// Combines Discord role permissions (when `config.inherit_discord_perms` is
    /// enabled) with any Black Mesa permission groups the user belongs to, either
    /// directly by user ID or via their Discord roles.
    #[instrument(skip(self, config), fields(guild_id = %guild.id, user_id = %user_id))]
    pub async fn resolve_member_permissions(
        &self,
        config: &Config,
        guild: &Guild,
        user_id: &bm_lib::discord::Id,
    ) -> Result<Permission, ApiError> {
        let member_roles = if config.inherit_discord_perms {
            self.get_member_roles(&guild.id, user_id).await?
        } else {
            None
        };

        let mut perms = if let Some(roles) = &member_roles {
            Permission::from_discord_permissions(&guild.roles, roles)
        } else {
            Permission::empty()
        };

        if let Some(groups) = &config.permission_groups {
            for group in groups {
                let in_group = group.users.contains(user_id)
                    || member_roles
                        .as_ref()
                        .map(|roles| group.roles.iter().any(|r| roles.contains(r)))
                        .unwrap_or(false);
                if in_group {
                    perms |= group.permissions;
                }
            }
        }

        Ok(perms)
    }

    #[instrument(skip(self, config, user), fields(guild_id = %config.id, user_id = %user.user_id))]
    pub async fn check_permission(
        &self,
        config: &Config,
        guild: Option<&Guild>,
        user: &AuthenticatedUser,
        perm: Permission,
    ) -> Result<bool, ApiError> {
        let Some(guild) = guild else {
            return Ok(false);
        };

        // Guild owner always bypasses permission checks.
        if guild.owner_id == Some(user.user_id) {
            tracing::debug!("User is guild owner, permission granted");
            return Ok(true);
        }

        let perms = self
            .resolve_member_permissions(config, guild, &user.user_id)
            .await?;

        if perms.has_permission(perm) {
            return Ok(true);
        }

        tracing::debug!("Permission check failed");
        Ok(false)
    }

    /// Fetch guild + config from cache and verify the user has the given permission.
    /// Returns `ApiError::NotFound` if guild/config is missing, `ApiError::Auth` if denied.
    pub async fn require_guild_permission(
        &self,
        user: &AuthenticatedUser,
        guild_id: &bm_lib::discord::Id,
        perm: Permission,
    ) -> Result<(), ApiError> {
        let guild = self
            .get_guild(guild_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Guild not found".into()))?;
        let config = self
            .get_config(guild_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Config not found".into()))?;

        if !self
            .check_permission(&config, Some(&guild), user, perm)
            .await?
        {
            return Err(ApiError::Auth("Insufficient permissions".into()));
        }

        Ok(())
    }
}
