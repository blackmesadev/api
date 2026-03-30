use std::time::Duration;
use std::collections::HashSet;

use crate::error::ApiError;
use bm_lib::{discord::{Guild, Id}, model::Config};
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

impl State {
    #[instrument(skip(self))]
    pub async fn get_guild(
        &self,
        guild_id: &Id,
    ) -> Result<Option<Guild>, ApiError> {
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
}
