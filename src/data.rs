use std::collections::HashSet;
use std::time::Duration;

use crate::error::ApiError;
use bm_lib::{
    discord::{Guild, Id},
    model::Config,
};
use tracing::instrument;

use crate::State;

const CONFIG_TTL: Duration = Duration::from_secs(60);

#[inline]
fn guild_cache_key(guild_id: &Id) -> String {
    format!("guild:{}", guild_id)
}

#[inline]
fn roles_cache_key(guild_id: &Id, user_id: &Id) -> String {
    format!("roles:{}:{}", guild_id, user_id)
}

#[inline]
fn member_guilds_cache_key(user_id: &Id) -> String {
    format!("member_guilds:{}", user_id)
}

impl State {
    #[instrument(skip(self))]
    pub async fn get_guild(&self, guild_id: &Id) -> Result<Option<Guild>, ApiError> {
        let key = guild_cache_key(guild_id);
        self.bot_cache
            .get::<String, Guild>(&key)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_config(&self, guild_id: &Id) -> Result<Option<Config>, ApiError> {
        if let Some(config) = self.bot_cache.get::<Id, Config>(&guild_id).await? {
            return Ok(Some(config));
        }

        let config = match self.db.get_config(&guild_id).await? {
            Some(config) => config,
            None => {
                return Ok(None);
            }
        };

        Ok(Some(config))
    }

    #[instrument(skip(self))]
    pub async fn update_config(&self, guild_id: &Id, update: &Config) -> Result<Config, ApiError> {
        let config = self.db.update_config(&guild_id, &update).await?;

        self.cache
            .set(guild_id, &config, Some(CONFIG_TTL))
            .await
            .map_err(ApiError::from)?;

        Ok(config)
    }

    #[instrument(skip(self))]
    pub async fn get_member_roles(
        &self,
        guild_id: &Id,
        user_id: &Id,
    ) -> Result<Option<HashSet<Id>>, ApiError> {
        let key = roles_cache_key(guild_id, user_id);
        self.bot_cache
            .get::<String, HashSet<Id>>(&key)
            .await
            .map_err(ApiError::from)
    }

    /// Returns the set of guild IDs the bot has observed this user in, using
    /// the `member_guilds:{user_id}` reverse index written by the bot on every
    /// `GuildMemberUpdate` event.  O(1) — no keyspace scan.
    #[instrument(skip(self))]
    pub async fn get_member_guilds(&self, user_id: &Id) -> Result<HashSet<Id>, ApiError> {
        let key = member_guilds_cache_key(user_id);
        Ok(self
            .bot_cache
            .get::<String, HashSet<Id>>(&key)
            .await
            .map_err(ApiError::from)?
            .unwrap_or_default())
    }

    /// Returns every role ID the user holds across all guilds the bot knows
    /// about, by fetching per-guild `roles:{guild_id}:{user_id}` entries for
    /// each guild in the reverse index.  O(m) pipelined where m = guild count.
    #[instrument(skip(self))]
    pub async fn get_all_member_roles(
        &self,
        user_id: &Id,
        guild_ids: &HashSet<Id>,
    ) -> Result<HashSet<Id>, ApiError> {
        let mut all_roles = HashSet::new();
        for guild_id in guild_ids {
            if let Some(roles) = self.get_member_roles(guild_id, user_id).await? {
                all_roles.extend(roles);
            }
        }
        Ok(all_roles)
    }
}
