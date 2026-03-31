mod api;
mod auth;
mod config;
mod data;
mod discord;
mod error;
mod guilds;
mod infractions;
mod jwt;
mod permissions;
mod telemetry;

use actix_cors::Cors;
use actix_web::{get, web::Data, App, HttpServer};
use bm_lib::{
    cache::{Cache, RedisCache},
    db::Database,
    discord::DiscordRestClient,
};
use config::Settings;
use discord::RestClient;
use tracing_actix_web::TracingLogger;

const SERVICE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

pub struct State {
    pub db: Database,
    pub cache: Cache<RedisCache>,
    pub bot_cache: Cache<RedisCache>,
    pub rest: RestClient,
    pub bot: DiscordRestClient,
    pub jwt_secret: String,
}

impl State {
    pub async fn new(settings: &Settings) -> Self {
        Self {
            db: {
                let db = Database::connect(settings.database_url.clone())
                    .await
                    .expect("Failed to connect to database");
                db.migrate()
                    .await
                    .expect("Failed to run database migrations");
                db
            },
            cache: Cache::new(
                RedisCache::new(settings.redis_uri.clone(), settings.redis_prefix.clone())
                    .await
                    .expect("Failed to connect to Redis"),
            ),
            bot_cache: Cache::new(
                RedisCache::new(
                    settings.redis_uri.clone(),
                    settings.bot_redis_prefix.clone(),
                )
                .await
                .expect("Failed to connect to bot Redis namespace"),
            ),
            rest: RestClient::new(
                settings.discord_client_id.clone(),
                settings.discord_client_secret.clone(),
                settings.discord_redirect_uri.clone(),
            ),
            bot: DiscordRestClient::new(&settings.discord_bot_token),
            jwt_secret: settings.jwt_secret.clone(),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let settings = Settings::from_env()?;

    let _tracer = telemetry::init(
        SERVICE_NAME,
        &settings.otlp_endpoint,
        settings.otlp_auth.as_deref(),
        settings.otlp_organization.as_deref(),
    );

    let state = Data::new(State::new(&settings).await);

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header(),
            )
            .wrap(TracingLogger::default())
            .service(healthz)
            // Auth
            .service(auth::oauth_discord)
            .service(auth::refresh_token)
            // Guild config
            .service(api::get_config)
            .service(api::post_config)
            // Guilds
            .service(guilds::get_guilds)
            // Infractions
            .service(infractions::get_infractions)
            .service(infractions::create_infraction)
            .service(infractions::deactivate_infraction)
            .app_data(state.clone())
    })
    .bind((settings.api_host.clone(), settings.api_port))?
    .run()
    .await
}

#[get("/healthz")]
async fn healthz() -> &'static str {
    "OK"
}
