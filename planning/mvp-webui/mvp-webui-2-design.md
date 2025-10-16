# MVP WebUI - Design Document

## 1. WebUIEvent Serializable Types

### Overview

The WebUI needs JSON-serializable event types to send over WebSocket connections. The existing `AgentEnvironmentEvent` uses non-serializable types (`Instant`, `Uuid`) and nested enums that are complex for JavaScript clients.

### Design: Flat Event Enum

Create a flat enum structure with serde's `tag` attribute for clean JSON representation:

```rust
use serde::{Deserialize, Serialize};

/// Serializable events for WebUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebUIEvent {
    // Worker Events
    WorkerCreated {
        worker_id: String,
        name: String,
        timestamp: f64, // Unix timestamp (seconds since epoch)
    },
    WorkerDeleted {
        worker_id: String,
        timestamp: f64,
    },
    WorkerStateChanged {
        worker_id: String,
        old_state: WorkerLifecycleState,
        new_state: WorkerLifecycleState,
        timestamp: f64,
    },
    
    // Job Events
    JobStarted {
        worker_id: String,
        job_id: String,
        task_type: String,
        timestamp: f64,
    },
    JobCompleted {
        worker_id: String,
        job_id: String,
        result: JobResult,
        timestamp: f64,
    },
    OutputChunk {
        worker_id: String,
        job_id: String,
        chunk: OutputChunkData,
        timestamp: f64,
    },
    
    // AgentLoop Events
    ResponseReceived {
        worker_id: String,
        job_id: String,
        text: String,
        timestamp: f64,
    },
    ToolUseRequested {
        worker_id: String,
        job_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
        timestamp: f64,
    },
    
    // System Events
    ShutdownInitiated {
        reason: String,
        timestamp: f64,
    },
}

/// Worker lifecycle state (serializable)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycleState {
    Idle,
    Busy,
    IdleFailed,
}

/// Job completion result (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JobResult {
    Success {
        #[serde(skip_serializing_if = "Option::is_none")]
        task_metadata: Option<serde_json::Value>,
        user_interaction_required: bool,
    },
    Cancelled,
    Failed {
        error: String,
    },
}

/// Output chunk data (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "chunk_type", rename_all = "snake_case")]
pub enum OutputChunkData {
    AssistantResponse {
        text: String,
    },
    ToolUse {
        tool_name: String,
        tool_input: serde_json::Value,
    },
    ToolResult {
        tool_name: String,
        result: String,
    },
}
```

### JSON Examples

**WorkerCreated:**
```json
{
  "type": "worker_created",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "timestamp": 1729132395.123
}
```

**JobStarted:**
```json
{
  "type": "job_started",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "job_id": "660e8400-e29b-41d4-a716-446655440001",
  "task_type": "agent_loop",
  "timestamp": 1729132395.456
}
```

**OutputChunk (AssistantResponse):**
```json
{
  "type": "output_chunk",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "job_id": "660e8400-e29b-41d4-a716-446655440001",
  "chunk": {
    "chunk_type": "assistant_response",
    "text": "Hello! How can I help you?"
  },
  "timestamp": 1729132395.789
}
```

**JobCompleted (Success):**
```json
{
  "type": "job_completed",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "job_id": "660e8400-e29b-41d4-a716-446655440001",
  "result": {
    "status": "success",
    "user_interaction_required": false
  },
  "timestamp": 1729132396.012
}
```

### Conversion Implementation

```rust
use std::time::{SystemTime, UNIX_EPOCH};

impl WebUIEvent {
    /// Convert AgentEnvironmentEvent to WebUIEvent
    pub fn from_agent_event(event: AgentEnvironmentEvent) -> Self {
        match event {
            AgentEnvironmentEvent::Worker(worker_event) => {
                Self::from_worker_event(worker_event)
            }
            AgentEnvironmentEvent::Job(job_event) => {
                Self::from_job_event(job_event)
            }
            AgentEnvironmentEvent::AgentLoop(agent_loop_event) => {
                Self::from_agent_loop_event(agent_loop_event)
            }
            AgentEnvironmentEvent::System(system_event) => {
                Self::from_system_event(system_event)
            }
        }
    }
    
    fn from_worker_event(event: WorkerEvent) -> Self {
        match event {
            WorkerEvent::Created { worker_id, name, timestamp } => {
                WebUIEvent::WorkerCreated {
                    worker_id: worker_id.to_string(),
                    name,
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
            WorkerEvent::Deleted { worker_id, timestamp } => {
                WebUIEvent::WorkerDeleted {
                    worker_id: worker_id.to_string(),
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
            WorkerEvent::LifecycleStateChanged { worker_id, old_state, new_state, timestamp } => {
                WebUIEvent::WorkerStateChanged {
                    worker_id: worker_id.to_string(),
                    old_state: old_state.into(),
                    new_state: new_state.into(),
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
        }
    }
    
    fn from_job_event(event: JobEvent) -> Self {
        match event {
            JobEvent::Started { worker_id, job_id, task_type, timestamp } => {
                WebUIEvent::JobStarted {
                    worker_id: worker_id.to_string(),
                    job_id: job_id.to_string(),
                    task_type,
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
            JobEvent::Completed { worker_id, job_id, result, timestamp } => {
                WebUIEvent::JobCompleted {
                    worker_id: worker_id.to_string(),
                    job_id: job_id.to_string(),
                    result: result.into(),
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
            JobEvent::OutputChunk { worker_id, job_id, chunk, timestamp } => {
                WebUIEvent::OutputChunk {
                    worker_id: worker_id.to_string(),
                    job_id: job_id.to_string(),
                    chunk: chunk.into(),
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
        }
    }
    
    fn from_agent_loop_event(event: AgentLoopEvent) -> Self {
        match event {
            AgentLoopEvent::ResponseReceived { worker_id, job_id, text, timestamp } => {
                WebUIEvent::ResponseReceived {
                    worker_id: worker_id.to_string(),
                    job_id: job_id.to_string(),
                    text,
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
            AgentLoopEvent::ToolUseRequestReceived { worker_id, job_id, tool_name, tool_input, timestamp } => {
                WebUIEvent::ToolUseRequested {
                    worker_id: worker_id.to_string(),
                    job_id: job_id.to_string(),
                    tool_name,
                    tool_input,
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
        }
    }
    
    fn from_system_event(event: SystemEvent) -> Self {
        match event {
            SystemEvent::ShutdownInitiated { reason, timestamp } => {
                WebUIEvent::ShutdownInitiated {
                    reason,
                    timestamp: instant_to_unix_timestamp(timestamp),
                }
            }
        }
    }
}

/// Convert Instant to Unix timestamp
/// 
/// Note: Instant is monotonic and doesn't have a fixed epoch.
/// We need to track the relationship between Instant and SystemTime.
/// This should be done at startup by storing both values.
fn instant_to_unix_timestamp(instant: Instant) -> f64 {
    // This requires a global start time reference
    // See implementation note below
    let start_instant = PROCESS_START_INSTANT.get().unwrap();
    let start_system_time = PROCESS_START_SYSTEM_TIME.get().unwrap();
    
    let elapsed = instant.duration_since(*start_instant);
    let system_time = *start_system_time + elapsed;
    
    system_time
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

// Global state for time conversion (initialized at process start)
use std::sync::OnceLock;
static PROCESS_START_INSTANT: OnceLock<Instant> = OnceLock::new();
static PROCESS_START_SYSTEM_TIME: OnceLock<SystemTime> = OnceLock::new();

/// Initialize time conversion (call at process start)
pub fn init_time_conversion() {
    PROCESS_START_INSTANT.get_or_init(Instant::now);
    PROCESS_START_SYSTEM_TIME.get_or_init(SystemTime::now);
}
```

### Implementation Location

**New file:** `crates/chat-cli/src/cli/chat/web_server/events.rs`

This keeps WebUI-specific types separate from core agent_env types.

### Key Design Decisions

1. **Flat Enum Structure**: Easier to handle in JavaScript with simple type discrimination
2. **Unix Timestamps**: Standard format, easy to work with in JavaScript (multiply by 1000 for Date)
3. **String IDs**: UUIDs converted to strings for JSON compatibility
4. **Serde Tags**: Use `#[serde(tag = "type")]` for clean discriminated unions
5. **Time Conversion**: Track process start time to convert Instant to SystemTime
6. **Separate Module**: Keep WebUI types separate from core types

### Trade-offs

**Pros:**
- Clean JSON representation
- Easy to consume in JavaScript
- Type-safe conversion from internal events
- No information loss

**Cons:**
- Requires global state for time conversion
- Duplication of type definitions
- Conversion overhead (minimal)

### Alternative Considered: Direct Serialization

Could make `AgentEnvironmentEvent` serializable with custom serializers for `Instant` and `Uuid`.

**Rejected because:**
- Pollutes core types with serialization concerns
- Nested enum structure is awkward in JSON
- Less control over JSON format
- Harder to evolve independently

---

## 2. WebSocket Protocol

### Overview

The WebSocket protocol handles bidirectional communication between the browser and the Q CLI backend. It supports:
- Event streaming from backend to frontend (WebUIEvent)
- Command sending from frontend to backend (WebSocketCommand)
- Connection lifecycle management (connect, disconnect, reconnect)

### Design: Per-Worker WebSocket Connections

Following the web-q reference implementation, use one WebSocket connection per worker:

**URL Pattern:** `/ws/worker/:worker_id`

**Rationale:**
- Simpler client-side filtering (all events for one worker)
- Natural isolation between workers
- Easier to implement and debug
- Matches web-q pattern

### Message Format

All messages are JSON with a `type` field for discrimination.

#### Backend → Frontend (Events)

Events are serialized `WebUIEvent` instances:

```json
{
  "type": "output_chunk",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "job_id": "660e8400-e29b-41d4-a716-446655440001",
  "chunk": {
    "chunk_type": "assistant_response",
    "text": "Hello!"
  },
  "timestamp": 1729132395.789
}
```

#### Frontend → Backend (Commands)

Commands are sent as JSON messages:

```rust
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
```

**Examples:**

```json
{
  "type": "prompt",
  "text": "What is the capital of France?"
}
```

```json
{
  "type": "cancel"
}
```

```json
{
  "type": "ping"
}
```

### Connection Lifecycle

#### 1. Connection Establishment

```
Client                          Server
  |                               |
  |--- GET /ws/worker/:id ------->|
  |<-- 101 Switching Protocols ---|
  |                               |
  |<-- Initial State Snapshot ----|
  |                               |
  |<-- Event Stream --------------|
```

**Initial State Snapshot:**
```json
{
  "type": "worker_state_snapshot",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "lifecycle_state": "idle",
  "current_job": null,
  "timestamp": 1729132395.123
}
```

Or if job is running:
```json
{
  "type": "worker_state_snapshot",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "lifecycle_state": "busy",
  "current_job": {
    "job_id": "660e8400-e29b-41d4-a716-446655440001",
    "task_type": "agent_loop",
    "started_at": 1729132390.456
  },
  "timestamp": 1729132395.123
}
```

#### 2. Normal Operation

```
Client                          Server
  |                               |
  |--- Prompt Command ----------->|
  |                               |
  |<-- JobStarted Event ----------|
  |<-- OutputChunk Events --------|
  |<-- OutputChunk Events --------|
  |<-- JobCompleted Event --------|
  |<-- WorkerStateChanged Event --|
```

#### 3. Disconnection

```
Client                          Server
  |                               |
  |--- Close Frame -------------->|
  |<-- Close Frame ---------------|
  |                               |
  (cleanup)                   (cleanup)
```

#### 4. Reconnection

```
Client                          Server
  |                               |
  |--- GET /ws/worker/:id ------->|
  |<-- 101 Switching Protocols ---|
  |                               |
  |<-- Initial State Snapshot ----|
  |<-- Event Stream --------------|
```

Client receives fresh state snapshot and continues from current state.

### Error Handling

#### Invalid Worker ID

```
Client                          Server
  |                               |
  |--- GET /ws/worker/invalid --->|
  |<-- 404 Not Found -------------|
```

#### Invalid Command

Server sends error event:
```json
{
  "type": "error",
  "error": "Invalid command format",
  "timestamp": 1729132395.789
}
```

#### Connection Lost

Client should:
1. Detect connection loss (WebSocket `onclose` event)
2. Wait 1-2 seconds
3. Attempt reconnection
4. Exponential backoff if reconnection fails

### WebSocket Handler Implementation

```rust
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use axum::extract::ws::{WebSocket, Message};
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

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
                "Invalid worker ID",
            ).into_response();
        }
    };
    
    // Check if worker exists
    if !state.session.worker_exists(worker_id) {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "Worker not found",
        ).into_response();
    }
    
    // Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_websocket(socket, worker_id, state))
        .into_response()
}

/// Handle WebSocket connection
async fn handle_websocket(
    socket: WebSocket,
    worker_id: Uuid,
    state: AppState,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // Send initial state snapshot
    if let Err(e) = send_state_snapshot(&mut sender, worker_id, &state).await {
        tracing::error!("Failed to send state snapshot: {}", e);
        return;
    }
    
    // Subscribe to events
    let mut event_rx = state.event_bus.subscribe();
    
    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Filter events for this worker
            if let Some(event_worker_id) = event.worker_id() {
                if event_worker_id != worker_id {
                    continue;
                }
            }
            
            // Convert to WebUIEvent
            let web_event = WebUIEvent::from_agent_event(event);
            
            // Serialize to JSON
            let json = match serde_json::to_string(&web_event) {
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
    });
    
    // Spawn task to handle incoming commands
    let session = state.session.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Err(e) = handle_command(&text, worker_id, &session).await {
                    tracing::error!("Failed to handle command: {}", e);
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
    let worker = state.session.get_worker(worker_id)?;
    
    let snapshot = WorkerStateSnapshot {
        worker_id: worker_id.to_string(),
        name: worker.name.clone(),
        lifecycle_state: worker.lifecycle_state.into(),
        current_job: worker.current_job.as_ref().map(|job| CurrentJobInfo {
            job_id: job.id.to_string(),
            task_type: job.task_type.clone(),
            started_at: instant_to_unix_timestamp(job.started_at),
        }),
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
async fn handle_command(
    text: &str,
    worker_id: Uuid,
    session: &Arc<Session>,
) -> Result<()> {
    let command: WebSocketCommand = serde_json::from_str(text)?;
    
    match command {
        WebSocketCommand::Prompt { text } => {
            // Add message to conversation history
            let worker = session.get_worker(worker_id)?;
            worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(text);
            
            // Launch agent loop
            session.run_task__agent_loop(worker, AgentLoopInput {})?;
        }
        WebSocketCommand::Cancel => {
            // Cancel current job
            session.cancel_worker_job(worker_id)?;
        }
        WebSocketCommand::Ping => {
            // No-op, just keep connection alive
        }
    }
    
    Ok(())
}

#[derive(Debug, Serialize)]
struct WorkerStateSnapshot {
    #[serde(rename = "type")]
    type_: String = "worker_state_snapshot".to_string(),
    worker_id: String,
    name: String,
    lifecycle_state: WorkerLifecycleState,
    current_job: Option<CurrentJobInfo>,
    timestamp: f64,
}

#[derive(Debug, Serialize)]
struct CurrentJobInfo {
    job_id: String,
    task_type: String,
    started_at: f64,
}
```

### Key Design Decisions

1. **Per-Worker Connections**: One WebSocket per worker for simplicity
2. **Initial State Snapshot**: Send current state on connection to handle reconnection
3. **Event Filtering**: Server filters events by worker_id before sending
4. **Bidirectional**: Same connection for events and commands
5. **JSON Messages**: All messages are JSON text frames
6. **Error Handling**: Invalid commands logged, connection continues

### Trade-offs

**Pros:**
- Simple client-side logic (no filtering needed)
- Natural isolation between workers
- Easy to implement and debug
- Matches web-q pattern

**Cons:**
- Multiple connections for multiple workers (acceptable for MVP)
- Slightly more server resources per client

### Alternative Considered: Single WebSocket with Subscriptions

Client connects to `/ws` and sends subscription messages to specify which workers to follow.

**Rejected because:**
- More complex protocol
- Client-side filtering required
- Harder to implement
- Not needed for MVP (single worker)

---

## 3. REST API Endpoints

### Overview

The REST API provides HTTP endpoints for querying worker state and performing operations that don't require real-time streaming. For MVP, we keep the API minimal since most interaction happens via WebSocket.

### Design: Minimal REST API

#### GET /api/workers

List all workers.

**Response:**
```json
{
  "workers": [
    {
      "worker_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "main",
      "lifecycle_state": "idle",
      "current_job": null
    }
  ]
}
```

**Implementation:**
```rust
async fn list_workers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let workers = state.session.list_workers();
    
    let response = WorkersResponse {
        workers: workers.into_iter().map(|w| WorkerInfo {
            worker_id: w.id.to_string(),
            name: w.name,
            lifecycle_state: w.lifecycle_state.into(),
            current_job: w.current_job.as_ref().map(|job| CurrentJobInfo {
                job_id: job.id.to_string(),
                task_type: job.task_type.clone(),
                started_at: instant_to_unix_timestamp(job.started_at),
            }),
        }).collect(),
    };
    
    Json(response)
}
```

#### GET /api/workers/:id

Get specific worker details.

**Response:**
```json
{
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "lifecycle_state": "busy",
  "current_job": {
    "job_id": "660e8400-e29b-41d4-a716-446655440001",
    "task_type": "agent_loop",
    "started_at": 1729132390.456
  },
  "metadata": {
    "created_at": 1729132385.123
  }
}
```

**Error Response (404):**
```json
{
  "error": "Worker not found"
}
```

**Implementation:**
```rust
async fn get_worker(
    Path(worker_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let worker_id = Uuid::parse_str(&worker_id)
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Invalid worker ID".to_string() }),
        ))?;
    
    let worker = state.session.get_worker(worker_id)
        .map_err(|_| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "Worker not found".to_string() }),
        ))?;
    
    Ok(Json(WorkerDetailResponse {
        worker_id: worker.id.to_string(),
        name: worker.name,
        lifecycle_state: worker.lifecycle_state.into(),
        current_job: worker.current_job.as_ref().map(|job| CurrentJobInfo {
            job_id: job.id.to_string(),
            task_type: job.task_type.clone(),
            started_at: instant_to_unix_timestamp(job.started_at),
        }),
        metadata: serde_json::json!({
            "created_at": instant_to_unix_timestamp(worker.created_at),
        }),
    }))
}
```

#### GET /api/health

Health check endpoint.

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

**Implementation:**
```rust
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
```

### API Types

```rust
#[derive(Debug, Serialize)]
struct WorkersResponse {
    workers: Vec<WorkerInfo>,
}

#[derive(Debug, Serialize)]
struct WorkerInfo {
    worker_id: String,
    name: String,
    lifecycle_state: WorkerLifecycleState,
    current_job: Option<CurrentJobInfo>,
}

#[derive(Debug, Serialize)]
struct WorkerDetailResponse {
    worker_id: String,
    name: String,
    lifecycle_state: WorkerLifecycleState,
    current_job: Option<CurrentJobInfo>,
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct CurrentJobInfo {
    job_id: String,
    task_type: String,
    started_at: f64,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}
```

### Future Endpoints (Post-MVP)

These endpoints are documented but not required for MVP:

#### POST /api/workers

Create a new worker.

**Request:**
```json
{
  "name": "worker-2",
  "config": {
    "model": "anthropic.claude-3-sonnet-20240229-v1:0"
  }
}
```

**Response:**
```json
{
  "worker_id": "770e8400-e29b-41d4-a716-446655440002",
  "name": "worker-2",
  "lifecycle_state": "idle"
}
```

#### DELETE /api/workers/:id

Delete a worker.

**Response:**
```json
{
  "success": true
}
```

#### POST /api/workers/:id/prompt

Send a prompt to a worker (alternative to WebSocket).

**Request:**
```json
{
  "text": "What is the capital of France?"
}
```

**Response:**
```json
{
  "job_id": "880e8400-e29b-41d4-a716-446655440003"
}
```

#### POST /api/workers/:id/cancel

Cancel the current job (alternative to WebSocket).

**Response:**
```json
{
  "success": true
}
```

### Key Design Decisions

1. **Minimal API**: Only essential endpoints for MVP (list, get, health)
2. **Read-Only**: No worker creation/deletion in MVP
3. **WebSocket Primary**: Commands go through WebSocket, not REST
4. **Standard HTTP**: Use standard status codes and JSON responses
5. **Error Handling**: Consistent error response format

### Trade-offs

**Pros:**
- Simple and focused
- Easy to implement
- Sufficient for MVP
- Can extend later

**Cons:**
- Limited functionality
- No worker management via API
- Requires WebSocket for commands

### Implementation Location

**New file:** `crates/chat-cli/src/cli/chat/web_server/api.rs`

---

## 4. WebUI Component

### Overview

The WebUI component acts as a bridge between the EventBus and WebSocket clients. It implements `HeadlessInterface` to receive events from AgentEnvironment and broadcasts them to all connected WebSocket clients.

### Design: Broadcast-Based WebUI

```rust
use async_trait::async_trait;
use eyre::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::agent_env::{
    agent_environment::HeadlessInterface,
    events::AgentEnvironmentEvent,
    session::Session,
};

use super::events::WebUIEvent;

/// WebUI component for broadcasting events to WebSocket clients
pub struct WebUI {
    session: Arc<Session>,
    event_tx: broadcast::Sender<WebUIEvent>,
}

impl WebUI {
    /// Create new WebUI
    pub fn new(session: Arc<Session>) -> Self {
        // Large buffer to handle bursts of events
        let (event_tx, _) = broadcast::channel(10000);
        
        Self {
            session,
            event_tx,
        }
    }
    
    /// Subscribe to WebUI events (for WebSocket handlers)
    pub fn subscribe(&self) -> broadcast::Receiver<WebUIEvent> {
        self.event_tx.subscribe()
    }
    
    /// Get session reference
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }
}

#[async_trait]
impl HeadlessInterface for WebUI {
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Convert to WebUIEvent
        let web_event = WebUIEvent::from_agent_event(event);
        
        // Broadcast to all subscribers (WebSocket handlers)
        // Ignore send errors (no subscribers is OK)
        let _ = self.event_tx.send(web_event);
    }
}
```

### Usage Pattern

```rust
// Create WebUI
let web_ui = Arc::new(WebUI::new(session.clone()));

// Add to AgentEnvironment as headless UI
let agent_env = AgentEnvironment::new(
    session.clone(),
    event_bus.clone(),
    Some(Arc::new(text_ui)),
    vec![web_ui.clone()], // WebUI as headless UI
    true,
);

// WebSocket handlers subscribe to WebUI
let mut event_rx = web_ui.subscribe();
while let Ok(event) = event_rx.recv().await {
    // Send event to WebSocket client
}
```

### Key Design Decisions

1. **HeadlessInterface**: WebUI doesn't have its own I/O, just forwards events
2. **Broadcast Channel**: Use tokio broadcast for efficient multi-client distribution
3. **Large Buffer**: 10,000 events to handle bursts without lagging
4. **No Filtering**: WebUI broadcasts all events, WebSocket handlers filter by worker_id
5. **Session Access**: Provide session reference for WebSocket handlers to query state

### Event Flow

```
EventBus → AgentEnvironment → WebUI → broadcast::Sender → WebSocket Handlers
                                                          ├─ Client 1
                                                          ├─ Client 2
                                                          └─ Client N
```

### Lag Handling

If a WebSocket handler is slow to consume events, it will receive `RecvError::Lagged(n)`:

```rust
// In WebSocket handler
while let Ok(event) = event_rx.recv().await {
    match event {
        Ok(event) => {
            // Send to client
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!("WebSocket lagged by {} events", n);
            // Send error to client or reconnect
        }
        Err(broadcast::error::RecvError::Closed) => {
            break;
        }
    }
}
```

### Alternative Considered: Direct EventBus Subscription

Each WebSocket handler could subscribe directly to EventBus.

**Rejected because:**
- Duplicates event conversion logic in each handler
- No centralized control over WebSocket event distribution
- Harder to add WebUI-specific features (filtering, rate limiting, etc.)
- Less efficient (multiple conversions of same event)

### Implementation Location

**New file:** `crates/chat-cli/src/cli/chat/web_server/web_ui.rs`

---

## 5. Web Server Architecture

### Overview

The Web Server provides HTTP and WebSocket endpoints using Axum. It integrates with the existing tokio runtime and shares Session and WebUI references with handlers.

### Design: Axum-Based Server

```rust
use axum::{
    Router,
    routing::{get, post},
    extract::State,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use eyre::Result;

use crate::agent_env::session::Session;
use super::web_ui::WebUI;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
}

/// Web server for serving WebUI
pub struct WebServer {
    addr: SocketAddr,
    state: AppState,
}

impl WebServer {
    /// Create new web server
    pub fn new(
        addr: SocketAddr,
        session: Arc<Session>,
        web_ui: Arc<WebUI>,
    ) -> Self {
        Self {
            addr,
            state: AppState { session, web_ui },
        }
    }
    
    /// Build router with all routes
    fn build_router(&self) -> Router {
        Router::new()
            // WebSocket endpoint
            .route("/ws/worker/:worker_id", get(super::websocket::websocket_handler))
            
            // REST API endpoints
            .route("/api/health", get(super::api::health_check))
            .route("/api/workers", get(super::api::list_workers))
            .route("/api/workers/:id", get(super::api::get_worker))
            
            // Static file serving (frontend)
            .nest_service("/", ServeDir::new("web/public"))
            
            // CORS for development
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any)
            )
            
            // Shared state
            .with_state(self.state.clone())
    }
    
    /// Run the web server (blocks until shutdown)
    pub async fn run(self) -> Result<()> {
        let router = self.build_router();
        
        tracing::info!("Web server listening on http://{}", self.addr);
        
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        
        axum::serve(listener, router)
            .await?;
        
        Ok(())
    }
    
    /// Run with graceful shutdown signal
    pub async fn run_with_shutdown(
        self,
        shutdown_signal: Arc<tokio::sync::Notify>,
    ) -> Result<()> {
        let router = self.build_router();
        
        tracing::info!("Web server listening on http://{}", self.addr);
        
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.notified().await;
                tracing::info!("Web server shutting down");
            })
            .await?;
        
        Ok(())
    }
}
```

### Module Structure

```
web_server/
├── mod.rs              # Module exports
├── server.rs           # WebServer implementation
├── web_ui.rs           # WebUI component
├── events.rs           # WebUIEvent types
├── websocket.rs        # WebSocket handlers
└── api.rs              # REST API handlers
```

**mod.rs:**
```rust
mod server;
mod web_ui;
mod events;
mod websocket;
mod api;

pub use server::{WebServer, AppState};
pub use web_ui::WebUI;
pub use events::WebUIEvent;
```

### Integration with ChatArgs

```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // ... existing initialization ...
        
        // Initialize time conversion for WebUI events
        super::web_server::events::init_time_conversion();
        
        // Create WebUI
        let web_ui = Arc::new(WebUI::new(session.clone()));
        
        // Check if web UI is enabled
        let enable_web = self.web_ui || std::env::var("Q_WEB_UI").is_ok();
        
        if enable_web {
            let web_port = self.web_port.unwrap_or(8080);
            let web_addr = SocketAddr::from(([127, 0, 0, 1], web_port));
            
            let web_server = WebServer::new(
                web_addr,
                session.clone(),
                web_ui.clone(),
            );
            
            let shutdown_signal = shutdown_signal.clone();
            tokio::spawn(async move {
                if let Err(e) = web_server.run_with_shutdown(shutdown_signal).await {
                    tracing::error!("Web server error: {}", e);
                }
            });
            
            tracing::info!("Web UI available at http://{}", web_addr);
        }
        
        // Create AgentEnvironment with WebUI as headless UI
        let headless_uis: Vec<Arc<dyn HeadlessInterface>> = if enable_web {
            vec![web_ui]
        } else {
            vec![]
        };
        
        let agent_env = AgentEnvironment::new(
            session.clone(),
            event_bus.clone(),
            Some(Arc::new(text_ui)),
            headless_uis,
            interactive,
        );
        
        // ... rest of execution ...
    }
}
```

### CLI Arguments

Add to `ChatArgs`:

```rust
#[derive(Debug, Parser)]
pub struct ChatArgs {
    // ... existing fields ...
    
    /// Enable web UI
    #[arg(long)]
    pub web_ui: bool,
    
    /// Web UI port (default: 8080)
    #[arg(long)]
    pub web_port: Option<u16>,
}
```

### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Web server
axum = { version = "0.7", features = ["ws"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# Already have:
# tokio (with "net" feature)
# serde, serde_json
# uuid
```

### Static File Structure

```
web/
└── public/
    ├── index.html      # Main HTML page
    ├── style.css       # Styles
    └── app.js          # Frontend JavaScript
```

### Key Design Decisions

1. **Axum Framework**: Native tokio integration, type-safe routing
2. **Shared State**: AppState with Arc<Session> and Arc<WebUI>
3. **Static Files**: Serve from `web/public/` directory
4. **CORS**: Permissive for development (local-only by default)
5. **Graceful Shutdown**: Integrate with existing shutdown signal
6. **CLI Flags**: `--web-ui` to enable, `--web-port` to configure port

### Port Configuration

**Default:** 8080  
**Override:** `--web-port 3000` or `Q_WEB_PORT=3000`  
**Conflict Handling:** Server fails to start if port in use (acceptable for MVP)

### Security Considerations

**MVP (Local-Only):**
- Bind to 127.0.0.1 (localhost only)
- No authentication required
- CORS permissive for development

**Future (Production):**
- Optional bind to 0.0.0.0 for remote access
- Authentication (API keys, OAuth)
- HTTPS support
- Rate limiting
- Input validation

### Error Handling

```rust
// Port already in use
if let Err(e) = web_server.run().await {
    if e.to_string().contains("Address already in use") {
        tracing::error!("Port {} already in use. Try --web-port <port>", web_port);
    } else {
        tracing::error!("Web server error: {}", e);
    }
}
```

### Implementation Location

**New directory:** `crates/chat-cli/src/cli/chat/web_server/`

### Alternative Considered: Separate Process

Run web server as separate process that communicates with Q CLI via IPC.

**Rejected because:**
- More complex architecture
- IPC overhead
- Harder to coordinate lifecycle
- Not needed for MVP

---

## 6. State Synchronization Strategy

### Overview

State synchronization ensures that WebSocket clients have an accurate view of worker state, even when connecting mid-session or after disconnection. The strategy combines initial state snapshots with real-time event streaming.

### Design: Snapshot + Event Stream

#### Initial Connection

When a client connects to `/ws/worker/:id`:

1. **Validate Worker**: Check if worker exists (404 if not)
2. **Send Snapshot**: Send current worker state immediately
3. **Stream Events**: Subscribe to EventBus and stream events

```rust
async fn handle_websocket(socket: WebSocket, worker_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Step 1: Send initial state snapshot
    if let Err(e) = send_state_snapshot(&mut sender, worker_id, &state).await {
        tracing::error!("Failed to send state snapshot: {}", e);
        return;
    }
    
    // Step 2: Subscribe to events
    let mut event_rx = state.web_ui.subscribe();
    
    // Step 3: Stream events (filtered by worker_id)
    // ... event streaming loop ...
}
```

#### State Snapshot Format

```rust
#[derive(Debug, Serialize)]
struct WorkerStateSnapshot {
    #[serde(rename = "type")]
    type_field: String, // "worker_state_snapshot"
    worker_id: String,
    name: String,
    lifecycle_state: WorkerLifecycleState,
    current_job: Option<CurrentJobInfo>,
    conversation_summary: ConversationSummary,
    timestamp: f64,
}

#[derive(Debug, Serialize)]
struct ConversationSummary {
    message_count: usize,
    last_message_timestamp: Option<f64>,
}

#[derive(Debug, Serialize)]
struct CurrentJobInfo {
    job_id: String,
    task_type: String,
    started_at: f64,
}
```

**Example Snapshot (Idle Worker):**
```json
{
  "type": "worker_state_snapshot",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "lifecycle_state": "idle",
  "current_job": null,
  "conversation_summary": {
    "message_count": 5,
    "last_message_timestamp": 1729132390.123
  },
  "timestamp": 1729132395.456
}
```

**Example Snapshot (Busy Worker):**
```json
{
  "type": "worker_state_snapshot",
  "worker_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "main",
  "lifecycle_state": "busy",
  "current_job": {
    "job_id": "660e8400-e29b-41d4-a716-446655440001",
    "task_type": "agent_loop",
    "started_at": 1729132390.456
  },
  "conversation_summary": {
    "message_count": 6,
    "last_message_timestamp": 1729132390.123
  },
  "timestamp": 1729132395.789
}
```

#### Event Streaming

After snapshot, stream events in real-time:

```rust
// Subscribe to WebUI events
let mut event_rx = state.web_ui.subscribe();

// Stream events
while let Ok(event) = event_rx.recv().await {
    // Filter by worker_id
    if let Some(event_worker_id) = event.worker_id() {
        if event_worker_id != worker_id {
            continue;
        }
    }
    
    // Serialize and send
    let json = serde_json::to_string(&event)?;
    if sender.send(Message::Text(json)).await.is_err() {
        break;
    }
}
```

### Reconnection Handling

#### Client-Side Reconnection

```javascript
class WorkerConnection {
    constructor(workerId) {
        this.workerId = workerId;
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000; // Start with 1 second
    }
    
    connect() {
        this.ws = new WebSocket(`ws://localhost:8080/ws/worker/${this.workerId}`);
        
        this.ws.onopen = () => {
            console.log('Connected to worker', this.workerId);
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;
        };
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.handleEvent(data);
        };
        
        this.ws.onclose = () => {
            console.log('Disconnected from worker', this.workerId);
            this.reconnect();
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }
    
    reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            return;
        }
        
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
        
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        
        setTimeout(() => {
            this.connect();
        }, delay);
    }
    
    handleEvent(event) {
        if (event.type === 'worker_state_snapshot') {
            // Reset UI state from snapshot
            this.resetState(event);
        } else {
            // Apply incremental update
            this.updateState(event);
        }
    }
}
```

#### Server-Side Considerations

- **No State Tracking**: Server doesn't track which events client has seen
- **Fresh Snapshot**: Each connection gets fresh snapshot of current state
- **Event Ordering**: Events are delivered in order via broadcast channel
- **Missed Events**: Client relies on snapshot to catch up, not event replay

### Race Condition Handling

#### Snapshot-Event Race

**Problem:** Events might occur between snapshot generation and event subscription.

**Solution:** Subscribe to events BEFORE generating snapshot:

```rust
async fn handle_websocket(socket: WebSocket, worker_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe FIRST
    let mut event_rx = state.web_ui.subscribe();
    
    // Then send snapshot
    // Any events that occur during snapshot generation will be queued
    send_state_snapshot(&mut sender, worker_id, &state).await?;
    
    // Stream events (might include duplicates of snapshot state)
    // Client should handle idempotent updates
}
```

**Client Handling:**
- Snapshot provides authoritative state
- Subsequent events update state incrementally
- Duplicate state changes are idempotent (e.g., "worker is idle" when already idle)

### Lag Handling

If client is slow to consume events:

```rust
while let Ok(result) = event_rx.recv().await {
    match result {
        Ok(event) => {
            // Normal event processing
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!("Client lagged by {} events, sending fresh snapshot", n);
            
            // Send fresh snapshot to resync
            send_state_snapshot(&mut sender, worker_id, &state).await?;
            
            // Continue streaming
        }
        Err(broadcast::error::RecvError::Closed) => {
            break;
        }
    }
}
```

**Client Handling:**
```javascript
handleEvent(event) {
    if (event.type === 'worker_state_snapshot') {
        // Full state reset
        this.resetState(event);
    } else if (event.type === 'error' && event.error.includes('lagged')) {
        // Expect snapshot to follow
        console.warn('Event lag detected, waiting for snapshot');
    } else {
        // Incremental update
        this.updateState(event);
    }
}
```

### Key Design Decisions

1. **Snapshot on Connect**: Always send fresh snapshot, no event replay
2. **Subscribe Before Snapshot**: Prevent race conditions
3. **Client-Side Reconnection**: Exponential backoff, max attempts
4. **Idempotent Updates**: Client handles duplicate state changes gracefully
5. **Lag Recovery**: Send fresh snapshot on lag, client resets state

### Trade-offs

**Pros:**
- Simple server implementation (no event history)
- Reliable state synchronization
- Handles reconnection cleanly
- No memory overhead for event history

**Cons:**
- No conversation history in snapshot (client must track)
- Potential duplicate events during snapshot-subscribe race
- Client must implement idempotent state updates

### Alternative Considered: Event Replay

Server tracks last N events and replays them on reconnection.

**Rejected because:**
- More complex server state management
- Memory overhead for event history
- Need to track per-client event positions
- Not needed for MVP (snapshot is sufficient)

---

## 7. Frontend Architecture

### Overview

The frontend is a single-page application using vanilla JavaScript (no framework) for simplicity. It displays worker state, streaming output, and provides input controls.

### Design: Vanilla JavaScript SPA

#### File Structure

```
web/public/
├── index.html          # Main HTML page
├── style.css           # Styles
└── app.js              # Application logic
```

#### HTML Structure (index.html)

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Q CLI - Web UI</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <div class="container">
        <header>
            <h1>Q CLI - Web UI</h1>
            <div class="status">
                <span class="status-indicator" id="connection-status"></span>
                <span id="connection-text">Connecting...</span>
            </div>
        </header>
        
        <main>
            <div class="worker-container">
                <div class="worker-header">
                    <h2 id="worker-name">Worker</h2>
                    <div class="worker-state">
                        <span class="state-badge" id="worker-state">idle</span>
                    </div>
                </div>
                
                <div class="output-container">
                    <div class="output" id="output"></div>
                </div>
                
                <div class="input-container">
                    <input 
                        type="text" 
                        id="prompt-input" 
                        placeholder="Enter your prompt..."
                        disabled
                    >
                    <button id="send-button" disabled>Send</button>
                    <button id="cancel-button" disabled>Cancel</button>
                </div>
            </div>
        </main>
    </div>
    
    <script src="app.js"></script>
</body>
</html>
```

#### CSS Styles (style.css)

```css
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    background: #f5f5f5;
    color: #333;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
}

header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
    padding: 20px;
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

header h1 {
    font-size: 24px;
    font-weight: 600;
}

.status {
    display: flex;
    align-items: center;
    gap: 8px;
}

.status-indicator {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ccc;
}

.status-indicator.connected {
    background: #4caf50;
}

.status-indicator.disconnected {
    background: #f44336;
}

.worker-container {
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
    overflow: hidden;
}

.worker-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px;
    border-bottom: 1px solid #e0e0e0;
}

.worker-header h2 {
    font-size: 20px;
    font-weight: 600;
}

.state-badge {
    padding: 4px 12px;
    border-radius: 12px;
    font-size: 14px;
    font-weight: 500;
    text-transform: uppercase;
}

.state-badge.idle {
    background: #e8f5e9;
    color: #2e7d32;
}

.state-badge.busy {
    background: #fff3e0;
    color: #e65100;
}

.state-badge.idle_failed {
    background: #ffebee;
    color: #c62828;
}

.output-container {
    height: 500px;
    overflow-y: auto;
    padding: 20px;
    background: #fafafa;
}

.output {
    font-family: 'Monaco', 'Menlo', 'Courier New', monospace;
    font-size: 14px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-wrap: break-word;
}

.output-chunk {
    margin-bottom: 12px;
    padding: 12px;
    border-radius: 4px;
    background: white;
}

.output-chunk.assistant {
    border-left: 3px solid #2196f3;
}

.output-chunk.tool-use {
    border-left: 3px solid #ff9800;
    background: #fff3e0;
}

.output-chunk.tool-result {
    border-left: 3px solid #4caf50;
    background: #e8f5e9;
}

.input-container {
    display: flex;
    gap: 8px;
    padding: 20px;
    border-top: 1px solid #e0e0e0;
}

#prompt-input {
    flex: 1;
    padding: 12px;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 14px;
}

#prompt-input:focus {
    outline: none;
    border-color: #2196f3;
}

button {
    padding: 12px 24px;
    border: none;
    border-radius: 4px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.2s;
}

#send-button {
    background: #2196f3;
    color: white;
}

#send-button:hover:not(:disabled) {
    background: #1976d2;
}

#cancel-button {
    background: #f44336;
    color: white;
}

#cancel-button:hover:not(:disabled) {
    background: #d32f2f;
}

button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
```

#### JavaScript Application (app.js)

```javascript
class QWebUI {
    constructor() {
        this.workerId = null;
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        
        this.elements = {
            connectionStatus: document.getElementById('connection-status'),
            connectionText: document.getElementById('connection-text'),
            workerName: document.getElementById('worker-name'),
            workerState: document.getElementById('worker-state'),
            output: document.getElementById('output'),
            promptInput: document.getElementById('prompt-input'),
            sendButton: document.getElementById('send-button'),
            cancelButton: document.getElementById('cancel-button'),
        };
        
        this.init();
    }
    
    async init() {
        // Fetch worker list
        const workers = await this.fetchWorkers();
        if (workers.length === 0) {
            this.showError('No workers found');
            return;
        }
        
        // Connect to first worker
        this.workerId = workers[0].worker_id;
        this.elements.workerName.textContent = workers[0].name;
        
        // Setup event listeners
        this.setupEventListeners();
        
        // Connect WebSocket
        this.connect();
    }
    
    async fetchWorkers() {
        try {
            const response = await fetch('/api/workers');
            const data = await response.json();
            return data.workers;
        } catch (error) {
            console.error('Failed to fetch workers:', error);
            return [];
        }
    }
    
    setupEventListeners() {
        this.elements.sendButton.addEventListener('click', () => this.sendPrompt());
        this.elements.cancelButton.addEventListener('click', () => this.cancelJob());
        
        this.elements.promptInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.sendPrompt();
            }
        });
    }
    
    connect() {
        this.updateConnectionStatus('connecting');
        
        this.ws = new WebSocket(`ws://${window.location.host}/ws/worker/${this.workerId}`);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.updateConnectionStatus('connected');
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;
        };
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.handleEvent(data);
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.updateConnectionStatus('disconnected');
            this.reconnect();
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }
    
    reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            this.showError('Failed to reconnect after multiple attempts');
            return;
        }
        
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
        
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        
        setTimeout(() => {
            this.connect();
        }, delay);
    }
    
    handleEvent(event) {
        console.log('Event:', event);
        
        switch (event.type) {
            case 'worker_state_snapshot':
                this.handleSnapshot(event);
                break;
            case 'worker_state_changed':
                this.updateWorkerState(event.new_state);
                break;
            case 'job_started':
                this.handleJobStarted(event);
                break;
            case 'job_completed':
                this.handleJobCompleted(event);
                break;
            case 'output_chunk':
                this.handleOutputChunk(event);
                break;
        }
    }
    
    handleSnapshot(snapshot) {
        this.elements.workerName.textContent = snapshot.name;
        this.updateWorkerState(snapshot.lifecycle_state);
        
        // Clear output for fresh start
        this.elements.output.innerHTML = '';
    }
    
    updateWorkerState(state) {
        this.elements.workerState.textContent = state;
        this.elements.workerState.className = `state-badge ${state}`;
        
        // Update input controls
        const isIdle = state === 'idle' || state === 'idle_failed';
        const isBusy = state === 'busy';
        
        this.elements.promptInput.disabled = !isIdle;
        this.elements.sendButton.disabled = !isIdle;
        this.elements.cancelButton.disabled = !isBusy;
    }
    
    handleJobStarted(event) {
        // Clear previous output
        this.elements.output.innerHTML = '';
    }
    
    handleJobCompleted(event) {
        const result = event.result;
        if (result.status === 'failed') {
            this.appendOutput('Error: ' + result.error, 'error');
        }
    }
    
    handleOutputChunk(event) {
        const chunk = event.chunk;
        
        switch (chunk.chunk_type) {
            case 'assistant_response':
                this.appendOutput(chunk.text, 'assistant');
                break;
            case 'tool_use':
                this.appendOutput(
                    `Tool: ${chunk.tool_name}\nInput: ${JSON.stringify(chunk.tool_input, null, 2)}`,
                    'tool-use'
                );
                break;
            case 'tool_result':
                this.appendOutput(
                    `Tool Result: ${chunk.tool_name}\n${chunk.result}`,
                    'tool-result'
                );
                break;
        }
    }
    
    appendOutput(text, type = 'assistant') {
        const chunk = document.createElement('div');
        chunk.className = `output-chunk ${type}`;
        chunk.textContent = text;
        this.elements.output.appendChild(chunk);
        
        // Scroll to bottom
        this.elements.output.scrollTop = this.elements.output.scrollHeight;
    }
    
    sendPrompt() {
        const text = this.elements.promptInput.value.trim();
        if (!text) return;
        
        const command = {
            type: 'prompt',
            text: text
        };
        
        this.ws.send(JSON.stringify(command));
        this.elements.promptInput.value = '';
    }
    
    cancelJob() {
        const command = {
            type: 'cancel'
        };
        
        this.ws.send(JSON.stringify(command));
    }
    
    updateConnectionStatus(status) {
        this.elements.connectionStatus.className = `status-indicator ${status}`;
        
        const statusText = {
            connecting: 'Connecting...',
            connected: 'Connected',
            disconnected: 'Disconnected'
        };
        
        this.elements.connectionText.textContent = statusText[status] || status;
    }
    
    showError(message) {
        this.elements.output.innerHTML = `<div class="error">${message}</div>`;
    }
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    new QWebUI();
});
```

### Key Design Decisions

1. **Vanilla JavaScript**: No framework dependencies, simple and lightweight
2. **Single Worker View**: MVP focuses on one worker at a time
3. **Auto-Connect**: Automatically connects to first available worker
4. **Real-Time Updates**: WebSocket events update UI immediately
5. **Responsive Design**: Works on desktop and mobile
6. **Minimal Dependencies**: No build process, just static files

### UI Components

1. **Header**: Title and connection status indicator
2. **Worker Header**: Worker name and lifecycle state badge
3. **Output Container**: Scrollable output with styled chunks
4. **Input Container**: Prompt input, send button, cancel button

### State Management

```javascript
// Application state
{
    workerId: "550e8400-e29b-41d4-a716-446655440000",
    workerName: "main",
    workerState: "idle",
    connected: true,
    reconnectAttempts: 0
}
```

### Event Handling

- **worker_state_snapshot**: Reset UI state
- **worker_state_changed**: Update state badge and controls
- **job_started**: Clear output
- **job_completed**: Show completion status
- **output_chunk**: Append to output with styling

### Future Enhancements (Post-MVP)

- Multiple worker tabs
- Conversation history view
- Syntax highlighting for code
- Markdown rendering
- File upload
- Export conversation
- Dark mode
- Settings panel

### Implementation Location

**New directory:** `web/public/`

### Alternative Considered: React/Vue Framework

Use modern framework for more structured code.

**Rejected because:**
- Adds build complexity
- Larger bundle size
- Not needed for MVP simplicity
- Can migrate later if needed

---

## 8. Shutdown Coordination Flow

### Overview

Shutdown coordination ensures that all components (AgentEnvironment, WebServer, WebSocket connections) shut down gracefully when the application exits. This includes handling Ctrl+C, quit commands, and error conditions.

### Design: Shared Shutdown Signal

Use a shared `Arc<Notify>` for coordinating shutdown across all components.

#### Shutdown Sources

1. **Ctrl+C**: User presses Ctrl+C in terminal
2. **Quit Command**: User sends quit command via UI
3. **Error**: Fatal error in any component
4. **Web UI Close**: All WebSocket clients disconnect (optional)

#### Shutdown Flow

```
Shutdown Source → shutdown_signal.notify() → All Components
                                            ├─ AgentEnvironment
                                            ├─ WebServer
                                            ├─ WebSocket Handlers
                                            └─ Main UI
```

#### Implementation

**Shared Shutdown Signal:**
```rust
// In ChatArgs::execute()
let shutdown_signal = Arc::new(Notify::new());
```

**AgentEnvironment Integration:**
```rust
let agent_env = AgentEnvironment::new(
    session.clone(),
    event_bus.clone(),
    Some(Arc::new(text_ui)),
    headless_uis,
    interactive,
);

// AgentEnvironment already uses shutdown_signal internally
// via its constructor
```

**WebServer Integration:**
```rust
let web_server = WebServer::new(
    web_addr,
    session.clone(),
    web_ui.clone(),
);

let shutdown_signal_clone = shutdown_signal.clone();
tokio::spawn(async move {
    if let Err(e) = web_server.run_with_shutdown(shutdown_signal_clone).await {
        tracing::error!("Web server error: {}", e);
    }
});
```

**WebSocket Handler Integration:**
```rust
async fn handle_websocket(
    socket: WebSocket,
    worker_id: Uuid,
    state: AppState,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // ... setup ...
    
    let send_task = tokio::spawn(async move {
        // Event streaming loop
    });
    
    let recv_task = tokio::spawn(async move {
        // Command handling loop
    });
    
    // Wait for either task or shutdown
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
        _ = state.shutdown_signal.notified() => {
            tracing::info!("WebSocket shutting down for worker {}", worker_id);
        }
    }
    
    // Cleanup
}
```

#### Shutdown Sequence

1. **Trigger**: Shutdown source calls `shutdown_signal.notify_waiters()`
2. **AgentEnvironment**: Exits main loop, cancels jobs, stops event multicast
3. **WebServer**: Stops accepting new connections, waits for active connections
4. **WebSocket Handlers**: Close connections gracefully, send close frames
5. **Main**: All tasks complete, process exits

#### Graceful WebSocket Closure

```rust
// In WebSocket handler shutdown
async fn close_websocket_gracefully(
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    // Send shutdown event to client
    let shutdown_event = WebUIEvent::ShutdownInitiated {
        reason: "Server shutting down".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    };
    
    let json = serde_json::to_string(&shutdown_event)?;
    sender.send(Message::Text(json)).await?;
    
    // Send close frame
    sender.send(Message::Close(None)).await?;
    
    Ok(())
}
```

#### Client-Side Handling

```javascript
ws.onclose = (event) => {
    if (event.code === 1000) {
        // Normal closure
        console.log('Server closed connection normally');
        this.updateConnectionStatus('disconnected');
        // Don't reconnect on normal closure
    } else {
        // Abnormal closure
        console.log('Connection lost, reconnecting...');
        this.reconnect();
    }
};
```

#### Timeout Handling

```rust
// In WebServer::run_with_shutdown()
pub async fn run_with_shutdown(
    self,
    shutdown_signal: Arc<Notify>,
) -> Result<()> {
    let router = self.build_router();
    let listener = tokio::net::TcpListener::bind(self.addr).await?;
    
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal.notified().await;
            tracing::info!("Web server shutting down");
            
            // Give connections 5 seconds to close
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        })
        .await?;
    
    Ok(())
}
```

### Shutdown Event

Add shutdown event to WebUIEvent:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebUIEvent {
    // ... existing variants ...
    
    /// Server is shutting down
    ShutdownInitiated {
        reason: String,
        timestamp: f64,
    },
}
```

### Key Design Decisions

1. **Shared Signal**: Single `Arc<Notify>` for all components
2. **Graceful Closure**: Send shutdown event before closing WebSocket
3. **Timeout**: 5-second grace period for connections to close
4. **No Reconnect**: Client doesn't reconnect on normal closure (code 1000)
5. **Task Cancellation**: Use `tokio::select!` for responsive shutdown

### Shutdown Scenarios

#### Scenario 1: Ctrl+C

```
User presses Ctrl+C
  → CtrlCHandler detects signal
  → shutdown_signal.notify_waiters()
  → AgentEnvironment exits loop
  → WebServer stops accepting connections
  → WebSocket handlers close connections
  → Process exits
```

#### Scenario 2: Quit Command

```
User sends quit command via UI
  → AgentEnvironment processes command
  → shutdown_signal.notify_waiters()
  → (same as Ctrl+C)
```

#### Scenario 3: Fatal Error

```
Component encounters fatal error
  → Component logs error
  → shutdown_signal.notify_waiters()
  → (same as Ctrl+C)
```

### Trade-offs

**Pros:**
- Simple coordination mechanism
- Graceful shutdown for all components
- Clients notified before disconnection
- Timeout prevents hanging

**Cons:**
- Fixed 5-second timeout (not configurable)
- All components must participate
- No partial shutdown (all or nothing)

### Alternative Considered: Separate Shutdown Signals

Each component has its own shutdown signal.

**Rejected because:**
- More complex coordination
- Risk of partial shutdown
- Harder to ensure all components shut down
- Not needed for MVP

---

## 9. Summary and Implementation Overview

### Architecture Summary

The WebUI implementation consists of three main layers:

1. **Backend Layer** (Rust)
   - WebUI component (HeadlessInterface)
   - Web Server (Axum)
   - WebSocket handlers
   - REST API handlers
   - Event serialization

2. **Protocol Layer**
   - WebSocket protocol (per-worker connections)
   - REST API (worker queries)
   - JSON message format
   - State synchronization

3. **Frontend Layer** (JavaScript)
   - Single-page application
   - WebSocket client
   - Event handling
   - UI rendering

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Browser (Frontend)                       │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  HTML/CSS/JS (Vanilla)                                 │ │
│  │  - Worker display                                      │ │
│  │  - Output rendering                                    │ │
│  │  - Input controls                                      │ │
│  │  - WebSocket client                                    │ │
│  └────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────┘
                            │ WebSocket + HTTP
┌───────────────────────────▼─────────────────────────────────┐
│                   Web Server (Axum)                          │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Routes:                                               │ │
│  │  - /ws/worker/:id (WebSocket)                          │ │
│  │  - /api/workers (REST)                                 │ │
│  │  - /api/workers/:id (REST)                             │ │
│  │  - / (Static files)                                    │ │
│  └────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                    WebUI Component                           │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  HeadlessInterface                                     │ │
│  │  - Receives AgentEnvironmentEvent                      │ │
│  │  - Converts to WebUIEvent                              │ │
│  │  - Broadcasts to WebSocket handlers                    │ │
│  └────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                   AgentEnvironment                           │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Event Multicast                                       │ │
│  │  - EventBus subscription                               │ │
│  │  - Forward to all UIs                                  │ │
│  └────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                      EventBus                                │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Broadcast Channel                                     │ │
│  │  - AgentEnvironmentEvent                               │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

#### Event Flow (Backend → Frontend)

```
Session → EventBus → AgentEnvironment → WebUI → WebSocket Handler → Browser
         (publish)   (multicast)        (convert) (filter)          (render)
```

#### Command Flow (Frontend → Backend)

```
Browser → WebSocket Handler → Session
         (JSON command)       (execute)
```

### File Structure

```
crates/chat-cli/src/cli/chat/
├── web_server/
│   ├── mod.rs              # Module exports
│   ├── server.rs           # WebServer implementation
│   ├── web_ui.rs           # WebUI component
│   ├── events.rs           # WebUIEvent types + conversion
│   ├── websocket.rs        # WebSocket handlers
│   └── api.rs              # REST API handlers
└── mod.rs                  # ChatArgs integration

web/public/
├── index.html              # Main HTML page
├── style.css               # Styles
└── app.js                  # Frontend JavaScript
```

### Dependencies

Add to `crates/chat-cli/Cargo.toml`:

```toml
[dependencies]
# Web server
axum = { version = "0.7", features = ["ws"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# Already have:
# tokio (with "net" feature)
# serde, serde_json
# uuid
# async-trait
# eyre
# tracing
```

### CLI Integration

```bash
# Start with web UI
q chat --web-ui

# Custom port
q chat --web-ui --web-port 3000

# Environment variable
Q_WEB_UI=1 q chat
```

### Testing Strategy

#### Unit Tests
- WebUIEvent conversion
- Event filtering
- State snapshot generation

#### Integration Tests
- WebSocket connection
- Event streaming
- Command handling
- Reconnection

#### Manual Tests
- Open browser to http://localhost:8080
- Send prompts
- Cancel jobs
- Reconnect after disconnect
- Multiple browser tabs

### Implementation Phases

**Phase 1: Backend Infrastructure (8-10 hours)**
- WebUIEvent types and conversion
- WebUI component
- Web Server setup
- WebSocket handler skeleton
- REST API handlers

**Phase 2: WebSocket Protocol (6-8 hours)**
- State snapshot generation
- Event streaming
- Command handling
- Connection lifecycle
- Error handling

**Phase 3: Frontend (8-10 hours)**
- HTML/CSS structure
- JavaScript application
- WebSocket client
- Event handling
- UI rendering

**Phase 4: Integration & Testing (4-6 hours)**
- CLI integration
- End-to-end testing
- Bug fixes
- Documentation

**Total Estimated Effort: 26-34 hours**

---

## 10. Risks and Mitigation

### Risk 1: WebSocket Connection Stability

**Risk:** WebSocket connections may be unstable on some networks or browsers.

**Impact:** Users experience frequent disconnections and reconnections.

**Mitigation:**
- Implement robust reconnection logic with exponential backoff
- Send state snapshot on reconnection for clean recovery
- Add connection status indicator in UI
- Test on multiple browsers and network conditions

**Likelihood:** Medium  
**Severity:** Medium

### Risk 2: Event Lag and Buffer Overflow

**Risk:** Slow clients may lag behind event stream, causing buffer overflow.

**Impact:** Clients miss events and receive `RecvError::Lagged`.

**Mitigation:**
- Use large buffer (10,000 events) in WebUI broadcast channel
- Send fresh snapshot on lag detection
- Monitor lag in production logs
- Consider rate limiting for very fast event streams

**Likelihood:** Low  
**Severity:** Low (recoverable with snapshot)

### Risk 3: Port Conflicts

**Risk:** Default port 8080 may be in use by other services.

**Impact:** Web server fails to start.

**Mitigation:**
- Make port configurable via `--web-port` flag
- Provide clear error message on port conflict
- Document port configuration in help text
- Consider auto-selecting available port (future)

**Likelihood:** Medium  
**Severity:** Low (easy workaround)

### Risk 4: Browser Compatibility

**Risk:** Frontend JavaScript may not work on older browsers.

**Impact:** Users with older browsers cannot use WebUI.

**Mitigation:**
- Use standard WebSocket API (widely supported)
- Avoid cutting-edge JavaScript features
- Test on major browsers (Chrome, Firefox, Safari, Edge)
- Document browser requirements

**Likelihood:** Low  
**Severity:** Low (modern browsers widely available)

### Risk 5: Security Vulnerabilities

**Risk:** Local-only web server may still have security issues.

**Impact:** Potential XSS, CSRF, or other attacks.

**Mitigation:**
- Bind to 127.0.0.1 only (no remote access)
- Validate all input from WebSocket commands
- Use serde_json for safe JSON parsing
- Escape output in frontend
- Document security considerations

**Likelihood:** Low (local-only)  
**Severity:** Medium

### Risk 6: Time Conversion Accuracy

**Risk:** Converting Instant to SystemTime may have edge cases.

**Impact:** Timestamps in UI may be slightly inaccurate.

**Mitigation:**
- Initialize time conversion at process start
- Use consistent conversion function
- Document time conversion approach
- Consider using SystemTime directly in events (future)

**Likelihood:** Low  
**Severity:** Low (timestamps are informational)

---

## 11. Future Enhancements

### Post-MVP Features

#### Multiple Worker Support
- Worker list view
- Create/delete workers from UI
- Switch between workers
- Worker tabs or sidebar

#### Advanced UI Features
- Syntax highlighting for code (highlight.js)
- Markdown rendering for responses
- Dark mode toggle
- Responsive mobile design
- Keyboard shortcuts

#### Conversation Management
- View full conversation history
- Export conversation (markdown, JSON)
- Clear conversation
- Search conversation

#### Tool Visualization
- Tool use timeline
- Tool input/output formatting
- Tool approval UI (when tools are implemented)

#### Performance Monitoring
- Token usage display
- Response time metrics
- Event rate monitoring
- Connection quality indicator

#### Remote Access (with Security)
- Authentication (API keys, OAuth)
- HTTPS support
- Rate limiting
- Access control

#### Developer Features
- Event log viewer
- Debug mode
- WebSocket message inspector
- Performance profiler

### Technical Improvements

#### State Management
- Event replay for reconnection
- Persistent conversation history
- Client-side caching

#### Performance
- Event batching for high-frequency updates
- Compression for large messages
- Lazy loading for long conversations

#### Testing
- Automated browser tests (Playwright)
- Load testing for multiple clients
- Chaos testing for connection stability

---

## 12. Conclusion

### Design Completeness

This design document provides a complete architecture for implementing a web-based UI for the Q CLI agent environment. All major components are specified:

✅ WebUIEvent serializable types  
✅ WebSocket protocol  
✅ REST API endpoints  
✅ WebUI component  
✅ Web Server architecture  
✅ State synchronization strategy  
✅ Frontend architecture  
✅ Shutdown coordination  

### Key Strengths

1. **Clean Architecture**: Separation of concerns between backend, protocol, and frontend
2. **Event-Driven**: Leverages existing EventBus for real-time updates
3. **Simple Protocol**: JSON over WebSocket, easy to implement and debug
4. **Robust Reconnection**: Handles disconnections gracefully with state snapshots
5. **No Build Process**: Vanilla JavaScript, no compilation required
6. **Extensible**: Easy to add features post-MVP

### Implementation Readiness

The design is ready for implementation. All technical decisions have been made, and the architecture integrates cleanly with the existing agent_env system.

**Next Steps:**
1. Review design with team
2. Create implementation plan
3. Begin Phase 1 (Backend Infrastructure)

### Success Criteria

The implementation will be considered successful when:

- ✅ Web server starts on `q chat --web-ui`
- ✅ Browser can connect to http://localhost:8080
- ✅ Worker state displays correctly
- ✅ Streaming output appears in real-time
- ✅ Prompts can be sent from UI
- ✅ Jobs can be cancelled from UI
- ✅ Reconnection works after disconnect
- ✅ Shutdown is graceful and coordinated

---

## Appendix: Quick Reference

### WebSocket URL
```
ws://localhost:8080/ws/worker/{worker_id}
```

### REST API Endpoints
```
GET  /api/health
GET  /api/workers
GET  /api/workers/:id
```

### WebSocket Commands
```json
{"type": "prompt", "text": "..."}
{"type": "cancel"}
{"type": "ping"}
```

### CLI Flags
```bash
--web-ui              # Enable web UI
--web-port <PORT>     # Set port (default: 8080)
```

### Environment Variables
```bash
Q_WEB_UI=1           # Enable web UI
Q_WEB_PORT=3000      # Set port
```

### File Locations
```
crates/chat-cli/src/cli/chat/web_server/  # Backend
web/public/                                # Frontend
```

