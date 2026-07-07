use anyhow::{Context, Result};
use backend::{
    app, config, db,
    repositories::{
        authz::AuthzRepository, events::EventRepository, products::ProductRepository,
        sessions::SessionRepository, users::UserRepository,
    },
    services::{
        auth::AuthService, authz::AuthzService, events::EventService, products::ProductService,
        review::ApplierRegistry, users::UserService,
    },
    state::AppState,
};
use chrono::Utc;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = config::load().context("failed to load application config")?;
    info!(
        bind_addr = %config.server.bind_addr,
        database_kind = %config.database.kind.as_config_value(),
        "loaded application config"
    );

    let db = db::connect_and_migrate(&config.database)
        .await
        .context("failed to initialize database")?;
    let users = UserRepository::new(db.clone());
    let sessions = SessionRepository::new(db.clone());
    let products = ProductRepository::new(db.clone());
    let auth = AuthService::new(
        config.dingtalk.clone(),
        users.clone(),
        sessions.clone(),
        config.session.ttl_seconds,
    );
    let authz = AuthzService::new(AuthzRepository::new(db.clone()))
        .await
        .context("failed to initialize authorization service")?;
    let users = UserService::new(users, sessions);
    let products = ProductService::new(products);

    // Business tables opt into the review flow here as they adopt it, e.g.:
    // registry.register::<ProductDoc>();
    // Each type declares its reviewer permission via
    // ReviewableResource::APPROVAL_PERMISSION; malformed declarations
    // panic here at startup.
    let registry = ApplierRegistry::new();
    let events = EventService::new(
        EventRepository::new(db),
        authz.clone(),
        Arc::new(registry),
        config.events.retention_days,
    );
    spawn_event_retention_sweeper(events.clone(), config.events.sweep_interval_seconds);

    let app = app::router(AppState::new(
        auth,
        authz,
        users,
        products,
        events,
        config.auth.clone(),
        config.session.clone(),
    ));
    let addr: SocketAddr = config
        .server
        .bind_addr
        .parse()
        .context("server.bind_addr must be a valid socket address")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind server address")?;
    info!(%addr, "starting HTTP server");

    axum::serve(listener, app)
        .await
        .context("server exited unexpectedly")?;

    Ok(())
}

/// Periodically deletes finalized events that exceeded their retention
/// window. The first tick fires immediately, so startup performs a sweep.
fn spawn_event_retention_sweeper(events: EventService, interval_seconds: u64) {
    let period = Duration::from_secs(interval_seconds.max(1));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match events.sweep_expired(Utc::now()).await {
                Ok(deleted) if deleted > 0 => {
                    info!(deleted, "event retention sweep removed expired events");
                }
                Ok(_) => debug!("event retention sweep found nothing to remove"),
                Err(error) => error!(%error, "event retention sweep failed"),
            }
        }
    });
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("ATLAS_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("backend=info,tower_http=warn,sea_orm=warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}
