use anyhow::{Context, Result};
use backend::{
    app, config, db,
    repositories::{
        authz::AuthzRepository, customers::CustomerRepository, departments::DepartmentRepository,
        events::EventRepository, product_categories::ProductCategoryRepository,
        products::ProductRepository, sales_records::SalesRecordRepository,
        sessions::SessionRepository, stores::StoreRepository, systems::SystemRepository,
        user_profiles::UserProfileRepository, users::UserRepository,
    },
    services::{
        auth::AuthService, authz::AuthzService, authz_catalog::PermissionCatalog,
        customers::CustomerService, departments::DepartmentService, events::EventService,
        product_categories::ProductCategoryService, products::ProductService,
        review::ApplierRegistry, sales_records::SalesRecordService, stores::StoreService,
        systems::SystemService, users::UserService,
    },
    state::{AppState, AppStateParts},
};
use chrono::Utc;
use std::{future::Future, net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::load().context("failed to load application config")?;
    let _log_guard = init_tracing(&config.logging).context("failed to initialize logging")?;
    info!(
        bind_addr = %config.server.bind_addr,
        database_kind = %config.database.kind.as_config_value(),
        log_directory = %config.logging.directory.display(),
        log_file_prefix = %config.logging.file_prefix,
        "loaded application config"
    );

    let db = db::connect_and_migrate(&config.database)
        .await
        .context("failed to initialize database")?;
    let users = UserRepository::new(db.clone());
    let profiles = UserProfileRepository::new(db.clone());
    let sessions = SessionRepository::new(db.clone());
    let product_categories = ProductCategoryRepository::new(db.clone());
    let products = ProductRepository::new(db.clone());
    let departments = DepartmentRepository::new(db.clone());
    let systems = SystemRepository::new(db.clone());
    let stores = StoreRepository::new(db.clone());
    let customers = CustomerRepository::new(db.clone());
    let sales_records = SalesRecordRepository::new(db.clone());
    // Business tables opt into the review flow here as they adopt it, e.g.:
    // registry.register::<ProductDoc>();
    // Each type declares its reviewer permission via
    // ReviewableResource::APPROVAL_PERMISSION; malformed declarations
    // panic here at startup.
    let registry = ApplierRegistry::new();

    // The catalog is the single enumeration of every enforceable
    // permission: builtin route permissions plus the approval permissions
    // of registered review resource types.
    let mut catalog = PermissionCatalog::builtin();
    for (resource_type, object, action) in registry.approval_permissions() {
        catalog.add_permission(object, action, "审核", resource_type);
    }
    let authz = AuthzService::with_catalog(AuthzRepository::new(db.clone()), catalog)
        .await
        .context("failed to initialize authorization service")?;
    let auth = AuthService::new_with_super_admin_bootstrap(
        config.dingtalk.clone(),
        users.clone(),
        profiles.clone(),
        EventRepository::new(db.clone()),
        sessions.clone(),
        config.session.ttl_seconds,
    );
    let users_service = UserService::new(users.clone(), profiles, sessions);
    let product_categories_service =
        ProductCategoryService::new(product_categories.clone(), products.clone());
    let products = ProductService::new(products, product_categories.clone());
    let stores_service = StoreService::new(stores.clone(), systems.clone());
    let systems_service = SystemService::new(systems.clone(), stores.clone());
    let departments_service = DepartmentService::new(config.dingtalk.clone(), departments);
    let customers_service =
        CustomerService::new(customers.clone(), systems.clone(), stores.clone());
    let sales_records_service = SalesRecordService::new(
        sales_records,
        customers,
        systems,
        stores,
        product_categories,
        users.clone(),
    );
    let events = EventService::new(
        EventRepository::new(db),
        authz.clone(),
        Arc::new(registry),
        config.events.retention_days,
    );
    spawn_event_retention_sweeper(events.clone(), config.events.sweep_interval_seconds);

    let app = app::router(AppState::new(AppStateParts {
        auth,
        authz,
        users: users_service,
        product_categories: product_categories_service,
        products,
        systems: systems_service,
        stores: stores_service,
        customers: customers_service,
        sales_records: sales_records_service,
        events,
        departments: departments_service,
        auth_config: config.auth.clone(),
        session_config: config.session.clone(),
    }));
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

fn init_tracing(logging: &config::LoggingConfig) -> Result<WorkerGuard> {
    ensure_log_directory(&logging.directory)?;

    let filter = EnvFilter::try_from_env("ATLAS_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("backend=info,tower_http=warn,sea_orm=warn"));
    let file_appender = tracing_appender::rolling::daily(&logging.directory, &logging.file_prefix);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let stdout_layer = tracing_subscriber::fmt::layer().compact();
    let file_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(file_writer);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(guard)
}

fn ensure_log_directory(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("failed to create log directory `{}`", directory.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        future::{pending, ready},
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn creates_missing_log_directory() {
        let dir = temp_path("log-dir");

        ensure_log_directory(&dir).expect("log directory should be created");

        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reports_log_directory_creation_error() {
        let root = temp_path("log-parent-file");
        fs::create_dir_all(&root).expect("test root should be created");
        let file_parent = root.join("not-a-directory");
        fs::write(&file_parent, "not a directory").expect("test file should be written");

        let error = ensure_log_directory(&file_parent.join("child"))
            .expect_err("file parent should prevent directory creation");

        assert!(error.to_string().contains("failed to create log directory"));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        std::env::temp_dir().join(format!("atlas-{prefix}-{}-{unique}", std::process::id()))
    }
}
