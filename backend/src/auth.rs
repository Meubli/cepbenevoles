//! Minimal OTP email login (specs.md §11.1) and server-side sessions
//! (§11.3). Anti-brute-force limits (§11.2) are not implemented yet —
//! this is a walking skeleton to prove the real HTTP+email+cookie path,
//! not the finished auth feature.

use crate::AppState;
use axum::async_trait;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration, Utc};
use lettre::message::Message;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_cookies::cookie::SameSite;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

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

    tracing::debug!(email = %body.email, known_admin = admin.is_some(), "request_code");

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
        tracing::debug!(email = %body.email, "verify_code: no active challenge");
        return StatusCode::UNAUTHORIZED;
    };

    if row.attempts >= MAX_ATTEMPTS || row.expires_at < Utc::now() {
        tracing::debug!(
            email = %body.email,
            attempts = row.attempts,
            expired = row.expires_at < Utc::now(),
            "verify_code: challenge locked out"
        );
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
        tracing::debug!(email = %body.email, "verify_code: wrong code");
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
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(state.session_cookie_secure);
    cookies.add(cookie);

    tracing::debug!(email = %body.email, admin_id = %admin.id, "verify_code: session created");
    StatusCode::OK
}

/// Validates the session cookie against the `sessions` table and, in the
/// same query, renews its sliding expiration (specs §11.3: "renouvelée à
/// chaque activité" — a session shouldn't creep toward its 180-day
/// `SESSION_TTL_DAYS` just from regular use). Returns the admin id on
/// success. `WHERE expires_at > now()` makes the check-and-renew atomic:
/// an already-expired session is never resurrected by this UPDATE.
async fn renew_session(state: &AppState, cookies: &Cookies) -> Option<Uuid> {
    let Some(cookie) = cookies.get(SESSION_COOKIE) else {
        tracing::debug!("renew_session: no session cookie");
        return None;
    };
    let expires_at = Utc::now() + Duration::days(SESSION_TTL_DAYS);

    let row = sqlx::query!(
        "UPDATE sessions SET last_activity_at = now(), expires_at = $2
         WHERE token_hash = $1 AND expires_at > now()
         RETURNING admin_id",
        hash(cookie.value()),
        expires_at,
    )
    .fetch_optional(&state.db)
    .await
    .expect("db query failed");

    if row.is_none() {
        tracing::debug!("renew_session: cookie does not match an active session");
    }

    row.map(|r| r.admin_id)
}

/// Shared by every protected route (specs §7.3 admin screens, §6.1
/// "Générer le planning", the manual sync trigger, ...).
pub async fn session_valid(state: &AppState, cookies: &Cookies) -> bool {
    renew_session(state, cookies).await.is_some()
}

#[derive(Serialize)]
pub struct SessionInfo {
    email: String,
    is_superadmin: bool,
}

/// Session-check route the frontend polls on load to know whether to show
/// the login screen or an admin screen (wireframe s0 vs the rest). On
/// success, returns the admin's identity so the admin shell can show it
/// (wireframe session-box, specs §11.3's "barre de navigation").
pub async fn require_session(State(state): State<AppState>, cookies: Cookies) -> Response {
    let Some(admin_id) = renew_session(&state, &cookies).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let admin = sqlx::query!(
        "SELECT email, is_superadmin FROM admins WHERE id = $1",
        admin_id
    )
    .fetch_one(&state.db)
    .await
    .expect("admin must exist for a valid session");

    Json(SessionInfo {
        email: admin.email,
        is_superadmin: admin.is_superadmin,
    })
    .into_response()
}

/// Destroys the current session server-side and clears the cookie (specs
/// §11.3: "déconnexion manuelle disponible à tout moment"). Idempotent —
/// no session cookie, or one that doesn't match any row, is not an error.
pub async fn logout(State(state): State<AppState>, cookies: Cookies) -> StatusCode {
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        sqlx::query!(
            "DELETE FROM sessions WHERE token_hash = $1",
            hash(cookie.value())
        )
        .execute(&state.db)
        .await
        .expect("db delete failed");
    }

    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_path("/");
    cookies.remove(cookie);

    StatusCode::NO_CONTENT
}

/// Extracts the authenticated admin's id, rejecting with 401 otherwise.
/// Handlers that need to stamp `created_by`/`updated_by` (specs.md §8) take
/// this instead of re-checking the cookie themselves.
pub struct AuthAdmin(pub Uuid);

#[async_trait]
impl FromRequestParts<AppState> for AuthAdmin {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        renew_session(state, &cookies)
            .await
            .map(AuthAdmin)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// Extracts the authenticated admin's id, rejecting with 403 if they're not
/// a superadmin (401 if not even logged in). For the Admins screen (specs.md
/// §11.4: "seul un superadmin peut ajouter, supprimer ou promouvoir un
/// admin").
pub struct SuperAdmin(pub Uuid);

#[async_trait]
impl FromRequestParts<AppState> for SuperAdmin {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthAdmin(admin_id) = AuthAdmin::from_request_parts(parts, state).await?;

        let is_superadmin: bool =
            sqlx::query_scalar!("SELECT is_superadmin FROM admins WHERE id = $1", admin_id)
                .fetch_one(&state.db)
                .await
                .expect("admin must exist for a valid session");

        if is_superadmin {
            Ok(SuperAdmin(admin_id))
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}
