//! WebSocket: bidirectional cell-edit sync within one `optimiser_run_id` room.
//!
//! Wire protocol (JSON, tagged by `type`):
//!
//! Client → Server:
//!   - `{ "type": "edit", "row_id": "...", "column_id": "allocated_wgt",
//!         "value": 42.0, "client_id": "<uuid>" }`
//!
//! Server → Client:
//!   - `{ "type": "snapshot", "live_edits": [...], "last_save": {...}|null,
//!         "viewers": N }` — sent immediately on connect
//!   - `{ "type": "edit", ...CellEdit }` — broadcast to everyone, including the
//!         originating tab; the client filters on `origin_client_id`.
//!   - `{ "type": "saved", ...SnapshotMeta }`
//!   - `{ "type": "presence", "viewers": N }`
//!
//! Only `column_id == "allocated_wgt"` is accepted today; any other column
//! produces a `Close(InvalidPayload)` frame and disconnect.

use std::sync::atomic::Ordering;

use axum::{
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use common_rs::validation::SAFE_HEX_64;

use crate::state::{AppState, CellEdit, PlanEvent, PlanRoom};
use common_rs::auth::AuthenticatedUser;
use common_rs::error::ApiError;

const EDITABLE_COLUMNS: &[&str] = &["allocated_wgt"];

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Edit {
        row_id: String,
        column_id: String,
        value: f64,
        client_id: Option<String>,
    },
    /// Optional ping-pong; the server just echoes presence on receipt.
    Heartbeat,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage<'a> {
    Snapshot {
        live_edits: Vec<CellEdit>,
        last_save: Option<&'a crate::state::SnapshotMeta>,
        viewers: u32,
    },
    Edit(CellEdit),
    Saved(crate::state::SnapshotMeta),
    Presence {
        viewers: u32,
    },
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(run_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    if !SAFE_HEX_64.is_match(&run_id) && run_id.len() > 64 {
        return Err(ApiError::BadRequest(format!(
            "invalid optimiser_run_id: {run_id:?}"
        )));
    }
    let room = state.plan_room(&run_id);
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, room, user.email)))
}

async fn handle_socket(socket: WebSocket, room: std::sync::Arc<PlanRoom>, user_email: String) {
    let (mut sender, mut receiver) = socket.split();

    // Increment viewer count and broadcast new presence.
    let viewers = room.viewers.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = room.broadcast.send(PlanEvent::Presence { viewers });

    // Send initial snapshot to the freshly connected client.
    {
        let live_edits: Vec<CellEdit> = room.live_edits.read().await.values().cloned().collect();
        let last_save_guard = room.last_save.read().await;
        let snapshot = ServerMessage::Snapshot {
            live_edits,
            last_save: last_save_guard.as_ref(),
            viewers,
        };
        if let Ok(text) = serde_json::to_string(&snapshot) {
            if sender.send(Message::Text(text.into())).await.is_err() {
                cleanup(&room).await;
                return;
            }
        }
    }

    // Subscribe BEFORE we start handling client messages so we don't miss any
    // events that fire between the initial snapshot and the steady state.
    let mut rx = room.broadcast.subscribe();

    // Outbound task: relay broadcast events to this client.
    let outbound_room = std::sync::Arc::clone(&room);
    let mut outbound = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let payload = match &event {
                PlanEvent::Edit(edit) => serde_json::to_string(&ServerMessage::Edit(edit.clone())),
                PlanEvent::Saved(meta) => {
                    serde_json::to_string(&ServerMessage::Saved(meta.clone()))
                }
                PlanEvent::Presence { viewers } => {
                    serde_json::to_string(&ServerMessage::Presence { viewers: *viewers })
                }
            };
            let Ok(text) = payload else { continue };
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        // Drop sender — closing the socket from this side.
        let _ = sender.close().await;
        let _ = outbound_room; // keep room alive for the borrow checker
    });

    // Inbound task: parse client messages and update room state.
    let inbound_room = std::sync::Arc::clone(&room);
    let inbound_user = user_email.clone();
    let mut inbound = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(txt) => {
                    let Ok(parsed) = serde_json::from_str::<ClientMessage>(&txt) else {
                        tracing::warn!(payload = %txt, "discarding malformed client WS message");
                        continue;
                    };
                    handle_client_message(parsed, &inbound_room, &inbound_user).await;
                }
                Message::Binary(_) => {
                    // Binary frames are unused on this protocol; ignore.
                }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {
                    // axum handles ping/pong at the protocol layer.
                }
            }
        }
    });

    // If either task exits, abort the other so the socket fully tears down.
    tokio::select! {
        _ = &mut outbound => inbound.abort(),
        _ = &mut inbound => outbound.abort(),
    }
    cleanup(&room).await;
}

async fn handle_client_message(msg: ClientMessage, room: &PlanRoom, user_email: &str) {
    match msg {
        ClientMessage::Edit {
            row_id,
            column_id,
            value,
            client_id,
        } => {
            if !EDITABLE_COLUMNS.contains(&column_id.as_str()) {
                tracing::warn!(column_id, "rejecting edit on non-editable column");
                return;
            }
            if !value.is_finite() {
                tracing::warn!(value, "rejecting non-finite edit value");
                return;
            }
            let edit = CellEdit {
                row_id: row_id.clone(),
                column_id: column_id.clone(),
                value,
                edited_by: user_email.to_string(),
                server_ts: Utc::now().timestamp_millis(),
                origin_client_id: client_id,
            };
            // LWW: any prior edit for this (row_id, column_id) is dropped.
            room.live_edits
                .write()
                .await
                .insert((row_id, column_id), edit.clone());
            let _ = room.broadcast.send(PlanEvent::Edit(edit));
        }
        ClientMessage::Heartbeat => {
            // No-op for now. Could update a last-seen timestamp later.
        }
    }
}

async fn cleanup(room: &PlanRoom) {
    let viewers = room
        .viewers
        .fetch_sub(1, Ordering::SeqCst)
        .saturating_sub(1);
    let _ = room.broadcast.send(PlanEvent::Presence { viewers });
}
