/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::HashSet;

use chorus::types::{jwt::generate_token, APIError, AuthError, RegisterSchema};
use poem::{
    handler,
    web::{Data, Json},
    IntoResponse, Request,
};
use serde_json::json;

use crate::{
    database::entities::{Config, Role, User},
    gateway::ConnectedUsers,
};

#[handler]
pub async fn register(
    Data(db): Data<&sqlx::PgPool>,
    Data(cfg): Data<&Config>,
    Data(connected_users): Data<&ConnectedUsers>,
    Json(payload): Json<RegisterSchema>,
    req: &Request,
) -> Result<impl IntoResponse, APIError> {
    // TODO: Check reg token

    if !payload.consent {
        // TODO: Fail consent
    }

    // TODO: Check registration disabled
    if let Some(exists) =
        User::get_user_by_email_or_phone(db, payload.email.as_ref().unwrap().as_str(), "")
            .await
            .expect("Failed to check if user exists")
    {
        return Err(APIError::Auth(AuthError::InvalidLogin)); // TODO: Change error
    }

    // TODO: All field checks

    let user = User::create(
        db,
        cfg,
        &payload.username,
        payload.password,
        payload.email,
        payload.fingerprint,
        payload.date_of_birth,
        false,
    )
    .await
    .expect("Failed to create user");

    let db = db.clone();
    let connected_users = connected_users.clone();
    let id = user.id;
    tokio::spawn(async move {
        let mut role_user_map = connected_users.role_user_map.lock().await;
        let role_ids = Role::get_ids_by_user(&db, id)
            .await
            .expect("Please report this error to the symfonia developers");
        // Add the user to each role they are in
        for role_id in role_ids.iter() {
            // Check if the role exists in the map
            let maybe_role_entry = role_user_map.get_mut(role_id);
            match maybe_role_entry {
                // Role exists - add user
                Some(role_entry) => {
                    role_entry.insert(id);
                }
                // Role doesn't exist - create role and add user to it
                None => {
                    role_user_map.insert(*role_id, HashSet::new());
                    role_user_map.get_mut(role_id).unwrap().insert(id);
                }
            };
        }
    });

    // TODO: Invite

    let token = generate_token(
        &user.id,
        user.email.clone().unwrap().as_str(),
        &cfg.security.jwt_secret,
    );

    Ok(Json(json!({"token": token})))
}
