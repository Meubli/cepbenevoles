//! Minimal OTP email login (specs.md §11.1) and server-side sessions
//! (§11.3). Anti-brute-force limits (§11.2) are not implemented yet —
//! this is a walking skeleton to prove the real HTTP+email+cookie path,
//! not the finished auth feature.

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use lettre::message::Message;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tower_cookies::{Cookie, Cookies};

const SESSION_COOKIE: &str = "session_id";
const CODE_TTL_MINUTES: i64 = 10;
const SESSION_TTL_DAYS: i64 = 180;

fn hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

#[derive(Deserialize)]
pub struct RequestCodeBody {
    pub email: String,
}

/// Always answers 202 regardless of whether the email is a known admin,
/// so the response can't be used to enumerate admin emails (specs §11.1).
pub async fn request_code(
    State(state): State<AppState>,
    Json(body): Json<RequestCodeBody>,
) -> StatusCode {
    let admin = sqlx::query!("SELECT id FROM admins WHERE email = $1", body.email)
        .fetch_optional(&state.db)
        .await
        .expect("db query failed");

    if admin.is_some() {
        let code: u32 = rand::thread_rng().gen_range(0..1_000_000);
        let code = format!("{code:06}");
        let expires_at = Utc::now() + Duration::minutes(CODE_TTL_MINUTES);

        sqlx::query!(
            "INSERT INTO login_challenges (email, code_hash, expires_at) VALUES ($1, $2, $3)",
            body.email,
            hash(&code),
            expires_at,
        )
        .execute(&state.db)
        .await
        .expect("db insert failed");

        send_code_email(&state, &body.email, &code).await;
    }

    StatusCode::ACCEPTED
}

async fn send_code_email(state: &AppState, to: &str, code: &str) {
    let email = Message::builder()
        .from(state.smtp_from.parse().expect("invalid SMTP from address"))
        .to(to.parse().expect("invalid recipient address"))
        .subject("Votre code de connexion")
        .body(format!("Votre code de connexion : {code}"))
        .expect("failed to build email message");

    let mailer = build_mailer(state);
    mailer
        .send(email)
        .await
        .expect("failed to send login code email");
}

fn build_mailer(state: &AppState) -> AsyncSmtpTransport<Tokio1Executor> {
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&state.smtp_host)
        .port(state.smtp_port);
    if let (Some(user), Some(pass)) = (&state.smtp_user, &state.smtp_pass) {
        builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }
    builder.build()
}

#[derive(Deserialize)]
pub struct VerifyCodeBody {
    pub email: String,
    pub code: String,
}

const MAX_ATTEMPTS: i32 = 5;

/// On success, sets an httpOnly session cookie (specs §11.3) and returns
/// 200. On failure, 401 — same response whether the email or code was
/// wrong, to avoid leaking which one was the problem.
pub async fn verify_code(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(body): Json<VerifyCodeBody>,
) -> StatusCode {
    let row = sqlx::query!(
        "SELECT id, code_hash, expires_at, attempts, used
         FROM login_challenges
         WHERE email = $1 AND used = false
         ORDER BY created_at DESC
         LIMIT 1",
        body.email
    )
    .fetch_optional(&state.db)
    .await
    .expect("db query failed");

    let Some(row) = row else {
        return StatusCode::UNAUTHORIZED;
    };

    if row.attempts >= MAX_ATTEMPTS || row.expires_at < Utc::now() {
        sqlx::query!(
            "UPDATE login_challenges SET used = true WHERE id = $1",
            row.id
        )
        .execute(&state.db)
        .await
        .expect("db update failed");
        return StatusCode::UNAUTHORIZED;
    }

    sqlx::query!(
        "UPDATE login_challenges SET attempts = attempts + 1 WHERE id = $1",
        row.id
    )
    .execute(&state.db)
    .await
    .expect("db update failed");

    if row.code_hash != hash(&body.code) {
        return StatusCode::UNAUTHORIZED;
    }

    sqlx::query!(
        "UPDATE login_challenges SET used = true WHERE id = $1",
        row.id
    )
    .execute(&state.db)
    .await
    .expect("db update failed");

    let admin = sqlx::query!("SELECT id FROM admins WHERE email = $1", body.email)
        .fetch_one(&state.db)
        .await
        .expect("admin must exist for a resolved challenge");

    let token: [u8; 32] = rand::thread_rng().gen();
    let token = hex::encode(token);
    let expires_at = Utc::now() + Duration::days(SESSION_TTL_DAYS);

    sqlx::query!(
        "INSERT INTO sessions (admin_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        admin.id,
        hash(&token),
        expires_at,
    )
    .execute(&state.db)
    .await
    .expect("db insert failed");

    let mut cookie = Cookie::new(SESSION_COOKIE, token);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookies.add(cookie);

    StatusCode::OK
}

/// Stub protected route: 200 with a valid, unexpired session cookie, 401
/// otherwise. Stands in for the real admin screens (specs §7.3).
pub async fn require_session(State(state): State<AppState>, cookies: Cookies) -> StatusCode {
    let Some(cookie) = cookies.get(SESSION_COOKIE) else {
        return StatusCode::UNAUTHORIZED;
    };

    let session = sqlx::query!(
        "SELECT expires_at FROM sessions WHERE token_hash = $1",
        hash(cookie.value())
    )
    .fetch_optional(&state.db)
    .await
    .expect("db query failed");

    match session {
        Some(session) if session.expires_at > Utc::now() => StatusCode::OK,
        _ => StatusCode::UNAUTHORIZED,
    }
}
