use serde::{Deserialize, Serialize};
use tracing::instrument;

const API_BASE: &str = "https://discord.com/api/v10";

pub struct RestClient {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    client: reqwest::Client,
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
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id,
            client_secret,
            redirect_uri,
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

    #[instrument(skip(self, code))]
    pub async fn oauth_token(
        &self,
        code: String,
        redirect_uri_override: Option<String>,
    ) -> Result<DiscordOAuthResponse, reqwest::Error> {
        let redirect = redirect_uri_override.unwrap_or_else(|| self.redirect_uri.clone());
        let response = self
            .client
            .post(&format!("{}/oauth2/token", API_BASE))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("client_id", self.client_id.clone()),
                ("client_secret", self.client_secret.clone()),
                ("grant_type", "authorization_code".to_owned()),
                ("code", code),
                ("redirect_uri", redirect),
                ("scope", "identify guilds".to_string()),
            ])
            .send()
            .await?;

        response.json().await
    }

    #[instrument(skip(self, token))]
    pub async fn get_self(&self, token: &str) -> Result<DiscordUser, reqwest::Error> {
        self.get("users/@me", token).await?.json().await
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

/// Stripped-down Discord user profile returned by `GET /users/@me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
}
