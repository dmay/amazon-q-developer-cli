# MVP WebUI - Research Document

## Overview

This document contains research findings about the existing Q CLI system components that are relevant to implementing a web-based user interface. The goal is to understand the current architecture, identify integration points, and surface potential challenges before moving to the design phase.

---

## 1. Reference Implementation Analysis (web-q)

### Architecture Overview

The web-q reference implementation at `/Volumes/workplace/web-q/` provides a working example of a web-based terminal interface for Q Chat. Key architectural components:

**Backend Stack:**
- **Framework**: Express.js (Node.js)
- **WebSocket**: `ws` library for real-time communication
- **Terminal**: `node-pty` for spawning Q Chat processes
- **Terminal Emulation**: `@xterm/headless` for server-side terminal state

**Frontend Stack:**
- **Framework**: Vanilla JavaScript (no framework)
- **Terminal Display**: `@xterm/xterm` for browser-based terminal rendering
- **Communication**: Standard WebSocket API + REST fetch API

### API Structure

**REST Endpoints:**
```
GET    /api/tasks           - List all tasks
POST   /api/tasks           - Create new task
GET    /api/tasks/:id       - Get task details
PUT    /api/tasks/:id       - Update task name
DELETE /api/tasks/:id       - Delete task
GET    /api/tasks/:id/qlog  - Get parsed conversation bubbles (Q Chat parsing)
GET    /api/config          - Get server configuration
```

**WebSocket Protocol:**
- **URL Pattern**: `/ws/task/:id`
- **Message Format**: JSON with `type` field
- **Input Message**: `{"type": "input", "data": "user typed content"}`
- **Output**: Raw terminal data streamed to client

### Key Design Patterns

1. **Session Management**: 
   - `SessionManager` interface with pluggable implementations
   - `InMemorySessionManager` for current implementation
   - Each task wraps a `TerminalSession` which manages PTY process

2. **Multi-Client Support**:
   - Multiple WebSocket clients can connect to same task
   - Each task maintains a Set of connected clients
   - Terminal output broadcast to all clients
   - Any client can send input

3. **Q Chat Parsing** (Phase 2.1 feature):
   - `QChatParser` transforms raw terminal output into structured "bubbles"
   - Bubble types: initMessage, humanRequest, aiResponse, toolUse, thinking
   - `BubbleStore` manages bubble collection
   - Separate WebSocket endpoint for real-time bubble updates
   - Escape sequence handling for ANSI codes

4. **Graceful Shutdown**:
   - SIGINT handler notifies all WebSocket clients
   - Sends red terminal message to active sessions
   - Cleans up all tasks and terminates processes
   - Closes HTTP server before exit

### Relevant Code Locations

- **Server**: `src/server.js` - Main entry point, HTTP/WebSocket setup
- **Session Manager**: `src/session/InMemorySessionManager.js`
- **Terminal Session**: `src/session/TerminalSession.js`
- **WebSocket Handler**: `src/websocket/TerminalWebSocketHandler.js`
- **Q Chat Parser**: `src/parsing/QChatParserV2.js`
- **REST API**: `src/routes/taskRoutes.js`
- **Frontend**: `public/js/app.js`, `public/js/taskManager.js`

### Applicability to Q CLI

**Directly Applicable:**
- WebSocket URL pattern and message format
- Multi-client connection management
- Static file serving approach
- Graceful shutdown coordination

**Needs Adaptation:**
- Replace Express.js with Axum (Rust)
- Replace node-pty with existing Q CLI Session/Worker architecture
- Replace QChatParser with event-based streaming (EventBus)
- Integrate with existing AgentEnvironment instead of separate SessionManager

---

## 2. EventBus and Event Streaming Architecture

### EventBus Implementation

**Location**: `crates/chat-cli/src/agent_env/event_bus.rs`

The EventBus is a central event distribution system built on tokio broadcast channels:

```rust
pub struct EventBus {
    sender: broadcast::Sender<AgentEnvironmentEvent>,
    buffer_size: usize,
}
```

**Key Characteristics:**
- **Pattern**: Publish-subscribe with tokio broadcast channels
- **Buffer Size**: Configurable (default 1000 events)
- **Cloneable**: EventBus is `Clone`, can be shared across components
- **Subscriber Count**: Can query current subscriber count
- **Lag Handling**: Receivers get `RecvError::Lagged(n)` when buffer overflows

**API:**
```rust
pub fn new(buffer_size: usize) -> Self
pub fn publish(&self, event: AgentEnvironmentEvent)
pub fn subscribe(&self) -> broadcast::Receiver<AgentEnvironmentEvent>
pub fn subscriber_count(&self) -> usize
```

### Event Types

**Location**: `crates/chat-cli/src/agent_env/events.rs`

Events are organized in a nested enum structure:

```rust
pub enum AgentEnvironmentEvent {
    Worker(WorkerEvent),
    Job(JobEvent),
    AgentLoop(AgentLoopEvent),
    System(SystemEvent),
}
```

**Worker Events:**
- `Created { worker_id, name, timestamp }`
- `Deleted { worker_id, timestamp }`
- `LifecycleStateChanged { worker_id, old_state, new_state, timestamp }`

**Job Events:**
- `Started { worker_id, job_id, task_type, timestamp }`
- `Completed { worker_id, job_id, result, timestamp }`
- `OutputChunk { worker_id, job_id, chunk, timestamp }`

**Output Chunk Types:**
```rust
pub enum OutputChunk {
    AssistantResponse(String),
    ToolUse { tool_name, tool_input },
    ToolResult { tool_name, result },
}
```

**AgentLoop Events:**
- `ResponseReceived { worker_id, job_id, text, timestamp }`
- `ToolUseRequestReceived { worker_id, job_id, tool_name, tool_input, timestamp }`

**System Events:**
- `ShutdownInitiated { reason, timestamp }`

### Event Helpers

All events provide:
- `worker_id()` - Extract worker_id if present
- `timestamp()` - Get event timestamp (Instant)
- Type checking: `is_worker_event()`, `is_job_event()`, etc.

### Integration Points for WebUI

1. **Subscribe to EventBus**: WebUI can subscribe to receive all events
2. **Filter by Worker**: Use `event.worker_id()` to route events to specific workers
3. **Convert to JSON**: Need serializable representation (Instant → timestamp)
4. **Broadcast to Clients**: Forward events to WebSocket clients

---

## 3. UI Implementation Patterns

### UserInterface Trait

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn start(&self) -> Result<()>;
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

**Responsibilities:**
- `start()`: Initialize UI, spawn tasks, return immediately
- `command_receiver()`: Provide channel for sending commands to AgentEnvironment
- `handle_event()`: Process events from EventBus

### HeadlessInterface Trait

```rust
#[async_trait]
pub trait HeadlessInterface: Send + Sync {
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

**Purpose**: Non-interactive UIs that only observe events (no command input)

### TextUi Implementation

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Architecture:**
- Uses `InputHandler` for readline-style input
- Spawns separate task for prompt loop
- Uses `Arc<Notify>` for prompt readiness signaling
- Handles Ctrl+C via `CtrlCHandler`
- Maintains command channel for sending to AgentEnvironment

**Key Patterns:**
```rust
pub struct TextUi {
    session: Arc<Session>,
    main_worker_id: Uuid,
    input_handler: Arc<tokio::sync::Mutex<InputHandler>>,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<std::sync::Mutex<Option<mpsc::Receiver<PromptResult>>>>,
    prompt_ready: Arc<Notify>,
    shutdown_signal: Arc<Notify>,
    interactive: bool,
}
```

**Event Handling:**
- Filters events by worker_id
- Updates prompt state based on worker lifecycle
- Displays output chunks as they arrive
- Signals prompt readiness when worker becomes Idle

### StructuredIO Implementation

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**Architecture:**
- Reads single-line prompts from stdin
- Outputs structured JSON events to stdout
- Suitable for scripting and automation
- Spawns reader task with responsive shutdown

**JSON Input Format:**
```json
{"command": "prompt", "worker_id": "uuid", "text": "user input"}
{"command": "quit"}
```

**JSON Output Format:**
```json
{"type": "worker_created", "worker_id": "uuid", "name": "worker name"}
{"type": "output_chunk", "worker_id": "uuid", "chunk": "text"}
{"type": "job_completed", "worker_id": "uuid", "success": true}
```

**Key Patterns:**
- Uses separate reader task for stdin (blocking I/O)
- Processor loop with `tokio::select!` for responsive shutdown
- Parses JSON commands or treats as plain text
- Defaults to first worker if worker_id not specified

### Common Patterns Across UIs

1. **Command Channel**: `mpsc::channel` for sending commands to AgentEnvironment
2. **Shutdown Signal**: `Arc<Notify>` for coordinated shutdown
3. **Session Access**: `Arc<Session>` for querying worker state
4. **Async Tasks**: Spawn separate tasks for I/O operations
5. **Event Filtering**: Filter events by worker_id for relevant updates

---

## 4. AgentEnvironment Coordination

### Architecture

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

```rust
pub struct AgentEnvironment {
    session: Arc<Session>,
    event_bus: EventBus,
    main_ui: Option<Arc<dyn UserInterface>>,
    headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    shutdown_signal: Arc<Notify>,
    interactive: bool,
}
```

**Responsibilities:**
- Coordinate event multicasting to all UIs
- Process commands from main UI
- Manage shutdown coordination
- Monitor job completion in non-interactive mode

### Event Multicasting

**Pattern**: Spawn dedicated task that subscribes to EventBus and forwards to all UIs

```rust
fn spawn_event_multicast(&self) -> JoinHandle<()> {
    let mut receiver = self.event_bus.subscribe();
    let headless_uis = self.headless_uis.clone();
    let main_ui = self.main_ui.clone();
    
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    // Forward to main UI
                    if let Some(ui) = &main_ui {
                        ui.handle_event(event.clone()).await;
                    }
                    // Forward to headless UIs
                    for headless_ui in &headless_uis {
                        headless_ui.handle_event(event.clone()).await;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("Event bus lagged by {} events", n);
                }
                Err(RecvError::Closed) => break,
            }
        }
    })
}
```

### Command Processing

**Flow**: UI → PromptResult → AgentEnvironmentCommand → Session

```rust
pub enum AgentEnvironmentCommand {
    Prompt { worker_id: Uuid, text: String },
    Compact { worker_id: Uuid, instruction: String },
    Quit,
}

pub enum PromptResult {
    Command(AgentEnvironmentCommand),
    Shutdown,
}
```

**Command Handling:**
- `Prompt`: Add message to conversation history, launch agent loop
- `Compact`: Launch compact conversation task
- `Quit`: Trigger shutdown signal

### Main Execution Loop

```rust
pub async fn run(&self) -> Result<()> {
    // Start Ctrl+C handler
    let ctrl_c_handler = Arc::new(CtrlCHandler::new(...));
    ctrl_c_handler.start_listening();
    
    // Spawn event multicast task
    let multicast_handle = self.spawn_event_multicast();
    
    // Start main UI
    if let Some(ui) = &self.main_ui {
        ui.start().await?;
        let mut cmd_receiver = ui.command_receiver();
        
        // Process commands
        loop {
            tokio::select! {
                Some(result) = cmd_receiver.recv() => {
                    // Handle command
                }
                _ = self.shutdown_signal.notified() => {
                    break;
                }
            }
        }
    }
    
    // Cleanup
    multicast_handle.abort();
    Ok(())
}
```

### Integration Points for WebUI

1. **Headless UI**: WebUI should implement `HeadlessInterface` (no stdin/stdout)
2. **Command Channel**: WebUI needs its own command channel for WebSocket commands
3. **Event Subscription**: WebUI subscribes to EventBus via AgentEnvironment multicast
4. **Shutdown Coordination**: WebUI participates in shutdown via `Arc<Notify>`
5. **Session Access**: WebUI can query worker state via `Arc<Session>`

---

## 5. Web Framework Options

### Current Dependencies

**Location**: `crates/chat-cli/Cargo.toml`

Already available:
- `tokio` - Async runtime
- `tokio-tungstenite` - WebSocket support (tokio-native)
- `hyper` - HTTP primitives
- `serde_json` - JSON serialization

### Recommended: Axum

**Why Axum:**
- Built on tokio and hyper (already in dependencies)
- Excellent tokio integration (native async/await)
- Type-safe routing and extractors
- Tower middleware ecosystem
- Active development and community

**Additional Dependencies Needed:**
- `axum` - Web framework
- `tower-http` - Static file serving, CORS
- `tower` - Middleware utilities

**Example Integration:**
```rust
use axum::{
    Router,
    routing::{get, post},
    extract::{State, Path, WebSocketUpgrade},
    response::IntoResponse,
};
use tower_http::services::ServeDir;

let app = Router::new()
    .route("/ws", get(websocket_handler))
    .route("/api/workers", get(list_workers))
    .route("/api/workers/:id/prompt", post(send_prompt))
    .nest_service("/", ServeDir::new("web/public"))
    .with_state(AppState {
        session: session.clone(),
        event_bus: event_bus.clone(),
    });
```

### Alternatives Considered

**Warp:**
- Also tokio-based
- Filter-based routing (less intuitive)
- Less active development than Axum

**Actix-web:**
- Different async runtime (actix)
- Would require runtime bridging
- Not recommended for tokio-heavy codebase

---

## 6. Potential Pitfalls and Challenges

### 6.1 Event Serialization

**Problem**: `AgentEnvironmentEvent` uses `Instant` (monotonic time, not serializable)

**Impact**: Cannot directly serialize events to JSON for WebSocket

**Solution Options:**
1. Create separate serializable event types for WebUI
2. Convert `Instant` to `SystemTime` or timestamp during serialization
3. Use serde with custom serializer for Instant

**Recommendation**: Create `WebUIEvent` enum that mirrors `AgentEnvironmentEvent` but uses serializable types

### 6.2 Event Lag and Buffer Management

**Problem**: Broadcast channel can lag if WebSocket clients are slow to consume events

**Impact**: 
- Clients receive `RecvError::Lagged(n)` and miss events
- Need to handle reconnection and state synchronization

**Solution Options:**
1. Increase buffer size for WebUI subscribers
2. Implement event replay mechanism
3. Send full state snapshot on reconnection
4. Use separate broadcast channel for WebUI with larger buffer

**Recommendation**: Combination of larger buffer + state snapshot on connect

### 6.3 Worker ID Routing

**Problem**: Events contain worker_id, but WebUI needs to handle multiple workers

**Impact**: Need filtering/routing logic to send events to correct WebSocket clients

**Solution Options:**
1. One WebSocket per worker (like web-q: `/ws/worker/:id`)
2. Single WebSocket with client-side filtering
3. Server-side filtering based on client subscriptions

**Recommendation**: One WebSocket per worker for simplicity (matches web-q pattern)

### 6.4 Shutdown Coordination

**Problem**: Multiple shutdown sources (Ctrl+C, WebSocket close, AgentEnvironment quit)

**Impact**: Need careful coordination to avoid race conditions

**Solution Options:**
1. Single shutdown signal (`Arc<Notify>`) shared by all components
2. Web server shutdown triggers AgentEnvironment shutdown
3. AgentEnvironment shutdown triggers web server shutdown

**Recommendation**: AgentEnvironment owns shutdown, web server participates via signal

### 6.5 Static File Serving

**Problem**: Need to serve HTML/JS/CSS files for frontend

**Impact**: Requires file serving capability in web server

**Solution**: Use `tower-http::services::ServeDir` with Axum

**Example:**
```rust
.nest_service("/", ServeDir::new("web/public"))
```

### 6.6 CORS Configuration

**Problem**: If frontend accessed from different origin, need CORS headers

**Impact**: Browser will block WebSocket and API requests

**Solution**: Use `tower-http::cors::CorsLayer` for development

**Recommendation**: Local-only by default (127.0.0.1), CORS for development mode

### 6.7 Port Configuration

**Problem**: Web server port might conflict with other services

**Impact**: Server fails to start if port in use

**Solution**: Make port configurable via CLI argument or environment variable

**Recommendation**: Default to 8080, allow override via `--web-port` flag

### 6.8 Session and EventBus Sharing

**Problem**: Need to share Session and EventBus with web server

**Impact**: Must ensure thread-safety and proper Arc usage

**Solution**: Both are already Arc-wrapped and Clone, safe to share

**Verification:**
- `Session` is `Arc<Session>` ✓
- `EventBus` is `Clone` ✓
- Both are `Send + Sync` ✓

### 6.9 Command Channel Architecture

**Problem**: WebUI needs to send commands to AgentEnvironment

**Impact**: Need command channel separate from main UI

**Solution Options:**
1. WebUI implements `HeadlessInterface` + separate command channel
2. WebUI implements `UserInterface` (but no stdin/stdout)
3. Direct command injection via shared channel

**Recommendation**: WebUI as `HeadlessInterface` + commands via REST API or WebSocket messages

### 6.10 Time Representation

**Problem**: Events use `Instant` (monotonic), but web needs wall-clock time

**Impact**: Cannot display absolute timestamps in UI

**Solution Options:**
1. Convert `Instant` to `SystemTime` during serialization
2. Store both `Instant` and `SystemTime` in events
3. Calculate wall-clock time from Instant + start time

**Recommendation**: Convert to ISO 8601 timestamp during WebUI event conversion

### 6.11 Binary Data in Output

**Problem**: `OutputChunk` might contain binary data or special characters

**Impact**: JSON encoding might fail or produce invalid JSON

**Solution**: Use base64 encoding for binary data, escape special characters

**Recommendation**: Treat all output as UTF-8 text, use serde_json's string escaping

### 6.12 WebSocket Connection Lifecycle

**Problem**: Need to handle connection, disconnection, reconnection gracefully

**Impact**: State synchronization, event replay, cleanup

**Solution Pattern:**
```rust
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (sender, receiver) = socket.split();
    
    // Send initial state snapshot
    send_state_snapshot(&sender, &state).await;
    
    // Subscribe to events
    let mut event_rx = state.event_bus.subscribe();
    
    // Spawn sender task
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Convert and send event
        }
    });
    
    // Spawn receiver task
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Handle incoming commands
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
    
    // Cleanup
}
```

---

## 7. Key Code Elements for Implementation

### 7.1 Event Conversion

Need to create serializable event types:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WebUIEvent {
    WorkerCreated {
        worker_id: String,
        name: String,
        timestamp: String, // ISO 8601
    },
    WorkerDeleted {
        worker_id: String,
        timestamp: String,
    },
    WorkerStateChanged {
        worker_id: String,
        old_state: String,
        new_state: String,
        timestamp: String,
    },
    JobStarted {
        worker_id: String,
        job_id: String,
        task_type: String,
        timestamp: String,
    },
    JobCompleted {
        worker_id: String,
        job_id: String,
        success: bool,
        timestamp: String,
    },
    OutputChunk {
        worker_id: String,
        job_id: String,
        chunk: String,
        timestamp: String,
    },
}

impl From<AgentEnvironmentEvent> for WebUIEvent {
    fn from(event: AgentEnvironmentEvent) -> Self {
        // Convert Instant to ISO 8601 timestamp
        // Convert Uuid to String
        // Flatten nested enums
    }
}
```

### 7.2 WebSocket Message Types

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketCommand {
    Prompt {
        worker_id: String,
        text: String,
    },
    Cancel {
        worker_id: String,
    },
}
```

### 7.3 Application State

```rust
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Session>,
    pub event_bus: EventBus,
    pub cmd_sender: mpsc::Sender<PromptResult>,
}
```

### 7.4 WebUI Structure

```rust
pub struct WebUI {
    session: Arc<Session>,
    event_bus: EventBus,
    cmd_sender: mpsc::Sender<PromptResult>,
    shutdown_signal: Arc<Notify>,
}

#[async_trait]
impl HeadlessInterface for WebUI {
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Convert to WebUIEvent
        // Broadcast to all WebSocket clients
    }
}
```

---

## 8. Summary and Recommendations

### Key Findings

1. **EventBus is Well-Suited**: The existing EventBus architecture is perfect for WebUI integration. It already supports multiple subscribers and handles event distribution.

2. **UI Patterns are Established**: TextUi and StructuredIO provide clear patterns for implementing new UIs. WebUI should follow similar patterns.

3. **Web-q Provides Blueprint**: The web-q reference implementation demonstrates a working web-based terminal interface with similar requirements.

4. **Axum is Best Choice**: Axum provides excellent tokio integration and is the most natural fit for the existing codebase.

5. **Serialization is Main Challenge**: Converting events from internal representation (Instant, Uuid) to web-friendly format (timestamps, strings) is the primary technical challenge.

### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     AgentEnvironment                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   TextUi     │  │ StructuredIO │  │    WebUI     │      │
│  │ (main UI)    │  │ (headless)   │  │ (headless)   │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│                     ┌──────▼──────┐                          │
│                     │  EventBus   │                          │
│                     └──────┬──────┘                          │
│                            │                                 │
│                     ┌──────▼──────┐                          │
│                     │   Session   │                          │
│                     └─────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                             │
                             │
                    ┌────────▼────────┐
                    │   Web Server    │
                    │     (Axum)      │
                    ├─────────────────┤
                    │ Static Files    │
                    │ REST API        │
                    │ WebSocket       │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │    Browser      │
                    │  (Frontend)     │
                    └─────────────────┘
```

### Next Steps for Design Phase

1. **Define WebUIEvent Types**: Create serializable event types
2. **Design WebSocket Protocol**: Define message formats for commands and events
3. **Design REST API**: Define endpoints for worker management
4. **Design WebUI Component**: Implement HeadlessInterface
5. **Design Web Server**: Axum server with routing and handlers
6. **Design Frontend**: HTML/JS/CSS structure
7. **Design State Synchronization**: Initial state + event streaming
8. **Design Shutdown Flow**: Coordinated shutdown across components

### Critical Design Decisions

1. **WebSocket per Worker vs Single WebSocket**: Recommend per-worker (simpler, matches web-q)
2. **HeadlessInterface vs UserInterface**: Recommend HeadlessInterface (no stdin/stdout)
3. **Command Channel**: Recommend WebSocket messages for commands (no separate channel)
4. **Event Buffering**: Recommend larger buffer + state snapshot on connect
5. **Time Representation**: Recommend ISO 8601 timestamps in WebUIEvent
6. **Port Configuration**: Recommend CLI flag `--web-ui` with optional `--web-port`

---

## Appendix: File Locations Reference

### Q CLI Codebase
- EventBus: `crates/chat-cli/src/agent_env/event_bus.rs`
- Events: `crates/chat-cli/src/agent_env/events.rs`
- AgentEnvironment: `crates/chat-cli/src/agent_env/agent_environment.rs`
- TextUi: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`
- StructuredIO: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
- Session: `crates/chat-cli/src/agent_env/session.rs`
- ChatArgs: `crates/chat-cli/src/cli/chat/mod.rs`

### Web-q Reference
- Server: `/Volumes/workplace/web-q/src/server.js`
- Session Manager: `/Volumes/workplace/web-q/src/session/InMemorySessionManager.js`
- Terminal Session: `/Volumes/workplace/web-q/src/session/TerminalSession.js`
- WebSocket Handler: `/Volumes/workplace/web-q/src/websocket/TerminalWebSocketHandler.js`
- REST API: `/Volumes/workplace/web-q/src/routes/taskRoutes.js`
- Q Chat Parser: `/Volumes/workplace/web-q/src/parsing/QChatParserV2.js`
- Frontend: `/Volumes/workplace/web-q/public/js/`
- Documentation: `/Volumes/workplace/web-q/codebase/`
