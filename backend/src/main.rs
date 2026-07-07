use anyhow::{Context, Result};
use backend::{
    app, config, db,
    repositories::{
        authz::AuthzRepository, departments::DepartmentRepository, products::ProductRepository,
        sessions::SessionRepository, systems::SystemRepository, users::UserRepository,
    },
    services::{
        auth::AuthService, authz::AuthzService, products::ProductService, systems::SystemService,
        users::UserService,
    },
    state::AppState,
};
use std::net::SocketAddr;
use tracing::info;
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
    let departments = DepartmentRepository::new(db.clone());
    let systems = SystemRepository::new(db.clone());
    let auth = AuthService::new(
        config.dingtalk.clone(),
        users.clone(),
        sessions.clone(),
        config.session.ttl_seconds,
    );
    let authz = AuthzService::new(AuthzRepository::new(db))
        .await
        .context("failed to initialize authorization service")?;
    let users = UserService::new(users, sessions);
    let products = ProductService::new(products);
    let systems = SystemService::new(systems, departments);
    let app = app::router(AppState::new(
        auth,
        authz,
        users,
        products,
        systems,
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

fn init_tracing() {
    let filter = EnvFilter::try_from_env("ATLAS_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("backend=info,tower_http=warn,sea_orm=warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}
