use std::time::Duration;

use crate::error::ApiError;
use bm_lib::{
    discord::{Guild, Id, Member},
    model::Config,
};
use tracing::instrument;

use crate::State;

const CONFIG_TTL: Duration = Duration::from_secs(60);

impl State {
    #[tracing::instrument(skip(self))]
    pub async fn get_config(&self, guild_id: &Id) -> Result<Option<Config>, ApiError> {
        if let Some(config) = self.cache.get::<Id, Config>(&guild_id).await? {
            return Ok(Some(config));
        }

        let config = match self.db.get_config(&guild_id).await? {
            Some(config) => config,
            None => {
                return Ok(None);
            }
        };

        self.cache
            .set(guild_id, &config, Some(CONFIG_TTL))
            .await
            .map_err(ApiError::from)?;

        Ok(Some(config))
    }

    #[instrument(skip(self))]
    pub async fn update_config(&self, guild_id: &Id, config: &Config) -> Result<(), ApiError> {
        self.db.update_config(&guild_id, &config).await?;

        self.cache
            .set(guild_id, &config, Some(CONFIG_TTL))
            .await
            .map_err(ApiError::from)?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_guild(&self, guild_id: &Id) -> Result<Option<Guild>, ApiError> {
        let key = format!("guild:{}", guild_id);
        if let Some(guild) = self.cache.get::<String, Guild>(&key).await? {
            return Ok(Some(guild));
        }

        let guild = match self.rest.get_guild(&guild_id).await {
            Ok(guild) => guild,
            Err(e) => {
                tracing::error!("Error getting guild: {:?}", e);
                return Ok(None);
            }
        };

        self.cache.set(&key, &guild, None).await?;

        Ok(Some(guild))
    }

    #[instrument(skip(self))]
    pub async fn get_member(&self, guild_id: &Id, user_id: &Id) -> Result<Member, ApiError> {
        let key = format!("member:{}:{}", guild_id, user_id);
        if let Some(member) = self.cache.get::<String, Member>(&key).await? {
            return Ok(member);
        }

        let member = match self.rest.get_member(&guild_id, &user_id).await {
            Ok(member) => member,
            Err(e) => {
                tracing::error!("Error getting member: {:?}", e);
                return Err(ApiError::from(e));
            }
        };

        self.cache.set(&key, &member, None).await?;

        Ok(member)
    }
}
