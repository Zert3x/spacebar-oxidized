/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::*;

use std::ops::{Deref, DerefMut};

use chorus::types::{PermissionFlags, Snowflake};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use sqlx_pg_uint::PgU64;

use crate::{eq_shared_event_publisher, errors::Error, SharedEventPublisherMap, QUERY_UPPER_LIMIT};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Role {
    #[sqlx(flatten)]
    inner: chorus::types::RoleObject,
    pub guild_id: Snowflake,
    #[sqlx(skip)]
    #[serde(skip)]
    pub publisher: SharedEventPublisher,
}

impl PartialEq for Role {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
            && self.guild_id == other.guild_id
            && eq_shared_event_publisher(&self.publisher, &other.publisher)
    }
}

impl Deref for Role {
    type Target = chorus::types::RoleObject;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Role {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Role {
    pub async fn create(
        db: &PgPool,
        shared_event_publisher_map: SharedEventPublisherMap,
        id: Option<Snowflake>,
        guild_id: Snowflake,
        name: &str,
        color: f64,
        hoist: bool,
        managed: bool,
        mentionable: bool,
        permissions: PermissionFlags,
        position: u16,
        icon: Option<String>,
        unicode_emoji: Option<String>,
    ) -> Result<Self, Error> {
        let role = Self {
            inner: chorus::types::RoleObject {
                id: id.unwrap_or_default(),
                color,
                hoist,
                managed,
                mentionable,
                name: name.to_string(),
                permissions,
                position: position.into(),
                icon: icon.to_owned(),
                unicode_emoji: unicode_emoji.to_owned(),
                ..Default::default()
            },
            guild_id: guild_id.to_owned(),
            publisher: SharedEventPublisher::default(),
        };
        shared_event_publisher_map
            .write()
            .insert(role.id, role.publisher.clone());
        sqlx::query("INSERT INTO roles (id, guild_id, name, color, hoist, managed, mentionable, permissions, position, icon, unicode_emoji) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(role.id)
            .bind(role.guild_id)
            .bind(&role.name)
            .bind(role.color)
            .bind(role.hoist)
            .bind(role.managed)
            .bind(role.mentionable)
            .bind(&role.permissions)
            .bind(&role.position)
            .bind(&role.icon)
            .bind(&role.unicode_emoji)
            .execute(db)
            .await?;

        Ok(role)
    }

    pub async fn get_by_id(db: &PgPool, id: Snowflake) -> Result<Option<Self>, Error> {
        sqlx::query_as("SELECT * FROM roles WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(Error::Sqlx)
    }

    pub async fn get_by_guild(db: &PgPool, guild_id: Snowflake) -> Result<Vec<Self>, Error> {
        sqlx::query_as("SELECT * FROM roles WHERE guild_id = ?")
            .bind(guild_id)
            .fetch_all(db)
            .await
            .map_err(Error::Sqlx)
    }

    /// Retrieve all roles associated with a specific user.
    // TODO(bitfl0wer): Write test
    pub async fn get_by_user(db: &PgPool, user_id: Snowflake) -> Result<Vec<Self>, Error> {
        sqlx::query_as(
            "SELECT r.* FROM roles r
                JOIN member_roles mr ON r.id = mr.role_id
                WHERE mr.index IN (
                    SELECT m.index FROM members m WHERE m.id = $1
                ) LIMIT $2;",
        )
        .bind(user_id)
        .bind(QUERY_UPPER_LIMIT)
        .fetch_all(db)
        .await
        .map_err(Error::Sqlx)
    }

    /// Retrieve the role ids of all roles associated with a specific user.
    // TODO(bitfl0wer): Write test
    pub async fn get_ids_by_user(db: &PgPool, user_id: Snowflake) -> Result<Vec<Snowflake>, Error> {
        Ok(sqlx::query_as(
            "SELECT mr.role_id
                FROM member_roles mr
                JOIN members m ON mr.index = m.index
                WHERE m.id = $1 LIMIT $2;",
        )
        .bind(user_id)
        .bind(QUERY_UPPER_LIMIT)
        .fetch_all(db)
        .await
        .map_err(Error::Sqlx)?
        .iter()
        .map(|x: &PgU64| Snowflake::from(x.to_uint()))
        .collect())
    }

    pub async fn count_by_guild(db: &PgPool, guild_id: Snowflake) -> Result<i32, Error> {
        sqlx::query("SELECT COUNT(*) FROM roles WHERE guild_id = ?")
            .bind(guild_id)
            .fetch_one(db)
            .await
            .map(|res| res.get::<i32, _>(0))
            .map_err(Error::Sqlx)
    }

    pub async fn save(&self, db: &PgPool) -> Result<(), Error> {
        sqlx::query("UPDATE roles SET name = ?, color = ?, hoist = ?, managed = ?, mentionable = ?, permissions = ?, position = ?, icon = ?, unicode_emoji = ? WHERE id = ?")
            .bind(&self.name)
            .bind(self.color)
            .bind(self.hoist)
            .bind(self.managed)
            .bind(self.mentionable)
            .bind(&self.permissions)
            .bind(self.position.clone())
            .bind(&self.icon)
            .bind(&self.unicode_emoji)
            .bind(self.id)
            .execute(db)
            .await
            .map(|_| ())
            .map_err(Error::Sqlx)
    }

    pub async fn delete(&self, db: &PgPool) -> Result<(), Error> {
        sqlx::query("DELETE FROM roles WHERE id = ?")
            .bind(self.id)
            .execute(db)
            .await
            .map(|_| ())
            .map_err(Error::Sqlx)
    }

    pub fn into_inner(self) -> chorus::types::RoleObject {
        self.inner
    }
}
