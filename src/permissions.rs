use bm_lib::{
    discord::Guild, model::Config, permissions::{Permission, PermissionSet}
};
use tracing::instrument;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

impl State {
    #[instrument(skip(self, config, user), fields(guild_id = %config.id, user_id = %user.user_id))]
    pub async fn check_permission(
        &self,
        config: &Config,
        guild: Option<&Guild>,
        user: &AuthenticatedUser,
        perm: Permission,
    ) -> Result<bool, ApiError> {
        // Check if user is guild owner first (always bypasses)
        if let Some(guild) = guild {
            if guild.owner_id == Some(user.user_id) {
                tracing::debug!("User is guild owner, permission granted");
                return Ok(true);
            }
        }

        // Check Discord role-based permissions if inherit_discord_perms is enabled
        if config.inherit_discord_perms {
            if let Some(guild) = guild {
                if let Ok(Some(user_roles)) = self.get_member_roles(&guild.id, &user.user_id).await {
                    tracing::debug!(role_count = user_roles.len(), "Checking Discord roles");
                    let perms = PermissionSet::from_discord_permissions(&guild.roles, &user_roles);
                    if perms.has_permission(&perm) {
                        tracing::debug!("User has Discord permission");
                        return Ok(true);
                    }
                }
            }
        }

        // Check permission groups
        if let Some(groups) = &config.permission_groups {
            tracing::debug!(group_count = groups.len(), "Checking permission groups");
            if groups
                .iter()
                .any(|group| group.users.contains(&user.user_id) && group.permissions.has_permission(&perm))
            {
                return Ok(true);
            }
        }

        tracing::debug!("Permission check failed");
        Ok(false)
    }
}
