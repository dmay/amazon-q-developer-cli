use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use eyre::Result;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::agent_env::session::Session;

use super::{
    api::ErrorResponse,
    events::{instant_to_unix_timestamp, WebUIEvent, WorkerLifecycleState as WebWorkerLifecycleState},
    AppState,
};

/// Commands sent from frontend to backend via WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketCommand {
    /// Send a prompt to the worker
    Prompt {
        text: String,
    },
    /// Cancel the current job
    Cancel,
    /// Ping to keep connection alive
    Ping,
}

/// Worker state snapshot sent on connection
#[derive(Debug, Serialize)]
struct WorkerStateSnapshot {
    #[serde(rename = "type")]
    type_field: String,
    worker_id: String,
    name: String,
    lifecycle_state: WebWorkerLifecycleState,
    timestamp: f64,
}

impl WebSocketCommand {
    /// Validate the command
    pub fn validate(&self) -> Result<(), String> {
        match self {
            WebSocketCommand::Prompt { text } => {
                if text.trim().is_empty() {
                    return Err("Prompt text cannot be empty".to_string());
                }
                Ok(())
            }
            WebSocketCommand::Cancel | WebSocketCommand::Ping => Ok(()),
        }
    }
}

/// WebSocket upgrade handler
pub async fn websocket_handler(
    Path(worker_id): Path<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Parse worker_id
    let worker_id = match Uuid::parse_str(&worker_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    error: "Invalid worker ID".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Check if worker exists
    if state.session.get_worker(worker_id).is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(ErrorResponse {
                error: "Worker not found".to_string(),
            }),
        )
            .into_response();
    }

    // Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_websocket(socket, worker_id, state))
        .into_response()
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, worker_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events BEFORE sending snapshot to prevent race condition
    let mut event_rx = state.web_ui.subscribe();

    // Send initial state snapshot
    if let Err(e) = send_state_snapshot(&mut sender, worker_id, &state).await {
        tracing::error!("Failed to send state snapshot for worker {}: {}", worker_id, e);
        return;
    }

    tracing::info!("WebSocket connected for worker {}", worker_id);

    // Spawn task to forward events to WebSocket
    let send_task = {
        let worker_id_str = worker_id.to_string();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        // Filter events for this worker
                        if let Some(event_worker_id) = event.worker_id() {
                            if event_worker_id != worker_id_str {
                                continue;
                            }
                        }

                        // Serialize to JSON
                        let json = match serde_json::to_string(&event) {
                            Ok(json) => json,
                            Err(e) => {
                                tracing::error!("Failed to serialize event: {}", e);
                                continue;
                            }
                        };

                        // Send to WebSocket
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WebSocket lagged by {} events for worker {}", n, worker_id_str);
                        // Could send fresh snapshot here, but for MVP just log
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        })
    };

    // Spawn task to handle incoming commands
    let session = state.session.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Err(e) = handle_command(&text, worker_id, &session).await {
                    tracing::error!("Failed to handle command for worker {}: {}", worker_id, e);
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {
            tracing::info!("WebSocket send task completed for worker {}", worker_id);
        }
        _ = recv_task => {
            tracing::info!("WebSocket receive task completed for worker {}", worker_id);
        }
    }
}

/// Send initial state snapshot
async fn send_state_snapshot(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    worker_id: Uuid,
    state: &AppState,
) -> Result<()> {
    let worker = state
        .session
        .get_worker(worker_id)
        .ok_or_else(|| eyre::eyre!("Worker not found"))?;

    let lifecycle_state = *worker.lifecycle_state.lock().unwrap();

    let snapshot = WorkerStateSnapshot {
        type_field: "worker_state_snapshot".to_string(),
        worker_id: worker_id.to_string(),
        name: worker.name.clone(),
        lifecycle_state: lifecycle_state.into(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    };

    let json = serde_json::to_string(&snapshot)?;
    sender.send(Message::Text(json)).await?;

    Ok(())
}

/// Handle incoming command
async fn handle_command(text: &str, worker_id: Uuid, session: &Arc<Session>) -> Result<()> {
    let command: WebSocketCommand = serde_json::from_str(text)?;

    // Validate command
    if let Err(e) = command.validate() {
        tracing::warn!("Invalid command for worker {}: {}", worker_id, e);
        return Ok(()); // Don't fail, just log
    }

    match command {
        WebSocketCommand::Prompt { text } => {
            // Get worker
            let worker = session
                .get_worker(worker_id)
                .ok_or_else(|| eyre::eyre!("Worker not found"))?;

            // Add message to conversation history
            worker
                .context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(text);

            // Launch agent loop
            use crate::agent_env::worker_tasks::agent_loop::AgentLoopInput;
            session.run_task__agent_loop(worker, AgentLoopInput {})?;

            tracing::info!("Started agent loop for worker {}", worker_id);
        }
        WebSocketCommand::Cancel => {
            // Cancel current job
            session.cancel_worker_jobs(worker_id)?;
            tracing::info!("Cancelled job for worker {}", worker_id);
        }
        WebSocketCommand::Ping => {
            // No-op, just keep connection alive
            tracing::debug!("Ping received for worker {}", worker_id);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt_command() {
        let json = r#"{"type": "prompt", "text": "Hello, world!"}"#;
        let cmd: WebSocketCommand = serde_json::from_str(json).unwrap();
        
        match cmd {
            WebSocketCommand::Prompt { text } => {
                assert_eq!(text, "Hello, world!");
            }
            _ => panic!("Expected Prompt command"),
        }
    }

    #[test]
    fn test_parse_cancel_command() {
        let json = r#"{"type": "cancel"}"#;
        let cmd: WebSocketCommand = serde_json::from_str(json).unwrap();
        
        matches!(cmd, WebSocketCommand::Cancel);
    }

    #[test]
    fn test_parse_ping_command() {
        let json = r#"{"type": "ping"}"#;
        let cmd: WebSocketCommand = serde_json::from_str(json).unwrap();
        
        matches!(cmd, WebSocketCommand::Ping);
    }

    #[test]
    fn test_validate_prompt_empty() {
        let cmd = WebSocketCommand::Prompt {
            text: "   ".to_string(),
        };
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn test_validate_prompt_valid() {
        let cmd = WebSocketCommand::Prompt {
            text: "Hello".to_string(),
        };
        assert!(cmd.validate().is_ok());
    }

    #[test]
    fn test_validate_cancel() {
        let cmd = WebSocketCommand::Cancel;
        assert!(cmd.validate().is_ok());
    }

    #[test]
    fn test_validate_ping() {
        let cmd = WebSocketCommand::Ping;
        assert!(cmd.validate().is_ok());
    }
}
