use bm_lib::{
    model::Config,
    permissions::{Permission, PermissionSet},
};
use tracing::instrument;

use crate::{auth::AuthenticatedUser, error::ApiError, State};

impl State {
    #[instrument(skip(self, config, user), fields(guild_id = %config.id, user_id = %user.user_id))]
    pub async fn check_permission(
        &self,
        config: &Config,
        user: &AuthenticatedUser,
        perm: Permission,
    ) -> Result<bool, ApiError> {
        if config.inherit_discord_perms {
            let discord_span = tracing::info_span!(
                "discord_permission_check",
                guild_id = %config.id,
                otel.kind = "client"
            );
            let _enter = discord_span.enter();

            tracing::debug!("Checking Discord permissions");
            if let Ok(Some(guild)) = self.get_guild(&config.id).await {
                let roles = &guild.roles;

                let member = self.get_member(&config.id, &user.user_id).await?;

                let perms = PermissionSet::from_discord_permissions(roles, &member.roles);

                if perms.has_permission(&perm) {
                    tracing::debug!("Permission granted via Discord roles");
                    return Ok(true);
                }
            }
        }

        if let Some(groups) = &config.permission_groups {
            tracing::debug!(group_count = groups.len(), "Checking permission groups");
            if groups
                .iter()
                .any(|group| group.users.contains(&user.user_id))
                && groups
                    .iter()
                    .any(|group| group.permissions.has_permission(&perm))
            {
                return Ok(true);
            }
        }

        tracing::debug!("Permission check failed");
        Ok(false)
    }
}
