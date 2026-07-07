use anyhow::{Context, Result};
use backend::{
    app, config, db,
    repositories::{
        authz::AuthzRepository, departments::DepartmentRepository,
        product_categories::ProductCategoryRepository, products::ProductRepository,
        sessions::SessionRepository, stores::StoreRepository, systems::SystemRepository,
        users::UserRepository,
    },
    services::{
        auth::AuthService, authz::AuthzService, product_categories::ProductCategoryService,
        products::ProductService, stores::StoreService, systems::SystemService, users::UserService,
    },
    state::AppState,
};
use std::{future::Future, net::SocketAddr};
use tracing::{info, warn};
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
    let product_categories = ProductCategoryRepository::new(db.clone());
    let products = ProductRepository::new(db.clone());
    let departments = DepartmentRepository::new(db.clone());
    let systems = SystemRepository::new(db.clone());
    let stores = StoreRepository::new(db.clone());
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
    let product_categories_service =
        ProductCategoryService::new(product_categories.clone(), products.clone());
    let products = ProductService::new(products, product_categories);
    let stores_service = StoreService::new(stores.clone(), systems.clone());
    let systems = SystemService::new(systems, departments, stores);
    let app = app::router(AppState::new(
        auth,
        authz,
        users,
        product_categories_service,
        products,
        systems,
        stores_service,
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
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server exited unexpectedly")?;

    info!("HTTP server stopped");

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownSignal {
    CtrlC,
    Sigterm,
}

impl ShutdownSignal {
    fn as_str(self) -> &'static str {
        match self {
            Self::CtrlC => "ctrl_c",
            Self::Sigterm => "sigterm",
        }
    }
}

async fn shutdown_signal() {
    let signal = wait_for_shutdown_signal().await;
    info!(signal = signal.as_str(), "received shutdown signal");
    info!("starting graceful shutdown");
}

async fn wait_for_shutdown_signal() -> ShutdownSignal {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(%error, "failed to listen for Ctrl-C shutdown signal");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                select_shutdown_signal(
                    ctrl_c,
                    Some(async move {
                        sigterm.recv().await;
                    }),
                )
                .await
            }
            Err(error) => {
                warn!(%error, "failed to listen for SIGTERM shutdown signal");
                select_shutdown_signal(ctrl_c, None::<std::future::Pending<()>>).await
            }
        }
    }

    #[cfg(not(unix))]
    {
        select_shutdown_signal(ctrl_c, None::<std::future::Pending<()>>).await
    }
}

async fn select_shutdown_signal<C, T>(ctrl_c: C, sigterm: Option<T>) -> ShutdownSignal
where
    C: Future<Output = ()>,
    T: Future<Output = ()>,
{
    if let Some(sigterm) = sigterm {
        tokio::pin!(ctrl_c);
        tokio::pin!(sigterm);

        tokio::select! {
            _ = &mut ctrl_c => ShutdownSignal::CtrlC,
            _ = &mut sigterm => ShutdownSignal::Sigterm,
        }
    } else {
        ctrl_c.await;
        ShutdownSignal::CtrlC
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::{pending, ready};

    #[tokio::test]
    async fn selects_ctrl_c_when_ctrl_c_completes_first() {
        let signal = select_shutdown_signal(ready(()), Some(pending::<()>())).await;

        assert_eq!(signal, ShutdownSignal::CtrlC);
    }

    #[tokio::test]
    async fn selects_sigterm_when_sigterm_completes_first() {
        let signal = select_shutdown_signal(pending::<()>(), Some(ready(()))).await;

        assert_eq!(signal, ShutdownSignal::Sigterm);
    }
}
