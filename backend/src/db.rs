use std::path::Path;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr,
    Statement,
};
use sea_orm_migration::MigratorTrait;
use tracing::{debug, error, info};

use crate::{
    config::{DatabaseConfig, DatabaseKind},
    migration::Migrator,
};

pub async fn connect_and_migrate(config: &DatabaseConfig) -> Result<DatabaseConnection, DbErr> {
    info!(
        database_kind = %config.kind.as_config_value(),
        "initializing database connection"
    );
    let db = connect(config).await?;
    if let Err(error) = Migrator::up(&db, None).await {
        error!(
            database_kind = %config.kind.as_config_value(),
            %error,
            "database migration failed"
        );
        return Err(error);
    }
    info!("database migrations are up to date");
    Ok(db)
}

pub async fn connect(config: &DatabaseConfig) -> Result<DatabaseConnection, DbErr> {
    if let Err(error) = ensure_sqlite_file_parent(config) {
        error!(
            database_kind = %config.kind.as_config_value(),
            %error,
            "failed to prepare database storage"
        );
        return Err(error);
    }
    let mut options = ConnectOptions::new(database_url(config));
    options.sqlx_logging(false);
    let db = match Database::connect(options).await {
        Ok(db) => db,
        Err(error) => {
            error!(
                database_kind = %config.kind.as_config_value(),
                %error,
                "database connection failed"
            );
            return Err(error);
        }
    };
    if let Err(error) = enable_sqlite_foreign_keys(config, &db).await {
        error!(
            database_kind = %config.kind.as_config_value(),
            %error,
            "failed to enable sqlite foreign keys"
        );
        return Err(error);
    }
    debug!("database connection established");
    Ok(db)
}

pub fn database_url(config: &DatabaseConfig) -> String {
    match config.kind {
        DatabaseKind::Postgres => config.url.clone(),
        DatabaseKind::SqliteMemory => "sqlite::memory:".to_string(),
        DatabaseKind::SqliteFile => {
            let sqlite_file = normalize_sqlite_path(&config.sqlite_file);
            format!("sqlite://{sqlite_file}?mode=rwc")
        }
    }
}

fn ensure_sqlite_file_parent(config: &DatabaseConfig) -> Result<(), DbErr> {
    if config.kind != DatabaseKind::SqliteFile {
        return Ok(());
    }

    if let Some(parent) = config
        .sqlite_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            DbErr::Custom(format!(
                "failed to create sqlite database directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }

    Ok(())
}

fn normalize_sqlite_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

async fn enable_sqlite_foreign_keys(
    config: &DatabaseConfig,
    db: &DatabaseConnection,
) -> Result<(), DbErr> {
    if matches!(
        config.kind,
        DatabaseKind::SqliteMemory | DatabaseKind::SqliteFile
    ) {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = ON".to_string(),
        ))
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    #[tokio::test]
    async fn builds_database_urls_from_config() {
        let postgres = DatabaseConfig {
            kind: DatabaseKind::Postgres,
            url: "postgres://postgres:postgres@localhost:5432/atlas".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        };
        assert_eq!(
            database_url(&postgres),
            "postgres://postgres:postgres@localhost:5432/atlas"
        );

        let memory = sqlite_memory_config();
        assert_eq!(database_url(&memory), "sqlite::memory:");

        let file = DatabaseConfig {
            kind: DatabaseKind::SqliteFile,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("data\\atlas.sqlite"),
        };
        assert_eq!(database_url(&file), "sqlite://data/atlas.sqlite?mode=rwc");
    }

    #[tokio::test]
    async fn connects_to_sqlite_memory() {
        let db = connect(&sqlite_memory_config())
            .await
            .expect("sqlite memory connection should be created");

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT 1".to_string(),
        ))
        .await
        .expect("sqlite memory connection should run a basic query");
    }

    #[tokio::test]
    async fn runs_migrations_on_sqlite_memory() {
        let db = connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("migrations should run on sqlite memory");

        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('users', 'auth_sessions', 'oauth_login_states', 'user_profiles') ORDER BY name".to_string(),
            ))
            .await
            .expect("sqlite schema should be queryable");

        assert_eq!(rows.len(), 4);
    }

    #[tokio::test]
    async fn users_dingtalk_user_id_unique_constraint_is_enforced() {
        let db = connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("migrations should run on sqlite memory");
        let sql = r#"
            INSERT INTO users (
                id,
                dingtalk_user_id,
                status,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?)
        "#;

        insert_user(&db, sql, "00000000-0000-0000-0000-000000000001")
            .await
            .expect("first user insert should succeed");

        let duplicate = insert_user(&db, sql, "00000000-0000-0000-0000-000000000002").await;

        assert!(
            duplicate.is_err(),
            "duplicate dingtalk_user_id should violate unique constraint"
        );
    }

    async fn insert_user(
        db: &DatabaseConnection,
        sql: &str,
        id: &str,
    ) -> Result<sea_orm::ExecResult, DbErr> {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            vec![
                Value::String(Some(Box::new(id.to_string()))),
                Value::String(Some(Box::new("ding-user-1".to_string()))),
                Value::String(Some(Box::new("active".to_string()))),
                Value::String(Some(Box::new("2026-07-07T00:00:00Z".to_string()))),
                Value::String(Some(Box::new("2026-07-07T00:00:00Z".to_string()))),
            ],
        ))
        .await
    }
}
