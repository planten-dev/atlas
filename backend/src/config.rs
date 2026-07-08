use std::path::{Path, PathBuf};

use ::config::{Config, ConfigError, Environment, File};
use clap::{Parser, ValueEnum};
use serde::Deserialize;

#[derive(Debug, Clone, Parser, Default)]
#[command(author, version, about)]
pub struct CliArgs {
    #[arg(long = "database-kind", value_enum)]
    pub database_kind: Option<DatabaseKind>,

    #[arg(long = "database-url")]
    pub database_url: Option<String>,

    #[arg(long = "sqlite-file")]
    pub sqlite_file: Option<PathBuf>,

    #[arg(long = "config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub dingtalk: DingTalkConfig,
    pub auth: AuthConfig,
    pub session: SessionConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub events: EventsConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind_addr: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub kind: DatabaseKind,
    pub url: String,
    pub sqlite_file: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DingTalkConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub corp_token_url: String,
    pub department_listsub_url: String,
    pub user_detail_url: String,
    pub getbyunionid_url: String,
    pub scope: String,
    pub corp_id: String,
    pub external_id_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AuthConfig {
    pub frontend_callback_url: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    pub ttl_seconds: u64,
    pub cookie_secure: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LoggingConfig {
    pub directory: PathBuf,
    pub file_prefix: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("logs"),
            file_prefix: "atlas.log".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct EventsConfig {
    pub retention_days: u32,
    pub sweep_interval_seconds: u64,
}

impl Default for EventsConfig {
    fn default() -> Self {
        Self {
            retention_days: 180,
            sweep_interval_seconds: 86_400,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseKind {
    Postgres,
    SqliteMemory,
    SqliteFile,
}

impl DatabaseKind {
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::SqliteMemory => "sqlite-memory",
            Self::SqliteFile => "sqlite-file",
        }
    }
}

pub fn load() -> Result<AppConfig, ConfigError> {
    load_from_cli(CliArgs::parse())
}

pub fn load_from_cli(cli: CliArgs) -> Result<AppConfig, ConfigError> {
    load_from_sources(&default_config_dir(), cli)
}

fn load_from_sources(config_dir: &Path, cli: CliArgs) -> Result<AppConfig, ConfigError> {
    let mut builder = Config::builder()
        .add_source(File::from(config_dir.join("default.toml")).required(true))
        .add_source(File::from(config_dir.join("local.toml")).required(false));

    if let Some(path) = cli.config.as_ref() {
        builder = builder.add_source(File::from(path.clone()).required(true));
    }

    builder = builder.add_source(
        Environment::with_prefix("ATLAS")
            .separator("__")
            .try_parsing(true),
    );

    if let Some(kind) = cli.database_kind {
        builder = builder.set_override("database.kind", kind.as_config_value())?;
    }

    if let Some(url) = cli.database_url {
        builder = builder.set_override("database.url", url)?;
    }

    if let Some(sqlite_file) = cli.sqlite_file {
        builder =
            builder.set_override("database.sqlite_file", sqlite_file.display().to_string())?;
    }

    builder.build()?.try_deserialize()
}

fn default_config_dir() -> PathBuf {
    PathBuf::from("config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn write_config_dir(default_toml: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("atlas-config-test-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir).expect("test config directory should be created");
        fs::write(dir.join("default.toml"), default_toml)
            .expect("default config should be written");
        dir
    }

    fn minimal_default_config() -> &'static str {
        r#"
[server]
bind_addr = "127.0.0.1:3000"

[database]
kind = "postgres"
url = "postgres://postgres:postgres@localhost:5432/atlas"
sqlite_file = "data/atlas.sqlite"

[dingtalk]
client_id = ""
client_secret = ""
redirect_uri = "http://127.0.0.1:3000/api/v1/auth/callback/dingtalk"
auth_url = "https://login.dingtalk.com/oauth2/auth"
token_url = "https://api.dingtalk.com/v1.0/oauth2/userAccessToken"
user_info_url = "https://api.dingtalk.com/v1.0/contact/users/me"
corp_token_url = "https://oapi.dingtalk.com/gettoken"
department_listsub_url = "https://oapi.dingtalk.com/topapi/v2/department/listsub"
user_detail_url = "https://oapi.dingtalk.com/topapi/v2/user/get"
getbyunionid_url = "https://oapi.dingtalk.com/topapi/user/getbyunionid"
scope = "openid corpid"
corp_id = ""
external_id_fields = ["userId", "userid", "user_id"]

[auth]
frontend_callback_url = ""

[session]
ttl_seconds = 86400
cookie_secure = false
"#
    }

    #[test]
    fn default_config_dir_uses_runtime_working_directory() {
        assert_eq!(default_config_dir(), PathBuf::from("config"));
    }

    #[test]
    fn loads_default_values() {
        let _guard = env_lock().lock().expect("env lock should be available");
        clear_test_env();
        let dir = write_config_dir(minimal_default_config());

        let config = load_from_sources(&dir, CliArgs::default()).expect("config should load");

        assert_eq!(config.server.bind_addr, "127.0.0.1:3000");
        assert_eq!(config.database.kind, DatabaseKind::Postgres);
        assert_eq!(
            config.database.url,
            "postgres://postgres:postgres@localhost:5432/atlas"
        );
        assert_eq!(config.session.ttl_seconds, 86_400);
        assert!(!config.session.cookie_secure);
        assert_eq!(config.logging.directory, PathBuf::from("logs"));
        assert_eq!(config.logging.file_prefix, "atlas.log");
        assert_eq!(config.events.retention_days, 180);
        assert_eq!(config.events.sweep_interval_seconds, 86_400);
    }

    #[test]
    fn environment_overrides_default_config() {
        let _guard = env_lock().lock().expect("env lock should be available");
        clear_test_env();
        let dir = write_config_dir(minimal_default_config());

        set_test_env("ATLAS__DATABASE__KIND", "sqlite-memory");
        set_test_env("ATLAS__SESSION__TTL_SECONDS", "60");
        set_test_env("ATLAS__EVENTS__RETENTION_DAYS", "30");
        set_test_env("ATLAS__LOGGING__DIRECTORY", "custom-logs");
        set_test_env("ATLAS__LOGGING__FILE_PREFIX", "custom.log");
        let config = load_from_sources(&dir, CliArgs::default()).expect("config should load");
        clear_test_env();

        assert_eq!(config.database.kind, DatabaseKind::SqliteMemory);
        assert_eq!(config.session.ttl_seconds, 60);
        assert_eq!(config.events.retention_days, 30);
        assert_eq!(config.logging.directory, PathBuf::from("custom-logs"));
        assert_eq!(config.logging.file_prefix, "custom.log");
    }

    #[test]
    fn command_line_overrides_environment_and_default_config() {
        let _guard = env_lock().lock().expect("env lock should be available");
        clear_test_env();
        let dir = write_config_dir(minimal_default_config());
        set_test_env("ATLAS__DATABASE__KIND", "sqlite-memory");

        let config = load_from_sources(
            &dir,
            CliArgs {
                database_kind: Some(DatabaseKind::SqliteFile),
                database_url: Some("postgres://override/atlas".to_string()),
                sqlite_file: Some(PathBuf::from("tmp/test.sqlite")),
                config: None,
            },
        )
        .expect("config should load");
        clear_test_env();

        assert_eq!(config.database.kind, DatabaseKind::SqliteFile);
        assert_eq!(config.database.url, "postgres://override/atlas");
        assert_eq!(
            config.database.sqlite_file,
            PathBuf::from("tmp/test.sqlite")
        );
    }

    #[test]
    fn extra_config_file_overrides_local_files_before_environment() {
        let _guard = env_lock().lock().expect("env lock should be available");
        clear_test_env();
        let dir = write_config_dir(minimal_default_config());
        let extra = dir.join("custom.toml");
        fs::write(
            &extra,
            r#"
[database]
kind = "sqlite-file"
sqlite_file = "custom.sqlite"

[logging]
directory = "custom-logs"
file_prefix = "custom.log"
"#,
        )
        .expect("custom config should be written");

        let config = load_from_sources(
            &dir,
            CliArgs {
                config: Some(extra),
                ..CliArgs::default()
            },
        )
        .expect("config should load");

        assert_eq!(config.database.kind, DatabaseKind::SqliteFile);
        assert_eq!(config.database.sqlite_file, PathBuf::from("custom.sqlite"));
        assert_eq!(config.logging.directory, PathBuf::from("custom-logs"));
        assert_eq!(config.logging.file_prefix, "custom.log");
    }

    fn set_test_env(key: &str, value: &str) {
        // SAFETY: Tests that mutate process environment hold a shared mutex so
        // this crate does not read or write the same variables concurrently.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn clear_test_env() {
        for key in [
            "ATLAS__DATABASE__KIND",
            "ATLAS__DATABASE__URL",
            "ATLAS__DATABASE__SQLITE_FILE",
            "ATLAS__SESSION__TTL_SECONDS",
            "ATLAS__SESSION__COOKIE_SECURE",
            "ATLAS__DINGTALK__USER_DETAIL_URL",
            "ATLAS__DINGTALK__GETBYUNIONID_URL",
            "ATLAS__EVENTS__RETENTION_DAYS",
            "ATLAS__LOGGING__DIRECTORY",
            "ATLAS__LOGGING__FILE_PREFIX",
        ] {
            // SAFETY: Tests that mutate process environment hold a shared mutex
            // so this crate does not read or write the same variables concurrently.
            unsafe {
                std::env::remove_var(key);
            }
        }
    }
}
