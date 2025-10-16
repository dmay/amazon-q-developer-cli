# MVP WebUI Plus - Scope

## 1. Problem Statement

### Current Limitations

The basic WebUI implementation (completed in mvp-webui) provides foundational WebSocket-based event streaming and single-worker interaction. However, it has significant limitations that prevent it from being a practical tool for users:

1. **Single Worker View Only**: The UI is hardcoded to display only one worker at a time. Users cannot see or manage multiple workers, which is a core capability of the Agent Environment architecture.

2. **No Worker Management**: Users cannot create new workers, switch between workers, or view worker details. The UI assumes a single pre-existing worker.

3. **Streaming-Only Output**: The UI displays raw output chunks as they arrive, but doesn't accumulate them into coherent conversation history. Users see a stream of fragments rather than a readable conversation.

4. **No Conversation Context**: The UI doesn't display the full conversation history stored in the Worker's ContextContainer. Users can't see previous exchanges or understand the context of the current interaction.

5. **Poor Visual Presentation**: Output chunks are displayed in a monospace container without proper formatting, making it difficult to distinguish between user prompts, assistant responses, tool use, and errors.

6. **Limited Usability**: The UI lacks basic features like worker details display, visual state indicators, and intuitive navigation that would make it useful for real work.

### Why This Matters

The Agent Environment architecture is designed to support multiple concurrent workers, each with independent conversation history and state. The current WebUI doesn't expose this capability, making it essentially a proof-of-concept rather than a usable interface. Users need to:

- Manage multiple concurrent tasks (workers) in a single interface
- See the full conversation history for each worker
- Create new workers with specific configurations
- Switch between workers easily
- Understand worker state and activity at a glance

Without these capabilities, the WebUI cannot serve as a practical alternative to the TextUI for interactive work.

### Reference Implementation

The `web-q` prototype (located at `/Volumes/workplace/web-q/public`) demonstrates the target user experience:
- Sidebar with task (worker) list
- Active task selection with visual indication
- Task details popup showing metadata
- Chat view with conversation bubbles
- New task creation dialog
- Clean, intuitive layout

This workflow aims to bring the WebUI to feature parity with this prototype, adapted for the Agent Environment architecture.

## 2. Goals and Non-Goals

### Goals

**Primary Goal**: Transform the basic WebUI into a practical multi-worker chat interface that matches the usability of the web-q prototype.

**Specific Goals**:

1. **Multi-Worker Display**
   - Display all workers in a sidebar list
   - Show worker state (Idle, Busy, IdleFailed) with visual indicators
   - Allow users to select and switch between workers
   - Update worker list in real-time as workers are created/deleted

2. **Conversation History View**
   - Display full conversation history from Worker's ContextContainer
   - Present conversation as styled bubbles (user prompts vs assistant responses)
   - Accumulate streaming response chunks into single response blocks
   - Auto-scroll to latest message
   - Distinguish between different message types (user, assistant, tool use, errors)

3. **Worker Creation**
   - Provide "New Worker" button in UI
   - Show dialog to collect worker configuration:
     - Worker name (optional, defaults to generated name)
     - Agent name (to pass to WorkerBuilder)
     - Working directory path (collected but not used yet - future feature)
   - Create worker via WebSocket command
   - Automatically select newly created worker

4. **Worker Details**
   - Display worker metadata (ID, name, agent, state)
   - Show details popup or panel with full worker information
   - Update details in real-time as worker state changes

5. **Improved Visual Design**
   - Clean two-column layout (sidebar + main content)
   - Bubble-style chat presentation inspired by web-q prototype
   - Color-coded state indicators
   - Responsive design that works on different screen sizes
   - Professional appearance suitable for daily use

### Non-Goals

**Explicitly Out of Scope**:

1. **Terminal Tab**: The web-q prototype has both Terminal and Chat tabs. We are implementing ONLY the Chat tab. Terminal functionality is not part of this workflow.

2. **Worker Deletion**: While the web-q prototype allows task deletion, we are not implementing worker deletion in this workflow. This requires careful consideration of job cancellation and cleanup that should be addressed separately.

3. **Worker Name Editing**: Inline editing of worker names (as in web-q prototype) is not included. Workers are named at creation time.

4. **Working Directory Usage**: The new worker dialog collects working directory path, but it's not actually used yet. This is a placeholder for future functionality where workers can have different working directories.

5. **Advanced Chat Features**: No markdown rendering, code syntax highlighting, or rich text formatting in this workflow. Messages are displayed as plain text with basic styling.

6. **Conversation History Persistence**: This workflow focuses on displaying the in-memory conversation history. Saving/loading conversation history to disk is out of scope.

7. **Tool Approval UI**: Advanced tool approval interfaces (approve/deny buttons for tool use) are not part of this workflow. Tool execution follows existing approval logic.

8. **Multi-User Support**: The WebUI is for single-user local use. No authentication, authorization, or multi-user features.

9. **Mobile Optimization**: While the design should be responsive, mobile-specific optimizations are not a priority.

### Success Criteria

This workflow is successful if:

1. Users can see all workers in a sidebar and select any worker to view its conversation
2. Users can create new workers with agent configuration
3. Conversation history is displayed as readable chat bubbles, not raw output chunks
4. Streaming responses accumulate into single response blocks in real-time
5. Worker state is clearly visible and updates in real-time
6. The UI is visually clean and intuitive to use
7. All functionality works via WebSocket communication (no page reloads)

## 3. Current State Analysis

### What Works Today

The basic WebUI implementation (mvp-webui) provides a solid foundation:

**Backend Infrastructure**:
- ✅ WebServer with Axum framework running on localhost
- ✅ WebSocket connection for bidirectional communication
- ✅ WebUIEvent serialization with flat enum structure
- ✅ Event conversion from AgentEnvironmentEvent to WebUIEvent
- ✅ WebUI component implementing HeadlessInterface
- ✅ Event broadcasting to multiple WebSocket clients
- ✅ Time conversion for Instant → Unix timestamp

**Event Types Supported**:
- ✅ WorkerCreated, WorkerDeleted, WorkerStateChanged
- ✅ JobStarted, JobCompleted
- ✅ OutputChunk (with AssistantResponse, ToolUse, ToolResult variants)
- ✅ ResponseReceived, ToolUseRequested
- ✅ ShutdownInitiated

**WebSocket Protocol**:
- ✅ Client → Server: Commands (Prompt, Cancel, Compact, Quit)
- ✅ Server → Client: Events (all WebUIEvent types)
- ✅ JSON serialization/deserialization
- ✅ Connection management with reconnection logic

**Frontend (Basic)**:
- ✅ HTML structure with worker container, output area, input field
- ✅ CSS styling with state badges and output formatting
- ✅ JavaScript WebSocket client with reconnection
- ✅ Event handling for worker state and output chunks
- ✅ User input with send/cancel buttons

**CLI Integration**:
- ✅ `--web-ui` flag to enable WebUI
- ✅ `--web-port` flag to specify port (default: 8080)
- ✅ WebUI added to AgentEnvironment's headless UIs
- ✅ Graceful shutdown coordination

### What's Missing

**Backend Gaps**:

1. **Worker List API**: No REST endpoint to fetch all workers from Session
2. **Worker Creation API**: No WebSocket command or REST endpoint to create new workers
3. **Conversation History API**: No way to fetch full conversation history from Worker's ContextContainer
4. **Worker Details API**: No endpoint to get detailed worker metadata

**Frontend Gaps**:

1. **No Multi-Worker UI**: Frontend is hardcoded to display single worker
2. **No Sidebar**: No worker list, no worker selection
3. **No Conversation View**: Only shows streaming output chunks, not full conversation history
4. **No Worker Creation Dialog**: No UI to create new workers
5. **No Worker Details Display**: No way to view worker metadata
6. **Poor Message Presentation**: Output chunks displayed as raw fragments, not accumulated responses
7. **Limited Styling**: Basic styling doesn't match web-q prototype's clean design

**Protocol Gaps**:

1. **No CreateWorker Command**: WebSocket protocol doesn't support worker creation
2. **No GetWorkers Command**: No way to request worker list via WebSocket
3. **No GetConversationHistory Command**: No way to request full conversation history
4. **No Snapshot Event**: No initial state event when client connects

### Architecture Considerations

**Session and Worker Access**:
- Session maintains `workers: Arc<Mutex<HashMap<Uuid, Arc<Worker>>>>` 
- WebUI has `session: Arc<Session>` reference
- WebSocket handlers need access to Session to:
  - List all workers
  - Create new workers via WorkerBuilder
  - Access Worker's ContextContainer for conversation history

**Event Flow**:
- AgentEnvironment receives events from EventBus
- AgentEnvironment forwards events to all HeadlessInterface implementations (including WebUI)
- WebUI converts events to WebUIEvent and broadcasts to WebSocket clients
- This flow works well for real-time updates

**Command Flow**:
- WebSocket clients send commands as JSON
- WebSocket handler parses commands and calls appropriate Session methods
- Need to extend this for worker creation and queries

**Conversation History**:
- Worker has `context_container: Arc<ContextContainer>`
- ContextContainer has `conversation_history: Arc<Mutex<ConversationHistory>>`
- ConversationHistory has `entries: Vec<ConversationEntry>`
- ConversationEntry is enum: InputMessage, OutputMessage, ToolUse, ToolResult
- Need to serialize this structure for WebUI

### Technical Debt

**From mvp-webui Implementation**:

1. **Hardcoded Worker ID**: Frontend assumes single worker, no worker selection logic
2. **No State Management**: Frontend doesn't maintain worker list or conversation state
3. **Fragment Display**: Output chunks displayed as received, not accumulated
4. **No Error Recovery**: Limited error handling for API failures
5. **Basic Styling**: CSS is functional but not polished

**Not Blocking This Workflow**:
- Tool approval UI (out of scope)
- Markdown rendering (out of scope)
- Conversation persistence (out of scope)
- Worker deletion (out of scope)

## 4. Target User Experience

### Visual Layout

The target UI follows the web-q prototype's two-column layout:

```
┌─────────────────────────────────────────────────────────────┐
│  Q Developer CLI - Web UI                                   │
├──────────────┬──────────────────────────────────────────────┤
│              │  Worker: main-worker                    [i]  │
│  Workers     ├──────────────────────────────────────────────┤
│              │                                              │
│  ● main      │  ┌────────────────────────────────────────┐ │
│    (Idle)    │  │ User: Hello, can you help me?          │ │
│              │  └────────────────────────────────────────┘ │
│  ⚙ research  │                                              │
│    (Busy)    │  ┌────────────────────────────────────────┐ │
│              │  │ Assistant: Of course! I'd be happy to  │ │
│  ✗ debug     │  │ help. What do you need assistance with?│ │
│    (Failed)  │  └────────────────────────────────────────┘ │
│              │                                              │
│              │  ┌────────────────────────────────────────┐ │
│  [+ New]     │  │ User: Show me the files in this dir    │ │
│              │  └────────────────────────────────────────┘ │
│              │                                              │
│              │  ┌────────────────────────────────────────┐ │
│              │  │ Assistant: [streaming response...]     │ │
│              │  └────────────────────────────────────────┘ │
│              │                                              │
│              ├──────────────────────────────────────────────┤
│              │  [Type your message...]            [Send]   │
└──────────────┴──────────────────────────────────────────────┘
```

### User Flows

**Flow 1: Starting the WebUI**

1. User runs `q chat --web-ui`
2. CLI starts WebServer on localhost:8080
3. CLI logs: "Web UI available at http://127.0.0.1:8080"
4. User opens browser to http://127.0.0.1:8080
5. WebUI loads and connects via WebSocket
6. WebUI receives initial snapshot with main worker
7. Sidebar shows main worker in list
8. Main worker is auto-selected
9. Conversation history (if any) is displayed

**Flow 2: Viewing Conversation History**

1. User selects a worker from sidebar
2. WebUI requests conversation history for that worker
3. Backend fetches ConversationHistory from Worker's ContextContainer
4. Backend sends conversation entries as events or snapshot
5. WebUI displays conversation as chat bubbles:
   - User messages: Right-aligned, blue background
   - Assistant messages: Left-aligned, green background
   - Tool use: Left-aligned, orange background
   - Errors: Left-aligned, red background
6. WebUI auto-scrolls to latest message

**Flow 3: Sending a Message**

1. User types message in input field
2. User clicks Send or presses Enter
3. WebUI sends Prompt command via WebSocket
4. Backend adds message to Worker's conversation history
5. Backend starts agent loop job
6. WebUI receives JobStarted event
7. WebUI shows "thinking" indicator
8. WebUI receives OutputChunk events as response streams
9. WebUI accumulates chunks into single response bubble
10. Response bubble updates in real-time as chunks arrive
11. WebUI receives JobCompleted event
12. Worker state changes to Idle
13. Input field is re-enabled

**Flow 4: Creating a New Worker**

1. User clicks "+ New" button in sidebar
2. WebUI shows dialog with fields:
   - Worker name (optional, placeholder: "auto-generated")
   - Agent name (dropdown or text input)
   - Working directory (text input, placeholder: "~/")
3. User fills in agent name (e.g., "rust-agent")
4. User clicks Create
5. WebUI sends CreateWorker command via WebSocket
6. Backend calls WorkerBuilder with provided configuration
7. Backend creates new worker
8. Backend publishes WorkerCreated event
9. WebUI receives WorkerCreated event
10. WebUI adds new worker to sidebar
11. WebUI auto-selects new worker
12. Dialog closes
13. User can start chatting with new worker

**Flow 5: Switching Between Workers**

1. User clicks on different worker in sidebar
2. WebUI updates active worker selection (visual highlight)
3. WebUI requests conversation history for selected worker
4. WebUI clears current conversation display
5. WebUI displays conversation history for selected worker
6. WebUI updates worker details in header
7. Input field is enabled/disabled based on worker state
8. User can send messages to selected worker

**Flow 6: Viewing Worker Details**

1. User clicks info icon [i] next to worker name in header
2. WebUI shows popup with worker details:
   - Worker ID (UUID)
   - Worker name
   - Agent name
   - Current state (Idle/Busy/IdleFailed)
   - Job ID (if busy)
   - Task metadata (if available)
3. User clicks outside popup to close
4. Popup disappears

**Flow 7: Real-Time Updates**

1. Worker A is busy processing a task
2. User is viewing Worker B's conversation
3. WebUI receives WorkerStateChanged event for Worker A
4. Sidebar updates Worker A's state indicator (⚙ → ●)
5. User doesn't see interruption in Worker B's view
6. User can switch to Worker A to see its updated state

### Visual Design Elements

**Sidebar**:
- Fixed width (250px)
- Light gray background (#f5f5f5)
- Worker list with scroll if many workers
- Each worker item shows:
  - State icon (● Idle, ⚙ Busy, ✗ Failed)
  - Worker name
  - State label in smaller text
- Active worker highlighted with blue background
- "+ New" button at bottom

**Main Content Area**:
- White background
- Header bar with:
  - Worker name (bold, 18px)
  - Info icon [i] for details popup
- Conversation area:
  - Scrollable container
  - Chat bubbles with appropriate styling
  - Auto-scroll to bottom on new messages
  - Padding between bubbles
- Input area at bottom:
  - Text input field (flex: 1)
  - Send button (blue)
  - Cancel button (red, only visible when busy)

**Chat Bubbles**:
- User messages:
  - Right-aligned (align-self: flex-end)
  - Blue background (#e3f2fd)
  - Blue border (#90caf9)
  - Max width 80%
  - Border radius 12px
  - Padding 12px 16px
- Assistant messages:
  - Left-aligned (align-self: flex-start)
  - Green background (#f1f8e9)
  - Green border (#aed581)
  - Max width 80%
  - Border radius 12px
  - Padding 12px 16px
  - Accumulates streaming chunks
- Tool use messages:
  - Left-aligned
  - Orange background (#fff3e0)
  - Orange border (#ffb74d)
  - Shows tool name and parameters
- Error messages:
  - Left-aligned
  - Red background (#ffebee)
  - Red border (#ef5350)

**State Indicators**:
- Idle: Green circle ●
- Busy: Orange gear ⚙ (or spinner animation)
- Failed: Red X ✗

**Responsive Behavior**:
- Sidebar collapses to icon-only on narrow screens (future)
- Chat bubbles adjust max-width on small screens
- Input area remains fixed at bottom

### Interaction Patterns

**Keyboard Shortcuts**:
- Enter: Send message (when input focused)
- Escape: Close dialog/popup
- Ctrl+N: New worker (future)

**Mouse Interactions**:
- Click worker in sidebar: Select worker
- Click info icon: Show details popup
- Click outside popup: Close popup
- Click Send: Send message
- Click Cancel: Cancel current job

**Loading States**:
- WebSocket connecting: Show "Connecting..." in header
- WebSocket disconnected: Show "Disconnected" with red indicator
- Worker busy: Show spinner or "thinking" indicator
- Streaming response: Show partial response with typing indicator

**Error Handling**:
- WebSocket connection failed: Show error message, retry automatically
- Worker creation failed: Show error in dialog, keep dialog open
- Message send failed: Show error toast, keep message in input
- Invalid command: Show error toast

## 5. Technical Requirements

### Backend Requirements

#### 5.1 REST API Endpoints

**GET /api/workers**
- Returns list of all workers from Session
- Response format:
  ```json
  {
    "workers": [
      {
        "id": "uuid-string",
        "name": "main-worker",
        "agent": "default",
        "state": "idle",
        "current_job_id": null
      }
    ]
  }
  ```
- Locks Session's workers HashMap briefly to collect data
- Returns serialized worker metadata

**GET /api/workers/:id/conversation**
- Returns full conversation history for specified worker
- Response format:
  ```json
  {
    "worker_id": "uuid-string",
    "entries": [
      {
        "type": "input_message",
        "content": "Hello, can you help me?",
        "timestamp": 1234567890.0
      },
      {
        "type": "output_message",
        "content": "Of course! I'd be happy to help.",
        "timestamp": 1234567891.0
      }
    ]
  }
  ```
- Locks Worker's ConversationHistory to read entries
- Converts ConversationEntry enum to serializable format
- Returns empty array if worker not found

**GET /api/workers/:id**
- Returns detailed metadata for specified worker
- Response format:
  ```json
  {
    "id": "uuid-string",
    "name": "main-worker",
    "agent": "default",
    "state": "idle",
    "current_job_id": null,
    "task_metadata": {},
    "created_at": 1234567890.0
  }
  ```
- Returns 404 if worker not found

#### 5.2 WebSocket Commands

**Existing Commands** (already implemented):
- `Prompt { worker_id, text }`: Send message to worker
- `Cancel { worker_id }`: Cancel current job
- `Compact { worker_id }`: Compact conversation (future)
- `Quit`: Shutdown server

**New Commands** (to be implemented):

**CreateWorker**
```json
{
  "type": "create_worker",
  "name": "optional-name",
  "agent": "rust-agent",
  "working_directory": "/path/to/dir"
}
```
- Creates new worker using WorkerBuilder
- Returns worker ID in response or via WorkerCreated event
- Validates agent name exists
- Working directory is stored but not used yet

**GetWorkers**
```json
{
  "type": "get_workers"
}
```
- Requests current worker list
- Server responds with WorkersSnapshot event

**GetConversationHistory**
```json
{
  "type": "get_conversation_history",
  "worker_id": "uuid-string"
}
```
- Requests full conversation history for worker
- Server responds with ConversationSnapshot event

#### 5.3 WebSocket Events

**Existing Events** (already implemented):
- `WorkerCreated`, `WorkerDeleted`, `WorkerStateChanged`
- `JobStarted`, `JobCompleted`
- `OutputChunk` (AssistantResponse, ToolUse, ToolResult)
- `ResponseReceived`, `ToolUseRequested`
- `ShutdownInitiated`

**New Events** (to be implemented):

**WorkersSnapshot**
```json
{
  "type": "workers_snapshot",
  "workers": [
    {
      "id": "uuid-string",
      "name": "main-worker",
      "agent": "default",
      "state": "idle",
      "current_job_id": null
    }
  ],
  "timestamp": 1234567890.0
}
```
- Sent on initial WebSocket connection
- Sent in response to GetWorkers command
- Provides complete worker list

**ConversationSnapshot**
```json
{
  "type": "conversation_snapshot",
  "worker_id": "uuid-string",
  "entries": [
    {
      "type": "input_message",
      "content": "Hello",
      "timestamp": 1234567890.0
    }
  ],
  "timestamp": 1234567890.0
}
```
- Sent on initial connection for selected worker
- Sent in response to GetConversationHistory command
- Sent when switching workers

**ConversationEntryAdded**
```json
{
  "type": "conversation_entry_added",
  "worker_id": "uuid-string",
  "entry": {
    "type": "input_message",
    "content": "Hello",
    "timestamp": 1234567890.0
  },
  "timestamp": 1234567890.0
}
```
- Sent when new entry added to conversation history
- Allows incremental updates instead of full snapshots

#### 5.4 Data Serialization

**ConversationEntry Serialization**

Need to convert `ConversationEntry` enum to JSON:

```rust
// Existing type (in context_container/conversation_entry.rs)
pub enum ConversationEntry {
    InputMessage(String),
    OutputMessage(String),
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { name: String, result: String },
}

// Serializable version for WebUI
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationEntryJson {
    InputMessage {
        content: String,
        timestamp: f64,
    },
    OutputMessage {
        content: String,
        timestamp: f64,
    },
    ToolUse {
        name: String,
        input: serde_json::Value,
        timestamp: f64,
    },
    ToolResult {
        name: String,
        result: String,
        timestamp: f64,
    },
}
```

**Worker Metadata Serialization**

```rust
#[derive(Serialize, Deserialize)]
pub struct WorkerMetadataJson {
    pub id: String,  // UUID as string
    pub name: String,
    pub agent: String,
    pub state: WorkerLifecycleState,  // Already serializable
    pub current_job_id: Option<String>,  // UUID as string
    pub task_metadata: HashMap<String, serde_json::Value>,
    pub created_at: f64,  // Unix timestamp
}
```

#### 5.5 Session API Extensions

Need to add methods to Session for WebUI access:

```rust
impl Session {
    // Get all workers (already exists, may need to make public)
    pub fn get_workers(&self) -> Vec<Arc<Worker>> {
        let workers = self.workers.lock().unwrap();
        workers.values().cloned().collect()
    }
    
    // Get worker by ID (already exists, may need to make public)
    pub fn get_worker(&self, id: Uuid) -> Option<Arc<Worker>> {
        let workers = self.workers.lock().unwrap();
        workers.get(&id).cloned()
    }
    
    // Create worker with builder (already exists via WorkerBuilder)
    // WorkerBuilder::new().agent(agent).build(session)
}
```

#### 5.6 Worker API Extensions

Need to add methods to Worker for conversation access:

```rust
impl Worker {
    // Get conversation history (need to add)
    pub fn get_conversation_history(&self) -> Vec<ConversationEntry> {
        let history = self.context_container
            .conversation_history
            .lock()
            .unwrap();
        history.entries.clone()
    }
    
    // Get metadata (need to add)
    pub fn get_metadata(&self) -> WorkerMetadataJson {
        WorkerMetadataJson {
            id: self.id.to_string(),
            name: self.name.clone(),
            agent: self.agent.clone(),
            state: self.lifecycle_state.lock().unwrap().clone(),
            current_job_id: self.current_job_id.lock().unwrap()
                .as_ref().map(|id| id.to_string()),
            task_metadata: self.task_metadata.lock().unwrap().clone(),
            created_at: self.created_at_timestamp,
        }
    }
}
```

### Frontend Requirements

#### 5.7 State Management

Frontend needs to maintain:

```javascript
class WebUIState {
  constructor() {
    this.workers = new Map();  // worker_id -> WorkerData
    this.selectedWorkerId = null;
    this.conversations = new Map();  // worker_id -> ConversationEntry[]
    this.activeResponses = new Map();  // worker_id -> accumulated response text
    this.connectionState = 'disconnected';  // disconnected, connecting, connected
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

class ConversationEntry {
  constructor(type, content, timestamp, metadata) {
    this.type = type;  // input_message, output_message, tool_use, tool_result
    this.content = content;
    this.timestamp = timestamp;
    this.metadata = metadata;  // For tool use: {name, input}
  }
}
```

#### 5.8 UI Components

**WorkerList Component**:
- Renders sidebar with worker list
- Shows state indicator for each worker
- Highlights selected worker
- Handles worker selection clicks
- Shows "+ New" button

**ConversationView Component**:
- Renders chat bubbles for conversation entries
- Accumulates streaming response chunks
- Auto-scrolls to bottom
- Handles different message types with appropriate styling

**WorkerDetailsPopup Component**:
- Shows worker metadata in popup
- Positioned near info icon
- Closes on outside click

**NewWorkerDialog Component**:
- Modal dialog with form fields
- Validates input
- Sends CreateWorker command
- Shows loading state during creation

**InputArea Component**:
- Text input field
- Send button
- Cancel button (conditional)
- Handles Enter key
- Disables when worker busy

#### 5.9 Event Handling

Frontend needs to handle:

1. **Initial Connection**:
   - Connect WebSocket
   - Request WorkersSnapshot
   - Select first worker (or main worker)
   - Request ConversationSnapshot for selected worker

2. **Worker Events**:
   - WorkerCreated: Add to worker list
   - WorkerDeleted: Remove from worker list
   - WorkerStateChanged: Update worker state in list
   - WorkersSnapshot: Replace entire worker list

3. **Conversation Events**:
   - ConversationSnapshot: Replace conversation view
   - ConversationEntryAdded: Append to conversation
   - OutputChunk: Accumulate into active response

4. **Job Events**:
   - JobStarted: Show thinking indicator
   - JobCompleted: Finalize response, update state

5. **User Actions**:
   - Select worker: Request conversation history
   - Send message: Send Prompt command
   - Create worker: Show dialog, send CreateWorker command
   - Cancel job: Send Cancel command

#### 5.10 Response Accumulation Logic

Critical requirement: Accumulate streaming OutputChunk events into single response bubble.

```javascript
class ResponseAccumulator {
  constructor() {
    this.activeResponses = new Map();  // worker_id -> {text, startTime}
  }
  
  handleOutputChunk(workerId, chunk) {
    if (chunk.chunk_type === 'assistant_response') {
      if (!this.activeResponses.has(workerId)) {
        // Start new response
        this.activeResponses.set(workerId, {
          text: chunk.text,
          startTime: Date.now()
        });
        // Create new bubble in UI
        this.createResponseBubble(workerId, chunk.text);
      } else {
        // Append to existing response
        const response = this.activeResponses.get(workerId);
        response.text += chunk.text;
        // Update existing bubble in UI
        this.updateResponseBubble(workerId, response.text);
      }
    }
  }
  
  handleJobCompleted(workerId) {
    // Finalize response
    if (this.activeResponses.has(workerId)) {
      const response = this.activeResponses.get(workerId);
      // Add to conversation history
      this.addToConversation(workerId, {
        type: 'output_message',
        content: response.text,
        timestamp: response.startTime / 1000
      });
      // Clear active response
      this.activeResponses.delete(workerId);
    }
  }
}
```

### Performance Requirements

- Worker list updates: < 100ms latency
- Conversation history load: < 500ms for 100 messages
- WebSocket message handling: < 50ms per event
- UI rendering: 60fps during streaming responses
- Memory usage: < 100MB for 10 workers with 1000 messages each

### Browser Compatibility

- Chrome/Edge: Latest 2 versions
- Firefox: Latest 2 versions
- Safari: Latest 2 versions
- No IE11 support required

### Security Requirements

- WebSocket connections only from localhost
- No authentication required (local-only use)
- No CORS issues (same origin)
- No XSS vulnerabilities in message rendering
- Sanitize user input before display

## 6. Architecture Changes

### 6.1 Backend Architecture

#### Current Architecture (mvp-webui)

```
ChatArgs::execute()
  ├─ Create EventBus
  ├─ Create Session
  ├─ Create main Worker
  ├─ Create WebUI (if --web-ui)
  ├─ Create AgentEnvironment
  │  └─ Forwards events to WebUI
  └─ Start WebServer
     ├─ Serves static files
     └─ WebSocket handler
        ├─ Subscribes to WebUI events
        └─ Handles commands (Prompt, Cancel)
```

**Event Flow**:
```
AgentLoop → EventBus → AgentEnvironment → WebUI → WebSocket → Browser
```

**Command Flow**:
```
Browser → WebSocket → Parse command → Session method → AgentLoop
```

#### Proposed Architecture (mvp-webui-plus)

**No major architectural changes needed**. The existing architecture supports multi-worker scenarios. We need to:

1. **Extend WebSocket Handler**:
   - Add CreateWorker command handling
   - Add GetWorkers command handling
   - Add GetConversationHistory command handling
   - Send WorkersSnapshot on connection
   - Send ConversationSnapshot on request

2. **Add REST API Endpoints**:
   - GET /api/workers
   - GET /api/workers/:id
   - GET /api/workers/:id/conversation
   - These are convenience endpoints, not required for core functionality

3. **Extend WebUIEvent**:
   - Add WorkersSnapshot variant
   - Add ConversationSnapshot variant
   - Add ConversationEntryAdded variant

4. **Add Serialization Helpers**:
   - ConversationEntryJson type
   - WorkerMetadataJson type
   - Conversion functions from internal types

#### File Structure Changes

```
crates/chat-cli/src/cli/chat/web_server/
├─ mod.rs                    # Module exports
├─ server.rs                 # WebServer (existing)
├─ websocket.rs              # WebSocket handler (EXTEND)
├─ api.rs                    # REST API handlers (EXTEND)
├─ events.rs                 # WebUIEvent (EXTEND)
├─ web_ui.rs                 # WebUI component (existing)
└─ serialization.rs          # NEW: Serialization helpers
   ├─ ConversationEntryJson
   ├─ WorkerMetadataJson
   └─ Conversion functions

web/public/
├─ index.html                # HTML structure (REWRITE)
├─ styles.css                # CSS styles (REWRITE)
└─ app.js                    # JavaScript application (REWRITE)
   ├─ WebUIState class
   ├─ WorkerList component
   ├─ ConversationView component
   ├─ NewWorkerDialog component
   └─ ResponseAccumulator
```

### 6.2 WebSocket Protocol Extensions

#### Connection Lifecycle

**Current**:
1. Client connects
2. Client receives events as they occur
3. Client sends commands

**Proposed**:
1. Client connects
2. **Server sends WorkersSnapshot** (initial state)
3. **Server sends ConversationSnapshot for main worker** (if exists)
4. Client receives events as they occur
5. Client sends commands
6. **Client can request snapshots at any time**

#### Command Extensions

**Add to websocket.rs**:

```rust
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    // Existing
    Prompt { worker_id: String, text: String },
    Cancel { worker_id: String },
    Compact { worker_id: String },
    Quit,
    
    // New
    CreateWorker {
        name: Option<String>,
        agent: String,
        working_directory: Option<String>,
    },
    GetWorkers,
    GetConversationHistory { worker_id: String },
}

async fn handle_command(
    session: Arc<Session>,
    command: ClientCommand,
) -> Result<(), Error> {
    match command {
        ClientCommand::CreateWorker { name, agent, working_directory } => {
            // Use WorkerBuilder to create worker
            let worker = WorkerBuilder::new()
                .name(name)
                .agent(Some(agent))
                .working_directory(working_directory)
                .build(session.clone())?;
            
            // WorkerCreated event is automatically published by Session
            Ok(())
        }
        ClientCommand::GetWorkers => {
            // Send WorkersSnapshot event
            let workers = session.get_workers();
            let snapshot = create_workers_snapshot(workers);
            // Send via WebSocket (need to pass sender)
            Ok(())
        }
        ClientCommand::GetConversationHistory { worker_id } => {
            // Send ConversationSnapshot event
            let worker = session.get_worker(Uuid::parse_str(&worker_id)?)?;
            let history = worker.get_conversation_history();
            let snapshot = create_conversation_snapshot(worker_id, history);
            // Send via WebSocket (need to pass sender)
            Ok(())
        }
        // ... existing command handling
    }
}
```

#### Event Extensions

**Add to events.rs**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebUIEvent {
    // ... existing variants ...
    
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

### 6.3 Frontend Architecture

#### Current Architecture (mvp-webui)

```javascript
// Single file: app.js
class QWebUI {
  constructor() {
    this.workerId = 'hardcoded-main';
    this.ws = null;
  }
  
  connect() { /* WebSocket connection */ }
  handleEvent(event) { /* Event handling */ }
  sendPrompt(text) { /* Send command */ }
}
```

**Limitations**:
- Hardcoded single worker
- No state management
- No component structure
- Monolithic event handling

#### Proposed Architecture (mvp-webui-plus)

```javascript
// app.js - Main application
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
  
  async init() {
    await this.ws.connect();
    this.components.workerList.render();
    this.components.conversationView.render();
    this.components.inputArea.render();
  }
  
  // State management
  selectWorker(workerId) {
    this.state.selectedWorkerId = workerId;
    this.ws.send({ type: 'get_conversation_history', worker_id: workerId });
    this.components.workerList.render();
    this.components.conversationView.clear();
  }
  
  // Event routing
  handleEvent(event) {
    switch (event.type) {
      case 'workers_snapshot':
        this.state.updateWorkers(event.workers);
        this.components.workerList.render();
        break;
      case 'conversation_snapshot':
        this.state.setConversation(event.worker_id, event.entries);
        this.components.conversationView.render();
        break;
      case 'output_chunk':
        this.accumulator.handleChunk(event);
        break;
      // ... other events
    }
  }
}

// State management
class WebUIState {
  constructor() {
    this.workers = new Map();
    this.selectedWorkerId = null;
    this.conversations = new Map();
    this.activeResponses = new Map();
  }
  
  updateWorkers(workers) { /* ... */ }
  setConversation(workerId, entries) { /* ... */ }
  getSelectedWorker() { /* ... */ }
}

// WebSocket client
class WebSocketClient {
  constructor(app) {
    this.app = app;
    this.ws = null;
    this.reconnectAttempts = 0;
  }
  
  connect() { /* ... */ }
  send(command) { /* ... */ }
  handleMessage(event) {
    const data = JSON.parse(event.data);
    this.app.handleEvent(data);
  }
}

// Components
class WorkerList {
  constructor(app) {
    this.app = app;
    this.element = document.getElementById('worker-list');
  }
  
  render() {
    const workers = Array.from(this.app.state.workers.values());
    this.element.innerHTML = workers.map(w => this.renderWorker(w)).join('');
  }
  
  renderWorker(worker) {
    const icon = this.getStateIcon(worker.state);
    const active = worker.id === this.app.state.selectedWorkerId ? 'active' : '';
    return `
      <div class="worker-item ${active}" onclick="app.selectWorker('${worker.id}')">
        <span class="worker-icon">${icon}</span>
        <span class="worker-name">${worker.name}</span>
        <span class="worker-state">${worker.state}</span>
      </div>
    `;
  }
}

class ConversationView {
  constructor(app) {
    this.app = app;
    this.element = document.getElementById('conversation');
  }
  
  render() {
    const workerId = this.app.state.selectedWorkerId;
    const entries = this.app.state.conversations.get(workerId) || [];
    this.element.innerHTML = entries.map(e => this.renderEntry(e)).join('');
    this.scrollToBottom();
  }
  
  renderEntry(entry) {
    const bubbleClass = this.getBubbleClass(entry.type);
    return `
      <div class="bubble ${bubbleClass}">
        ${this.escapeHtml(entry.content)}
      </div>
    `;
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
}

class ResponseAccumulator {
  constructor(app) {
    this.app = app;
    this.activeResponses = new Map();
  }
  
  handleChunk(event) {
    const { worker_id, chunk } = event;
    
    if (chunk.chunk_type === 'assistant_response') {
      if (!this.activeResponses.has(worker_id)) {
        // Start new response
        this.activeResponses.set(worker_id, chunk.text);
        this.app.components.conversationView.appendEntry({
          type: 'output_message',
          content: chunk.text,
          timestamp: event.timestamp,
        });
      } else {
        // Append to existing response
        const accumulated = this.activeResponses.get(worker_id) + chunk.text;
        this.activeResponses.set(worker_id, accumulated);
        this.app.components.conversationView.updateLastEntry(accumulated);
      }
    }
  }
  
  finalize(workerId) {
    this.activeResponses.delete(workerId);
  }
}
```

**Benefits**:
- Clear separation of concerns
- Reusable components
- Centralized state management
- Easy to test and extend
- Matches web-q prototype structure

### 6.4 Data Flow

#### Worker Creation Flow

```
User clicks "+ New"
  ↓
NewWorkerDialog.show()
  ↓
User fills form and clicks Create
  ↓
WebSocketClient.send({ type: 'create_worker', ... })
  ↓
WebSocket handler receives command
  ↓
WorkerBuilder.new().agent(agent).build(session)
  ↓
Session creates worker, publishes WorkerCreated event
  ↓
EventBus → AgentEnvironment → WebUI → WebSocket
  ↓
Browser receives WorkerCreated event
  ↓
WebUIApp.handleEvent() updates state
  ↓
WorkerList.render() shows new worker
  ↓
WebUIApp.selectWorker() auto-selects new worker
```

#### Conversation Display Flow

```
User selects worker in sidebar
  ↓
WebUIApp.selectWorker(workerId)
  ↓
WebSocketClient.send({ type: 'get_conversation_history', worker_id })
  ↓
WebSocket handler receives command
  ↓
Session.get_worker(id).get_conversation_history()
  ↓
Create ConversationSnapshot event
  ↓
Send via WebSocket
  ↓
Browser receives ConversationSnapshot event
  ↓
WebUIApp.handleEvent() updates state.conversations
  ↓
ConversationView.render() displays bubbles
```

#### Streaming Response Flow

```
User sends message
  ↓
WebSocketClient.send({ type: 'prompt', worker_id, text })
  ↓
Session.run_task__agent_loop()
  ↓
AgentLoop starts, publishes JobStarted event
  ↓
AgentLoop streams response, publishes OutputChunk events
  ↓
EventBus → AgentEnvironment → WebUI → WebSocket
  ↓
Browser receives OutputChunk events
  ↓
ResponseAccumulator.handleChunk()
  ├─ First chunk: Create new bubble
  └─ Subsequent chunks: Update existing bubble
  ↓
AgentLoop completes, publishes JobCompleted event
  ↓
ResponseAccumulator.finalize()
  ↓
ConversationView shows complete response
```

### 6.5 Backward Compatibility

**Existing WebUI users**:
- Old HTML/CSS/JS will be completely replaced
- No backward compatibility needed (WebUI is new feature)
- Users must refresh browser after update

**Existing WebSocket protocol**:
- All existing events remain unchanged
- All existing commands remain unchanged
- New events and commands are additions only
- Old clients will ignore new events (graceful degradation)

**CLI flags**:
- `--web-ui` flag remains unchanged
- `--web-port` flag remains unchanged
- No breaking changes to CLI interface

## 7. Implementation Considerations

### 7.1 Critical Design Decisions

#### Decision 1: REST API vs WebSocket-Only

**Options**:
1. **REST API for queries, WebSocket for events**: GET /api/workers, GET /api/workers/:id/conversation
2. **WebSocket-only**: All queries via WebSocket commands (GetWorkers, GetConversationHistory)
3. **Hybrid**: REST for initial load, WebSocket for updates

**Recommendation**: **Hybrid approach**
- REST endpoints for initial page load and direct access
- WebSocket commands for dynamic queries during session
- Provides flexibility for future features (e.g., bookmarking worker URLs)

**Rationale**:
- REST is simpler for initial state loading
- WebSocket is better for real-time updates
- Both are easy to implement with Axum
- No significant performance difference for local use

#### Decision 2: Snapshot vs Incremental Updates

**Options**:
1. **Full snapshots**: Send complete worker list and conversation history on every change
2. **Incremental updates**: Send only changes (WorkerCreated, ConversationEntryAdded)
3. **Hybrid**: Snapshots on connection, incremental updates during session

**Recommendation**: **Hybrid approach**
- Send WorkersSnapshot on initial connection
- Send ConversationSnapshot when switching workers
- Send incremental updates (WorkerCreated, ConversationEntryAdded) during session

**Rationale**:
- Snapshots ensure consistency on connection
- Incremental updates reduce bandwidth
- Simpler client-side state management
- Matches web-q prototype behavior

#### Decision 3: Response Accumulation Strategy

**Options**:
1. **Client-side accumulation**: Browser accumulates OutputChunk events
2. **Server-side accumulation**: Server sends complete response after job completes
3. **Hybrid**: Client accumulates for display, server sends final version

**Recommendation**: **Client-side accumulation**
- Browser accumulates OutputChunk events in real-time
- Updates bubble as chunks arrive
- Finalizes on JobCompleted event

**Rationale**:
- Provides real-time streaming experience
- No server-side changes needed
- Matches existing event flow
- User sees response as it's generated

#### Decision 4: Worker Name Generation

**Options**:
1. **User must provide name**: Required field in new worker dialog
2. **Auto-generate if empty**: Generate name like "worker-1", "worker-2"
3. **Smart generation**: Generate based on agent name, e.g., "rust-agent-1"

**Recommendation**: **Smart generation**
- If user provides name, use it
- If empty, generate: `{agent}-{counter}` (e.g., "rust-agent-1")
- Counter is per-agent to avoid conflicts

**Rationale**:
- User-friendly (name is optional)
- Meaningful names (includes agent)
- Avoids conflicts (counter)
- Matches common UX patterns

#### Decision 5: Conversation History Storage

**Options**:
1. **In-memory only**: ConversationHistory in Worker's ContextContainer
2. **Persist to disk**: Save conversation history to files
3. **Hybrid**: In-memory with optional persistence

**Recommendation**: **In-memory only** (for this workflow)
- Use existing ConversationHistory in ContextContainer
- No persistence in this workflow
- Persistence is out of scope (see Non-Goals)

**Rationale**:
- Simplest implementation
- Matches current architecture
- Persistence can be added later
- Sufficient for MVP use cases

### 7.2 Implementation Challenges

#### Challenge 1: Thread Safety

**Issue**: Multiple WebSocket connections accessing shared Session and Worker state.

**Solution**:
- Session already uses `Arc<Mutex<HashMap<Uuid, Arc<Worker>>>>`
- Worker uses `Arc<Mutex<...>>` for mutable state
- ConversationHistory uses `Arc<Mutex<Vec<ConversationEntry>>>`
- All access is already thread-safe

**Considerations**:
- Keep lock durations short
- Clone data before sending over WebSocket
- Don't hold locks across await points

#### Challenge 2: Worker ID Serialization

**Issue**: Worker IDs are `Uuid`, WebSocket uses JSON strings.

**Solution**:
- Serialize Uuid as string: `uuid.to_string()`
- Deserialize string to Uuid: `Uuid::parse_str(&s)?`
- Use String in WebUIEvent and API responses

**Considerations**:
- Validate UUID format on deserialization
- Return 400 Bad Request for invalid UUIDs
- Use consistent format (lowercase, hyphenated)

#### Challenge 3: Conversation Entry Timestamps

**Issue**: ConversationEntry doesn't have timestamps, but WebUI needs them for display.

**Solution**:
- **Option A**: Add timestamp field to ConversationEntry (breaking change)
- **Option B**: Generate timestamps when serializing (approximate)
- **Option C**: Store timestamps separately in Worker metadata

**Recommendation**: **Option B for MVP**
- Generate timestamps when creating ConversationSnapshot
- Use current time for all entries (approximate)
- Add proper timestamps in future refactor

**Rationale**:
- No breaking changes to core types
- Sufficient for MVP (timestamps not critical)
- Can be improved later without protocol changes

#### Challenge 4: Response Accumulation Edge Cases

**Issue**: What if OutputChunk events arrive out of order or are missed?

**Solution**:
- Use job_id to track which response chunks belong together
- If new job starts, finalize previous response
- If JobCompleted arrives without chunks, show empty response

**Edge Cases**:
- **Multiple jobs in quick succession**: Use job_id to separate responses
- **Job cancelled mid-stream**: Finalize partial response, mark as cancelled
- **WebSocket reconnection**: Request ConversationSnapshot to resync

#### Challenge 5: Worker Creation Validation

**Issue**: What if agent name doesn't exist or WorkerBuilder fails?

**Solution**:
- Validate agent name before creating worker
- Return error response via WebSocket if validation fails
- Show error message in dialog, keep dialog open

**Validation**:
- Check if agent exists in agent registry
- Validate working directory path (if provided)
- Check for duplicate worker names (optional)

**Error Response**:
```json
{
  "type": "error",
  "command": "create_worker",
  "message": "Agent 'invalid-agent' not found",
  "timestamp": 1234567890.0
}
```

#### Challenge 6: Memory Management

**Issue**: Conversation history grows unbounded, could cause memory issues.

**Solution** (for this workflow):
- No automatic cleanup (out of scope)
- User can use Compact command to reduce history
- Document memory considerations in user guide

**Future Improvements**:
- Automatic compaction after N messages
- Configurable history limits
- Conversation persistence to disk

### 7.3 Testing Strategy

#### Unit Tests

**Backend**:
- Serialization: ConversationEntry → ConversationEntryJson
- Worker metadata extraction
- WebSocket command parsing
- Event conversion: AgentEnvironmentEvent → WebUIEvent

**Frontend**:
- State management: WebUIState operations
- Response accumulation: ResponseAccumulator logic
- Component rendering: WorkerList, ConversationView

#### Integration Tests

**Backend**:
- REST API endpoints return correct data
- WebSocket connection lifecycle
- Command handling end-to-end
- Event broadcasting to multiple clients

**Frontend**:
- WebSocket connection and reconnection
- Worker creation flow
- Conversation display flow
- Response streaming and accumulation

#### Manual Testing

**Scenarios**:
1. Start WebUI, verify initial state
2. Create new worker, verify it appears in list
3. Send message, verify streaming response
4. Switch between workers, verify conversation history
5. Multiple workers busy simultaneously
6. WebSocket reconnection after disconnect
7. Browser refresh, verify state recovery
8. Multiple browser tabs (multiple WebSocket connections)

#### Performance Testing

**Metrics**:
- Worker list update latency
- Conversation history load time (100, 1000, 10000 messages)
- WebSocket message throughput
- Memory usage with multiple workers
- CPU usage during streaming responses

**Targets**:
- < 100ms for worker list updates
- < 500ms for 1000 message conversation load
- < 50ms per WebSocket message
- < 100MB memory for 10 workers with 1000 messages each

### 7.4 Migration Path

#### From mvp-webui to mvp-webui-plus

**Backend Changes**:
- Add new WebSocket commands (backward compatible)
- Add new WebUIEvent variants (backward compatible)
- Add REST API endpoints (new, no migration needed)
- Add serialization helpers (new code)

**Frontend Changes**:
- Complete rewrite of HTML/CSS/JS
- No migration needed (users refresh browser)

**User Impact**:
- Users must refresh browser after update
- No data loss (conversation history in memory)
- No configuration changes needed

#### Rollback Plan

If critical issues found:
1. Revert frontend files to mvp-webui version
2. Keep backend changes (backward compatible)
3. Old frontend will work with new backend
4. Fix issues and redeploy

### 7.5 Future Enhancements

**Not in this workflow, but worth considering**:

1. **Worker Deletion**:
   - Add DeleteWorker command
   - Cancel running jobs before deletion
   - Remove from Session's worker map
   - Publish WorkerDeleted event

2. **Worker Name Editing**:
   - Add RenameWorker command
   - Update Worker's name field
   - Publish WorkerRenamed event

3. **Conversation Persistence**:
   - Save conversation history to disk
   - Load on worker creation
   - Export/import conversations

4. **Markdown Rendering**:
   - Parse markdown in assistant responses
   - Syntax highlighting for code blocks
   - Render tables, lists, etc.

5. **Tool Approval UI**:
   - Show approve/deny buttons for tool use
   - Pause execution until user responds
   - Show tool parameters in readable format

6. **Multi-User Support**:
   - Authentication and authorization
   - Per-user worker isolation
   - Shared workers (optional)

7. **Mobile Optimization**:
   - Responsive design for mobile screens
   - Touch-friendly controls
   - Collapsible sidebar

8. **Keyboard Shortcuts**:
   - Ctrl+N: New worker
   - Ctrl+W: Close worker
   - Ctrl+K: Focus input
   - Arrow keys: Navigate workers

9. **Search and Filter**:
   - Search conversation history
   - Filter workers by state
   - Search across all workers

10. **Themes**:
    - Dark mode
    - Custom color schemes
    - Accessibility improvements

### 7.6 Dependencies

**Backend Dependencies** (already in Cargo.toml):
- axum: Web server framework
- tokio: Async runtime
- serde: Serialization
- serde_json: JSON support
- uuid: Worker IDs
- tower-http: Static file serving

**Frontend Dependencies** (none required):
- Vanilla JavaScript (no framework)
- No build step needed
- No npm dependencies

**Development Dependencies**:
- cargo test: Unit tests
- cargo clippy: Linting
- cargo fmt: Formatting

### 7.7 Documentation Needs

**User Documentation**:
- How to start WebUI (`q chat --web-ui`)
- How to create workers
- How to switch between workers
- How to interpret worker states
- Keyboard shortcuts
- Troubleshooting guide

**Developer Documentation**:
- WebSocket protocol specification
- REST API specification
- Event type reference
- Command type reference
- Architecture overview
- Testing guide

**Code Documentation**:
- Rustdoc comments for public APIs
- JSDoc comments for JavaScript functions
- Inline comments for complex logic
- README in web/public/ directory

## 8. Success Criteria

### 8.1 Functional Requirements

The implementation is successful if all of the following work correctly:

#### Multi-Worker Management
- ✅ User can see all workers in sidebar list
- ✅ User can select any worker from the list
- ✅ Selected worker is visually highlighted
- ✅ Worker state (Idle/Busy/IdleFailed) is displayed with appropriate icon
- ✅ Worker list updates in real-time when workers are created
- ✅ Worker list updates in real-time when worker state changes

#### Worker Creation
- ✅ User can click "+ New" button to open creation dialog
- ✅ Dialog has fields for name, agent, and working directory
- ✅ User can create worker with only agent name (name is optional)
- ✅ Worker name is auto-generated if not provided
- ✅ New worker appears in sidebar immediately after creation
- ✅ New worker is auto-selected after creation
- ✅ Error message is shown if creation fails

#### Conversation History Display
- ✅ User can see full conversation history for selected worker
- ✅ Conversation is displayed as chat bubbles, not raw output chunks
- ✅ User messages are right-aligned with blue styling
- ✅ Assistant messages are left-aligned with green styling
- ✅ Tool use messages are left-aligned with orange styling
- ✅ Error messages are left-aligned with red styling
- ✅ Conversation auto-scrolls to latest message
- ✅ Conversation history persists when switching between workers

#### Message Sending
- ✅ User can type message in input field
- ✅ User can send message by clicking Send button
- ✅ User can send message by pressing Enter key
- ✅ Input field is disabled when worker is busy
- ✅ Input field is enabled when worker is idle
- ✅ Message appears in conversation immediately after sending

#### Response Streaming
- ✅ Assistant response appears as single bubble (not fragments)
- ✅ Response bubble updates in real-time as chunks arrive
- ✅ Response accumulation works correctly across multiple chunks
- ✅ Response is finalized when job completes
- ✅ Multiple workers can stream responses simultaneously
- ✅ Streaming response in one worker doesn't affect other workers

#### Worker Details
- ✅ User can click info icon to view worker details
- ✅ Details popup shows worker ID, name, agent, state
- ✅ Details popup closes when clicking outside
- ✅ Details update in real-time as worker state changes

#### Real-Time Updates
- ✅ Worker state changes are reflected immediately in sidebar
- ✅ New workers appear in sidebar without page refresh
- ✅ Conversation updates appear without page refresh
- ✅ Multiple browser tabs receive updates simultaneously

#### Error Handling
- ✅ WebSocket connection errors are handled gracefully
- ✅ WebSocket reconnects automatically after disconnect
- ✅ Error messages are displayed to user when operations fail
- ✅ Invalid commands are rejected with error messages
- ✅ Invalid worker IDs return appropriate errors

### 8.2 Non-Functional Requirements

#### Performance
- ✅ Worker list updates appear within 100ms of event
- ✅ Conversation history loads within 500ms for 100 messages
- ✅ WebSocket messages are processed within 50ms
- ✅ UI remains responsive during streaming responses
- ✅ Memory usage is reasonable (< 100MB for 10 workers)

#### Usability
- ✅ UI is intuitive and easy to navigate
- ✅ Visual design is clean and professional
- ✅ State indicators are clear and understandable
- ✅ Error messages are helpful and actionable
- ✅ No page reloads required during normal use

#### Reliability
- ✅ WebSocket connection is stable
- ✅ Reconnection works after network interruption
- ✅ No data loss during reconnection
- ✅ No race conditions in state updates
- ✅ No memory leaks during extended use

#### Compatibility
- ✅ Works in Chrome/Edge (latest 2 versions)
- ✅ Works in Firefox (latest 2 versions)
- ✅ Works in Safari (latest 2 versions)
- ✅ Responsive design works on different screen sizes

### 8.3 Acceptance Tests

#### Test 1: Basic Workflow
1. Start `q chat --web-ui`
2. Open browser to http://127.0.0.1:8080
3. Verify main worker appears in sidebar
4. Send message "Hello"
5. Verify response streams and appears as single bubble
6. Verify worker state changes from Idle → Busy → Idle

**Expected**: All steps complete successfully, conversation is readable.

#### Test 2: Multi-Worker Workflow
1. Start WebUI with main worker
2. Click "+ New" button
3. Enter agent name "rust-agent"
4. Click Create
5. Verify new worker appears in sidebar
6. Verify new worker is auto-selected
7. Send message to new worker
8. Switch back to main worker
9. Verify main worker's conversation is still visible
10. Switch back to new worker
11. Verify new worker's conversation is still visible

**Expected**: Both workers maintain independent conversations, switching works smoothly.

#### Test 3: Concurrent Workers
1. Create 3 workers: main, worker-1, worker-2
2. Send long-running task to main worker
3. Switch to worker-1, send message
4. Switch to worker-2, send message
5. Verify all 3 workers show correct state in sidebar
6. Verify responses appear in correct conversations
7. Switch between workers while responses are streaming

**Expected**: All workers operate independently, no cross-contamination of responses.

#### Test 4: Error Handling
1. Create worker with invalid agent name
2. Verify error message appears in dialog
3. Verify dialog remains open
4. Correct agent name and create successfully
5. Disconnect network
6. Verify "Disconnected" indicator appears
7. Reconnect network
8. Verify WebSocket reconnects automatically
9. Verify state is recovered correctly

**Expected**: Errors are handled gracefully, recovery works automatically.

#### Test 5: Response Accumulation
1. Send message that generates long response
2. Observe response bubble as it streams
3. Verify response appears as single bubble
4. Verify bubble updates smoothly without flickering
5. Verify final response is complete and readable
6. Send another message immediately
7. Verify new response appears in separate bubble

**Expected**: Responses accumulate correctly, no fragments or duplicates.

#### Test 6: Browser Refresh
1. Create 2 workers, send messages to both
2. Refresh browser page
3. Verify WebSocket reconnects
4. Verify worker list is restored
5. Verify conversation history is restored for selected worker
6. Switch to other worker
7. Verify its conversation is also restored

**Expected**: State is recovered after refresh, no data loss.

#### Test 7: Multiple Browser Tabs
1. Open WebUI in two browser tabs
2. Create worker in tab 1
3. Verify worker appears in tab 2
4. Send message in tab 1
5. Verify response appears in both tabs
6. Switch worker in tab 2
7. Verify tab 1 is unaffected

**Expected**: Both tabs receive updates, operate independently.

### 8.4 Definition of Done

This workflow is **DONE** when:

1. ✅ All functional requirements are implemented and tested
2. ✅ All acceptance tests pass
3. ✅ Code is reviewed and approved
4. ✅ Unit tests are written and passing
5. ✅ Integration tests are written and passing
6. ✅ Manual testing is complete
7. ✅ Performance targets are met
8. ✅ Documentation is written (user guide, API docs)
9. ✅ Code is merged to main branch
10. ✅ No critical bugs remain

### 8.5 Out of Scope Validation

The following are explicitly **NOT** required for this workflow to be considered complete:

- ❌ Worker deletion functionality
- ❌ Worker name editing
- ❌ Terminal tab (only Chat tab required)
- ❌ Markdown rendering in responses
- ❌ Code syntax highlighting
- ❌ Tool approval UI (approve/deny buttons)
- ❌ Conversation persistence to disk
- ❌ Working directory usage (collected but not used)
- ❌ Mobile-specific optimizations
- ❌ Keyboard shortcuts
- ❌ Search and filter features
- ❌ Themes and dark mode
- ❌ Multi-user support

These features may be added in future workflows but are not part of mvp-webui-plus.

---

## 9. Summary

### What This Workflow Delivers

**mvp-webui-plus** transforms the basic WebUI into a practical multi-worker chat interface by:

1. **Adding multi-worker support**: Sidebar with worker list, worker selection, real-time state updates
2. **Improving conversation display**: Chat bubbles instead of raw output, full conversation history, response accumulation
3. **Enabling worker creation**: Dialog to create new workers with agent configuration
4. **Enhancing visual design**: Clean two-column layout, color-coded states, professional appearance
5. **Maintaining real-time updates**: All changes reflected immediately via WebSocket events

### Key Technical Changes

**Backend**:
- Add REST API endpoints for worker list and conversation history
- Add WebSocket commands for worker creation and queries
- Add WebUIEvent variants for snapshots and incremental updates
- Add serialization helpers for conversation entries and worker metadata

**Frontend**:
- Complete rewrite with component-based architecture
- State management for workers and conversations
- Response accumulation logic for streaming responses
- Clean visual design inspired by web-q prototype

### Success Metrics

The workflow is successful if:
- Users can manage multiple workers in a single interface
- Conversation history is readable and well-formatted
- Real-time updates work smoothly
- Performance targets are met
- All acceptance tests pass

### Next Steps After This Workflow

After mvp-webui-plus is complete, future enhancements could include:
- Worker deletion and name editing
- Markdown rendering and syntax highlighting
- Tool approval UI
- Conversation persistence
- Advanced features (search, themes, keyboard shortcuts)

---
