use sqlx::{Executor, PgPool};
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

/// The default image tag in testcontainers-modules 0.11 is postgres:11,
/// which predates core `gen_random_uuid()` (added in PG13). Pin to match
/// what's actually run in prod (postgres:16-alpine, cf. docker-compose.test.yml).
const POSTGRES_TAG: &str = "16-alpine";

/// One Postgres container shared by the whole test binary run, so tests
/// don't each pay container-startup cost (~1-2s). Per-test isolation comes
/// from creating a fresh, uniquely-named database per test inside this one
/// instance and running migrations against it — the same approach
/// `#[sqlx::test]` uses internally, done explicitly here so we control the
/// container lifecycle (`#[sqlx::test]`'s own container support landed
/// after 0.8 and isn't used here).
static CONTAINER: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();

async fn admin_connection_string() -> String {
    let container = CONTAINER
        .get_or_init(|| async {
            Postgres::default()
                .with_tag(POSTGRES_TAG)
                .start()
                .await
                .expect("failed to start postgres testcontainer")
        })
        .await;
    let port = container
        .get_host_port_ipv4(5432.tcp())
        .await
        .expect("failed to get mapped port");
    format!("postgres://postgres:postgres@127.0.0.1:{port}")
}

/// Creates a fresh database on the shared container, runs migrations
/// against it, and returns a connected pool. Each call is isolated from
/// every other call.
pub async fn spawn_test_db() -> PgPool {
    let admin_url = admin_connection_string().await;

    let admin_pool = PgPool::connect(&format!("{admin_url}/postgres"))
        .await
        .expect("failed to connect to postgres admin db");

    let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{db_name}""#).as_str())
        .await
        .expect("failed to create test database");

    let pool = PgPool::connect(&format!("{admin_url}/{db_name}"))
        .await
        .expect("failed to connect to fresh test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations on test database");

    pool
}
