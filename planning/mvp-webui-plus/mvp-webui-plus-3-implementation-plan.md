# MVP WebUI Plus - Implementation Plan

## Overview

This implementation plan transforms the basic WebUI (mvp-webui) into a practical multi-worker chat interface. The plan is organized into phases, with each task referencing the relevant design document section.

**Design Document Reference**: `planning/mvp-webui-plus/mvp-webui-plus-2-design.md`

## Implementation Phases

1. **Phase 1: Backend Protocol Changes** - WebSocket route, commands, events
2. **Phase 2: Conversation Serialization** - JSON types and conversion functions
3. **Phase 3: Worker Creation** - AppState extension and command handling
4. **Phase 4: Frontend Architecture** - State management and components
5. **Phase 5: Response Accumulation** - Streaming response handling
6. **Phase 6: Initial State Sync** - Snapshots and reconnection
7. **Phase 7: Visual Design** - HTML structure and CSS styling
8. **Phase 8: Testing & Validation** - Unit, integration, and manual tests

---

## Phase 1: Backend Protocol Changes

**Goal**: Change WebSocket protocol from single-worker to multi-worker design.

**Design Reference**: §3 (WebSocket Protocol Design)

### 1.1 WebSocket Route Changes

- [ ] **Task 1.1.1**: Change WebSocket route from `/ws/worker/:worker_id` to `/ws` (Design Doc §3.2)
  - Modify route definition in `server.rs`
  - Update handler signature to remove `Path(worker_id)` parameter
  - Remove worker_id validation logic from handler

- [ ] **Task 1.1.2**: Remove event filtering by worker_id (Design Doc §3.1)
  - Remove filter logic in event forwarding task
  - Send all events to all connected clients
  - Update event forwarding loop to not check worker_id

### 1.2 WebSocket Command Extensions

- [ ] **Task 1.2.1**: Add `worker_id` parameter to existing commands (Design Doc §3.3)
  - Modify `Prompt` command to include `worker_id: String`
  - Modify `Cancel` command to include `worker_id: String`
  - Update command handlers to use worker_id parameter

- [ ] **Task 1.2.2**: Add `CreateWorker` command (Design Doc §3.3)
  - Add `CreateWorker` variant to `WebSocketCommand` enum
  - Include fields: `name: Option<String>`, `agent: String`, `working_directory: Option<String>`
  - Add serde tags for JSON serialization

- [ ] **Task 1.2.3**: Add `GetWorkers` command (Design Doc §3.3)
  - Add `GetWorkers` variant to `WebSocketCommand` enum
  - No parameters needed (requests all workers)

- [ ] **Task 1.2.4**: Add `GetConversationHistory` command (Design Doc §3.3)
  - Add `GetConversationHistory` variant to `WebSocketCommand` enum
  - Include field: `worker_id: String`

### 1.3 WebSocket Event Extensions

- [ ] **Task 1.3.1**: Add `WorkersSnapshot` event (Design Doc §3.4)
  - Add `WorkersSnapshot` variant to `WebUIEvent` enum
  - Include fields: `workers: Vec<WorkerMetadataJson>`, `timestamp: f64`
  - Add serde tags for JSON serialization

- [ ] **Task 1.3.2**: Add `ConversationSnapshot` event (Design Doc §3.4)
  - Add `ConversationSnapshot` variant to `WebUIEvent` enum
  - Include fields: `worker_id: String`, `entries: Vec<ConversationEntryJson>`, `timestamp: f64`

- [ ] **Task 1.3.3**: Add `Error` event (Design Doc §3.4)
  - Add `Error` variant to `WebUIEvent` enum
  - Include fields: `command: String`, `message: String`, `timestamp: f64`

### 1.4 Command Handler Updates

- [ ] **Task 1.4.1**: Update `handle_command` function signature (Design Doc §3.6)
  - Add `os: &Arc<Os>` parameter for worker creation
  - Add `sender: &mut SplitSink<WebSocket, Message>` parameter for responses
  - Update match arms to handle new commands

- [ ] **Task 1.4.2**: Implement `GetWorkers` command handler (Design Doc §3.6)
  - Call `session.get_workers()` to fetch all workers
  - Convert workers to `WorkerMetadataJson` (will be implemented in Phase 2)
  - Create `WorkersSnapshot` event and send via WebSocket

- [ ] **Task 1.4.3**: Implement `GetConversationHistory` command handler (Design Doc §3.6)
  - Parse and validate worker_id
  - Fetch worker from session
  - Get conversation history from worker's context container
  - Convert to `ConversationEntryJson` (will be implemented in Phase 2)
  - Create `ConversationSnapshot` event and send via WebSocket

- [ ] **Task 1.4.4**: Add error handling for invalid commands (Design Doc §3.7)
  - Validate worker_id exists before processing commands
  - Send `Error` event if validation fails
  - Handle UUID parsing errors gracefully

---

## Phase 2: Conversation Serialization

**Goal**: Create simplified JSON types for conversation history and worker metadata.

**Design Reference**: §4 (Conversation Serialization Design)

### 2.1 Create Serialization Module

- [ ] **Task 2.1.1**: Create `serialization.rs` module (Design Doc §4.6)
  - Create file `crates/chat-cli/src/cli/chat/web_server/serialization.rs`
  - Add module export to `mod.rs`
  - Import necessary types from agent_env and cli/chat

### 2.2 Define JSON Types

- [ ] **Task 2.2.1**: Define `ConversationEntryJson` enum (Design Doc §4.3)
  - Create enum with variants: `UserMessage`, `AssistantMessage`, `ToolUse`
  - Each variant includes: `content: String`, `timestamp: f64`
  - `ToolUse` additionally includes: `tool_uses: Vec<ToolUseJson>`
  - Add serde tags: `#[serde(tag = "type", rename_all = "snake_case")]`

- [ ] **Task 2.2.2**: Define `ToolUseJson` struct (Design Doc §4.3)
  - Create struct with fields: `tool_name: String`, `tool_input: serde_json::Value`
  - Add Serialize and Deserialize derives

- [ ] **Task 2.2.3**: Define `WorkerMetadataJson` struct (Design Doc §4.3)
  - Create struct with fields: `id: String`, `name: String`, `agent: String`, `state: WorkerLifecycleState`, `current_job_id: Option<String>`
  - Add Serialize and Deserialize derives

### 2.3 Implement Conversion Functions

- [ ] **Task 2.3.1**: Implement `convert_conversation_entry` function (Design Doc §4.4)
  - Take `&ConversationEntry` as input, return `ConversationEntryJson`
  - Handle `user: Option<UserMessage>` case
  - Handle `assistant: Option<AssistantMessage>` case with Response and ToolUse variants
  - Handle empty entry case (shouldn't happen, but be defensive)

- [ ] **Task 2.3.2**: Implement `extract_user_content` helper function (Design Doc §4.4)
  - Take `&UserMessage` as input, return `String`
  - Extract text from `UserMessageContent::Prompt`
  - Handle `CancelledToolUses` variant (return prompt or placeholder)
  - Handle `ToolUseResults` variant (return placeholder)

- [ ] **Task 2.3.3**: Implement `convert_tool_use` helper function (Design Doc §4.4)
  - Take `&AssistantToolUse` as input, return `ToolUseJson`
  - Extract tool_name and tool_input fields

- [ ] **Task 2.3.4**: Implement `convert_worker_metadata` function (Design Doc §4.4)
  - Take `&Worker` as input, return `WorkerMetadataJson`
  - Convert UUID to string
  - Extract lifecycle state from mutex
  - Extract current_job_id from mutex and convert to Option<String>

- [ ] **Task 2.3.5**: Implement `current_unix_timestamp` helper function (Design Doc §4.4)
  - Return current time as f64 (Unix timestamp)
  - Use `SystemTime::now().duration_since(UNIX_EPOCH)`

### 2.4 Handle Timestamp Edge Cases

- [ ] **Task 2.4.1**: Use UserMessage timestamp if available (Design Doc §4.5)
  - Check if `UserMessage.timestamp` is Some
  - Convert `DateTime<FixedOffset>` to Unix timestamp
  - Fall back to current time if None

- [ ] **Task 2.4.2**: Document timestamp limitations (Design Doc §4.5)
  - Add comment explaining approximate timestamps for assistant messages
  - Note that timestamps are not preserved across restarts
  - Suggest future improvement: add timestamp to ConversationEntry

### 2.5 Testing Serialization

- [ ] **Task 2.5.1**: Write unit test for user message conversion (Design Doc §4.8)
  - Create test ConversationEntry with UserMessage
  - Call convert_conversation_entry
  - Assert content and type are correct

- [ ] **Task 2.5.2**: Write unit test for assistant response conversion (Design Doc §4.8)
  - Create test ConversationEntry with AssistantMessage::Response
  - Call convert_conversation_entry
  - Assert content and type are correct

- [ ] **Task 2.5.3**: Write unit test for tool use conversion (Design Doc §4.8)
  - Create test ConversationEntry with AssistantMessage::ToolUse
  - Call convert_conversation_entry
  - Assert content, tool_uses, and type are correct

- [ ] **Task 2.5.4**: Write unit test for worker metadata conversion
  - Create test Worker
  - Call convert_worker_metadata
  - Assert all fields are correctly serialized

---

## Phase 3: Worker Creation

**Goal**: Enable worker creation via WebSocket command using WorkerBuilder.

**Design Reference**: §5 (Worker Creation Design)

### 3.1 AppState Extension

- [ ] **Task 3.1.1**: Add `Os` to `AppState` struct (Design Doc §5.2)
  - Modify `AppState` in `server.rs` to include `os: Arc<Os>`
  - Update all AppState construction sites

- [ ] **Task 3.1.2**: Pass `Os` to `WebServer::new()` (Design Doc §5.7)
  - Add `os: Arc<Os>` parameter to WebServer constructor
  - Store Os in AppState

- [ ] **Task 3.1.3**: Wrap `Os` in `Arc` in `ChatArgs::execute()` (Design Doc §5.7)
  - Change `os: Os` to `os: Arc<Os>` in execute method
  - Pass `os.clone()` to WebServer::new()

### 3.2 Worker Name Generation

- [ ] **Task 3.2.1**: Implement `generate_worker_name` function (Design Doc §5.5)
  - Take agent name and session as parameters
  - Count existing workers with same agent prefix
  - Return formatted name: `{agent}-{count+1}`

- [ ] **Task 3.2.2**: Add unit test for name generation (Design Doc §5.8)
  - Test with empty session (should return "agent-1")
  - Test with existing workers (should increment counter)
  - Test with different agent names

### 3.3 Agent Validation

- [ ] **Task 3.3.1**: Implement `validate_agent_name` function (Design Doc §5.4)
  - Check if agent name is not empty
  - Optionally check against known agent names (if registry available)
  - Return Result<(), String> with error message

- [ ] **Task 3.3.2**: Add unit test for agent validation (Design Doc §5.8)
  - Test empty agent name (should fail)
  - Test valid agent name (should pass)

### 3.4 CreateWorker Command Handler

- [ ] **Task 3.4.1**: Implement `handle_create_worker` function (Design Doc §5.3)
  - Take parameters: name, agent, working_directory, session, os
  - Generate worker name if not provided
  - Use WorkerBuilder to create worker with agent configuration
  - Store working_directory in worker's task_metadata (for future use)
  - Return Result with worker reference

- [ ] **Task 3.4.2**: Implement `handle_create_worker_with_error` wrapper (Design Doc §5.4)
  - Validate agent name before creation
  - Call handle_create_worker
  - Send Error event if validation or creation fails
  - Send error via WebSocket sender

- [ ] **Task 3.4.3**: Implement `send_error_event` helper function (Design Doc §5.4)
  - Take command name, error message, and sender
  - Create Error event with timestamp
  - Serialize to JSON and send via WebSocket

- [ ] **Task 3.4.4**: Integrate CreateWorker handler into command dispatcher (Design Doc §5.3)
  - Add match arm for CreateWorker command
  - Call handle_create_worker_with_error
  - Note: WorkerCreated event is automatically published by Session

### 3.5 Working Directory Handling

- [ ] **Task 3.5.1**: Store working_directory in task_metadata (Design Doc §5.6)
  - After worker creation, lock task_metadata mutex
  - Insert "working_directory" key with value
  - Document that this is for future use

### 3.6 Integration Testing

- [ ] **Task 3.6.1**: Write integration test for worker creation (Design Doc §5.8)
  - Create test session and os
  - Call handle_create_worker with test parameters
  - Assert worker is created successfully
  - Assert worker appears in session's worker list

- [ ] **Task 3.6.2**: Write integration test for error handling
  - Test with invalid agent name
  - Assert error is returned
  - Assert no worker is created

---

## Phase 4: Frontend Architecture

**Goal**: Implement component-based frontend with state management.

**Design Reference**: §6 (Frontend Architecture Design)

### 4.1 State Management

- [ ] **Task 4.1.1**: Create `WebUIState` class (Design Doc §6.2)
  - Define class with properties: workers (Map), selectedWorkerId, conversations (Map), activeResponses (Map), connectionState
  - Implement constructor to initialize empty state

- [ ] **Task 4.1.2**: Implement worker management methods (Design Doc §6.2)
  - `addWorker(worker)`: Add worker to workers Map
  - `removeWorker(workerId)`: Remove worker and related data
  - `updateWorkerState(workerId, newState)`: Update worker's state
  - `selectWorker(workerId)`: Set selected worker
  - `getSelectedWorker()`: Return selected worker object

- [ ] **Task 4.1.3**: Implement conversation management methods (Design Doc §6.2)
  - `setConversation(workerId, entries)`: Replace conversation for worker
  - `appendConversationEntry(workerId, entry)`: Add entry to conversation
  - `getConversation(workerId)`: Return conversation entries array

- [ ] **Task 4.1.4**: Create `WorkerData` class (Design Doc §6.2)
  - Define class with properties: id, name, agent, state, currentJobId
  - Implement constructor

### 4.2 WebSocket Client

- [ ] **Task 4.2.1**: Create `WebSocketClient` class (Design Doc §6.3)
  - Define class with properties: app, ws, reconnectAttempts, maxReconnectAttempts, reconnectDelay
  - Implement constructor

- [ ] **Task 4.2.2**: Implement `connect()` method (Design Doc §6.3)
  - Create WebSocket connection to `ws://127.0.0.1:8080/ws`
  - Set up onopen, onmessage, onerror, onclose handlers
  - Update connection state in app.state

- [ ] **Task 4.2.3**: Implement `reconnect()` method (Design Doc §6.3)
  - Check max reconnection attempts
  - Calculate exponential backoff delay
  - Schedule reconnection with setTimeout

- [ ] **Task 4.2.4**: Implement `send(command)` method (Design Doc §6.3)
  - Check WebSocket readyState
  - Serialize command to JSON
  - Send via WebSocket
  - Log error if not connected

### 4.3 WorkerList Component

- [ ] **Task 4.3.1**: Create `WorkerList` class (Design Doc §6.4)
  - Define class with properties: app, element
  - Get element reference from DOM: `document.getElementById('worker-list')`

- [ ] **Task 4.3.2**: Implement `render()` method (Design Doc §6.4)
  - Get workers from app.state
  - Map workers to HTML using renderWorker()
  - Set element.innerHTML
  - Add click event listeners to worker items

- [ ] **Task 4.3.3**: Implement `renderWorker(worker)` method (Design Doc §6.4)
  - Get state icon using getStateIcon()
  - Check if worker is active (selected)
  - Return HTML string with worker-item div
  - Include worker icon, name, and state

- [ ] **Task 4.3.4**: Implement `getStateIcon(state)` helper (Design Doc §6.4)
  - Return '●' for idle
  - Return '⚙' for busy
  - Return '✗' for idle_failed

- [ ] **Task 4.3.5**: Implement `escapeHtml(text)` helper (Design Doc §6.4)
  - Create temporary div element
  - Set textContent to escape HTML
  - Return innerHTML

### 4.4 ConversationView Component

- [ ] **Task 4.4.1**: Create `ConversationView` class (Design Doc §6.4)
  - Define class with properties: app, element
  - Get element reference from DOM: `document.getElementById('conversation')`

- [ ] **Task 4.4.2**: Implement `render()` method (Design Doc §6.4)
  - Check if worker is selected
  - Get conversation entries from app.state
  - Map entries to HTML using renderEntry()
  - Set element.innerHTML
  - Call scrollToBottom()

- [ ] **Task 4.4.3**: Implement `renderEntry(entry)` method (Design Doc §6.4)
  - Get bubble class using getBubbleClass()
  - Format content using formatContent()
  - Escape HTML
  - Return HTML string with bubble div

- [ ] **Task 4.4.4**: Implement `getBubbleClass(type)` helper (Design Doc §6.4)
  - Return 'bubble-user' for user_message
  - Return 'bubble-assistant' for assistant_message
  - Return 'bubble-tool' for tool_use

- [ ] **Task 4.4.5**: Implement `formatContent(entry)` helper (Design Doc §6.4)
  - For tool_use: append tool names and inputs
  - For other types: return content as-is

- [ ] **Task 4.4.6**: Implement `appendEntry(entry)` method (Design Doc §6.4)
  - Render entry to HTML
  - Use insertAdjacentHTML to append
  - Call scrollToBottom()

- [ ] **Task 4.4.7**: Implement `updateLastEntry(content)` method (Design Doc §6.4)
  - Get last bubble element
  - Update textContent with new content

- [ ] **Task 4.4.8**: Implement `scrollToBottom()` helper (Design Doc §6.4)
  - Set element.scrollTop to element.scrollHeight

### 4.5 InputArea Component

- [ ] **Task 4.5.1**: Create `InputArea` class (Design Doc §6.4)
  - Define class with properties: app, input, sendButton, cancelButton
  - Get element references from DOM

- [ ] **Task 4.5.2**: Implement `setupEventListeners()` method (Design Doc §6.4)
  - Add click listener to sendButton
  - Add keydown listener to input (Enter key)
  - Add click listener to cancelButton

- [ ] **Task 4.5.3**: Implement `sendMessage()` method (Design Doc §6.4)
  - Get text from input, trim whitespace
  - Check if text is empty
  - Get selected worker ID
  - Send Prompt command via WebSocket
  - Clear input field

- [ ] **Task 4.5.4**: Implement `cancelJob()` method (Design Doc §6.4)
  - Get selected worker ID
  - Send Cancel command via WebSocket

- [ ] **Task 4.5.5**: Implement `updateState()` method (Design Doc §6.4)
  - Get selected worker
  - Check if worker is busy
  - Disable/enable input and send button
  - Show/hide cancel button

### 4.6 NewWorkerDialog Component

- [ ] **Task 4.6.1**: Create `NewWorkerDialog` class (Design Doc §6.4)
  - Define class with properties: app, dialog, form, nameInput, agentInput, workingDirInput, createButton, cancelButton
  - Get element references from DOM

- [ ] **Task 4.6.2**: Implement `setupEventListeners()` method (Design Doc §6.4)
  - Add click listener to createButton
  - Add click listener to cancelButton
  - Add submit listener to form (prevent default)
  - Add click listener to dialog (close on outside click)

- [ ] **Task 4.6.3**: Implement `show()` method (Design Doc §6.4)
  - Set dialog display to 'flex'
  - Focus agentInput

- [ ] **Task 4.6.4**: Implement `close()` method (Design Doc §6.4)
  - Set dialog display to 'none'
  - Reset form

- [ ] **Task 4.6.5**: Implement `createWorker()` method (Design Doc §6.4)
  - Get agent name from input, trim whitespace
  - Validate agent name is not empty
  - Send CreateWorker command via WebSocket
  - Close dialog

### 4.7 Main Application Class

- [ ] **Task 4.7.1**: Create `WebUIApp` class (Design Doc §6.5)
  - Define class with properties: state, ws, components, accumulator
  - Initialize all components in constructor

- [ ] **Task 4.7.2**: Implement `init()` method (Design Doc §6.5)
  - Call setupGlobalEventListeners()
  - Call ws.connect()

- [ ] **Task 4.7.3**: Implement `setupGlobalEventListeners()` method (Design Doc §6.5)
  - Add click listener to new-worker-button

- [ ] **Task 4.7.4**: Implement `onConnected()` callback (Design Doc §6.5)
  - Log connection message
  - Wait for initial snapshots from server

- [ ] **Task 4.7.5**: Implement `selectWorker(workerId)` method (Design Doc §6.5)
  - Update state.selectedWorkerId
  - Request conversation history if not loaded
  - Re-render workerList, conversationView, inputArea

- [ ] **Task 4.7.6**: Implement `handleEvent(event)` dispatcher (Design Doc §6.5)
  - Switch on event.type
  - Route to appropriate handler method
  - Log unhandled event types

- [ ] **Task 4.7.7**: Implement event handler methods (Design Doc §6.5)
  - `handleWorkersSnapshot(event)`: Add workers to state, auto-select first
  - `handleConversationSnapshot(event)`: Set conversation in state, render if selected
  - `handleWorkerCreated(event)`: Add worker to state, auto-select
  - `handleWorkerStateChanged(event)`: Update worker state, re-render
  - `handleJobStarted(event)`: Update worker's currentJobId
  - `handleJobCompleted(event)`: Clear currentJobId, finalize response
  - `handleError(event)`: Show alert with error message

- [ ] **Task 4.7.8**: Add DOMContentLoaded initialization (Design Doc §6.5)
  - Create global app variable
  - Initialize WebUIApp on page load

---

## Phase 5: Response Accumulation

**Goal**: Accumulate streaming OutputChunk events into single response bubbles.

**Design Reference**: §7 (Response Accumulation Design)

### 5.1 ResponseAccumulator Class

- [ ] **Task 5.1.1**: Create `ResponseAccumulator` class (Design Doc §7.2)
  - Define class with properties: app, activeResponses (Map)
  - Implement constructor

- [ ] **Task 5.1.2**: Implement `handleChunk(event)` method (Design Doc §7.2)
  - Extract worker_id and chunk from event
  - Route to appropriate handler based on chunk_type
  - Call handleAssistantChunk for assistant_response
  - Call handleToolUseChunk for tool_use
  - Call handleToolResultChunk for tool_result

### 5.2 Assistant Response Accumulation

- [ ] **Task 5.2.1**: Implement `handleAssistantChunk(workerId, text)` method (Design Doc §7.2)
  - Check if this is first chunk for worker
  - If first: create new entry, add to activeResponses, append bubble to UI
  - If not first: append text to existing entry, update bubble in UI
  - Only update UI if worker is currently selected

### 5.3 Tool Use Handling

- [ ] **Task 5.3.1**: Implement `handleToolUseChunk(workerId, chunk)` method (Design Doc §7.2)
  - Create tool_use entry with tool name and input
  - Append to conversation view if worker is selected
  - Add to conversation history in state

- [ ] **Task 5.3.2**: Implement `handleToolResultChunk(workerId, chunk)` method (Design Doc §7.2)
  - Create tool_result entry with tool name and result
  - Append to conversation view if worker is selected
  - Add to conversation history in state

### 5.4 Response Finalization

- [ ] **Task 5.4.1**: Implement `finalize(workerId)` method (Design Doc §7.2)
  - Get active response state for worker
  - Add entry to conversation history in app.state
  - Clear activeResponses for this worker
  - Called when JobCompleted event received

### 5.5 Multi-Worker Support

- [ ] **Task 5.5.1**: Test concurrent response accumulation (Design Doc §7.4)
  - Verify multiple workers can stream simultaneously
  - Verify responses are tracked independently by worker_id
  - Verify switching workers mid-stream works correctly

### 5.6 Edge Case Handling

- [ ] **Task 5.6.1**: Handle job cancellation mid-stream (Design Doc §7.5)
  - In handleJobCompleted, check if result.status is 'cancelled'
  - Append '[Cancelled]' to response content
  - Update bubble in UI
  - Finalize response

- [ ] **Task 5.6.2**: Handle WebSocket reconnection (Design Doc §7.5)
  - In onConnected callback, clear all activeResponses
  - Request fresh conversation snapshot
  - Document that partial responses are lost on reconnection

- [ ] **Task 5.6.3**: Handle worker switching mid-stream (Design Doc §7.5)
  - No special handling needed
  - Accumulator continues tracking in background
  - UI shows selected worker's conversation

### 5.7 Performance Optimization (Optional)

- [ ] **Task 5.7.1**: Consider throttling UI updates (Design Doc §7.6)
  - If performance issues observed, implement throttling
  - Update UI at most every 50ms
  - Force final update on finalize
  - Document trade-off: smoother updates vs slightly delayed text

---

## Phase 6: Initial State Synchronization

**Goal**: Send initial snapshots on WebSocket connection and handle reconnection.

**Design Reference**: §8 (Initial State Synchronization Design)

### 6.1 Server-Side Snapshot Sending

- [ ] **Task 6.1.1**: Implement `send_initial_snapshots` function (Design Doc §8.2)
  - Take sender and state as parameters
  - Call send_workers_snapshot
  - Find main worker and call send_conversation_snapshot
  - Return Result

- [ ] **Task 6.1.2**: Implement `send_workers_snapshot` function (Design Doc §4.7)
  - Get all workers from session
  - Convert to WorkerMetadataJson using convert_worker_metadata
  - Create WorkersSnapshot event
  - Serialize to JSON and send via WebSocket

- [ ] **Task 6.1.3**: Implement `send_conversation_snapshot` function (Design Doc §4.7)
  - Parse worker_id and get worker from session
  - Get conversation history from worker's context container
  - Convert entries using convert_conversation_entry
  - Create ConversationSnapshot event
  - Serialize to JSON and send via WebSocket

- [ ] **Task 6.1.4**: Integrate snapshot sending into WebSocket handler (Design Doc §8.2)
  - Subscribe to events BEFORE sending snapshots (prevent race condition)
  - Call send_initial_snapshots after subscription
  - Handle errors gracefully

### 6.2 Client-Side Snapshot Handling

- [ ] **Task 6.2.1**: Implement WorkersSnapshot handler (Design Doc §8.3)
  - Add workers to state
  - Auto-select first worker (or main worker if exists)
  - Render worker list

- [ ] **Task 6.2.2**: Implement ConversationSnapshot handler (Design Doc §8.3)
  - Set conversation in state for worker_id
  - Render conversation view if worker is selected

### 6.3 Reconnection Logic

- [ ] **Task 6.3.1**: Implement reconnection with exponential backoff (Design Doc §8.4)
  - Track reconnection attempts
  - Calculate delay: reconnectDelay * 2^(attempts-1)
  - Max 5 attempts, then show error

- [ ] **Task 6.3.2**: Handle state recovery on reconnection (Design Doc §8.4)
  - Clear activeResponses (partial responses lost)
  - Wait for fresh WorkersSnapshot
  - Wait for fresh ConversationSnapshot
  - Document limitation: partial responses not recovered

### 6.4 Race Condition Prevention

- [ ] **Task 6.4.1**: Ensure event subscription before snapshots (Design Doc §8.5)
  - In WebSocket handler, subscribe to events first
  - Then send snapshots
  - Events received after subscription are queued
  - Prevents missing events during snapshot sending

---

## Phase 7: Visual Design

**Goal**: Implement clean two-column layout with chat bubbles and sidebar.

**Design Reference**: §6.6 (HTML Structure) and Scope §4 (Target User Experience)

### 7.1 HTML Structure

- [ ] **Task 7.1.1**: Create two-column layout (Design Doc §6.6)
  - Container div with flexbox
  - Sidebar (aside) with fixed width
  - Main content area (main) with flex: 1

- [ ] **Task 7.1.2**: Create sidebar structure (Design Doc §6.6)
  - Sidebar header with "Workers" title
  - "+ New" button
  - Worker list container (div#worker-list)

- [ ] **Task 7.1.3**: Create main content structure (Design Doc §6.6)
  - Content header with worker title and connection status
  - Conversation container (div#conversation)
  - Input area with textarea, send button, cancel button

- [ ] **Task 7.1.4**: Create new worker dialog (Design Doc §6.6)
  - Modal dialog overlay
  - Dialog content with form
  - Form fields: worker name, agent name, working directory
  - Create and Cancel buttons

### 7.2 CSS Styling

- [ ] **Task 7.2.1**: Style container and layout
  - Full viewport height
  - Flexbox row layout
  - No gaps or margins

- [ ] **Task 7.2.2**: Style sidebar
  - Fixed width: 250px
  - Light gray background: #f5f5f5
  - Scrollable worker list
  - Padding and spacing

- [ ] **Task 7.2.3**: Style worker items
  - Flexbox layout with icon, name, state
  - Hover effect
  - Active state with blue background
  - State icons with appropriate colors

- [ ] **Task 7.2.4**: Style main content area
  - White background
  - Header bar with border
  - Scrollable conversation area
  - Fixed input area at bottom

- [ ] **Task 7.2.5**: Style chat bubbles
  - User messages: right-aligned, blue background (#e3f2fd), blue border
  - Assistant messages: left-aligned, green background (#f1f8e9), green border
  - Tool use: left-aligned, orange background (#fff3e0), orange border
  - Max width 80%, border radius 12px, padding 12px 16px

- [ ] **Task 7.2.6**: Style input area
  - Flexbox layout
  - Textarea with flex: 1
  - Buttons with appropriate colors
  - Padding and spacing

- [ ] **Task 7.2.7**: Style new worker dialog
  - Modal overlay with semi-transparent background
  - Centered dialog content
  - Form layout with labels and inputs
  - Button styling

- [ ] **Task 7.2.8**: Style connection status indicator
  - Green dot for connected
  - Red dot for disconnected
  - Orange dot for connecting

### 7.3 Responsive Design

- [ ] **Task 7.3.1**: Add responsive breakpoints (optional for MVP)
  - Adjust bubble max-width on small screens
  - Maintain usability on different screen sizes
  - Document that mobile optimization is out of scope

---

## Phase 8: Testing & Validation

**Goal**: Comprehensive testing of all functionality.

**Design Reference**: §10 (Testing Strategy)

### 8.1 Backend Unit Tests

- [ ] **Task 8.1.1**: Test conversation entry serialization (Design Doc §10.1)
  - Test user message conversion
  - Test assistant response conversion
  - Test tool use conversion
  - Test empty entry handling

- [ ] **Task 8.1.2**: Test worker metadata serialization (Design Doc §10.1)
  - Test UUID to string conversion
  - Test state extraction
  - Test current_job_id handling

- [ ] **Task 8.1.3**: Test command parsing (Design Doc §10.1)
  - Test CreateWorker command deserialization
  - Test GetWorkers command deserialization
  - Test GetConversationHistory command deserialization
  - Test invalid JSON handling

- [ ] **Task 8.1.4**: Test worker name generation (Design Doc §10.1)
  - Test with empty session
  - Test with existing workers
  - Test with different agent names

- [ ] **Task 8.1.5**: Test agent validation (Design Doc §10.1)
  - Test empty agent name
  - Test valid agent name

### 8.2 Backend Integration Tests

- [ ] **Task 8.2.1**: Test WebSocket connection lifecycle (Design Doc §10.2)
  - Test connection establishment
  - Test initial snapshots sent
  - Test event forwarding
  - Test disconnection handling

- [ ] **Task 8.2.2**: Test CreateWorker command end-to-end (Design Doc §10.2)
  - Send CreateWorker command via WebSocket
  - Verify worker is created in session
  - Verify WorkerCreated event is received
  - Verify worker appears in workers list

- [ ] **Task 8.2.3**: Test GetWorkers command (Design Doc §10.2)
  - Send GetWorkers command
  - Verify WorkersSnapshot event is received
  - Verify all workers are included

- [ ] **Task 8.2.4**: Test GetConversationHistory command (Design Doc §10.2)
  - Create worker with conversation history
  - Send GetConversationHistory command
  - Verify ConversationSnapshot event is received
  - Verify all entries are included

- [ ] **Task 8.2.5**: Test error handling (Design Doc §10.2)
  - Send CreateWorker with invalid agent
  - Verify Error event is received
  - Verify error message is descriptive

### 8.3 Frontend Unit Tests (Optional)

- [ ] **Task 8.3.1**: Test WebUIState class (Design Doc §10.3)
  - Test addWorker method
  - Test selectWorker method
  - Test updateWorkerState method
  - Test conversation management methods

- [ ] **Task 8.3.2**: Test ResponseAccumulator class (Design Doc §10.3)
  - Test chunk accumulation
  - Test multiple workers independently
  - Test finalization

### 8.4 Manual Testing Scenarios

- [ ] **Task 8.4.1**: Test Scenario 1 - Basic Workflow (Design Doc §10.4)
  - Start `q chat --web-ui`
  - Open browser to http://127.0.0.1:8080
  - Verify main worker appears in sidebar
  - Send message "Hello"
  - Verify response streams as single bubble
  - Verify worker state changes Idle → Busy → Idle

- [ ] **Task 8.4.2**: Test Scenario 2 - Multi-Worker (Design Doc §10.4)
  - Create new worker with agent "rust-agent"
  - Verify worker appears in sidebar
  - Send message to new worker
  - Switch back to main worker
  - Verify conversation is preserved
  - Send message to main worker
  - Verify both workers operate independently

- [ ] **Task 8.4.3**: Test Scenario 3 - Concurrent Workers (Design Doc §10.4)
  - Create 3 workers
  - Send long-running task to each
  - Verify all show "Busy" state
  - Switch between workers while responses stream
  - Verify responses appear in correct conversations

- [ ] **Task 8.4.4**: Test Scenario 4 - Error Handling (Design Doc §10.4)
  - Create worker with invalid agent name
  - Verify error message appears
  - Disconnect network
  - Verify "Disconnected" indicator
  - Reconnect network
  - Verify automatic reconnection

- [ ] **Task 8.4.5**: Test Scenario 5 - Browser Refresh (Design Doc §10.4)
  - Create 2 workers, send messages
  - Refresh browser
  - Verify worker list restored
  - Verify conversation history restored

### 8.5 Browser Compatibility Testing

- [ ] **Task 8.5.1**: Test in Chrome (Design Doc §10.6)
  - Verify all features work
  - Check console for errors
  - Test WebSocket connection
  - Test UI rendering

- [ ] **Task 8.5.2**: Test in Firefox (Design Doc §10.6)
  - Verify all features work
  - Check console for errors
  - Test WebSocket connection
  - Test UI rendering

- [ ] **Task 8.5.3**: Test in Safari (Design Doc §10.6)
  - Verify all features work
  - Check console for errors
  - Test WebSocket connection
  - Test UI rendering

### 8.6 Performance Testing

- [ ] **Task 8.6.1**: Test with 10 workers (Design Doc §10.5)
  - Create 10 workers
  - Send messages to each
  - Measure memory usage
  - Verify UI remains responsive

- [ ] **Task 8.6.2**: Test with large conversation history (Design Doc §10.5)
  - Create worker with 100+ messages
  - Measure conversation load time
  - Verify scrolling performance
  - Check for memory leaks

- [ ] **Task 8.6.3**: Test rapid message sending (Design Doc §10.5)
  - Send 10 messages in quick succession
  - Verify all responses are handled correctly
  - Check for UI lag or freezing

### 8.7 Accessibility Testing

- [ ] **Task 8.7.1**: Test keyboard navigation
  - Tab through interactive elements
  - Verify focus indicators
  - Test Enter key in input field
  - Test Escape key in dialog

- [ ] **Task 8.7.2**: Test screen reader compatibility (optional)
  - Verify ARIA labels are present
  - Test with screen reader if available
  - Document any issues

---

## Summary

This implementation plan provides a comprehensive roadmap for transforming the basic WebUI into a practical multi-worker chat interface. The plan is organized into 8 phases with 150+ atomic tasks.

### Key Deliverables

1. **Multi-Worker WebSocket Protocol**: Global `/ws` endpoint with WorkersSnapshot and ConversationSnapshot events
2. **Conversation Serialization**: Simplified JSON types for easy frontend rendering
3. **Worker Creation**: CreateWorker command with agent configuration and validation
4. **Component-Based Frontend**: State management, WebSocket client, and reusable UI components
5. **Response Accumulation**: Real-time streaming response handling with multi-worker support
6. **Initial State Sync**: Automatic snapshots on connection with reconnection handling
7. **Clean Visual Design**: Two-column layout with chat bubbles and sidebar
8. **Comprehensive Testing**: Unit tests, integration tests, and manual testing scenarios

### Implementation Order

The phases are designed to be implemented sequentially:
1. Backend protocol changes enable multi-worker communication
2. Serialization provides data format for frontend
3. Worker creation enables user-driven worker management
4. Frontend architecture provides structure for UI
5. Response accumulation enables real-time streaming
6. State sync ensures consistency on connection
7. Visual design provides polished user experience
8. Testing validates all functionality

### Success Criteria

Implementation is complete when:
- ✅ Users can see all workers in sidebar and select any worker
- ✅ Users can create new workers with agent configuration
- ✅ Conversation history displays as readable chat bubbles
- ✅ Streaming responses accumulate in real-time
- ✅ Worker state updates are immediate and clear
- ✅ All manual testing scenarios pass
- ✅ Works in Chrome, Firefox, and Safari

### Estimated Effort

- **Backend**: ~40 tasks (Protocol, Serialization, Worker Creation, State Sync)
- **Frontend**: ~80 tasks (Architecture, Components, Accumulation, Visual Design)
- **Testing**: ~30 tasks (Unit, Integration, Manual, Browser Compatibility)
- **Total**: ~150 tasks

### Next Steps

1. Review this plan with team
2. Create implementation log document
3. Begin Phase 1: Backend Protocol Changes
4. Track progress in implementation log
5. Update primary plan sheet when complete

---

**Document Version**: 1.0  
**Created**: 2025-10-16  
**Status**: Ready for Implementation
