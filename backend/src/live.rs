//! Minimal "something changed, go refetch" live broadcast
//! (technical-specs.md §10, generalized from Planning-only to every admin
//! screen — commissions, mapping, événements, admins, sync — per user
//! request). Push-only: every write still goes through the normal
//! authenticated HTTP routes; the socket only ever carries a resource tag
//! (e.g. "commissions"), never the data itself, so a connected client just
//! refetches whatever it's already displaying.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use tokio::sync::broadcast;

use crate::auth::AuthAdmin;
use crate::AppState;

pub type LiveSender = broadcast::Sender<String>;

/// Bounded so a slow/stuck client can't grow memory unboundedly — a client
/// that falls behind gets `RecvError::Lagged` and is told to refetch
/// everything (`"*"`) instead of replaying a backlog of tags.
pub fn channel() -> LiveSender {
    broadcast::channel(256).0
}

/// Upgrades to a websocket and streams resource tags as they change.
/// Requires a valid admin session, same as every other route — the
/// handshake carries the same httpOnly session cookie.
pub async fn ws_handler(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.live.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    // A reverse proxy (or an idle browser tab's OS) can kill a seemingly-idle
    // connection; a periodic ping keeps it alive and also detects a dead
    // socket quickly instead of leaking the task forever.
    let mut ping = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            event = rx.recv() => {
                let tag = match event {
                    Ok(tag) => tag,
                    Err(broadcast::error::RecvError::Lagged(_)) => "*".to_string(),
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if socket.send(Message::Text(tag)).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    None => break,
                    Some(Err(_)) => break,
                    // Push-only: whatever the client sent (including a Pong
                    // reply) is ignored, just keeps the loop alive.
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

/// After a successful (2xx) write (any non-GET method), broadcasts the
/// first path segment as the changed resource — e.g. `POST
/// /commissions/:id/postes` -> `"commissions"` — so every connected admin
/// screen watching that resource refetches. `/auth/*` is excluded: login
/// churn isn't a shared resource any screen displays.
pub async fn emit_live_event(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;

    if method != Method::GET && response.status().is_success() {
        let relative = path
            .strip_prefix("/api/")
            .unwrap_or_else(|| path.trim_start_matches('/'));
        if let Some(tag) = relative
            .split('/')
            .next()
            .filter(|s| !s.is_empty() && *s != "auth")
        {
            // No receivers yet (nobody connected) is not an error.
            let _ = state.live.send(tag.to_string());
        }
    }

    response
}
