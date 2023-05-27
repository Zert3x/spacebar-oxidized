use crate::{database::Queryer, errors::Error};
use chorus::types::Snowflake;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Emoji {
    #[sqlx(flatten)]
    inner: chorus::types::Emoji,
    pub guild_id: Snowflake,
    pub user_id: Option<Snowflake>,
}

impl Deref for Emoji {
    type Target = chorus::types::Emoji;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Emoji {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Emoji {
    pub async fn get_by_id<'c, C: Queryer<'c>>(
        db: C,
        id: &Snowflake,
    ) -> Result<Option<Self>, Error> {
        sqlx::query_as("SELECT * FROM emojis WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(Error::SQLX)
    }
}
