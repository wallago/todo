//! Websocket layer.

use std::net::SocketAddr;

use axum::{
    Router,
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, Utf8Bytes, WebSocket},
    },
    response::IntoResponse,
    routing::any,
};
use axum_extra::{TypedHeader, headers};
use futures_util::{SinkExt, StreamExt};
use tokio::{sync::broadcast, task::JoinSet};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

/// Channel shared by every connection: what one client sends, all others receive.
/// Each message is tagged with its sender so it isn't echoed back to them.
type Hub = broadcast::Sender<(SocketAddr, Utf8Bytes)>;

/// Websocket routes.
pub(super) fn routes() -> Router {
    let (hub, _) = broadcast::channel::<(SocketAddr, Utf8Bytes)>(100);
    Router::new()
        .route("/", any(ws_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .with_state(hub)
}

/// Websocket handler messages.
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(hub): State<Hub>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    tracing::info!("`{user_agent}` at {addr} connected.");
    ws.on_upgrade(move |socket| handle_socket(socket, addr, hub))
}

/// Websocket connection and interactions handler.
async fn handle_socket(socket: WebSocket, who: SocketAddr, hub: Hub) {
    let mut tasks = JoinSet::new();
    let (mut ws_send, mut ws_recv) = socket.split();
    let mut hub_recv = hub.subscribe();

    // Client → hub: publish every text message to all connections.
    tasks.spawn(async move {
        while let Some(Ok(msg)) = ws_recv.next().await {
            match msg {
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => (),
                Message::Close(_) => break,
                Message::Text(text) => {
                    if let Err(err) = hub.send((who, text)) {
                        tracing::trace!("Broadcasting message failed: {err}");
                        break;
                    }
                }
            }
        }
    });

    // Hub → client: forward every published message to this connection.
    tasks.spawn(async move {
        while let Ok((from, text)) = hub_recv.recv().await {
            if from == who {
                continue; // our own message
            }
            if let Err(err) = ws_send.send(Message::Text(text)).await {
                tracing::trace!("Broadcasting message failed: {err}");
                break;
            }
        }
    });

    let _ = tasks.join_next().await;
    tracing::info!("{who} disconnected");
}
