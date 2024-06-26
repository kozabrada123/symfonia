use std::ops::{Deref, DerefMut};

use chorus::types::{AuditLogActionType, Snowflake};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};

use crate::errors::Error;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    #[serde(flatten)]
    #[sqlx(flatten)]
    inner: chorus::types::AuditLogEntry,
    pub guild_id: Snowflake,
}

impl Deref for AuditLogEntry {
    type Target = chorus::types::AuditLogEntry;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AuditLogEntry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AuditLogEntry {
    pub async fn create(db: &MySqlPool) -> Result<Self, Error> {
        todo!()
    }

    pub async fn get_by_id(db: &MySqlPool, id: Snowflake) -> Result<Option<Self>, Error> {
        sqlx::query_as("SELECT * FROM audit_logs WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(Error::from)
    }

    pub async fn get_by_guild(
        db: &MySqlPool,
        guild_id: Snowflake,
        before: Option<Snowflake>,
        after: Option<Snowflake>,
        limit: u8,
        user_id: Option<Snowflake>,
        action_type: Option<AuditLogActionType>,
    ) -> Result<Vec<Self>, Error> {
        let mut builder = sqlx::QueryBuilder::new("SELECT * FROM audit_logs WHERE guild_id = ? ");

        if let Some(before) = before {
            builder.push("AND id < ");
            builder.push_bind(before);
            builder.push(" ");
        }

        if let Some(after) = after {
            builder.push("AND id > ");
            builder.push_bind(after);
            builder.push(" ");
        }

        if let Some(user_id) = user_id {
            builder.push("AND user_id = ");
            builder.push_bind(user_id);
            builder.push(" ");
        }

        if let Some(action_type) = action_type {
            builder.push("AND action_type = ");
            builder.push_bind(action_type);
            builder.push(" ");
        }

        builder.push("LIMIT ");
        builder.push_bind(limit);

        let query = builder.build();

        let r = query
            .bind(guild_id)
            .fetch_all(db)
            .await
            .map_err(Error::SQLX)?;

        Ok(r.into_iter()
            .map(|r| AuditLogEntry::from_row(&r))
            .flatten()
            .collect::<Vec<_>>())
    }

    pub fn into_inner(self) -> chorus::types::AuditLogEntry {
        self.inner
    }
}
