use std::sync::Arc;
use std::time::Duration;

use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
use benefrance_backend::sheets::SheetsClient;
use benefrance_backend::sync::orchestrate::sync_all_and_record;
use benefrance_backend::{app, config::Config, AppState, SyncContext};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::Mutex;

const SHEETS_API_BASE_URL: &str = "https://sheets.googleapis.com";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

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

    let sync = build_sync_context(&config.google_service_account_key_path);

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
    };
    let router = app(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listen address");

    tracing::info!("listening on {}", config.listen_addr);

    axum::serve(listener, router).await.expect("server error");
}

fn build_sync_context(key_path: &Option<String>) -> Option<SyncContext> {
    let key_path = key_path.as_ref()?;
    let key_json = std::fs::read_to_string(key_path).unwrap_or_else(|e| {
        panic!("failed to read GOOGLE_SERVICE_ACCOUNT_KEY_PATH ({key_path}): {e}")
    });
    let key = ServiceAccountKey::from_json(&key_json)
        .expect("GOOGLE_SERVICE_ACCOUNT_KEY_PATH does not contain a valid service account key");

    Some(SyncContext {
        auth: GoogleAuth::new(key),
        sheets: SheetsClient::new(SHEETS_API_BASE_URL.to_string()),
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
