use anyhow::{Context, Result, bail};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use std::{env, fmt, time::Duration};

use crate::cli::DatabaseKind;

const SQLITE_MEMORY_URL: &str = "sqlite::memory:";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DatabaseSettings {
    kind: DatabaseKind,
    url: String,
    url_source: DatabaseUrlSource,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DatabaseUrlSource {
    Argument,
    Environment,
    SqliteMemoryDefault,
}

impl fmt::Display for DatabaseUrlSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument => f.write_str("argument"),
            Self::Environment => f.write_str("environment"),
            Self::SqliteMemoryDefault => f.write_str("sqlite_memory_default"),
        }
    }
}

impl DatabaseSettings {
    pub fn from_parts(kind: DatabaseKind, database_url: Option<String>) -> Result<Self> {
        let (url, url_source) = match (kind, database_url) {
            (_, Some(url)) if url.trim().is_empty() => bail!("database URL cannot be empty"),
            (_, Some(url)) => (url, DatabaseUrlSource::Argument),
            (DatabaseKind::Postgres, None) => (
                env::var("DATABASE_URL")
                    .context("DATABASE_URL environment variable must be set")?,
                DatabaseUrlSource::Environment,
            ),
            (DatabaseKind::Sqlite, None) => (
                SQLITE_MEMORY_URL.to_string(),
                DatabaseUrlSource::SqliteMemoryDefault,
            ),
        };

        tracing::debug!(
            database = %kind,
            url_source = %url_source,
            "resolved database settings"
        );

        Ok(Self {
            kind,
            url,
            url_source,
        })
    }

    pub fn kind(&self) -> DatabaseKind {
        self.kind
    }

    pub fn url_source(&self) -> DatabaseUrlSource {
        self.url_source
    }

    fn url(&self) -> &str {
        &self.url
    }
}

#[tracing::instrument(
    name = "database.connect",
    skip(settings),
    fields(database = %settings.kind(), url_source = %settings.url_source())
)]
pub async fn connect(settings: &DatabaseSettings) -> Result<DatabaseConnection> {
    if settings.kind == DatabaseKind::Sqlite {
        tracing::warn!("using SQLite database; this mode is intended for tests");
    }

    tracing::info!("connecting database");

    let mut options = ConnectOptions::new(settings.url().to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .sqlx_logging(true);

    let db = Database::connect(options)
        .await
        .with_context(|| format!("failed to connect to {}", settings.kind()))?;

    if settings.kind == DatabaseKind::Sqlite {
        initialize_sqlite_test_schema(&db).await?;
    }

    tracing::info!("database connection ready");

    Ok(db)
}

#[tracing::instrument(name = "database.sqlite.initialize_schema", skip(db))]
async fn initialize_sqlite_test_schema(db: &DatabaseConnection) -> Result<()> {
    tracing::debug!("initializing SQLite users test schema");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username VARCHAR(64) NOT NULL UNIQUE,
    phone VARCHAR(20) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
)
"#
        .to_string(),
    ))
    .await
    .context("failed to initialize SQLite users test schema")?;

    tracing::info!("SQLite users test schema ready");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dto::users::{CreateUserRequest, LoginUserRequest},
        services::users::UserService,
    };

    #[test]
    fn sqlite_defaults_to_in_memory_database() {
        let settings = DatabaseSettings::from_parts(DatabaseKind::Sqlite, None)
            .expect("sqlite defaults should resolve");

        assert_eq!(settings.kind(), DatabaseKind::Sqlite);
        assert_eq!(settings.url(), SQLITE_MEMORY_URL);
        assert_eq!(
            settings.url_source(),
            DatabaseUrlSource::SqliteMemoryDefault
        );
    }

    #[test]
    fn explicit_database_url_is_used() {
        let settings =
            DatabaseSettings::from_parts(DatabaseKind::Postgres, Some("postgres://db".to_string()))
                .expect("explicit url should resolve");

        assert_eq!(settings.kind(), DatabaseKind::Postgres);
        assert_eq!(settings.url(), "postgres://db");
        assert_eq!(settings.url_source(), DatabaseUrlSource::Argument);
    }

    #[test]
    fn empty_database_url_is_rejected() {
        let err = DatabaseSettings::from_parts(DatabaseKind::Sqlite, Some(" ".to_string()))
            .expect_err("empty url should be rejected");

        assert!(err.to_string().contains("database URL cannot be empty"));
    }

    #[tokio::test]
    async fn sqlite_test_database_initializes_users_schema() {
        let settings = DatabaseSettings::from_parts(DatabaseKind::Sqlite, None)
            .expect("sqlite defaults should resolve");
        let db = connect(&settings)
            .await
            .expect("sqlite database should connect");

        let created = UserService::create_user(
            &db,
            CreateUserRequest {
                username: "alice".to_string(),
                phone: "13800138000".to_string(),
                password: "secret-password".to_string(),
            },
        )
        .await
        .expect("user should be created in sqlite");

        let fetched = UserService::get_user(&db, created.id)
            .await
            .expect("user should be fetched from sqlite");
        assert_eq!(fetched.username, "alice");

        let logged_in = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: "13800138000".to_string(),
                password: "secret-password".to_string(),
            },
        )
        .await
        .expect("user should login against sqlite");
        assert_eq!(logged_in.id, created.id);
    }
}
