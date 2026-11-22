pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from: String,
    /// Path to the Google service-account JSON key, mounted read-only
    /// (technical-specs.md §3, §1). Absent in local dev / e2e: the
    /// scheduler and manual sync endpoint are then disabled rather than
    /// failing startup.
    pub google_service_account_key_path: Option<String>,
    /// Seconds between scheduled syncs (specs.md §3.3: "toutes les 10-15
    /// min"). 0 disables the periodic scheduler even if a key is
    /// configured — manual sync still works.
    pub sync_interval_seconds: u64,
    /// Directory holding the built SvelteKit SPA (set by the Dockerfile,
    /// technical-specs.md §3). Absent in local dev, where the frontend is
    /// served separately by `vite dev` (proxying `/api` to this backend).
    pub static_dir: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            listen_addr: std::env::var("LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            smtp_host: std::env::var("SMTP_HOST").expect("SMTP_HOST must be set"),
            smtp_port: std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .expect("SMTP_PORT must be a valid port number"),
            smtp_user: std::env::var("SMTP_USER").ok(),
            smtp_pass: std::env::var("SMTP_PASS").ok(),
            smtp_from: std::env::var("SMTP_FROM")
                .unwrap_or_else(|_| "benefrance@example.org".to_string()),
            google_service_account_key_path: std::env::var("GOOGLE_SERVICE_ACCOUNT_KEY_PATH").ok(),
            sync_interval_seconds: std::env::var("SYNC_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .expect("SYNC_INTERVAL_SECONDS must be a non-negative integer"),
            static_dir: std::env::var("STATIC_DIR").ok(),
        }
    }
}
