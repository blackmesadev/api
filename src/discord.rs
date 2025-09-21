use bm_lib::discord::{Guild, Id, Member};
use serde::Deserialize;
use serde_json::Value;
use tracing::instrument;

const API_BASE: &str = "https://discord.com/api/v10";

pub struct RestClient {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    client: reqwest::Client,
    bot_token: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordOAuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
}

impl RestClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id: std::env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID must be set"),
            client_secret: std::env::var("DISCORD_CLIENT_SECRET")
                .expect("DISCORD_CLIENT_SECRET must be set"),
            redirect_uri: std::env::var("DISCORD_REDIRECT_URI")
                .expect("DISCORD_REDIRECT_URI must be set"),
            bot_token: std::env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN must be set"),
        }
    }

    #[instrument(skip(self, token))]
    pub async fn get(&self, path: &str, token: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.client
            .get(&format!("{}/{}", API_BASE, path))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
    }

    #[instrument(skip(self))]
    pub async fn bot_get(&self, path: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.client
            .get(&format!("{}/{}", API_BASE, path))
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
    }

    #[instrument(skip(self, code))]
    pub async fn oauth_token(&self, code: String) -> Result<DiscordOAuthResponse, reqwest::Error> {
        let response = self
            .client
            .post(&format!("{}/oauth2/token", API_BASE))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("client_id", self.client_id.clone()),
                ("client_secret", self.client_secret.clone()),
                ("grant_type", "authorization_code".to_owned()),
                ("code", code),
                ("redirect_uri", self.redirect_uri.clone()),
                ("scope", "identify".to_string()),
            ])
            .send()
            .await?;

        response.json().await
    }

    #[instrument(skip(self, token))]
    pub async fn get_self(&self, token: &str) -> Result<Value, reqwest::Error> {
        self.get("users/@me", token).await?.json().await
    }

    #[instrument(skip(self))]
    pub async fn get_guild(&self, guild_id: &Id) -> Result<Guild, reqwest::Error> {
        self.bot_get(&format!("guilds/{}", guild_id))
            .await?
            .json()
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_member(&self, guild_id: &Id, user_id: &Id) -> Result<Member, reqwest::Error> {
        self.bot_get(&format!("guilds/{}/members/{}", guild_id, user_id))
            .await?
            .json()
            .await
    }

    #[instrument(skip(self, refresh_token))]
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<DiscordOAuthResponse, reqwest::Error> {
        let response = self
            .client
            .post(&format!("{}/oauth2/token", API_BASE))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("client_id", self.client_id.clone()),
                ("client_secret", self.client_secret.clone()),
                ("grant_type", "refresh_token".to_owned()),
                ("refresh_token", refresh_token.to_owned()),
            ])
            .send()
            .await?;

        response.json().await
    }
}
