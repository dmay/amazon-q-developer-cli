# MVP WebUI - Research and Analysis

## Overview

This document analyzes the existing architecture and identifies key elements for implementing a web-based UI for the agent_env system. The WebUI will leverage the event-driven architecture to provide a browser-based interface for interacting with workers.

## Related System Elements

### 1. EventBus Architecture (Core Foundation)

**Location**: `crates/chat-cli/src/agent_env/event_bus.rs`

**Key Characteristics**:
- Uses tokio `broadcast::Sender` for efficient multicasting
- Default buffer size: 1000 events
- Handles lagged events gracefully (drops oldest when buffer full)
- Cheap to clone (Arc internally)
- Non-blocking publish (never waits for subscribers)

**Event Types** (`crates/chat-cli/src/agent_env/events.rs`):
```rust
pub enum AgentEnvironmentEvent {
    Worker(WorkerEvent),      // Created, Deleted, LifecycleStateChanged
    Job(JobEvent),            // Started, Completed, OutputChunk
    AgentLoop(AgentLoopEvent), // ResponseReceived, ToolUseRequestReceived
    System(SystemEvent),      // ShutdownInitiated
}
```

**Relevance to WebUI**:
- WebUI will subscribe to EventBus like other UIs
- Events are already Clone-able for multicasting
- Need to serialize events to JSON for WebSocket transmission
- Buffer size may need tuning for slow WebSocket connections

**Potential Issues**:
- Events contain `Instant` timestamps (not serializable to JSON by default)
- Some event data (like `JobCompletionResult`) contains complex types
- Need to create JSON-friendly event representations

---

### 2. AgentEnvironment Coordinator

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

**Key Characteristics**:
- Manages event multicasting to all UIs
- Processes commands from main UI via mpsc channel
- Supports one main UI + multiple headless UIs
- Coordinates shutdown and cleanup

**UI Trait Definitions**:
```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn start(&self) -> Result<()>;
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}

#[async_trait]
pub trait HeadlessInterface: Send + Sync {
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

**Relevance to WebUI**:
- WebUI could be implemented as either UserInterface or HeadlessInterface
- If main UI: needs command channel for sending commands
- If headless: only receives events (simpler, but less interactive)
- **Decision needed**: Should WebUI be main UI or headless?

**Architecture Options**:

**Option A: WebUI as Main UI**
- Pros: Can send commands directly, full control
- Cons: Only one main UI allowed, conflicts with TextUi/StructuredIO
- Use case: Web-only mode

**Option B: WebUI as Headless UI**
- Pros: Can run alongside TextUi, multiple instances possible
- Cons: Needs separate command channel mechanism
- Use case: Monitoring/observability

**Option C: Hybrid Approach**
- WebUI runs as separate service with its own command handling
- Subscribes to EventBus as headless UI
- Provides REST API for commands
- Use case: Most flexible, supports multiple web clients

**Recommendation**: Option C (Hybrid) - most flexible and scalable

---

### 3. Existing UI Implementations

#### TextUi (Reference Implementation)

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Key Patterns**:
- **Prompt Queue Pattern**: Only reads input when worker is Idle
- Uses `Notify` to signal when prompt is ready
- Handles UI commands internally (/usage, /context, /status, /workers)
- Forwards agent commands to AgentEnvironment
- Spawns separate task for prompt loop

**Event Handling**:
```rust
async fn handle_event(&self, event: AgentEnvironmentEvent) {
    // Filter by worker_id
    if let Some(wid) = event.worker_id() {
        if wid != self.main_worker_id {
            return;
        }
    }
    
    match event {
        AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
            // Print output with flush
        }
        AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { 
            new_state: WorkerLifecycleState::Idle, .. 
        }) => {
            // Signal prompt_ready
        }
        _ => {}
    }
}
```

**Relevance to WebUI**:
- Similar event filtering needed (by worker_id)
- Need to convert OutputChunk to WebSocket messages
- Lifecycle state changes should update UI state
- Prompt queue pattern may not apply (web is always-ready)

#### StructuredIO (JSON Output Reference)

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**Key Patterns**:
- **Always-Reading Pattern**: Continuously reads stdin
- Outputs JSON for events
- No prompt queue (suitable for piped input)

**JSON Output Format**:
```json
{"worker_id": "...", "assistant_response": "..."}
{"worker_id": "...", "tool_use_request": {"tool_name": "...", "tool_input": {...}}}
{"worker_id": "...", "lifecycle_state": "idle"}
```

**Relevance to WebUI**:
- Good reference for JSON serialization
- Similar event-to-JSON conversion needed
- WebSocket messages will use similar structure

---

### 4. Command System

**Location**: `crates/chat-cli/src/agent_env/commands.rs`

**Command Types**:
```rust
pub enum AgentEnvironmentCommand {
    Prompt { worker_id: Uuid, text: String },
    Compact { worker_id: Uuid, instruction: Option<String> },
    Quit,
}

pub enum UiCommand {
    Usage,
    Context,
    Status,
    Workers,
}
```

**Relevance to WebUI**:
- Need to parse commands from WebSocket messages
- REST API should support same command types
- JSON command format needed:
  ```json
  {"type": "prompt", "worker_id": "...", "text": "..."}
  {"type": "cancel", "worker_id": "..."}
  ```

---

### 5. Session and Worker Management

**Location**: `crates/chat-cli/src/agent_env/session.rs`, `worker.rs`

**Key Characteristics**:
- Session manages multiple workers
- Workers have lifecycle states (Idle, Busy, IdleFailed)
- Workers have task metadata (extensible HashMap)
- Session provides `get_worker()`, `list_workers()`, etc.

**Relevance to WebUI**:
- Need API endpoints to list workers
- Need to display worker states in UI
- Initial state loading on WebSocket connect
- Worker creation/deletion support (future)

**API Endpoints Needed**:
- `GET /api/workers` - List all workers
- `GET /api/workers/:id` - Get worker details
- `POST /api/workers/:id/prompt` - Send prompt
- `POST /api/workers/:id/cancel` - Cancel job
- `WS /ws/events` - Event stream

---

## Reference Implementation Analysis

### web-q Project Structure

**Location**: `/Volumes/workplace/web-q/`

**Key Components**:
1. **Express Server** (`src/server.js`):
   - HTTP server for static files
   - WebSocket server for real-time communication
   - REST API for task management
   - Graceful shutdown handling

2. **WebSocket Handler** (`src/websocket/`):
   - Connection routing by URL path
   - Client management per task
   - Message forwarding (input/output)
   - Multi-client support

3. **Frontend** (`public/`):
   - Single-page application
   - xterm.js for terminal display
   - Task list sidebar
   - Tab-based interface (Terminal/Chat)

**Relevant Patterns**:
- **URL-based routing**: `/ws/task/:taskId` for WebSocket connections
- **Client sets**: Multiple browsers can connect to same task
- **Broadcast pattern**: Output sent to all connected clients
- **JSON messages**: Structured communication protocol

**Differences from agent_env**:
- web-q manages terminal sessions (PTY)
- agent_env manages AI agent workers
- web-q has task creation/deletion
- agent_env has event-driven architecture

---

## Technical Challenges and Solutions

### Challenge 1: Event Serialization

**Problem**: Events contain non-serializable types (Instant, complex enums)

**Solution**:
- Create separate JSON-friendly event types
- Convert `Instant` to ISO 8601 timestamps
- Flatten complex enums to simple structures

**Example**:
```rust
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum WebUIEvent {
    WorkerCreated {
        worker_id: String,
        name: String,
        timestamp: String, // ISO 8601
    },
    OutputChunk {
        worker_id: String,
        job_id: String,
        chunk_type: String, // "assistant_response", "tool_use", "tool_result"
        data: serde_json::Value,
    },
    // ...
}
```

### Challenge 2: WebSocket Connection Management

**Problem**: Multiple browser tabs connecting to same worker

**Solution**:
- Maintain `HashMap<Uuid, HashSet<WebSocketId>>` for worker-to-clients mapping
- Broadcast events to all clients of a worker
- Handle disconnections gracefully
- Send initial state snapshot on connect

**Implementation Pattern** (from web-q):
```rust
struct WebSocketManager {
    clients: Arc<Mutex<HashMap<Uuid, HashSet<WebSocketId>>>>,
}

impl WebSocketManager {
    async fn add_client(&self, worker_id: Uuid, ws_id: WebSocketId) {
        let mut clients = self.clients.lock().await;
        clients.entry(worker_id).or_default().insert(ws_id);
    }
    
    async fn broadcast(&self, worker_id: Uuid, message: &str) {
        let clients = self.clients.lock().await;
        if let Some(client_set) = clients.get(&worker_id) {
            for ws_id in client_set {
                // Send message to each client
            }
        }
    }
}
```

### Challenge 3: Web Server Integration

**Problem**: Running HTTP/WebSocket server in same process as CLI

**Solution Options**:

**Option A: Embedded Server**
- Run axum server in tokio task
- Share Session and EventBus via Arc
- Pros: Simple deployment, single process
- Cons: Port conflicts, resource sharing

**Option B: Separate Process**
- Run web server as separate binary
- Communicate via IPC or shared EventBus
- Pros: Isolation, independent scaling
- Cons: Complex deployment, IPC overhead

**Recommendation**: Option A (Embedded) for MVP

**Implementation**:
```rust
// In ChatArgs::execute()
if enable_web_ui {
    let web_server = WebServer::new(
        "127.0.0.1:8080".parse().unwrap(),
        session.clone(),
        event_bus.clone(),
    );
    
    tokio::spawn(async move {
        if let Err(e) = web_server.run().await {
            error!("Web server error: {}", e);
        }
    });
    
    info!("Web UI available at http://127.0.0.1:8080");
}
```

### Challenge 4: Initial State Loading

**Problem**: New WebSocket connections need current worker state

**Solution**:
- Send snapshot of worker state on connect
- Include: worker list, current states, recent output
- Use Session API to query current state

**Snapshot Format**:
```json
{
  "type": "snapshot",
  "workers": [
    {
      "id": "...",
      "name": "main",
      "state": "idle",
      "metadata": {...}
    }
  ],
  "recent_output": [
    {"worker_id": "...", "chunk": "..."}
  ]
}
```

### Challenge 5: Command Authentication

**Problem**: WebSocket commands need validation

**Solution** (MVP - local only):
- No authentication for localhost
- Validate worker_id exists
- Validate command format
- Rate limiting (future)

**Future Considerations**:
- Token-based auth for remote access
- User-based access control
- Command audit logging

---

## Architecture Recommendations

### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      ChatArgs::execute()                     │
│  ┌────────────┐  ┌─────────┐  ┌────────┐  ┌──────────────┐ │
│  │  EventBus  │  │ Session │  │ Worker │  │ AgentEnviron │ │
│  └────────────┘  └─────────┘  └────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
         │                │
         │                │
         ▼                ▼
┌─────────────────────────────────────┐
│         WebServer (axum)             │
│  ┌────────────┐  ┌────────────────┐ │
│  │ HTTP/REST  │  │   WebSocket    │ │
│  │   API      │  │    Handler     │ │
│  └────────────┘  └────────────────┘ │
│  ┌────────────────────────────────┐ │
│  │      Static File Serving       │ │
│  └────────────────────────────────┘ │
└─────────────────────────────────────┘
         │                │
         │                │
         ▼                ▼
┌─────────────────────────────────────┐
│          Browser (Frontend)          │
│  ┌────────────┐  ┌────────────────┐ │
│  │ Worker List│  │  Output Display│ │
│  └────────────┘  └────────────────┘ │
│  ┌────────────────────────────────┐ │
│  │       Prompt Input             │ │
│  └────────────────────────────────┘ │
└─────────────────────────────────────┘
```

### Component Responsibilities

**WebServer**:
- HTTP server (axum)
- Static file serving
- REST API endpoints
- WebSocket endpoint
- CORS configuration

**WebSocketHandler**:
- Connection management
- Event subscription and forwarding
- Command parsing and validation
- Client lifecycle management

**WebUI (Headless UI)**:
- Subscribes to EventBus
- Converts events to JSON
- Broadcasts to WebSocket clients
- No command sending (uses REST API)

**Frontend**:
- Worker list display
- Streaming output display
- Prompt input
- WebSocket client
- REST API client

---

## Critical Implementation Details

### 1. Event-to-JSON Conversion

**Key Consideration**: Maintain event semantics while being JSON-friendly

**Approach**:
```rust
impl From<AgentEnvironmentEvent> for WebUIEvent {
    fn from(event: AgentEnvironmentEvent) -> Self {
        match event {
            AgentEnvironmentEvent::Job(JobEvent::OutputChunk { 
                worker_id, job_id, chunk, timestamp 
            }) => {
                match chunk {
                    OutputChunk::AssistantResponse(text) => {
                        WebUIEvent::OutputChunk {
                            worker_id: worker_id.to_string(),
                            job_id: job_id.to_string(),
                            chunk_type: "assistant_response".to_string(),
                            data: json!({"text": text}),
                            timestamp: format_timestamp(timestamp),
                        }
                    }
                    // ... other chunk types
                }
            }
            // ... other events
        }
    }
}
```

### 2. WebSocket Message Protocol

**Client → Server**:
```json
{"type": "prompt", "worker_id": "...", "text": "..."}
{"type": "cancel", "worker_id": "..."}
```

**Server → Client**:
```json
{"type": "worker_created", "worker_id": "...", "name": "...", "timestamp": "..."}
{"type": "output_chunk", "worker_id": "...", "chunk_type": "...", "data": {...}}
{"type": "worker_state_changed", "worker_id": "...", "state": "...", "timestamp": "..."}
```

### 3. Axum Dependencies

**Required Crates**:
```toml
[dependencies]
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }
tokio-tungstenite = "0.21"
```

### 4. Static File Structure

```
web/
├── public/
│   ├── index.html
│   ├── css/
│   │   └── styles.css
│   └── js/
│       ├── app.js
│       ├── websocket.js
│       └── api.js
```

---

## Potential Pitfalls

### 1. EventBus Buffer Overflow

**Issue**: Slow WebSocket clients may cause EventBus lag

**Mitigation**:
- Increase EventBus buffer size for web deployments
- Implement backpressure in WebSocket handler
- Drop old events for lagged clients
- Monitor lag metrics

### 2. Worker ID Filtering

**Issue**: WebUI receives events for all workers, needs filtering

**Mitigation**:
- Filter events by worker_id in WebSocket handler
- Allow clients to subscribe to specific workers
- Send only relevant events to each client

### 3. Concurrent Modifications

**Issue**: Multiple web clients sending commands simultaneously

**Mitigation**:
- Session already handles concurrent access (Arc + Mutex)
- Commands are queued via mpsc channel
- Worker state prevents concurrent job execution

### 4. WebSocket Reconnection

**Issue**: Browser refresh or network issues cause disconnection

**Mitigation**:
- Send snapshot on reconnect
- Client-side reconnection logic
- Exponential backoff for retries
- Display connection status in UI

### 5. Port Conflicts

**Issue**: Port 8080 may be in use

**Mitigation**:
- Make port configurable via CLI arg or env var
- Try multiple ports if first fails
- Display actual port in startup message

---

## Dependencies and Integration Points

### Rust Dependencies

**New Dependencies**:
- `axum` - Web framework
- `tower-http` - HTTP middleware (CORS, static files)
- `tokio-tungstenite` - WebSocket support
- `serde_json` - JSON serialization (already present)

**Existing Dependencies**:
- `tokio` - Async runtime (already present)
- `uuid` - Worker IDs (already present)
- `eyre` - Error handling (already present)

### Integration with Existing Code

**No Changes Required**:
- EventBus (already supports multiple subscribers)
- Session (already thread-safe with Arc)
- Worker (already serializable)
- AgentEnvironment (already supports headless UIs)

**Minor Changes Required**:
- Add `--web-ui` flag to ChatArgs
- Add web server initialization in ChatArgs::execute()
- Create WebUI event conversion functions

**New Code Required**:
- WebServer struct and implementation
- WebSocket handler
- WebUI headless interface
- Frontend HTML/CSS/JS
- REST API handlers

---

## Testing Strategy

### Unit Tests

1. **Event Serialization**:
   - Test conversion of all event types to JSON
   - Verify timestamp formatting
   - Test round-trip serialization

2. **WebSocket Message Parsing**:
   - Test command parsing from JSON
   - Test invalid message handling
   - Test malformed JSON

3. **Client Management**:
   - Test client add/remove
   - Test broadcast to multiple clients
   - Test cleanup on disconnect

### Integration Tests

1. **WebSocket Connection**:
   - Connect and receive snapshot
   - Send command and verify execution
   - Disconnect and verify cleanup

2. **Event Flow**:
   - Trigger event in Session
   - Verify WebSocket receives event
   - Verify JSON format

3. **Multi-Client**:
   - Connect multiple clients
   - Verify all receive events
   - Disconnect one, verify others unaffected

### Manual Testing

1. **Browser Testing**:
   - Open in Chrome, Firefox, Safari
   - Test WebSocket connection
   - Test prompt sending
   - Test output display

2. **Reconnection Testing**:
   - Refresh browser
   - Kill and restart server
   - Network interruption simulation

3. **Concurrent Access**:
   - Multiple browser tabs
   - Multiple browsers
   - Verify state consistency

---

## Performance Considerations

### Expected Load

**MVP Assumptions**:
- Single user (localhost only)
- 1-5 concurrent browser tabs
- 1-10 workers
- 10-100 events per second
- 1-10 KB per event

**Scalability Limits**:
- EventBus buffer: 1000 events (configurable)
- WebSocket connections: 100+ (OS limit)
- Memory per client: ~10 KB
- CPU overhead: Minimal (async I/O)

### Optimization Opportunities

1. **Event Batching**:
   - Batch multiple events into single WebSocket message
   - Reduce WebSocket frame overhead
   - Implement in Phase 2

2. **Selective Subscriptions**:
   - Allow clients to subscribe to specific workers
   - Reduce unnecessary event transmission
   - Implement in Phase 2

3. **Compression**:
   - Enable WebSocket compression
   - Reduce bandwidth for large outputs
   - Implement in Phase 2

---

## Security Considerations

### MVP (Local Only)

**Assumptions**:
- Runs on localhost (127.0.0.1)
- Single user
- No authentication required
- No encryption required

**Basic Security**:
- Validate all input
- Sanitize worker_id and command parameters
- Prevent path traversal in static files
- Rate limiting (basic)

### Future (Remote Access)

**Required Security**:
- HTTPS/WSS (TLS encryption)
- Token-based authentication
- User-based access control
- CORS configuration
- Input validation and sanitization
- Rate limiting and DDoS protection
- Audit logging

---

## Summary

### Key Findings

1. **EventBus is well-suited for WebUI**: Already supports multiple subscribers, events are Clone-able
2. **Headless UI pattern is ideal**: WebUI should be headless, use REST API for commands
3. **Reference implementation exists**: web-q provides good patterns for WebSocket handling
4. **Minimal changes to core**: No changes needed to EventBus, Session, or Worker
5. **Event serialization is main challenge**: Need JSON-friendly event types

### Critical Success Factors

1. **Event-to-JSON conversion**: Must preserve event semantics
2. **WebSocket connection management**: Handle multiple clients per worker
3. **Initial state loading**: Send snapshot on connect
4. **Error handling**: Graceful degradation on failures
5. **Testing**: Thorough testing of WebSocket lifecycle

### Recommended Approach

1. **Phase 1: Backend** (8-12 hours)
   - Implement WebServer with axum
   - Implement WebSocket handler
   - Implement event-to-JSON conversion
   - Implement REST API endpoints

2. **Phase 2: Frontend** (8-12 hours)
   - Port web-q design
   - Implement WebSocket client
   - Implement worker list display
   - Implement output display and prompt input

3. **Phase 3: Polish** (4-6 hours)
   - Error handling and reconnection
   - UI polish and styling
   - Testing and bug fixes
   - Documentation

**Total Estimated Effort**: 20-30 hours

---

## Next Steps

1. **Design Phase**: Create detailed technical design document
   - Define exact API endpoints
   - Define WebSocket message protocol
   - Design frontend component structure
   - Create mockups/wireframes

2. **Implementation Phase**: Follow design document
   - Implement backend first (testable without frontend)
   - Implement frontend second (can test with backend)
   - Integrate and test end-to-end

3. **Testing Phase**: Comprehensive testing
   - Unit tests for all components
   - Integration tests for WebSocket flow
   - Manual testing in browsers
   - Performance testing

4. **Documentation Phase**: Update documentation
   - User guide for web UI
   - Developer guide for extending web UI
   - API documentation
   - Troubleshooting guide
