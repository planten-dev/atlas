use clap::{Parser, ValueEnum};
use std::{fmt, net::IpAddr};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = DatabaseKind::Postgres)]
    pub database: DatabaseKind,

    #[arg(long, value_name = "URL")]
    pub database_url: Option<String>,

    #[arg(long, default_value = "127.0.0.1")]
    pub host: IpAddr,

    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum DatabaseKind {
    #[value(alias = "postgresql")]
    Postgres,
    Sqlite,
}

impl fmt::Display for DatabaseKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => f.write_str("postgres"),
            Self::Sqlite => f.write_str("sqlite"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_postgres_on_localhost() {
        let cli = Cli::try_parse_from(["backend"]).expect("defaults should parse");

        assert_eq!(cli.database, DatabaseKind::Postgres);
        assert_eq!(cli.database_url, None);
        assert_eq!(cli.host.to_string(), "127.0.0.1");
        assert_eq!(cli.port, 3000);
    }

    #[test]
    fn parses_sqlite_database_options() {
        let cli = Cli::try_parse_from([
            "backend",
            "--database",
            "sqlite",
            "--database-url",
            "sqlite::memory:",
            "--host",
            "0.0.0.0",
            "--port",
            "4000",
        ])
        .expect("sqlite options should parse");

        assert_eq!(cli.database, DatabaseKind::Sqlite);
        assert_eq!(cli.database_url.as_deref(), Some("sqlite::memory:"));
        assert_eq!(cli.host.to_string(), "0.0.0.0");
        assert_eq!(cli.port, 4000);
    }

    #[test]
    fn accepts_postgresql_alias() {
        let cli = Cli::try_parse_from(["backend", "--database", "postgresql"])
            .expect("postgresql alias should parse");

        assert_eq!(cli.database, DatabaseKind::Postgres);
    }
}
