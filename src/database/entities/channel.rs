use crate::{database::Queryer, errors::Error};
use chorus::types::{ChannelType, Snowflake};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Channel {
    #[sqlx(flatten)]
    pub(crate) inner: chorus::types::Channel,
}

impl Deref for Channel {
    type Target = chorus::types::Channel;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Channel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Channel {
    pub async fn create<'c, C: Queryer<'c>>(
        db: C,
        channel_type: ChannelType,
        name: Option<String>,
        nsfw: bool,
        guild_id: Option<Snowflake>,
        parent_id: Option<Snowflake>,
        exists_check: bool,
        permission_check: bool,
        event_emit: bool,
        name_checks: bool,
    ) -> Result<Self, Error> {
        if permission_check {
            todo!()
        }

        if name_checks {
            todo!()
        }

        match channel_type {
            ChannelType::GuildText | ChannelType::GuildNews | ChannelType::GuildVoice => {
                if parent_id.is_some() && exists_check {
                    todo!()
                }
            }
            ChannelType::Dm | ChannelType::GroupDm => {
                todo!() // TODO: No dms in a guild!
            }
            ChannelType::GuildCategory | ChannelType::Unhandled => {}
            ChannelType::GuildStore => {}
            _ => {}
        }

        // TODO: permission overwrites

        let channel = Self {
            inner: chorus::types::Channel {
                channel_type,
                name,
                nsfw: Some(nsfw),
                guild_id,
                ..Default::default()
            },
        };

        sqlx::query("INSERT INTO channels (id, type, name, nsfw, guild_id) VALUES (?, ?, ?, ?, ?)")
            .bind(&channel.id)
            .bind(channel.channel_type)
            .bind(&channel.name)
            .bind(&channel.nsfw)
            .bind(&channel.guild_id)
            .execute(db)
            .await?;

        Ok(channel)
    }

    pub async fn get_by_id<'c, C: Queryer<'c>>(
        db: C,
        id: &Snowflake,
    ) -> Result<Option<Self>, Error> {
        sqlx::query_as("SELECT * FROM channels WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(Error::SQLX)
    }
}
