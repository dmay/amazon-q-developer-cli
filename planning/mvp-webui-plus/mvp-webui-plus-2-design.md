# MVP WebUI Plus - Technical Design

## 1. Overview and Goals

### 1.1 Purpose

This design document specifies the technical architecture for transforming the basic WebUI (mvp-webui) into a practical multi-worker chat interface (mvp-webui-plus). The design enables users to manage multiple concurrent AI workers, view conversation history, create new workers, and interact with a clean, intuitive interface.

### 1.2 Design Goals

**Primary Goal**: Enable multi-worker management in WebUI with minimal changes to existing architecture.

**Specific Goals**:
1. **Multi-Worker Support**: Display all workers in sidebar, allow selection and switching
2. **Conversation History**: Display full conversation as chat bubbles, not raw output chunks
3. **Worker Creation**: Allow users to create new workers with agent configuration
4. **Real-Time Updates**: All state changes reflected immediately via WebSocket events
5. **Response Streaming**: Accumulate streaming response chunks into single bubbles
6. **Clean Visual Design**: Professional two-column layout inspired by web-q prototype

### 1.3 Non-Goals

The following are explicitly out of scope:
- Worker deletion functionality
- Worker name editing
- Terminal tab (only Chat tab)
- Markdown rendering in responses
- Tool approval UI (approve/deny buttons)
- Conversation persistence to disk
- Working directory usage (collected but not used)
- Mobile-specific optimizations

### 1.4 Success Criteria

The design is successful if:
- Users can see and select any worker from sidebar
- Users can create new workers with agent configuration
- Conversation history displays as readable chat bubbles
- Streaming responses accumulate in real-time
- Worker state updates are immediate and clear
- All functionality works via WebSocket (no page reloads)

### 1.5 Key Design Principles

1. **Backward Compatibility**: Extend existing protocol, don't break it
2. **Event-Driven**: Leverage existing EventBus architecture
3. **Minimal Backend Changes**: Reuse Session and Worker APIs
4. **Component-Based Frontend**: Clean separation of concerns
5. **Performance**: Fast updates, responsive UI
6. **Simplicity**: Avoid over-engineering, focus on MVP requirements


## 2. Current State Analysis

### 2.1 Backend Architecture (mvp-webui)

**WebSocket Handler** (`websocket.rs`):
- Route: `/ws/worker/:worker_id` (single-worker design)
- Validates worker exists before upgrading connection
- Sends `WorkerStateSnapshot` on connection
- **Filters events by worker_id** before sending to client
- Handles commands: Prompt, Cancel, Ping

**WebUIEvent System** (`events.rs`):
- Flat enum structure with serde tags
- Event types: WorkerCreated, WorkerDeleted, WorkerStateChanged, JobStarted, JobCompleted, OutputChunk, ResponseReceived, ToolUseRequested, ShutdownInitiated
- Includes worker_id in all relevant events
- Converts Instant to Unix timestamp

**WebUI Component** (`web_ui.rs`):
- Implements HeadlessInterface
- Broadcast channel with 10,000 event buffer
- Converts AgentEnvironmentEvent to WebUIEvent
- Provides subscribe() for WebSocket handlers
- Provides session() for accessing Session

**REST API** (`api.rs`):
- GET /api/health: Health check
- GET /api/workers: List all workers
- GET /api/workers/:id: Get worker details

### 2.2 Frontend Architecture (mvp-webui)

**HTML Structure**:
- Single-worker layout: header + worker container + output + input
- No sidebar, no worker list
- Hardcoded to display one worker

**JavaScript** (`app.js`):
- QWebUI class with hardcoded workerId
- Connects to `/ws/worker/:workerId`
- Displays OutputChunk events as raw fragments
- No state management, no component structure

**CSS**:
- Basic styling with state badges
- Monospace output container
- No chat bubble styling

### 2.3 Key Limitations

**Backend**:
1. WebSocket URL requires worker_id (single-worker design)
2. Events filtered by worker_id (client only sees one worker)
3. No worker creation command
4. No conversation history API
5. No snapshot events for initial state

**Frontend**:
1. Hardcoded single worker
2. No worker list or sidebar
3. No conversation history view
4. Raw output chunks, not accumulated responses
5. No worker creation UI
6. No component architecture

### 2.4 What Works Well

**Strengths to Preserve**:
- Event-driven architecture with EventBus
- WebUIEvent serialization system
- Broadcast channel for multiple subscribers
- Session and Worker APIs provide necessary access
- Clean separation of concerns
- Graceful shutdown coordination


## 3. WebSocket Protocol Design

### 3.1 Design Decision: Single-Worker to Multi-Worker

**Current Design**: `/ws/worker/:worker_id` with event filtering

**Problem**: Client only receives events for one worker, cannot manage multiple workers.

**Proposed Design**: `/ws` (global connection) without event filtering

**Rationale**:
- Frontend needs events from all workers for sidebar updates
- Opening multiple WebSockets is complex and wasteful
- Global WebSocket can filter events client-side if needed
- Simpler backend (no filtering logic)
- Matches web-q prototype architecture

### 3.2 Route Changes

**Before**:
```rust
.route("/ws/worker/:worker_id", get(websocket_handler))
```

**After**:
```rust
.route("/ws", get(websocket_handler))
```

**Handler Signature**:
```rust
// Before
pub async fn websocket_handler(
    Path(worker_id): Path<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse

// After
pub async fn websocket_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse
```

### 3.3 New Commands

Extend `WebSocketCommand` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketCommand {
    // Existing commands
    Prompt { worker_id: String, text: String },  // Add worker_id
    Cancel { worker_id: String },                 // Add worker_id
    Ping,
    
    // New commands
    CreateWorker {
        name: Option<String>,
        agent: String,
        working_directory: Option<String>,
    },
    GetWorkers,
    GetConversationHistory { worker_id: String },
}
```

**Command Descriptions**:

**CreateWorker**:
- Creates new worker using WorkerBuilder
- `name`: Optional worker name (auto-generated if not provided)
- `agent`: Agent name to load configuration
- `working_directory`: Collected but not used (future feature)
- Returns worker ID via WorkerCreated event

**GetWorkers**:
- Requests current worker list
- Server responds with WorkersSnapshot event

**GetConversationHistory**:
- Requests full conversation history for worker
- Server responds with ConversationSnapshot event

### 3.4 New Events

Extend `WebUIEvent` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebUIEvent {
    // ... existing events ...
    
    // New events
    WorkersSnapshot {
        workers: Vec<WorkerMetadataJson>,
        timestamp: f64,
    },
    ConversationSnapshot {
        worker_id: String,
        entries: Vec<ConversationEntryJson>,
        timestamp: f64,
    },
    Error {
        command: String,
        message: String,
        timestamp: f64,
    },
}
```

**Event Descriptions**:

**WorkersSnapshot**:
- Sent on initial WebSocket connection
- Sent in response to GetWorkers command
- Contains complete list of all workers with metadata

**ConversationSnapshot**:
- Sent on initial connection for main worker
- Sent in response to GetConversationHistory command
- Contains complete conversation history for one worker

**Error**:
- Sent when command fails (e.g., invalid agent name)
- Includes command type and error message
- Client displays error to user

### 3.5 Connection Lifecycle

**New Connection Flow**:
```
1. Client connects to /ws
2. Server sends WorkersSnapshot (all workers)
3. Server sends ConversationSnapshot for main worker (if exists)
4. Client displays worker list and main worker conversation
5. Client receives real-time events for all workers
6. Client can request additional conversations as needed
```

**Event Filtering**:
- Server sends ALL events to client (no filtering)
- Client filters/routes events to appropriate UI components
- Simpler backend, more flexible frontend

### 3.6 Command Handling

**Command Handler Structure**:
```rust
async fn handle_command(
    text: &str,
    session: &Arc<Session>,
    os: &Arc<Os>,
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    let command: WebSocketCommand = serde_json::from_str(text)?;
    
    match command {
        WebSocketCommand::Prompt { worker_id, text } => {
            // Existing logic, now with worker_id parameter
        }
        WebSocketCommand::Cancel { worker_id } => {
            // Existing logic, now with worker_id parameter
        }
        WebSocketCommand::CreateWorker { name, agent, working_directory } => {
            handle_create_worker(name, agent, working_directory, session, os).await?;
        }
        WebSocketCommand::GetWorkers => {
            send_workers_snapshot(session, sender).await?;
        }
        WebSocketCommand::GetConversationHistory { worker_id } => {
            send_conversation_snapshot(worker_id, session, sender).await?;
        }
        WebSocketCommand::Ping => {
            // No-op
        }
    }
    
    Ok(())
}
```

### 3.7 Error Handling

**Command Validation**:
- Validate worker_id exists before processing
- Validate agent name exists before creating worker
- Return Error event if validation fails

**Error Response Example**:
```json
{
  "type": "error",
  "command": "create_worker",
  "message": "Agent 'invalid-agent' not found",
  "timestamp": 1234567890.0
}
```

**Client Handling**:
- Display error message in dialog or toast
- Keep dialog open for CreateWorker errors
- Log errors to console

### 3.8 Backward Compatibility

**Breaking Changes**:
- WebSocket URL changes from `/ws/worker/:id` to `/ws`
- Prompt and Cancel commands now require worker_id parameter

**Migration**:
- Old clients will fail to connect (URL mismatch)
- Users must refresh browser after update
- No data loss (conversation history in memory)

**Mitigation**:
- Document breaking change in release notes
- Provide clear error message if old URL used
- Consider supporting both URLs temporarily (optional)


## 4. Conversation Serialization Design

### 4.1 Current Conversation Structure

**ConversationEntry** (`conversation_entry.rs`):
```rust
pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

**UserMessage** (`message.rs`):
```rust
pub struct UserMessage {
    pub additional_context: String,
    pub env_context: UserEnvContext,
    pub content: UserMessageContent,
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub images: Option<Vec<ImageBlock>>,
}

pub enum UserMessageContent {
    Prompt { prompt: String },
    CancelledToolUses { prompt: Option<String>, tool_use_results: Vec<ToolUseResult> },
    ToolUseResults { tool_use_results: Vec<ToolUseResult> },
}
```

**AssistantMessage** (`message.rs`):
```rust
pub enum AssistantMessage {
    Response {
        message_id: Option<String>,
        content: String,
    },
    ToolUse {
        message_id: Option<String>,
        content: String,
        tool_uses: Vec<AssistantToolUse>,
    },
}
```

### 4.2 Serialization Challenge

**Problem**: Complex nested structures with optional fields, enums, and metadata that are not needed for display.

**Requirements**:
- Simple JSON representation for frontend
- Easy to render as chat bubbles
- Distinguish between user messages, assistant responses, and tool use
- Include timestamps for display

### 4.3 Simplified JSON Types

**ConversationEntryJson**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntryJson {
    UserMessage {
        content: String,
        timestamp: f64,
    },
    AssistantMessage {
        content: String,
        timestamp: f64,
    },
    ToolUse {
        content: String,
        tool_uses: Vec<ToolUseJson>,
        timestamp: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseJson {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}
```

**WorkerMetadataJson**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetadataJson {
    pub id: String,  // UUID as string
    pub name: String,
    pub agent: String,
    pub state: WorkerLifecycleState,
    pub current_job_id: Option<String>,  // UUID as string
}
```

### 4.4 Conversion Logic

**ConversationEntry → ConversationEntryJson**:
```rust
fn convert_conversation_entry(entry: &ConversationEntry) -> ConversationEntryJson {
    if let Some(user_msg) = &entry.user {
        let content = extract_user_content(user_msg);
        let timestamp = user_msg.timestamp
            .map(|dt| dt.timestamp() as f64)
            .unwrap_or_else(|| current_unix_timestamp());
        
        ConversationEntryJson::UserMessage { content, timestamp }
    } else if let Some(assistant_msg) = &entry.assistant {
        match assistant_msg {
            AssistantMessage::Response { content, .. } => {
                ConversationEntryJson::AssistantMessage {
                    content: content.clone(),
                    timestamp: current_unix_timestamp(),
                }
            }
            AssistantMessage::ToolUse { content, tool_uses, .. } => {
                ConversationEntryJson::ToolUse {
                    content: content.clone(),
                    tool_uses: tool_uses.iter().map(convert_tool_use).collect(),
                    timestamp: current_unix_timestamp(),
                }
            }
        }
    } else {
        // Empty entry - shouldn't happen, but handle gracefully
        ConversationEntryJson::UserMessage {
            content: "".to_string(),
            timestamp: current_unix_timestamp(),
        }
    }
}

fn extract_user_content(user_msg: &UserMessage) -> String {
    match &user_msg.content {
        UserMessageContent::Prompt { prompt } => prompt.clone(),
        UserMessageContent::CancelledToolUses { prompt, .. } => {
            prompt.clone().unwrap_or_else(|| "[Cancelled tool uses]".to_string())
        }
        UserMessageContent::ToolUseResults { .. } => {
            "[Tool use results]".to_string()
        }
    }
}

fn convert_tool_use(tool_use: &AssistantToolUse) -> ToolUseJson {
    ToolUseJson {
        tool_name: tool_use.name.clone(),
        tool_input: tool_use.input.clone(),
    }
}

fn current_unix_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
```

**Worker → WorkerMetadataJson**:
```rust
fn convert_worker_metadata(worker: &Worker) -> WorkerMetadataJson {
    WorkerMetadataJson {
        id: worker.id.to_string(),
        name: worker.name.clone(),
        agent: worker.agent_name.clone(),  // Assuming Worker has agent_name field
        state: (*worker.lifecycle_state.lock().unwrap()).into(),
        current_job_id: worker.current_job_id.lock().unwrap()
            .as_ref().map(|id| id.to_string()),
    }
}
```

### 4.5 Timestamp Handling

**Issue**: ConversationEntry doesn't store timestamps, only UserMessage has optional timestamp.

**Solution for MVP**:
- Use UserMessage.timestamp if available
- Otherwise, use current time as approximation
- AssistantMessage doesn't have timestamp, use current time

**Trade-offs**:
- ✅ Simple implementation
- ✅ No breaking changes to core types
- ❌ Approximate timestamps for assistant messages
- ❌ Timestamps not preserved across restarts

**Future Improvement**: Add timestamp field to ConversationEntry in core types.

### 4.6 Serialization Module

**New File**: `crates/chat-cli/src/cli/chat/web_server/serialization.rs`

```rust
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agent_env::context_container::conversation_entry::ConversationEntry;
use crate::agent_env::worker::Worker;
use crate::cli::chat::message::{AssistantMessage, UserMessage, UserMessageContent};

// Type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntryJson { /* ... */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseJson { /* ... */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetadataJson { /* ... */ }

// Conversion functions
pub fn convert_conversation_entry(entry: &ConversationEntry) -> ConversationEntryJson { /* ... */ }
pub fn convert_worker_metadata(worker: &Worker) -> WorkerMetadataJson { /* ... */ }
fn extract_user_content(user_msg: &UserMessage) -> String { /* ... */ }
fn convert_tool_use(tool_use: &AssistantToolUse) -> ToolUseJson { /* ... */ }
fn current_unix_timestamp() -> f64 { /* ... */ }
```

### 4.7 Usage in WebSocket Handler

**Sending ConversationSnapshot**:
```rust
async fn send_conversation_snapshot(
    worker_id: String,
    session: &Arc<Session>,
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    let worker = session.get_worker(Uuid::parse_str(&worker_id)?)
        .ok_or_else(|| eyre::eyre!("Worker not found"))?;
    
    let history = worker.context_container
        .conversation_history
        .lock()
        .unwrap();
    
    let entries: Vec<ConversationEntryJson> = history.get_entries()
        .iter()
        .map(convert_conversation_entry)
        .collect();
    
    let snapshot = WebUIEvent::ConversationSnapshot {
        worker_id,
        entries,
        timestamp: current_unix_timestamp(),
    };
    
    let json = serde_json::to_string(&snapshot)?;
    sender.send(Message::Text(json)).await?;
    
    Ok(())
}
```

**Sending WorkersSnapshot**:
```rust
async fn send_workers_snapshot(
    session: &Arc<Session>,
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    let workers = session.get_workers();
    
    let workers_metadata: Vec<WorkerMetadataJson> = workers
        .iter()
        .map(|w| convert_worker_metadata(w))
        .collect();
    
    let snapshot = WebUIEvent::WorkersSnapshot {
        workers: workers_metadata,
        timestamp: current_unix_timestamp(),
    };
    
    let json = serde_json::to_string(&snapshot)?;
    sender.send(Message::Text(json)).await?;
    
    Ok(())
}
```

### 4.8 Testing Serialization

**Unit Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convert_user_message() {
        let user_msg = UserMessage::new_prompt("Hello".to_string(), None);
        let entry = ConversationEntry::new_user(user_msg);
        let json = convert_conversation_entry(&entry);
        
        match json {
            ConversationEntryJson::UserMessage { content, .. } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected UserMessage"),
        }
    }
    
    #[test]
    fn test_convert_assistant_response() {
        let assistant_msg = AssistantMessage::new_response(None, "Hi there".to_string());
        let entry = ConversationEntry::new_assistant(assistant_msg);
        let json = convert_conversation_entry(&entry);
        
        match json {
            ConversationEntryJson::AssistantMessage { content, .. } => {
                assert_eq!(content, "Hi there");
            }
            _ => panic!("Expected AssistantMessage"),
        }
    }
}
```


## 5. Worker Creation Design

### 5.1 WorkerBuilder Integration

**Current WorkerBuilder API** (`worker_builder.rs`):
```rust
impl WorkerBuilder {
    pub fn new() -> Self
    pub fn agent(self, agent_name: Option<String>) -> Self
    pub fn platform(self, platform: Platform) -> Self
    pub fn model(self, model: Option<String>) -> Self
    pub fn initial_input(self, input: Option<String>) -> Self
    pub async fn build(self, session: Arc<Session>, os: &Os) -> Result<Arc<Worker>>
}
```

**Challenge**: `build()` requires `Os` reference, which is not available in WebSocket handler.

### 5.2 AppState Extension

**Current AppState**:
```rust
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
}
```

**Proposed AppState**:
```rust
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
    pub os: Arc<Os>,  // Add this
}
```

**Rationale**:
- WorkerBuilder needs Os for agent configuration loading
- Os is created in ChatArgs::execute()
- Sharing Os via AppState is clean and simple
- Enables proper agent loading with resources

### 5.3 Worker Creation Flow

**Command Handler**:
```rust
async fn handle_create_worker(
    name: Option<String>,
    agent: String,
    working_directory: Option<String>,
    session: Arc<Session>,
    os: Arc<Os>,
) -> Result<()> {
    // Generate name if not provided
    let worker_name = name.unwrap_or_else(|| {
        generate_worker_name(&agent, &session)
    });
    
    // Create worker using WorkerBuilder
    let worker = WorkerBuilder::new()
        .agent(Some(agent))
        .initial_input(None)
        .build(session.clone(), &os)
        .await?;
    
    // Set worker name (WorkerBuilder doesn't support name yet)
    // Note: May need to add name support to WorkerBuilder
    
    // WorkerCreated event is automatically published by session.build_worker()
    
    Ok(())
}

fn generate_worker_name(agent: &str, session: &Arc<Session>) -> String {
    let workers = session.get_workers();
    let count = workers.iter()
        .filter(|w| w.name.starts_with(agent))
        .count();
    
    format!("{}-{}", agent, count + 1)
}
```

**Flow Diagram**:
```
User clicks "+ New" button
  ↓
NewWorkerDialog shows form
  ↓
User enters agent name, clicks Create
  ↓
Frontend sends CreateWorker command via WebSocket
  ↓
Backend receives command
  ↓
Validate agent name exists
  ↓
Generate worker name if not provided
  ↓
WorkerBuilder.new().agent(agent).build(session, os)
  ↓
Session.build_worker() creates worker
  ↓
Session publishes WorkerCreated event
  ↓
EventBus → AgentEnvironment → WebUI → WebSocket
  ↓
Frontend receives WorkerCreated event
  ↓
Frontend adds worker to sidebar
  ↓
Frontend auto-selects new worker
  ↓
Dialog closes
```

### 5.4 Agent Validation

**Validation Logic**:
```rust
fn validate_agent_name(agent: &str, os: &Os) -> Result<(), String> {
    // Check if agent config exists
    // This logic depends on how agents are registered
    // For MVP, we can try to load agent config and catch error
    
    // Placeholder - actual implementation depends on agent registry
    if agent.is_empty() {
        return Err("Agent name cannot be empty".to_string());
    }
    
    // Could check against known agent names
    // let known_agents = ["default", "rust-agent", "python-agent"];
    // if !known_agents.contains(&agent) {
    //     return Err(format!("Unknown agent: {}", agent));
    // }
    
    Ok(())
}
```

**Error Handling**:
```rust
async fn handle_create_worker_with_error(
    name: Option<String>,
    agent: String,
    working_directory: Option<String>,
    session: Arc<Session>,
    os: Arc<Os>,
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    // Validate agent
    if let Err(e) = validate_agent_name(&agent, &os) {
        send_error_event("create_worker", &e, sender).await?;
        return Ok(());
    }
    
    // Try to create worker
    match handle_create_worker(name, agent, working_directory, session, os).await {
        Ok(()) => Ok(()),
        Err(e) => {
            send_error_event("create_worker", &e.to_string(), sender).await?;
            Ok(())
        }
    }
}

async fn send_error_event(
    command: &str,
    message: &str,
    sender: &mut SplitSink<WebSocket, Message>,
) -> Result<()> {
    let error_event = WebUIEvent::Error {
        command: command.to_string(),
        message: message.to_string(),
        timestamp: current_unix_timestamp(),
    };
    
    let json = serde_json::to_string(&error_event)?;
    sender.send(Message::Text(json)).await?;
    
    Ok(())
}
```

### 5.5 Worker Name Generation

**Strategy**: Generate meaningful names based on agent name and counter.

**Examples**:
- First rust-agent worker: `rust-agent-1`
- Second rust-agent worker: `rust-agent-2`
- First default worker: `default-1`

**Implementation**:
```rust
fn generate_worker_name(agent: &str, session: &Arc<Session>) -> String {
    let workers = session.get_workers();
    
    // Count workers with same agent prefix
    let count = workers.iter()
        .filter(|w| w.name.starts_with(agent))
        .count();
    
    format!("{}-{}", agent, count + 1)
}
```

**Alternative**: Use UUID suffix for guaranteed uniqueness:
```rust
fn generate_worker_name_uuid(agent: &str) -> String {
    let short_uuid = Uuid::new_v4().to_string()[..8].to_string();
    format!("{}-{}", agent, short_uuid)
}
```

### 5.6 Working Directory Handling

**Current Scope**: Working directory is collected but not used.

**Storage**: Store in Worker's task_metadata for future use.

**Implementation**:
```rust
async fn handle_create_worker(
    name: Option<String>,
    agent: String,
    working_directory: Option<String>,
    session: Arc<Session>,
    os: Arc<Os>,
) -> Result<()> {
    let worker_name = name.unwrap_or_else(|| generate_worker_name(&agent, &session));
    
    let worker = WorkerBuilder::new()
        .agent(Some(agent))
        .build(session.clone(), &os)
        .await?;
    
    // Store working directory in metadata
    if let Some(wd) = working_directory {
        worker.task_metadata.lock().unwrap()
            .insert("working_directory".to_string(), serde_json::Value::String(wd));
    }
    
    Ok(())
}
```

**Future Use**: When working directory feature is implemented, read from task_metadata.

### 5.7 Integration with ChatArgs::execute()

**Current Flow**:
```rust
impl ChatArgs {
    pub async fn execute(self, os: Os) -> Result<()> {
        let event_bus = EventBus::default();
        let session = Arc::new(Session::new(event_bus.clone(), model_providers));
        let main_worker = session.build_worker("main".to_string());
        
        if self.web_ui {
            let web_ui = Arc::new(WebUI::new(session.clone()));
            let web_server = WebServer::new(
                session.clone(),
                web_ui.clone(),
                self.web_port,
            );
            // ...
        }
    }
}
```

**Proposed Flow**:
```rust
impl ChatArgs {
    pub async fn execute(self, os: Os) -> Result<()> {
        let os = Arc::new(os);  // Wrap in Arc for sharing
        let event_bus = EventBus::default();
        let session = Arc::new(Session::new(event_bus.clone(), model_providers));
        let main_worker = session.build_worker("main".to_string());
        
        if self.web_ui {
            let web_ui = Arc::new(WebUI::new(session.clone()));
            let web_server = WebServer::new(
                session.clone(),
                web_ui.clone(),
                os.clone(),  // Pass Os to WebServer
                self.web_port,
            );
            // ...
        }
    }
}
```

**WebServer Constructor**:
```rust
impl WebServer {
    pub fn new(
        session: Arc<Session>,
        web_ui: Arc<WebUI>,
        os: Arc<Os>,  // Add parameter
        port: u16,
    ) -> Self {
        let app_state = AppState {
            session,
            web_ui,
            os,  // Store in AppState
        };
        
        // ... rest of constructor
    }
}
```

### 5.8 Testing Worker Creation

**Unit Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_worker_name() {
        let session = create_test_session();
        let name = generate_worker_name("rust-agent", &session);
        assert_eq!(name, "rust-agent-1");
    }
    
    #[test]
    fn test_validate_agent_name_empty() {
        let os = create_test_os();
        let result = validate_agent_name("", &os);
        assert!(result.is_err());
    }
}
```

**Integration Tests**:
```rust
#[tokio::test]
async fn test_create_worker_command() {
    let (session, os) = create_test_session_and_os();
    
    let result = handle_create_worker(
        None,
        "test-agent".to_string(),
        None,
        session.clone(),
        os,
    ).await;
    
    assert!(result.is_ok());
    
    let workers = session.get_workers();
    assert_eq!(workers.len(), 2);  // main + new worker
}
```


## 6. Frontend Architecture Design

### 6.1 Current vs Proposed Architecture

**Current** (mvp-webui):
```javascript
class QWebUI {
    constructor() {
        this.workerId = null;  // Single worker
        this.ws = null;
    }
    
    handleEvent(data) {
        // Monolithic event handling
    }
}
```

**Proposed** (mvp-webui-plus):
```javascript
class WebUIApp {
    constructor() {
        this.state = new WebUIState();
        this.ws = new WebSocketClient(this);
        this.components = {
            workerList: new WorkerList(this),
            conversationView: new ConversationView(this),
            inputArea: new InputArea(this),
            newWorkerDialog: new NewWorkerDialog(this),
            workerDetails: new WorkerDetailsPopup(this),
        };
        this.accumulator = new ResponseAccumulator(this);
    }
}
```

### 6.2 State Management

**WebUIState Class**:
```javascript
class WebUIState {
    constructor() {
        this.workers = new Map();  // worker_id -> WorkerData
        this.selectedWorkerId = null;
        this.conversations = new Map();  // worker_id -> ConversationEntry[]
        this.activeResponses = new Map();  // worker_id -> accumulated text
        this.connectionState = 'disconnected';  // disconnected, connecting, connected
    }
    
    // Worker management
    addWorker(worker) {
        this.workers.set(worker.id, worker);
    }
    
    removeWorker(workerId) {
        this.workers.delete(workerId);
        this.conversations.delete(workerId);
        this.activeResponses.delete(workerId);
    }
    
    updateWorkerState(workerId, newState) {
        const worker = this.workers.get(workerId);
        if (worker) {
            worker.state = newState;
        }
    }
    
    // Worker selection
    selectWorker(workerId) {
        this.selectedWorkerId = workerId;
    }
    
    getSelectedWorker() {
        return this.workers.get(this.selectedWorkerId);
    }
    
    // Conversation management
    setConversation(workerId, entries) {
        this.conversations.set(workerId, entries);
    }
    
    appendConversationEntry(workerId, entry) {
        const conversation = this.conversations.get(workerId) || [];
        conversation.push(entry);
        this.conversations.set(workerId, conversation);
    }
    
    getConversation(workerId) {
        return this.conversations.get(workerId) || [];
    }
}

class WorkerData {
    constructor(id, name, agent, state, currentJobId) {
        this.id = id;
        this.name = name;
        this.agent = agent;
        this.state = state;  // idle, busy, idle_failed
        this.currentJobId = currentJobId;
    }
}
```

### 6.3 WebSocket Client

**WebSocketClient Class**:
```javascript
class WebSocketClient {
    constructor(app) {
        this.app = app;
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 1000;  // ms
    }
    
    connect() {
        this.app.state.connectionState = 'connecting';
        this.ws = new WebSocket('ws://127.0.0.1:8080/ws');
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.app.state.connectionState = 'connected';
            this.reconnectAttempts = 0;
            this.app.onConnected();
        };
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.app.handleEvent(data);
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket closed');
            this.app.state.connectionState = 'disconnected';
            this.reconnect();
        };
    }
    
    reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            return;
        }
        
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * this.reconnectAttempts;
        
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        setTimeout(() => this.connect(), delay);
    }
    
    send(command) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(command));
        } else {
            console.error('WebSocket not connected');
        }
    }
}
```

### 6.4 Component Architecture

**WorkerList Component**:
```javascript
class WorkerList {
    constructor(app) {
        this.app = app;
        this.element = document.getElementById('worker-list');
    }
    
    render() {
        const workers = Array.from(this.app.state.workers.values());
        
        this.element.innerHTML = workers
            .map(w => this.renderWorker(w))
            .join('');
        
        // Add event listeners
        this.element.querySelectorAll('.worker-item').forEach(item => {
            item.addEventListener('click', (e) => {
                const workerId = e.currentTarget.dataset.workerId;
                this.app.selectWorker(workerId);
            });
        });
    }
    
    renderWorker(worker) {
        const icon = this.getStateIcon(worker.state);
        const active = worker.id === this.app.state.selectedWorkerId ? 'active' : '';
        
        return `
            <div class="worker-item ${active}" data-worker-id="${worker.id}">
                <span class="worker-icon">${icon}</span>
                <div class="worker-info">
                    <div class="worker-name">${this.escapeHtml(worker.name)}</div>
                    <div class="worker-state">${worker.state}</div>
                </div>
            </div>
        `;
    }
    
    getStateIcon(state) {
        switch (state) {
            case 'idle': return '●';
            case 'busy': return '⚙';
            case 'idle_failed': return '✗';
            default: return '?';
        }
    }
    
    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}
```

**ConversationView Component**:
```javascript
class ConversationView {
    constructor(app) {
        this.app = app;
        this.element = document.getElementById('conversation');
    }
    
    render() {
        const workerId = this.app.state.selectedWorkerId;
        if (!workerId) {
            this.element.innerHTML = '<div class="no-worker">Select a worker to view conversation</div>';
            return;
        }
        
        const entries = this.app.state.getConversation(workerId);
        this.element.innerHTML = entries
            .map(e => this.renderEntry(e))
            .join('');
        
        this.scrollToBottom();
    }
    
    renderEntry(entry) {
        const bubbleClass = this.getBubbleClass(entry.type);
        const content = this.formatContent(entry);
        
        return `
            <div class="bubble ${bubbleClass}">
                ${this.escapeHtml(content)}
            </div>
        `;
    }
    
    getBubbleClass(type) {
        switch (type) {
            case 'user_message': return 'bubble-user';
            case 'assistant_message': return 'bubble-assistant';
            case 'tool_use': return 'bubble-tool';
            default: return 'bubble-default';
        }
    }
    
    formatContent(entry) {
        if (entry.type === 'tool_use') {
            const tools = entry.tool_uses
                .map(t => `${t.tool_name}(${JSON.stringify(t.tool_input)})`)
                .join(', ');
            return `${entry.content}\n\nTools: ${tools}`;
        }
        return entry.content;
    }
    
    appendEntry(entry) {
        const html = this.renderEntry(entry);
        this.element.insertAdjacentHTML('beforeend', html);
        this.scrollToBottom();
    }
    
    updateLastEntry(content) {
        const lastBubble = this.element.lastElementChild;
        if (lastBubble) {
            lastBubble.textContent = content;
        }
    }
    
    scrollToBottom() {
        this.element.scrollTop = this.element.scrollHeight;
    }
    
    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}
```

**InputArea Component**:
```javascript
class InputArea {
    constructor(app) {
        this.app = app;
        this.input = document.getElementById('prompt-input');
        this.sendButton = document.getElementById('send-button');
        this.cancelButton = document.getElementById('cancel-button');
        
        this.setupEventListeners();
    }
    
    setupEventListeners() {
        this.sendButton.addEventListener('click', () => this.sendMessage());
        
        this.input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.sendMessage();
            }
        });
        
        this.cancelButton.addEventListener('click', () => this.cancelJob());
    }
    
    sendMessage() {
        const text = this.input.value.trim();
        if (!text) return;
        
        const workerId = this.app.state.selectedWorkerId;
        if (!workerId) {
            alert('Please select a worker');
            return;
        }
        
        this.app.ws.send({
            type: 'prompt',
            worker_id: workerId,
            text: text,
        });
        
        this.input.value = '';
    }
    
    cancelJob() {
        const workerId = this.app.state.selectedWorkerId;
        if (!workerId) return;
        
        this.app.ws.send({
            type: 'cancel',
            worker_id: workerId,
        });
    }
    
    updateState() {
        const worker = this.app.state.getSelectedWorker();
        const isBusy = worker && worker.state === 'busy';
        
        this.input.disabled = isBusy;
        this.sendButton.disabled = isBusy;
        this.cancelButton.style.display = isBusy ? 'inline-block' : 'none';
    }
}
```

**NewWorkerDialog Component**:
```javascript
class NewWorkerDialog {
    constructor(app) {
        this.app = app;
        this.dialog = document.getElementById('new-worker-dialog');
        this.form = document.getElementById('new-worker-form');
        this.nameInput = document.getElementById('worker-name-input');
        this.agentInput = document.getElementById('worker-agent-input');
        this.workingDirInput = document.getElementById('worker-workdir-input');
        this.createButton = document.getElementById('create-worker-button');
        this.cancelButton = document.getElementById('cancel-dialog-button');
        
        this.setupEventListeners();
    }
    
    setupEventListeners() {
        this.createButton.addEventListener('click', () => this.createWorker());
        this.cancelButton.addEventListener('click', () => this.close());
        
        this.form.addEventListener('submit', (e) => {
            e.preventDefault();
            this.createWorker();
        });
        
        // Close on outside click
        this.dialog.addEventListener('click', (e) => {
            if (e.target === this.dialog) {
                this.close();
            }
        });
    }
    
    show() {
        this.dialog.style.display = 'flex';
        this.agentInput.focus();
    }
    
    close() {
        this.dialog.style.display = 'none';
        this.form.reset();
    }
    
    createWorker() {
        const agent = this.agentInput.value.trim();
        if (!agent) {
            alert('Agent name is required');
            return;
        }
        
        this.app.ws.send({
            type: 'create_worker',
            name: this.nameInput.value.trim() || null,
            agent: agent,
            working_directory: this.workingDirInput.value.trim() || null,
        });
        
        this.close();
    }
}
```

### 6.5 Main Application Class

**WebUIApp Class**:
```javascript
class WebUIApp {
    constructor() {
        this.state = new WebUIState();
        this.ws = new WebSocketClient(this);
        this.components = {
            workerList: new WorkerList(this),
            conversationView: new ConversationView(this),
            inputArea: new InputArea(this),
            newWorkerDialog: new NewWorkerDialog(this),
        };
        this.accumulator = new ResponseAccumulator(this);
    }
    
    async init() {
        this.setupGlobalEventListeners();
        await this.ws.connect();
    }
    
    setupGlobalEventListeners() {
        document.getElementById('new-worker-button').addEventListener('click', () => {
            this.components.newWorkerDialog.show();
        });
    }
    
    onConnected() {
        // Initial snapshots are sent automatically by server
        console.log('Connected, waiting for initial snapshots');
    }
    
    selectWorker(workerId) {
        this.state.selectWorker(workerId);
        
        // Request conversation history if not loaded
        if (!this.state.conversations.has(workerId)) {
            this.ws.send({
                type: 'get_conversation_history',
                worker_id: workerId,
            });
        }
        
        this.components.workerList.render();
        this.components.conversationView.render();
        this.components.inputArea.updateState();
    }
    
    handleEvent(event) {
        console.log('Event received:', event.type);
        
        switch (event.type) {
            case 'workers_snapshot':
                this.handleWorkersSnapshot(event);
                break;
            case 'conversation_snapshot':
                this.handleConversationSnapshot(event);
                break;
            case 'worker_created':
                this.handleWorkerCreated(event);
                break;
            case 'worker_state_changed':
                this.handleWorkerStateChanged(event);
                break;
            case 'output_chunk':
                this.accumulator.handleChunk(event);
                break;
            case 'job_started':
                this.handleJobStarted(event);
                break;
            case 'job_completed':
                this.handleJobCompleted(event);
                break;
            case 'error':
                this.handleError(event);
                break;
            default:
                console.log('Unhandled event type:', event.type);
        }
    }
    
    handleWorkersSnapshot(event) {
        event.workers.forEach(w => {
            this.state.addWorker(new WorkerData(
                w.id, w.name, w.agent, w.state, w.current_job_id
            ));
        });
        
        // Auto-select first worker if none selected
        if (!this.state.selectedWorkerId && event.workers.length > 0) {
            this.selectWorker(event.workers[0].id);
        }
        
        this.components.workerList.render();
    }
    
    handleConversationSnapshot(event) {
        this.state.setConversation(event.worker_id, event.entries);
        
        if (event.worker_id === this.state.selectedWorkerId) {
            this.components.conversationView.render();
        }
    }
    
    handleWorkerCreated(event) {
        this.state.addWorker(new WorkerData(
            event.worker_id, event.name, 'default', 'idle', null
        ));
        this.components.workerList.render();
        
        // Auto-select new worker
        this.selectWorker(event.worker_id);
    }
    
    handleWorkerStateChanged(event) {
        this.state.updateWorkerState(event.worker_id, event.new_state);
        this.components.workerList.render();
        this.components.inputArea.updateState();
    }
    
    handleJobStarted(event) {
        const worker = this.state.workers.get(event.worker_id);
        if (worker) {
            worker.currentJobId = event.job_id;
        }
        this.components.inputArea.updateState();
    }
    
    handleJobCompleted(event) {
        const worker = this.state.workers.get(event.worker_id);
        if (worker) {
            worker.currentJobId = null;
        }
        this.accumulator.finalize(event.worker_id);
        this.components.inputArea.updateState();
    }
    
    handleError(event) {
        alert(`Error: ${event.message}`);
        console.error('Command error:', event);
    }
}

// Initialize app on page load
let app;
document.addEventListener('DOMContentLoaded', () => {
    app = new WebUIApp();
    app.init();
});
```

### 6.6 HTML Structure

**Updated Layout**:
```html
<!DOCTYPE html>
<html>
<head>
    <title>Q CLI - Web UI</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <div class="container">
        <!-- Sidebar -->
        <aside class="sidebar">
            <div class="sidebar-header">
                <h2>Workers</h2>
                <button id="new-worker-button" class="btn-new">+ New</button>
            </div>
            <div id="worker-list" class="worker-list"></div>
        </aside>
        
        <!-- Main Content -->
        <main class="main-content">
            <header class="content-header">
                <h1 id="worker-title">Q CLI - Web UI</h1>
                <div id="connection-status" class="connection-status"></div>
            </header>
            
            <div id="conversation" class="conversation"></div>
            
            <div class="input-area">
                <textarea id="prompt-input" placeholder="Type your message..."></textarea>
                <button id="send-button" class="btn-send">Send</button>
                <button id="cancel-button" class="btn-cancel" style="display: none;">Cancel</button>
            </div>
        </main>
    </div>
    
    <!-- New Worker Dialog -->
    <div id="new-worker-dialog" class="dialog" style="display: none;">
        <div class="dialog-content">
            <h2>Create New Worker</h2>
            <form id="new-worker-form">
                <label>
                    Worker Name (optional):
                    <input type="text" id="worker-name-input" placeholder="auto-generated">
                </label>
                <label>
                    Agent Name:
                    <input type="text" id="worker-agent-input" placeholder="default" required>
                </label>
                <label>
                    Working Directory (optional):
                    <input type="text" id="worker-workdir-input" placeholder="~/">
                </label>
                <div class="dialog-buttons">
                    <button type="submit" id="create-worker-button" class="btn-primary">Create</button>
                    <button type="button" id="cancel-dialog-button" class="btn-secondary">Cancel</button>
                </div>
            </form>
        </div>
    </div>
    
    <script src="app.js"></script>
</body>
</html>
```


## 7. Response Accumulation Design

### 7.1 Problem Statement

**Current Behavior**: OutputChunk events arrive as fragments and are displayed separately.

**Example**:
```
OutputChunk { chunk_type: AssistantResponse, text: "Hello" }
OutputChunk { chunk_type: AssistantResponse, text: ", " }
OutputChunk { chunk_type: AssistantResponse, text: "world!" }
```

**Current Display**: Three separate fragments in output container.

**Desired Behavior**: Single chat bubble with "Hello, world!" that updates in real-time.

### 7.2 ResponseAccumulator Design

**Class Structure**:
```javascript
class ResponseAccumulator {
    constructor(app) {
        this.app = app;
        this.activeResponses = new Map();  // worker_id -> ResponseState
    }
    
    handleChunk(event) {
        const { worker_id, chunk } = event;
        
        if (chunk.chunk_type === 'assistant_response') {
            this.handleAssistantChunk(worker_id, chunk.text);
        } else if (chunk.chunk_type === 'tool_use') {
            this.handleToolUseChunk(worker_id, chunk);
        } else if (chunk.chunk_type === 'tool_result') {
            this.handleToolResultChunk(worker_id, chunk);
        }
    }
    
    handleAssistantChunk(workerId, text) {
        if (!this.activeResponses.has(workerId)) {
            // Start new response
            const entry = {
                type: 'assistant_message',
                content: text,
                timestamp: Date.now() / 1000,
            };
            
            this.activeResponses.set(workerId, {
                entry: entry,
                startTime: Date.now(),
            });
            
            // Create bubble in UI if this is the selected worker
            if (workerId === this.app.state.selectedWorkerId) {
                this.app.components.conversationView.appendEntry(entry);
            }
        } else {
            // Append to existing response
            const state = this.activeResponses.get(workerId);
            state.entry.content += text;
            
            // Update bubble in UI if this is the selected worker
            if (workerId === this.app.state.selectedWorkerId) {
                this.app.components.conversationView.updateLastEntry(state.entry.content);
            }
        }
    }
    
    handleToolUseChunk(workerId, chunk) {
        // Tool use is typically sent as single chunk, not streamed
        const entry = {
            type: 'tool_use',
            content: 'Using tool...',
            tool_uses: [{
                tool_name: chunk.tool_name,
                tool_input: chunk.tool_input,
            }],
            timestamp: Date.now() / 1000,
        };
        
        if (workerId === this.app.state.selectedWorkerId) {
            this.app.components.conversationView.appendEntry(entry);
        }
        
        this.app.state.appendConversationEntry(workerId, entry);
    }
    
    handleToolResultChunk(workerId, chunk) {
        // Tool result is typically sent as single chunk
        const entry = {
            type: 'tool_result',
            content: `Tool ${chunk.tool_name} result: ${chunk.result}`,
            timestamp: Date.now() / 1000,
        };
        
        if (workerId === this.app.state.selectedWorkerId) {
            this.app.components.conversationView.appendEntry(entry);
        }
        
        this.app.state.appendConversationEntry(workerId, entry);
    }
    
    finalize(workerId) {
        // Called on JobCompleted
        const state = this.activeResponses.get(workerId);
        if (state) {
            // Add to conversation history
            this.app.state.appendConversationEntry(workerId, state.entry);
            
            // Clear active response
            this.activeResponses.delete(workerId);
        }
    }
}
```

**ResponseState Structure**:
```javascript
class ResponseState {
    constructor(entry, startTime) {
        this.entry = entry;  // ConversationEntry being accumulated
        this.startTime = startTime;  // When accumulation started
    }
}
```

### 7.3 Accumulation Flow

**Flow Diagram**:
```
OutputChunk event arrives
  ↓
ResponseAccumulator.handleChunk()
  ↓
Is this first chunk for this worker?
  ├─ Yes: Create new entry, add to activeResponses, append bubble to UI
  └─ No: Append text to existing entry, update bubble in UI
  ↓
JobCompleted event arrives
  ↓
ResponseAccumulator.finalize()
  ↓
Add entry to conversation history
  ↓
Clear activeResponses for this worker
```

### 7.4 Multi-Worker Handling

**Challenge**: Multiple workers can stream responses simultaneously.

**Solution**: Track active responses per worker_id.

**Example Scenario**:
```
Worker A starts streaming: "Hello..."
Worker B starts streaming: "Goodbye..."
Worker A continues: "Hello, world!"
Worker B continues: "Goodbye, friend!"
Worker A completes
Worker B completes
```

**State During Streaming**:
```javascript
activeResponses = {
    'worker-a-id': { entry: { content: 'Hello, world!' }, startTime: ... },
    'worker-b-id': { entry: { content: 'Goodbye, friend!' }, startTime: ... },
}
```

**UI Behavior**:
- If Worker A is selected: User sees "Hello, world!" updating in real-time
- If Worker B is selected: User sees "Goodbye, friend!" updating in real-time
- If user switches workers mid-stream: UI updates to show selected worker's response

### 7.5 Edge Cases

**Case 1: Job Cancelled Mid-Stream**

**Scenario**: User cancels job while response is streaming.

**Handling**:
```javascript
handleJobCompleted(event) {
    if (event.result.status === 'cancelled') {
        const state = this.accumulator.activeResponses.get(event.worker_id);
        if (state) {
            state.entry.content += ' [Cancelled]';
            this.components.conversationView.updateLastEntry(state.entry.content);
        }
    }
    this.accumulator.finalize(event.worker_id);
}
```

**Case 2: Multiple Jobs in Quick Succession**

**Scenario**: User sends new message before previous response finishes.

**Handling**: Use job_id to track which chunks belong together.

**Enhanced Design**:
```javascript
class ResponseAccumulator {
    constructor(app) {
        this.app = app;
        this.activeResponses = new Map();  // job_id -> ResponseState
    }
    
    handleChunk(event) {
        const { job_id, chunk } = event;
        // Use job_id instead of worker_id for tracking
    }
    
    finalize(jobId) {
        this.activeResponses.delete(jobId);
    }
}
```

**Trade-off**: More complex, but handles edge case correctly.

**Decision for MVP**: Use worker_id tracking, assume one job per worker at a time. Document limitation.

**Case 3: WebSocket Reconnection**

**Scenario**: WebSocket disconnects and reconnects mid-stream.

**Handling**:
- On reconnection, request ConversationSnapshot to resync
- Clear activeResponses (partial responses are lost)
- User sees complete conversation history up to last finalized message

**Implementation**:
```javascript
onConnected() {
    // Clear active responses on reconnection
    this.accumulator.activeResponses.clear();
    
    // Request fresh conversation snapshot
    if (this.state.selectedWorkerId) {
        this.ws.send({
            type: 'get_conversation_history',
            worker_id: this.state.selectedWorkerId,
        });
    }
}
```

**Case 4: Switching Workers Mid-Stream**

**Scenario**: User switches to different worker while response is streaming.

**Handling**:
- Active response continues accumulating in background
- UI shows selected worker's conversation
- When user switches back, sees complete accumulated response

**Implementation**: No special handling needed, accumulator tracks all workers independently.

### 7.6 Performance Considerations

**Chunk Frequency**: OutputChunk events can arrive rapidly (10-100 per second).

**DOM Update Optimization**:
```javascript
updateLastEntry(content) {
    const lastBubble = this.element.lastElementChild;
    if (lastBubble) {
        // Use textContent for performance (no HTML parsing)
        lastBubble.textContent = content;
    }
}
```

**Throttling** (optional):
```javascript
class ResponseAccumulator {
    constructor(app) {
        this.app = app;
        this.activeResponses = new Map();
        this.updateThrottle = 50;  // ms
        this.lastUpdate = new Map();  // worker_id -> timestamp
    }
    
    handleAssistantChunk(workerId, text) {
        const state = this.activeResponses.get(workerId);
        if (state) {
            state.entry.content += text;
            
            // Throttle UI updates
            const now = Date.now();
            const lastUpdate = this.lastUpdate.get(workerId) || 0;
            
            if (now - lastUpdate > this.updateThrottle) {
                this.updateUI(workerId, state.entry.content);
                this.lastUpdate.set(workerId, now);
            }
        }
    }
    
    finalize(workerId) {
        // Force final update
        const state = this.activeResponses.get(workerId);
        if (state) {
            this.updateUI(workerId, state.entry.content);
            this.app.state.appendConversationEntry(workerId, state.entry);
            this.activeResponses.delete(workerId);
        }
    }
    
    updateUI(workerId, content) {
        if (workerId === this.app.state.selectedWorkerId) {
            this.app.components.conversationView.updateLastEntry(content);
        }
    }
}
```

**Trade-off**: Smoother UI updates vs slightly delayed text display.

**Decision for MVP**: No throttling initially, add if performance issues observed.

### 7.7 Testing Response Accumulation

**Unit Tests** (JavaScript):
```javascript
describe('ResponseAccumulator', () => {
    it('should accumulate chunks into single response', () => {
        const app = createMockApp();
        const accumulator = new ResponseAccumulator(app);
        
        accumulator.handleChunk({
            worker_id: 'test-worker',
            chunk: { chunk_type: 'assistant_response', text: 'Hello' },
        });
        
        accumulator.handleChunk({
            worker_id: 'test-worker',
            chunk: { chunk_type: 'assistant_response', text: ', world!' },
        });
        
        const state = accumulator.activeResponses.get('test-worker');
        expect(state.entry.content).toBe('Hello, world!');
    });
    
    it('should handle multiple workers independently', () => {
        const app = createMockApp();
        const accumulator = new ResponseAccumulator(app);
        
        accumulator.handleChunk({
            worker_id: 'worker-a',
            chunk: { chunk_type: 'assistant_response', text: 'A' },
        });
        
        accumulator.handleChunk({
            worker_id: 'worker-b',
            chunk: { chunk_type: 'assistant_response', text: 'B' },
        });
        
        expect(accumulator.activeResponses.get('worker-a').entry.content).toBe('A');
        expect(accumulator.activeResponses.get('worker-b').entry.content).toBe('B');
    });
});
```

**Integration Tests**:
- Send message, verify response accumulates correctly
- Send multiple messages to different workers, verify independent accumulation
- Cancel job mid-stream, verify partial response is preserved
- Reconnect WebSocket mid-stream, verify state recovery


## 8. Initial State Synchronization Design

### 8.1 Connection Lifecycle

**Connection Flow**:
```
1. Client connects to /ws
2. Server sends WorkersSnapshot (all workers)
3. Server sends ConversationSnapshot for main worker (if exists)
4. Client displays worker list and main worker conversation
5. Client receives real-time events for all workers
6. Client can request additional conversations as needed
```

### 8.2 Server-Side Implementation

**WebSocket Handler**:
```rust
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to events BEFORE sending snapshots
    let mut event_rx = state.web_ui.subscribe();
    
    // Send initial snapshots
    if let Err(e) = send_initial_snapshots(&mut sender, &state).await {
        tracing::error!("Failed to send initial snapshots: {}", e);
        return;
    }
    
    // Continue with event forwarding and command handling...
}

async fn send_initial_snapshots(
    sender: &mut SplitSink<WebSocket, Message>,
    state: &AppState,
) -> Result<()> {
    // Send WorkersSnapshot
    send_workers_snapshot(&state.session, sender).await?;
    
    // Send ConversationSnapshot for main worker (if exists)
    let workers = state.session.get_workers();
    if let Some(main_worker) = workers.iter().find(|w| w.name == "main") {
        send_conversation_snapshot(
            main_worker.id.to_string(),
            &state.session,
            sender,
        ).await?;
    }
    
    Ok(())
}
```

### 8.3 Client-Side Implementation

**Connection Handler**:
```javascript
class WebSocketClient {
    connect() {
        this.ws = new WebSocket('ws://127.0.0.1:8080/ws');
        
        this.ws.onopen = () => {
            console.log('WebSocket connected, waiting for snapshots');
            this.app.state.connectionState = 'connected';
        };
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.app.handleEvent(data);
        };
    }
}

class WebUIApp {
    handleWorkersSnapshot(event) {
        // First snapshot received
        event.workers.forEach(w => {
            this.state.addWorker(new WorkerData(
                w.id, w.name, w.agent, w.state, w.current_job_id
            ));
        });
        
        // Auto-select first worker (or main worker)
        const mainWorker = event.workers.find(w => w.name === 'main');
        const firstWorker = mainWorker || event.workers[0];
        
        if (firstWorker) {
            this.state.selectWorker(firstWorker.id);
        }
        
        this.components.workerList.render();
    }
    
    handleConversationSnapshot(event) {
        // Conversation snapshot received
        this.state.setConversation(event.worker_id, event.entries);
        
        if (event.worker_id === this.state.selectedWorkerId) {
            this.components.conversationView.render();
        }
    }
}
```

### 8.4 Reconnection Handling

**Reconnection Flow**:
```
1. WebSocket disconnects
2. Client attempts reconnection (with exponential backoff)
3. Client reconnects successfully
4. Server sends fresh WorkersSnapshot
5. Server sends ConversationSnapshot for previously selected worker
6. Client updates UI with fresh state
```

**Implementation**:
```javascript
class WebSocketClient {
    reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            this.app.showReconnectionError();
            return;
        }
        
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
        
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        setTimeout(() => this.connect(), delay);
    }
}

class WebUIApp {
    onConnected() {
        // Clear active responses (partial responses lost)
        this.accumulator.activeResponses.clear();
        
        // Wait for WorkersSnapshot and ConversationSnapshot
        // No explicit action needed, server sends automatically
    }
}
```

### 8.5 Race Condition Prevention

**Issue**: Events might arrive before snapshots are processed.

**Solution**: Subscribe to events BEFORE sending snapshots (server-side).

**Server Implementation**:
```rust
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // IMPORTANT: Subscribe BEFORE sending snapshots
    let mut event_rx = state.web_ui.subscribe();
    
    // Now send snapshots
    send_initial_snapshots(&mut sender, &state).await?;
    
    // Events received after subscription will be queued
    // and processed after snapshots are sent
}
```

**Client Handling**: Process events in order received, snapshots arrive first.

## 9. Implementation Considerations

### 9.1 File Structure

**Backend Changes**:
```
crates/chat-cli/src/cli/chat/web_server/
├── mod.rs                    # Add serialization module export
├── websocket.rs              # MODIFY: Change route, add commands, remove filtering
├── events.rs                 # MODIFY: Add WorkersSnapshot, ConversationSnapshot, Error
├── serialization.rs          # NEW: Conversion functions
├── web_ui.rs                 # No changes needed
├── api.rs                    # Optional: Add conversation history endpoint
└── server.rs                 # MODIFY: Add Os to AppState
```

**Frontend Changes**:
```
web/public/
├── index.html                # REWRITE: New layout with sidebar
├── styles.css                # REWRITE: New styling
└── app.js                    # REWRITE: Component-based architecture
```

### 9.2 Backward Compatibility

**Breaking Changes**:
- WebSocket URL: `/ws/worker/:id` → `/ws`
- Prompt/Cancel commands now require `worker_id` parameter

**Migration Strategy**:
- Users must refresh browser after update
- No data loss (conversation history in memory)
- Document breaking changes in release notes

### 9.3 Performance Targets

**Metrics**:
- Worker list update: < 100ms latency
- Conversation history load: < 500ms for 100 messages
- WebSocket message handling: < 50ms per event
- UI rendering: 60fps during streaming responses
- Memory usage: < 100MB for 10 workers with 1000 messages each

### 9.4 Security Considerations

**Local-Only Use**:
- WebSocket connections only from localhost
- No authentication required
- No CORS issues (same origin)

**Input Sanitization**:
- Escape HTML in user messages and assistant responses
- Validate worker_id format (UUID)
- Validate agent names before creation

**Error Handling**:
- Don't expose internal errors to client
- Log detailed errors server-side
- Return user-friendly error messages

### 9.5 Browser Compatibility

**Target Browsers**:
- Chrome/Edge: Latest 2 versions
- Firefox: Latest 2 versions
- Safari: Latest 2 versions
- No IE11 support required

**Required Features**:
- WebSocket API
- ES6 classes and modules
- Map and Set
- Async/await
- Fetch API (for future REST calls)

### 9.6 Accessibility

**Basic Requirements**:
- Semantic HTML elements
- ARIA labels for interactive elements
- Keyboard navigation support
- Focus management in dialogs

**Implementation**:
```html
<button id="new-worker-button" aria-label="Create new worker">+ New</button>
<div role="dialog" aria-labelledby="dialog-title" aria-modal="true">
  <h2 id="dialog-title">Create New Worker</h2>
</div>
```

## 10. Testing Strategy

### 10.1 Backend Unit Tests

**Serialization Tests**:
```rust
#[test]
fn test_convert_user_message() { /* ... */ }

#[test]
fn test_convert_assistant_response() { /* ... */ }

#[test]
fn test_convert_tool_use() { /* ... */ }

#[test]
fn test_worker_metadata_serialization() { /* ... */ }
```

**Command Parsing Tests**:
```rust
#[test]
fn test_parse_create_worker_command() { /* ... */ }

#[test]
fn test_parse_get_workers_command() { /* ... */ }

#[test]
fn test_validate_worker_id() { /* ... */ }
```

### 10.2 Backend Integration Tests

**WebSocket Tests**:
```rust
#[tokio::test]
async fn test_websocket_connection() { /* ... */ }

#[tokio::test]
async fn test_initial_snapshots_sent() { /* ... */ }

#[tokio::test]
async fn test_create_worker_command() { /* ... */ }

#[tokio::test]
async fn test_conversation_snapshot() { /* ... */ }
```

### 10.3 Frontend Unit Tests

**State Management Tests**:
```javascript
describe('WebUIState', () => {
    it('should add worker', () => { /* ... */ });
    it('should select worker', () => { /* ... */ });
    it('should update worker state', () => { /* ... */ });
});
```

**Response Accumulation Tests**:
```javascript
describe('ResponseAccumulator', () => {
    it('should accumulate chunks', () => { /* ... */ });
    it('should handle multiple workers', () => { /* ... */ });
    it('should finalize on job completion', () => { /* ... */ });
});
```

### 10.4 Manual Testing Scenarios

**Scenario 1: Basic Workflow**
1. Start `q chat --web-ui`
2. Open browser to http://127.0.0.1:8080
3. Verify main worker appears in sidebar
4. Send message "Hello"
5. Verify response streams and appears as single bubble
6. Verify worker state changes Idle → Busy → Idle

**Scenario 2: Multi-Worker**
1. Create new worker with agent "rust-agent"
2. Verify worker appears in sidebar
3. Send message to new worker
4. Switch back to main worker
5. Verify conversation is preserved
6. Send message to main worker
7. Verify both workers operate independently

**Scenario 3: Concurrent Workers**
1. Create 3 workers
2. Send long-running task to each
3. Verify all show "Busy" state
4. Switch between workers while responses stream
5. Verify responses appear in correct conversations

**Scenario 4: Error Handling**
1. Create worker with invalid agent name
2. Verify error message appears
3. Disconnect network
4. Verify "Disconnected" indicator
5. Reconnect network
6. Verify automatic reconnection

**Scenario 5: Browser Refresh**
1. Create 2 workers, send messages
2. Refresh browser
3. Verify worker list restored
4. Verify conversation history restored

### 10.5 Performance Testing

**Load Tests**:
- 10 workers with 1000 messages each
- Measure memory usage
- Measure conversation load time
- Measure UI responsiveness

**Stress Tests**:
- Rapid message sending (10 messages/second)
- Multiple workers streaming simultaneously
- Large messages (10KB+ text)

### 10.6 Browser Compatibility Testing

**Test Matrix**:
- Chrome (latest): All features
- Firefox (latest): All features
- Safari (latest): All features
- Edge (latest): All features

**Test Cases**:
- WebSocket connection
- Event handling
- UI rendering
- Dialog interactions
- Keyboard navigation

---

## Summary

This design document specifies the technical architecture for mvp-webui-plus, transforming the basic WebUI into a practical multi-worker chat interface. Key design decisions:

1. **WebSocket Protocol**: Change from `/ws/worker/:id` to `/ws` (global connection)
2. **Conversation Serialization**: Simplified JSON types for easy frontend rendering
3. **Worker Creation**: Use WorkerBuilder with Os from AppState
4. **Frontend Architecture**: Component-based with centralized state management
5. **Response Accumulation**: Track active responses per worker, update bubbles in real-time
6. **Initial Sync**: Send WorkersSnapshot + ConversationSnapshot on connection

The design maintains backward compatibility where possible, leverages existing EventBus architecture, and focuses on MVP requirements without over-engineering. All components are testable and the implementation can be phased to reduce risk.

