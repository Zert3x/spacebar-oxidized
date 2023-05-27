use crate::database::entities::Config;
use chorus::types::APIError;
use poem::{
    handler,
    web::{Data, Json},
    IntoResponse,
};
use serde_json::json;

#[handler]
pub async fn domain(
    Data(db): Data<&sqlx::MySqlPool>,
    Data(cfg): Data<&Config>,
) -> Result<impl IntoResponse, APIError> {
    let cdn = if let Some(endpoint) = &cfg.cdn.endpoint_public {
        endpoint.to_owned()
    } else if let Ok(endpoint) = std::env::var("CDN") {
        endpoint
    } else {
        "http://localhost:3001".to_string()
    };

    let gateway = if let Some(endpoint) = &cfg.gateway.endpoint_public {
        endpoint.to_owned()
    } else if let Ok(endpoint) = std::env::var("GATEWAY") {
        endpoint
    } else {
        "ws://localhost:3001".to_string()
    };

    let api = if let Some(endpoint) = &cfg.api.endpoint_public {
        endpoint.to_owned()
    } else {
        "http://localhost:3001/api".to_string()
    };

    Ok(Json(json!({
        "cdn": cdn,
        "gateway": gateway,
        "defaultApiVersion": cfg.api.default_version,
        "apiEndpoint": api
    })))
}
