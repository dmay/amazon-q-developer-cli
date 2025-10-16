# MVP WebUI Plus - Research Document

## 1. Executive Summary

This research document analyzes the existing WebUI implementation (mvp-webui) and identifies the changes needed to transform it into a practical multi-worker chat interface (mvp-webui-plus). The analysis covers backend APIs, frontend architecture, WebSocket protocol, and conversation history management.

**Key Findings**:
- Backend infrastructure is solid but needs extensions for multi-worker support
- Frontend requires complete rewrite from single-worker to multi-worker architecture
- Session and Worker APIs already provide necessary access methods
- ConversationHistory structure is complex and requires careful serialization
- WorkerBuilder exists and can be used for worker creation
- WebSocket protocol needs new commands and events for multi-worker scenarios

## 2. Current Implementation Analysis (mvp-webui)

### 2.1 Backend Architecture

#### WebUI Component (`web_ui.rs`)

**Purpose**: Implements HeadlessInterface to receive events from AgentEnvironment and broadcast them to WebSocket clients.

**Key Features**:
- Creates broadcast channel with 10,000 event buffer
- Converts AgentEnvironmentEvent to WebUIEvent
- Provides `subscribe()` method for WebSocket handlers
- Provides `session()` method for accessing Session

**Strengths**:
- Clean separation of concerns
- Large buffer prevents event loss
- Already supports multiple WebSocket subscribers

**Limitations**:
- No worker creation functionality
- No conversation history access
- No worker list management

#### WebUIEvent System (`events.rs`)

**Purpose**: Serializable event types for WebSocket communication.

**Event Types**:
```rust
pub enum WebUIEvent {
    // Worker Events
    WorkerCreated { worker_id, name, timestamp },
    WorkerDeleted { worker_id, timestamp },
    WorkerStateChanged { worker_id, old_state, new_state, timestamp },
    
    // Job Events
    JobStarted { worker_id, job_id, task_type, timestamp },
    JobCompleted { worker_id, job_id, result, timestamp },
    OutputChunk { worker_id, job_id, chunk, timestamp },
    
    // AgentLoop Events
    ResponseReceived { worker_id, job_id, text, timestamp },
    ToolUseRequested { worker_id, job_id, tool_name, tool_input, timestamp },
    
    // System Events
    ShutdownInitiated { reason, timestamp },
}
```

**Strengths**:
- Flat enum structure with serde tags
- Includes worker_id in all relevant events
- Timestamp conversion from Instant to Unix timestamp
- Helper method `worker_id()` to extract worker ID

**Limitations**:
- No snapshot events for initial state
- No conversation history events
- No worker list events

#### WebSocket Handler (`websocket.rs`)

**Purpose**: Handles WebSocket connections and command processing.

**Current Flow**:
1. Client connects to `/ws/worker/:worker_id`
2. Handler validates worker exists
3. Sends initial `WorkerStateSnapshot` with worker state
4. Spawns task to forward events (filtered by worker_id)
5. Spawns task to handle incoming commands

**Commands**:
```rust
pub enum WebSocketCommand {
    Prompt { text },
    Cancel,
    Ping,
}
```

**Strengths**:
- Clean async architecture with tokio
- Proper error handling
- Reconnection support via lagged event handling
- Command validation

**Limitations**:
- **Hardcoded to single worker**: URL path requires worker_id
- **Filters events by worker_id**: Only sends events for connected worker
- No worker creation command
- No worker list query command
- No conversation history query command

#### REST API (`api.rs`)

**Purpose**: Provides HTTP endpoints for worker information.

**Endpoints**:
- `GET /api/health`: Health check
- `GET /api/workers`: List all workers
- `GET /api/workers/:id`: Get worker details

**Strengths**:
- Simple and functional
- Proper error handling
- Returns serializable worker metadata

**Limitations**:
- No conversation history endpoint
- Worker metadata is minimal (id, name, lifecycle_state)
- No worker creation endpoint

#### Web Server (`server.rs`)

**Purpose**: Axum-based HTTP server with static file serving.

**Routes**:
- `/ws/worker/:worker_id`: WebSocket endpoint
- `/api/*`: REST API endpoints
- `/`: Static file serving from `web/public/`

**Strengths**:
- Clean Axum setup
- CORS enabled for development
- Graceful shutdown support
- Shared AppState with Session and WebUI

**Limitations**:
- None significant for this workflow

### 2.2 Frontend Architecture

#### HTML Structure (`index.html`)

**Current Layout**:
```
<header>
  <h1>Q CLI - Web UI</h1>
  <connection-status>
</header>

<main>
  <worker-container>
    <worker-header>
      <worker-name>
      <worker-state-badge>
    </worker-header>
    
    <output-container>
      <output> (displays raw output chunks)
    </output-container>
    
    <input-container>
      <prompt-input>
      <send-button>
      <cancel-button>
    </input-container>
  </worker-container>
</main>
```

**Limitations**:
- No sidebar for worker list
- No multi-worker support
- No conversation history view
- No worker creation UI

#### JavaScript Application (`app.js`)

**Current Architecture**:
```javascript
class QWebUI {
  constructor() {
    this.workerId = null;  // Hardcoded to single worker
    this.ws = null;
    // ...
  }
  
  async init() {
    // Fetch workers, connect to first one
    const workers = await this.fetchWorkers();
    this.workerId = workers[0].worker_id;
    this.connect();
  }
  
  connect() {
    // Connect to /ws/worker/:workerId
    this.ws = new WebSocket(`ws://.../ws/worker/${this.workerId}`);
  }
  
  handleEvent(data) {
    // Handle events for single worker
    switch (data.type) {
      case 'worker_state_snapshot':
      case 'worker_state_changed':
      case 'output_chunk':
      // ...
    }
  }
}
```

**Limitations**:
- **Hardcoded single worker**: No worker list state
- **No state management**: No separation of data and UI
- **No component structure**: Monolithic event handling
- **Raw output display**: Displays OutputChunk events as fragments
- **No conversation history**: Only shows streaming output
- **No worker creation**: No UI for creating workers

#### CSS Styling (`style.css`)

**Current Styling**:
- Basic layout with header and main content
- State badges with colors (idle=green, busy=orange, failed=red)
- Output container with monospace font
- Input area with buttons

**Limitations**:
- No sidebar styling
- No chat bubble styling
- No multi-worker layout
- Basic visual design

### 2.3 Key Architectural Decisions

#### Event-Driven Communication

**Current Design**: All components communicate via EventBus → AgentEnvironment → WebUI → WebSocket.

**Strengths**:
- Decoupled components
- Real-time updates
- Multiple subscribers supported
- Clean event flow

**Implications for mvp-webui-plus**:
- Event system already supports multi-worker scenarios
- No changes needed to event flow
- Need to add new event types for snapshots and conversation history

#### Single-Worker WebSocket Connection

**Current Design**: WebSocket URL includes worker_id: `/ws/worker/:worker_id`

**Rationale**: Simple and explicit - each connection is tied to one worker.

**Implications for mvp-webui-plus**:
- **Option A**: Keep single-worker connections, open multiple WebSockets
- **Option B**: Change to global WebSocket, send all events
- **Recommendation**: **Option B** - simpler for multi-worker UI

**Reasoning**:
- Frontend needs events from all workers for sidebar updates
- Opening multiple WebSockets is complex and wasteful
- Global WebSocket can filter events client-side if needed
- Matches web-q prototype architecture

#### REST vs WebSocket for Queries

**Current Design**: REST API for worker list, WebSocket for events.

**Implications for mvp-webui-plus**:
- Can add REST endpoint for conversation history
- Can add WebSocket commands for queries
- **Recommendation**: Hybrid approach
  - REST for initial page load
  - WebSocket commands for dynamic queries
  - Both provide flexibility

## 3. Session and Worker API Analysis

### 3.1 Session API

**Location**: `crates/chat-cli/src/agent_env/session.rs`

**Key Methods**:

```rust
impl Session {
    // Worker management
    pub fn build_worker(&self, name: String) -> Arc<Worker>
    pub fn get_worker(&self, worker_id: Uuid) -> Option<Arc<Worker>>
    pub fn get_workers(&self) -> Vec<Arc<Worker>>
    
    // Task execution
    pub fn run_task__agent_loop(&self, worker: Arc<Worker>, input: AgentLoopInput) -> Result<Arc<WorkerJob>>
    pub fn cancel_all_jobs(&self)
    
    // Lifecycle management
    pub fn set_worker_lifecycle_state(&self, worker_id: Uuid, new_state: WorkerLifecycleState)
    
    // Job management
    pub fn has_active_jobs(&self) -> bool
    pub fn wait_for_all_jobs(&self) -> async
    pub fn cleanup_inactive_jobs(&self)
}
```

**Storage**:
```rust
workers: Arc<Mutex<Vec<Arc<Worker>>>>
jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>
```

**Event Publishing**:
- `build_worker()` publishes `WorkerEvent::Created`
- `set_worker_lifecycle_state()` publishes `WorkerEvent::LifecycleStateChanged`
- `run_task__agent_loop()` publishes `JobEvent::Started`
- Job completion handler publishes `JobEvent::Completed`

**Strengths**:
- All necessary methods are public
- Thread-safe with Arc<Mutex<>>
- Event publishing is automatic
- Clean API surface

**Gaps for mvp-webui-plus**:
- ❌ No worker deletion method (out of scope for this workflow)
- ✅ Worker creation via `build_worker()` exists
- ✅ Worker list via `get_workers()` exists
- ✅ Worker lookup via `get_worker()` exists

### 3.2 Worker API

**Location**: `crates/chat-cli/src/agent_env/worker.rs`

**Key Fields**:
```rust
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub context_container: ContextContainer,
    pub lifecycle_state: Arc<Mutex<WorkerLifecycleState>>,
    pub task_metadata: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub model_provider: Option<Arc<dyn ModelProvider>>,
    // ... other fields
}
```

**Strengths**:
- All fields are public
- Thread-safe mutable state
- Clean structure

**Gaps for mvp-webui-plus**:
- ✅ Worker metadata is accessible
- ✅ Lifecycle state is accessible
- ✅ Context container is accessible
- ❌ No helper method to get conversation history (need to add)
- ❌ No helper method to serialize metadata (need to add)

### 3.3 ConversationHistory API

**Location**: `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`

**Structure**:
```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,
}

pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

**Access**:
```rust
// From Worker
worker.context_container.conversation_history.lock().unwrap()
```

**Methods**:
```rust
impl ConversationHistory {
    pub fn push_input_message(&mut self, content: String)
    pub fn push_assistant_message(&mut self, assistant: AssistantMessage)
    pub fn get_entries(&self) -> &[ConversationEntry]
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
}
```

**Strengths**:
- Simple and clean API
- Thread-safe with Mutex
- Entries are accessible

**Complexity**:
- `UserMessage` and `AssistantMessage` are complex types
- `UserMessage` has: additional_context, env_context, content (enum), timestamp, images
- `AssistantMessage` has: content (enum with text/tool_use/tool_result), timestamp, metadata
- Need to serialize these for WebUI

**Serialization Challenge**:

The scope document suggests a simplified serialization:
```rust
pub enum ConversationEntryJson {
    InputMessage { content: String, timestamp: f64 },
    OutputMessage { content: String, timestamp: f64 },
    ToolUse { name: String, input: serde_json::Value, timestamp: f64 },
    ToolResult { name: String, result: String, timestamp: f64 },
}
```

**Problem**: This doesn't match the actual structure!

**Actual Structure**:
- `ConversationEntry` has `user: Option<UserMessage>` and `assistant: Option<AssistantMessage>`
- `UserMessage.content` is an enum: `Prompt`, `CancelledToolUses`, `ToolUseResults`
- `AssistantMessage` is in `crates/chat-cli/src/cli/chat/message.rs`

**AssistantMessage Structure**:
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

**Serialization Strategy**:

For mvp-webui-plus, we need a simplified representation for the frontend:

```rust
#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct ToolUseJson {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}
```

**Conversion Logic**:
- Extract text content from `UserMessage.content` enum variants
- Extract content and tool_uses from `AssistantMessage` variants
- Use current time as timestamp (ConversationEntry doesn't have timestamps)
- Simplify complex structures to essential display information

**Trade-offs**:
- ✅ Simple frontend representation
- ✅ Easy to render as chat bubbles
- ❌ Loses some metadata (additional_context, env_context, etc.)
- ❌ Approximate timestamps (not stored in ConversationEntry)

**Future Improvement**: Add timestamps to ConversationEntry in core types.

### 3.4 WorkerBuilder API

**Location**: `crates/chat-cli/src/agent_env/worker_builder.rs`

**Purpose**: Builder pattern for creating workers with configuration.

**API**:
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

**Build Process**:
1. Load agent config by name (or use default)
2. Extract resource references from agent
3. Create worker through `session.build_worker()` (publishes WorkerCreated event)
4. Set Os in worker for resource loading
5. Populate context container with agent prompt and resources
6. Add initial input if provided

**Strengths**:
- Clean builder pattern
- Handles agent configuration loading
- Publishes events automatically
- Flexible configuration

**Challenges for mvp-webui-plus**:
- ❌ Requires `Os` reference (not available in WebSocket handler)
- ❌ Async method (need to handle in async context)
- ✅ Agent name is optional (can use default)
- ✅ Initial input is optional

**Solution for Worker Creation**:

Option 1: Pass Os reference through AppState
```rust
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
    pub os: Arc<Os>,  // Add this
}
```

Option 2: Create simplified worker creation that doesn't use WorkerBuilder
```rust
// In WebSocket handler
let worker = session.build_worker(name);
// Skip agent loading, resource loading, etc.
```

**Recommendation**: **Option 1** - Pass Os through AppState
- Maintains consistency with agent configuration
- Allows future features (agent selection in UI)
- Proper resource loading
- More complete implementation

## 4. Gap Analysis

### 4.1 Backend Gaps

#### Critical Gaps (Must Fix)

1. **WebSocket Protocol - Single Worker Design**
   - **Current**: WebSocket URL is `/ws/worker/:worker_id`
   - **Problem**: Hardcoded to single worker, filters events by worker_id
   - **Solution**: Change to `/ws` (global connection), send all events
   - **Impact**: Medium - requires WebSocket handler rewrite

2. **No Worker Creation Command**
   - **Current**: No WebSocket command to create workers
   - **Problem**: Frontend can't create workers
   - **Solution**: Add `CreateWorker` command with agent_name, name, working_directory
   - **Impact**: Medium - requires command handling and Os access

3. **No Conversation History API**
   - **Current**: No way to fetch conversation history
   - **Problem**: Frontend can't display full conversation
   - **Solution**: Add REST endpoint and/or WebSocket command
   - **Impact**: Medium - requires serialization logic

4. **No Workers Snapshot Event**
   - **Current**: No initial state event on connection
   - **Problem**: Frontend doesn't know about existing workers
   - **Solution**: Add `WorkersSnapshot` event sent on connection
   - **Impact**: Low - simple event addition

5. **No Conversation Snapshot Event**
   - **Current**: No event to send full conversation history
   - **Problem**: Frontend can't load conversation when switching workers
   - **Solution**: Add `ConversationSnapshot` event
   - **Impact**: Medium - requires serialization logic

#### Important Gaps (Should Fix)

6. **No ConversationEntry Serialization**
   - **Current**: ConversationEntry is not serializable
   - **Problem**: Can't send conversation history to frontend
   - **Solution**: Create ConversationEntryJson type with conversion
   - **Impact**: Medium - requires careful serialization

7. **No Worker Metadata Serialization**
   - **Current**: Worker metadata is minimal in API responses
   - **Problem**: Frontend can't display full worker details
   - **Solution**: Create WorkerMetadataJson type with all fields
   - **Impact**: Low - simple serialization

8. **No Os in AppState**
   - **Current**: AppState doesn't have Os reference
   - **Problem**: WorkerBuilder.build() requires Os
   - **Solution**: Add Os to AppState
   - **Impact**: Low - simple addition

#### Nice-to-Have Gaps (Optional)

9. **No Incremental Conversation Updates**
   - **Current**: Only full snapshots
   - **Problem**: Inefficient for large conversations
   - **Solution**: Add `ConversationEntryAdded` event
   - **Impact**: Low - optimization

10. **No Worker Name in build_worker()**
    - **Current**: `build_worker()` takes name as String
    - **Problem**: No auto-generation of names
    - **Solution**: Make name optional, generate if not provided
    - **Impact**: Low - convenience feature

### 4.2 Frontend Gaps

#### Critical Gaps (Must Fix)

1. **No Multi-Worker State Management**
   - **Current**: Single worker hardcoded
   - **Problem**: Can't manage multiple workers
   - **Solution**: Create WebUIState class with workers Map
   - **Impact**: High - complete rewrite

2. **No Worker List UI**
   - **Current**: No sidebar
   - **Problem**: Can't see or select workers
   - **Solution**: Create WorkerList component with sidebar
   - **Impact**: High - new UI component

3. **No Conversation History Display**
   - **Current**: Only streaming output chunks
   - **Problem**: Can't see full conversation
   - **Solution**: Create ConversationView component with bubbles
   - **Impact**: High - new UI component

4. **No Response Accumulation**
   - **Current**: OutputChunk events displayed as fragments
   - **Problem**: Responses appear as disconnected pieces
   - **Solution**: Create ResponseAccumulator to combine chunks
   - **Impact**: Medium - new logic

5. **No Worker Creation UI**
   - **Current**: No dialog or form
   - **Problem**: Can't create workers
   - **Solution**: Create NewWorkerDialog component
   - **Impact**: Medium - new UI component

#### Important Gaps (Should Fix)

6. **No Component Architecture**
   - **Current**: Monolithic QWebUI class
   - **Problem**: Hard to maintain and extend
   - **Solution**: Split into components (WorkerList, ConversationView, etc.)
   - **Impact**: High - architectural change

7. **No Visual Design**
   - **Current**: Basic styling
   - **Problem**: Not user-friendly
   - **Solution**: Implement web-q prototype design
   - **Impact**: Medium - CSS work

8. **No Worker Details Display**
   - **Current**: No details popup
   - **Problem**: Can't see worker metadata
   - **Solution**: Create WorkerDetailsPopup component
   - **Impact**: Low - simple popup

### 4.3 Protocol Gaps

#### New Commands Needed

```rust
pub enum WebSocketCommand {
    // Existing
    Prompt { text: String },
    Cancel,
    Ping,
    
    // New
    CreateWorker {
        name: Option<String>,
        agent: String,
        working_directory: Option<String>,
    },
    GetWorkers,
    GetConversationHistory { worker_id: String },
}
```

#### New Events Needed

```rust
pub enum WebUIEvent {
    // Existing events...
    
    // New
    WorkersSnapshot {
        workers: Vec<WorkerMetadataJson>,
        timestamp: f64,
    },
    ConversationSnapshot {
        worker_id: String,
        entries: Vec<ConversationEntryJson>,
        timestamp: f64,
    },
    ConversationEntryAdded {
        worker_id: String,
        entry: ConversationEntryJson,
        timestamp: f64,
    },
}
```

## 5. Technical Challenges and Solutions

### 5.1 Challenge: WebSocket Single-Worker to Multi-Worker

**Problem**: Current WebSocket design is hardcoded to single worker.

**Current Flow**:
```
Client connects to /ws/worker/:worker_id
  → Handler validates worker exists
  → Handler filters events by worker_id
  → Client receives only events for that worker
```

**Proposed Flow**:
```
Client connects to /ws
  → Handler sends WorkersSnapshot (all workers)
  → Handler sends all events (no filtering)
  → Client filters events if needed
```

**Implementation**:
1. Change WebSocket route from `/ws/worker/:worker_id` to `/ws`
2. Remove worker_id validation from handler
3. Remove event filtering in send task
4. Send WorkersSnapshot on connection
5. Update frontend to handle all events

**Trade-offs**:
- ✅ Simpler backend (no filtering)
- ✅ Frontend gets all updates
- ✅ Matches web-q prototype
- ❌ More events sent to client (but local network, not a problem)

### 5.2 Challenge: ConversationEntry Serialization

**Problem**: ConversationEntry contains complex nested types that are hard to serialize.

**Structure**:
```
ConversationEntry
  ├─ user: Option<UserMessage>
  │  ├─ additional_context: String
  │  ├─ env_context: UserEnvContext
  │  ├─ content: UserMessageContent (enum)
  │  ├─ timestamp: Option<DateTime>
  │  └─ images: Option<Vec<ImageBlock>>
  └─ assistant: Option<AssistantMessage>
     ├─ Response { message_id, content }
     └─ ToolUse { message_id, content, tool_uses }
```

**Solution**: Create simplified JSON representation

```rust
#[derive(Serialize, Deserialize)]
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
```

**Conversion Function**:
```rust
fn convert_conversation_entry(entry: &ConversationEntry) -> ConversationEntryJson {
    if let Some(user_msg) = &entry.user {
        let content = extract_user_content(user_msg);
        let timestamp = user_msg.timestamp
            .map(|dt| dt.timestamp() as f64)
            .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64());
        
        ConversationEntryJson::UserMessage { content, timestamp }
    } else if let Some(assistant_msg) = &entry.assistant {
        match assistant_msg {
            AssistantMessage::Response { content, .. } => {
                ConversationEntryJson::AssistantMessage {
                    content: content.clone(),
                    timestamp: SystemTime::now()...
                }
            }
            AssistantMessage::ToolUse { content, tool_uses, .. } => {
                ConversationEntryJson::ToolUse {
                    content: content.clone(),
                    tool_uses: tool_uses.iter().map(convert_tool_use).collect(),
                    timestamp: SystemTime::now()...
                }
            }
        }
    } else {
        // Empty entry - shouldn't happen
        ConversationEntryJson::UserMessage {
            content: "".to_string(),
            timestamp: 0.0,
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
```

**Trade-offs**:
- ✅ Simple frontend representation
- ✅ Easy to render
- ❌ Loses metadata
- ❌ Approximate timestamps

### 5.3 Challenge: Worker Creation with Os Dependency

**Problem**: WorkerBuilder.build() requires Os reference, but WebSocket handler doesn't have it.

**Current AppState**:
```rust
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
}
```

**Solution**: Add Os to AppState

```rust
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
    pub os: Arc<Os>,  // Add this
}
```

**Implementation**:
1. Create Os in ChatArgs::execute()
2. Pass Os to WebServer::new()
3. Store Os in AppState
4. Use Os in CreateWorker command handler

**Command Handler**:
```rust
async fn handle_create_worker(
    name: Option<String>,
    agent: String,
    working_directory: Option<String>,
    session: Arc<Session>,
    os: Arc<Os>,
) -> Result<Arc<Worker>> {
    let worker = WorkerBuilder::new()
        .agent(Some(agent))
        .initial_input(None)
        .build(session, &os)
        .await?;
    
    // WorkerCreated event is automatically published by session.build_worker()
    
    Ok(worker)
}
```

**Trade-offs**:
- ✅ Proper agent configuration loading
- ✅ Resource loading works
- ✅ Consistent with rest of codebase
- ❌ Requires Os in AppState (minor)

### 5.4 Challenge: Response Accumulation

**Problem**: OutputChunk events arrive as fragments, need to accumulate into single response.

**Current Behavior**:
```
OutputChunk { chunk_type: AssistantResponse, text: "Hello" }
OutputChunk { chunk_type: AssistantResponse, text: ", " }
OutputChunk { chunk_type: AssistantResponse, text: "world!" }
```

**Desired Behavior**:
```
Single bubble with "Hello, world!" that updates in real-time
```

**Solution**: ResponseAccumulator class

```javascript
class ResponseAccumulator {
    constructor(app) {
        this.app = app;
        this.activeResponses = new Map();  // worker_id -> {text, bubbleElement}
    }
    
    handleChunk(event) {
        const { worker_id, chunk } = event;
        
        if (chunk.chunk_type === 'assistant_response') {
            if (!this.activeResponses.has(worker_id)) {
                // Start new response
                const bubbleElement = this.app.components.conversationView
                    .createBubble('assistant', chunk.text);
                
                this.activeResponses.set(worker_id, {
                    text: chunk.text,
                    bubbleElement,
                });
            } else {
                // Append to existing response
                const response = this.activeResponses.get(worker_id);
                response.text += chunk.text;
                response.bubbleElement.textContent = response.text;
            }
        }
    }
    
    finalize(workerId) {
        // Called on JobCompleted
        this.activeResponses.delete(workerId);
    }
}
```

**Trade-offs**:
- ✅ Clean real-time updates
- ✅ Single bubble per response
- ✅ Handles multiple workers
- ❌ Need to track active responses per worker

### 5.5 Challenge: Initial State Synchronization

**Problem**: When client connects, it needs to know about existing workers and their conversations.

**Solution**: Send snapshots on connection

**Connection Flow**:
```
1. Client connects to /ws
2. Server sends WorkersSnapshot with all workers
3. Client selects first worker (or main worker)
4. Client sends GetConversationHistory for selected worker
5. Server sends ConversationSnapshot
6. Client displays conversation
7. Client receives real-time events for updates
```

**Alternative Flow** (simpler):
```
1. Client connects to /ws
2. Server sends WorkersSnapshot with all workers
3. Server sends ConversationSnapshot for main worker (if exists)
4. Client displays main worker conversation
5. Client can request other conversations as needed
```

**Recommendation**: **Alternative flow** - simpler and faster initial load.

## 6. Implementation Recommendations

### 6.1 Backend Changes

#### Priority 1: WebSocket Protocol Changes

1. **Change WebSocket route**: `/ws/worker/:worker_id` → `/ws`
2. **Remove event filtering**: Send all events to all clients
3. **Add WorkersSnapshot event**: Sent on connection
4. **Add ConversationSnapshot event**: Sent on connection or request
5. **Add CreateWorker command**: With agent, name, working_directory

#### Priority 2: Serialization

1. **Create ConversationEntryJson type**: Simplified representation
2. **Create WorkerMetadataJson type**: Complete worker metadata
3. **Add conversion functions**: From internal types to JSON types
4. **Add Os to AppState**: For WorkerBuilder

#### Priority 3: API Endpoints

1. **Add GET /api/workers/:id/conversation**: REST endpoint for conversation history
2. **Update GET /api/workers/:id**: Include more metadata

### 6.2 Frontend Changes

#### Priority 1: Architecture

1. **Create WebUIState class**: Centralized state management
2. **Create component structure**: WorkerList, ConversationView, InputArea, etc.
3. **Create ResponseAccumulator**: Handle streaming responses

#### Priority 2: UI Components

1. **Create WorkerList component**: Sidebar with worker list
2. **Create ConversationView component**: Chat bubbles
3. **Create NewWorkerDialog component**: Worker creation form
4. **Create WorkerDetailsPopup component**: Worker metadata display

#### Priority 3: Visual Design

1. **Implement two-column layout**: Sidebar + main content
2. **Style chat bubbles**: User vs assistant styling
3. **Add state indicators**: Icons for idle/busy/failed
4. **Improve overall styling**: Match web-q prototype

### 6.3 Testing Strategy

#### Backend Tests

1. **Unit tests**: Serialization functions
2. **Integration tests**: WebSocket commands and events
3. **API tests**: REST endpoints

#### Frontend Tests

1. **Manual testing**: User flows
2. **Browser testing**: Chrome, Firefox, Safari
3. **Reconnection testing**: Network interruption

### 6.4 Phased Implementation

**Phase 1: Backend Foundation**
- Change WebSocket protocol to multi-worker
- Add WorkersSnapshot event
- Add ConversationSnapshot event
- Add serialization types

**Phase 2: Worker Creation**
- Add CreateWorker command
- Add Os to AppState
- Implement worker creation handler

**Phase 3: Frontend Foundation**
- Create WebUIState class
- Create component structure
- Implement WorkerList component

**Phase 4: Conversation Display**
- Create ConversationView component
- Implement ResponseAccumulator
- Display conversation history

**Phase 5: Worker Creation UI**
- Create NewWorkerDialog component
- Implement worker creation flow

**Phase 6: Polish**
- Visual design improvements
- Worker details popup
- Error handling
- Testing

## 7. Potential Pitfalls

### 7.1 Timestamp Handling

**Issue**: ConversationEntry doesn't have timestamps, but WebUI needs them.

**Impact**: Timestamps will be approximate (generated during serialization).

**Mitigation**: Document this limitation, consider adding timestamps to ConversationEntry in future.

### 7.2 Complex Serialization

**Issue**: UserMessage and AssistantMessage have complex nested structures.

**Impact**: Serialization logic will be non-trivial, risk of bugs.

**Mitigation**: Write thorough unit tests, handle all enum variants.

### 7.3 WebSocket Reconnection

**Issue**: When WebSocket reconnects, client needs to resync state.

**Impact**: Potential for state inconsistencies.

**Mitigation**: Send WorkersSnapshot on every connection, client can request fresh data.

### 7.4 Multiple Browser Tabs

**Issue**: Multiple tabs connecting to same WebSocket endpoint.

**Impact**: All tabs receive all events, need to handle gracefully.

**Mitigation**: Each tab maintains independent state, events update all tabs.

### 7.5 Worker Name Conflicts

**Issue**: User might create workers with duplicate names.

**Impact**: Confusing UI, hard to distinguish workers.

**Mitigation**: Auto-generate unique names if not provided, or append counter.

### 7.6 Large Conversation History

**Issue**: Conversations with thousands of messages could be slow to load.

**Impact**: Poor performance, slow initial load.

**Mitigation**: For MVP, accept this limitation. Future: pagination or lazy loading.

### 7.7 Os Lifecycle

**Issue**: Os is created in ChatArgs::execute(), needs to be shared with WebServer.

**Impact**: Need to ensure Os is available when WebServer starts.

**Mitigation**: Create Os before WebServer, pass through AppState.

## 8. Success Criteria

### 8.1 Functional Requirements

- ✅ User can see all workers in sidebar
- ✅ User can select any worker
- ✅ User can create new workers with agent configuration
- ✅ User can see full conversation history for selected worker
- ✅ Streaming responses accumulate into single bubbles
- ✅ Worker state updates in real-time
- ✅ Multiple workers can be busy simultaneously

### 8.2 Non-Functional Requirements

- ✅ WebSocket reconnection works
- ✅ Performance is acceptable (< 500ms for conversation load)
- ✅ Visual design is clean and intuitive
- ✅ Works in Chrome, Firefox, Safari

### 8.3 Out of Scope

- ❌ Worker deletion
- ❌ Worker name editing
- ❌ Terminal tab
- ❌ Markdown rendering
- ❌ Tool approval UI
- ❌ Conversation persistence

## 9. Conclusion

The mvp-webui-plus workflow requires significant changes to both backend and frontend, but the existing architecture provides a solid foundation. The main challenges are:

1. **WebSocket protocol redesign**: Single-worker to multi-worker
2. **Conversation serialization**: Complex types to simple JSON
3. **Frontend rewrite**: Monolithic to component-based
4. **Response accumulation**: Fragments to complete bubbles

All challenges have clear solutions, and the implementation can be phased to reduce risk. The existing EventBus architecture, Session API, and Worker structure are well-designed and require minimal changes.

**Estimated Complexity**: Medium-High
- Backend changes: Medium (protocol changes, serialization)
- Frontend changes: High (complete rewrite)
- Testing: Medium (manual testing, multiple browsers)

**Recommended Approach**: Phased implementation starting with backend foundation, then frontend components, then polish.
