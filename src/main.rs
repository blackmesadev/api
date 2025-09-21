mod api;
mod auth;
mod cache;
mod discord;
mod error;
mod jwt;
mod permissions;
mod telemetry;

use crate::telemetry::TracingMiddleware;
use actix_web::{get, web::Data, App, HttpServer};
use bm_lib::{
    cache::{Cache, RedisCache},
    db::Database,
};
use discord::RestClient;

const SERVICE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

pub struct State {
    pub db: Database,
    pub rest: RestClient,
    pub cache: Cache<RedisCache>,
}

impl State {
    pub async fn new() -> Self {
        let database_url = std::env::var("MONGO_URI").expect("MONGO_URI must be set");
        let redis_uri = std::env::var("REDIS_URI").expect("REDIS_URI must be set");

        Self {
            db: Database::connect(database_url, "black-mesa")
                .await
                .expect("Failed to connect to database"),
            rest: RestClient::new(),
            cache: Cache::new(
                RedisCache::new(redis_uri)
                    .await
                    .expect("Failed to connect to Redis"),
            ),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let openobserve_endpoint = std::env::var("OPENOBSERVE_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:5080/api/mesa-api/v1/traces".to_string());

    let openobserve_email =
        std::env::var("OPENOBSERVE_EMAIL").expect("OPENOBSERVE_EMAIL not found");
    let openobserve_password =
        std::env::var("OPENOBSERVE_PASSWORD").expect("OPENOBSERVE_PASSWORD not found");

    telemetry::init_telemetry(
        &openobserve_endpoint,
        &openobserve_email,
        &openobserve_password,
    );

    let state = Data::new(State::new().await);

    HttpServer::new(move || {
        App::new()
            .wrap(TracingMiddleware)
            .service(healthz)
            .service(api::post_config)
            .service(api::get_config)
            .service(auth::oauth_discord)
            .service(auth::refresh_token)
            .app_data(state.clone())
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[get("/healthz")]
async fn healthz() -> &'static str {
    "OK"
}
