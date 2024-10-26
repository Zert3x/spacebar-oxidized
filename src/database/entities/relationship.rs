use std::ops::{Deref, DerefMut};

use bigdecimal::BigDecimal;
use chorus::types::{PublicUser, Snowflake};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{errors::Error, QUERY_UPPER_LIMIT};

use super::*;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    #[sqlx(flatten)]
    pub(crate) inner: chorus::types::Relationship,
    pub from_id: Snowflake,
    #[sqlx(skip)]
    pub user: PublicUser,
}

impl Deref for Relationship {
    type Target = chorus::types::Relationship;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Relationship {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// TODO: Lots of missing methods
impl Relationship {
    pub fn into_inner(self) -> chorus::types::Relationship {
        self.inner
    }

    pub fn as_inner(&self) -> &chorus::types::Relationship {
        &self.inner
    }

    pub fn as_inner_mut(&mut self) -> &mut chorus::types::Relationship {
        &mut self.inner
    }

    /// Retrieve all relationships for a user by their ID
    pub async fn get_by_from_id(from_id: Snowflake, db: &PgPool) -> Result<Vec<Self>, Error> {
        sqlx::query_as("SELECT * from relationships WHERE from_id = $1 LIMIT $2")
            .bind(from_id)
            .bind(QUERY_UPPER_LIMIT)
            .fetch_all(db)
            .await
            .map_err(Error::from)
    }

    /// Retrieve all relationships where the specified user is the target
    pub async fn get_by_to_id(to_id: Snowflake, db: &PgPool) -> Result<Vec<Self>, Error> {
        sqlx::query_as("SELECT * from relationships WHERE to_id = $1 LIMIT $2")
            .bind(to_id)
            .bind(QUERY_UPPER_LIMIT)
            .fetch_all(db)
            .await
            .map_err(Error::from)
    }

    /// Retrieve all relationships for a user by their ID, regardless of whether they are the source
    /// or target of the relationship
    pub async fn get_all_by_id(id: Snowflake, db: &PgPool) -> Result<Vec<Self>, Error> {
        sqlx::query_as("SELECT * from relationships WHERE from_id = $1 OR to_id = $1 LIMIT $2")
            .bind(id)
            .bind(QUERY_UPPER_LIMIT)
            .fetch_all(db)
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod relationship_unit_tests {
    use bigdecimal::BigDecimal;
    use chorus::types::RelationshipType;
    use sqlx::PgPool;
    use sqlx_pg_uint::PgU8;

    #[sqlx::test(fixtures(path = "../../../fixtures", scripts("users")))]
    async fn get_by_from_id(pool: PgPool) {
        sqlx::query!(
            "INSERT INTO relationships(from_id, to_id, nickname, type) VALUES($1, $2, $3, $4);",
            BigDecimal::from(7248639845155737600_u64),
            BigDecimal::from(7248639891561517057_u64),
            "Janana Banana 🍌",
            BigDecimal::from(RelationshipType::Outgoing as u8)
        )
        .execute(&pool)
        .await
        .unwrap();

        let relationships = super::Relationship::get_by_from_id(7248639845155737600.into(), &pool)
            .await
            .unwrap();

        assert_eq!(relationships.len(), 1);
        let relationship = &relationships[0];
        assert_eq!(relationship.from_id, 7248639845155737600.into());
        assert_eq!(relationship.id, 7248639891561517057.into());
        assert_eq!(relationship.nickname, Some("Janana Banana 🍌".to_string()));
        assert_eq!(relationship.relationship_type, RelationshipType::Outgoing);
    }

    #[sqlx::test(fixtures(path = "../../../fixtures", scripts("users")))]
    async fn get_by_to_id(pool: PgPool) {
        sqlx::query!(
            "INSERT INTO relationships(from_id, to_id, nickname, type) VALUES($1, $2, $3, $4);",
            BigDecimal::from(7248639845155737600_u64),
            BigDecimal::from(7248639891561517057_u64),
            "Janana Banana 🍌",
            BigDecimal::from(RelationshipType::Outgoing as u8)
        )
        .execute(&pool)
        .await
        .unwrap();

        let relationships = super::Relationship::get_by_to_id(7248639891561517057.into(), &pool)
            .await
            .unwrap();

        assert_eq!(relationships.len(), 1);
        let relationship = &relationships[0];
        assert_eq!(relationship.from_id, 7248639845155737600.into());
        assert_eq!(relationship.id, 7248639891561517057.into());
        assert_eq!(relationship.nickname, Some("Janana Banana 🍌".to_string()));
        assert_eq!(relationship.relationship_type, RelationshipType::Outgoing);
    }

    #[sqlx::test(fixtures(path = "../../../fixtures", scripts("users")))]
    async fn get_all_by_id(pool: PgPool) {
        sqlx::query!(
            "INSERT INTO relationships(from_id, to_id, nickname, type) VALUES($1, $2, $3, $4);",
            BigDecimal::from(7248639845155737600_u64),
            BigDecimal::from(7248639891561517057_u64),
            "Janana Banana 🍌",
            BigDecimal::from(RelationshipType::Outgoing as u8)
        )
        .execute(&pool)
        .await
        .unwrap();

        let relationships = super::Relationship::get_all_by_id(7248639845155737600.into(), &pool)
            .await
            .unwrap();

        assert_eq!(relationships.len(), 1);
        let relationship = &relationships[0];
        assert_eq!(relationship.from_id, 7248639845155737600.into());
        assert_eq!(relationship.id, 7248639891561517057.into());
        assert_eq!(relationship.nickname, Some("Janana Banana 🍌".to_string()));
        assert_eq!(relationship.relationship_type, RelationshipType::Outgoing);

        sqlx::query!(
            "INSERT INTO relationships(from_id, to_id, nickname, type) VALUES($1, $2, $3, $4);",
            BigDecimal::from(7248639845155737600_u64),
            BigDecimal::from(7248640296244744192_u64),
            "Banana Janana 🍌",
            BigDecimal::from(RelationshipType::Incoming as u8)
        )
        .execute(&pool)
        .await
        .unwrap();

        let relationships = super::Relationship::get_all_by_id(7248639845155737600.into(), &pool)
            .await
            .unwrap();

        assert_eq!(relationships.len(), 2);
        let relationship = &relationships[1];
        assert_eq!(relationship.from_id, 7248639845155737600.into());
        assert_eq!(relationship.id, 7248640296244744192.into());
        assert_eq!(relationship.nickname, Some("Banana Janana 🍌".to_string()));
        assert_eq!(relationship.relationship_type, RelationshipType::Incoming);
    }
}
