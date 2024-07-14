use poem::{
    EndpointExt,
    http::Method,
    IntoResponse,
    listener::TcpListener,
    middleware::{Cors, NormalizePath, TrailingSlash}, Route, Server, web::Json,
};
use serde_json::json;
use sqlx::MySqlPool;

use crate::{
    api::{
        middleware::{
            authentication::AuthenticationMiddleware, current_user::CurrentUserMiddleware,
        },
        routes::{auth, channels, guilds, users},
    },
    database::entities::Config,
    errors::Error,
};

mod middleware;
mod routes;

pub async fn start_api(db: MySqlPool) -> Result<(), Error> {
    log::info!(target: "symfonia::api::cfg", "Loading configuration");
    let config = Config::init(&db).await?;

    if config.sentry.enabled {
        let _guard = sentry::init((
            "https://241c6fb08adb469da1bb82522b25c99f@sentry.quartzinc.space/3",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: config.sentry.trace_sample_rate as f32,
                ..Default::default()
            },
        ));
    }

    let routes = Route::new()
        .nest("/auth", auth::setup_routes())
        .nest(
            "/users",
            users::setup_routes()
                .with(AuthenticationMiddleware)
                .with(CurrentUserMiddleware),
        )
        .nest(
            "/guilds",
            guilds::setup_routes()
                .with(AuthenticationMiddleware)
                .with(CurrentUserMiddleware),
        )
        .nest(
            "/channels",
            channels::setup_routes()
                .with(AuthenticationMiddleware)
                .with(CurrentUserMiddleware),
        )
        .nest(
            "/invites",
            routes::invites::setup_routes()
                .with(AuthenticationMiddleware)
                .with(CurrentUserMiddleware),
        )
        .nest("/policies", routes::policies::setup_routes())
        .nest("/-", routes::health::setup_routes());

    let v9_api = Route::new()
        // .nest("/api/v9", &routes)
        .nest("/api", routes)
        .data(db)
        .data(config)
        .with(NormalizePath::new(TrailingSlash::Trim))
        .with(Cors::new().allow_methods(&[
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ]))
        .catch_all_error(custom_error);

    let bind = std::env::var("API_BIND").unwrap_or_else(|_| String::from("localhost:3001"));

    log::info!(target: "symfonia::api", "Starting HTTP Server");
    Server::new(TcpListener::bind(bind)).run(v9_api).await?;
    Ok(())
}

async fn custom_error(err: poem::Error) -> impl IntoResponse {
    Json(json! ({
        "success": false,
        "message": err.to_string(),
    }))
    .with_status(err.status())
}
