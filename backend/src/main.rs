mod app_state;
mod cli;
mod database;
mod dto;
mod entities;
mod errors;
mod handlers;
mod repositories;
mod services;

use anyhow::Context;
use app_state::AppState;
use axum::{Router, routing::get};
use clap::Parser;
use cli::Cli;
use std::net::SocketAddr;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let database_settings = database::DatabaseSettings::from_parts(cli.database, cli.database_url)
        .context("failed to resolve database settings")?;

    tracing::info!(
        database = %database_settings.kind(),
        database_url_source = %database_settings.url_source(),
        host = %cli.host,
        port = cli.port,
        "starting backend"
    );

    let db = database::connect(&database_settings)
        .await
        .context("failed to initialize database connection")?;
    let state = AppState::new(db);
    tracing::debug!("application state initialized");

    let app = Router::new()
        .route("/health", get(handlers::health))
        .merge(handlers::user_routes())
        .with_state(state);
    tracing::debug!("router initialized");

    let addr = SocketAddr::new(cli.host, cli.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind server address {addr}"))?;
    tracing::info!(address = %addr, "server listening");

    axum::serve(listener, app)
        .await
        .context("server exited unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("backend=debug,sea_orm=info,sqlx=warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}
