/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

static DEFAULT_API_BIND: &str = "0.0.0.0:3001";

use poem::{
    listener::TcpListener,
    middleware::{Cors, NormalizePath, TrailingSlash},
    web::Json,
    EndpointExt, IntoResponse, Route, Server,
};
use reqwest::Method;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    api::{
        middleware::{
            authentication::AuthenticationMiddleware, current_user::CurrentUserMiddleware,
        },
        routes::{auth, channels, guilds, users},
    },
    database::entities::Config,
    errors::Error,
    gateway::ConnectedUsers,
    SharedEventPublisherMap,
};

mod middleware;
mod routes;

lazy_static::lazy_static! {
    pub static ref BIND_API: String = std::env::var("API_BIND").unwrap_or_else(|_| {
        log::warn!(target: "symfonia::db", "You did not specify API_BIND environment variable. Defaulting to '{DEFAULT_API_BIND}'.");
        DEFAULT_API_BIND.to_string()
    });
}

pub async fn start_api(
    db: PgPool,
    connected_users: ConnectedUsers,
    config: Config,
) -> Result<(), Error> {
    log::info!(target: "symfonia::api::cfg", "Loading configuration");

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
        .nest("/-", routes::health::setup_routes())
        .at("/version", routes::version::setup_routes())
        .at("/ping", routes::ping::setup_routes());

    let v9_api = Route::new()
        .at("/ping", routes::ping::setup_routes())
        .at("/version", routes::version::setup_routes())
        .nest("/api/v9", routes)
        .data(db)
        .data(config)
        .data(connected_users)
        .with(NormalizePath::new(TrailingSlash::Trim))
        .with(Cors::new().allow_methods(&[
            Method::CONNECT,
            Method::DELETE,
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::PATCH,
            Method::POST,
            Method::PUT,
            Method::TRACE,
        ]))
        .catch_all_error(custom_error);

    log::info!(target: "symfonia::api", "Starting HTTP Server");

    tokio::task::spawn(async move {
        Server::new(TcpListener::bind(BIND_API.to_string()))
            .run(v9_api)
            .await
            .expect("Failed to start HTTP server");
        log::info!(target: "symfonia::api", "HTTP Server stopped");
    });

    log::info!(target: "symfonia::api", "HTTP Server listening on {}", BIND_API.as_str());
    Ok(())
}

async fn custom_error(err: poem::Error) -> impl IntoResponse {
    Json(json! ({
        "success": false,
        "message": err.to_string(),
    }))
    .with_status(err.status())
}
