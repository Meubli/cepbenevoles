pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from: String,
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
        }
    }
}
