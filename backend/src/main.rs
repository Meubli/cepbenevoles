use std::sync::Arc;
use std::time::Duration;

use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
use benefrance_backend::sheets::SheetsClient;
use benefrance_backend::sync::orchestrate::sync_all_and_record;
use benefrance_backend::{app, config::Config, AppState, SyncContext};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // Default to debug for this crate's own code (app-level visibility
    // without drowning in dependency chatter); RUST_LOG, when set,
    // overrides this entirely.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("benefrance_backend=debug,tower_http=debug,info")
            }),
        )
        .init();

    let config = Config::from_env();

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");

    bootstrap_superadmin(&db, &config.bootstrap_superadmin_email).await;

    let sync = build_sync_context(
        &config.google_service_account_key_path,
        &config.sheets_api_base_url,
    );

    if let Some(sync) = sync.clone() {
        if config.sync_interval_seconds > 0 {
            spawn_scheduler(db.clone(), sync, config.sync_interval_seconds);
        } else {
            tracing::info!(
                "SYNC_INTERVAL_SECONDS=0 — periodic sync disabled, manual sync still works"
            );
        }
    } else {
        tracing::info!("no GOOGLE_SERVICE_ACCOUNT_KEY_PATH configured — sync disabled");
    }

    let state = AppState {
        db,
        smtp_host: config.smtp_host,
        smtp_port: config.smtp_port,
        smtp_user: config.smtp_user,
        smtp_pass: config.smtp_pass,
        smtp_from: config.smtp_from,
        sync,
        session_cookie_secure: config.session_cookie_secure,
        sheet_validation_required: config.sheet_validation_required,
    };
    let router = app(state, config.static_dir.as_deref());

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listen address");

    tracing::info!("listening on {}", config.listen_addr);

    axum::serve(listener, router).await.expect("server error");
}

/// Seeds the first superadmin so someone can log in at all (specs.md
/// §11.4). Idempotent: a conflict on the unique email is a no-op, so this
/// is safe to run on every startup.
async fn bootstrap_superadmin(db: &PgPool, email: &Option<String>) {
    let Some(email) = email else {
        tracing::info!("no BOOTSTRAP_SUPERADMIN_EMAIL configured — skipping superadmin seed");
        return;
    };

    let inserted = sqlx::query!(
        "INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
         ON CONFLICT (email) DO NOTHING",
        email,
    )
    .execute(db)
    .await
    .expect("failed to seed bootstrap superadmin");

    if inserted.rows_affected() > 0 {
        tracing::info!(%email, "seeded bootstrap superadmin");
    }
}

fn build_sync_context(key_path: &Option<String>, sheets_api_base_url: &str) -> Option<SyncContext> {
    let key_path = key_path.as_ref()?;
    let key_json = std::fs::read_to_string(key_path).unwrap_or_else(|e| {
        panic!("failed to read GOOGLE_SERVICE_ACCOUNT_KEY_PATH ({key_path}): {e}")
    });
    let key = ServiceAccountKey::from_json(&key_json)
        .expect("GOOGLE_SERVICE_ACCOUNT_KEY_PATH does not contain a valid service account key");

    Some(SyncContext {
        auth: GoogleAuth::new(key),
        sheets: SheetsClient::new(sheets_api_base_url.to_string()),
        running: Arc::new(Mutex::new(())),
    })
}

/// Runs `sync_all_and_record` on a fixed interval. Never panics the whole
/// process on a sync failure — individual sheet failures are already
/// captured per-sheet (sync_run.error); anything that escapes that (a
/// database error) is just logged, the next tick tries again.
fn spawn_scheduler(db: PgPool, sync: SyncContext, interval_seconds: u64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            tracing::debug!("scheduled sync tick");

            let Ok(_guard) = sync.running.try_lock() else {
                tracing::warn!("scheduled sync skipped: a sync is already in progress");
                continue;
            };

            match sync_all_and_record(&db, &sync.auth, &sync.sheets).await {
                Ok(outcomes) => {
                    for outcome in &outcomes {
                        if let Err(err) = &outcome.result {
                            tracing::warn!(sheet_id = %outcome.sheet_id, error = %err, "scheduled sync failed for this sheet");
                        }
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "scheduled sync_all_and_record failed");
                }
            }
        }
    });
}
