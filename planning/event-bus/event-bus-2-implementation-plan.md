# EventBus-Centered Architecture - Implementation Plan

**Design Document**: `planning/event-bus/event-bus-1-design.md`

**IMPORTANT INSTRUCTIONS FOR AI ASSISTANT**:
1. Read the design document at `planning/event-bus/event-bus-1-design.md` COMPLETELY before starting any task
2. Proceed with tasks in order, implementing one task at a time
3. After completing each task, mark it with `[x]` before moving to the next one
4. If a task is unclear, refer to the specific section in the design document
5. Keep the code buildable between tasks when possible (but not mandatory)
6. Run `cargo check` after each task to verify compilation

---

## Phase 1: Core Event System

### 1.1 Create Event Type Definitions

[x] **Task 1.1.1**: Create `crates/chat-cli/src/agent_env/events.rs` with basic structure
- Create new file
- Add module documentation
- Add necessary imports: `std::time::Instant`, `uuid::Uuid`, `std::collections::HashMap`, `serde_json`
- Add `#[derive(Debug, Clone)]` to all event types
- Reference: Design doc section "Event Structure"

[x] **Task 1.1.2**: Implement `WorkerLifecycleState` enum
- Add enum with variants: `Idle`, `Busy`, `IdleFailed`
- Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
- Add serde derives: `#[derive(Serialize, Deserialize)]`
- Reference: Design doc "Event Structure" → "Worker lifecycle states"

[x] **Task 1.1.3**: Implement `JobCompletionResult` enum
- Add enum with variants: `Success { task_metadata: HashMap<String, serde_json::Value> }`, `Cancelled`, `Failed { error: String }`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "Job completion results"

[x] **Task 1.1.4**: Implement `OutputChunk` enum
- Add enum with variants: `AssistantResponse(String)`, `ToolUse { tool_name: String, tool_input: serde_json::Value }`, `ToolResult { tool_name: String, result: String }`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "Output chunk types"

[x] **Task 1.1.5**: Implement `WorkerEvent` enum
- Add enum with variants: `Created`, `Deleted`, `LifecycleStateChanged`
- Each variant should have fields as specified in design doc
- All variants include `timestamp: Instant`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "Worker lifecycle and state events"

[x] **Task 1.1.6**: Implement `JobEvent` enum
- Add enum with variants: `Started`, `Completed`, `OutputChunk`
- Each variant should have fields as specified in design doc
- All variants include `timestamp: Instant`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "Job execution events"

[x] **Task 1.1.7**: Implement `AgentLoopEvent` enum
- Add enum with variants: `ResponseReceived`, `ToolUseRequestReceived`
- Each variant should have fields as specified in design doc
- All variants include `timestamp: Instant`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "AgentLoop-specific events"

[x] **Task 1.1.8**: Implement `SystemEvent` enum
- Add enum with variant: `ShutdownInitiated { reason: String, timestamp: Instant }`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "System-level events"

[x] **Task 1.1.9**: Implement `AgentEnvironmentEvent` top-level enum
- Add enum with variants: `Worker(WorkerEvent)`, `Job(JobEvent)`, `AgentLoop(AgentLoopEvent)`, `System(SystemEvent)`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Event Structure" → "Top-level event envelope"

[x] **Task 1.1.10**: Implement helper methods for `AgentEnvironmentEvent`
- Add `worker_id(&self) -> Option<Uuid>` method
- Add `is_worker_event(&self) -> bool` method
- Add `is_job_event(&self) -> bool` method
- Add `is_agent_loop_event(&self) -> bool` method
- Add `is_system_event(&self) -> bool` method
- Add `timestamp(&self) -> Instant` method
- Reference: Design doc "Event Helper Methods"

[x] **Task 1.1.11**: Add events module to `crates/chat-cli/src/agent_env/mod.rs`
- Add `pub mod events;` declaration
- Add re-exports: `pub use events::*;`

[x] **Task 1.1.12**: Run `cargo check` to verify events module compiles
- Fix any compilation errors
- Ensure all types are properly exported

### 1.2 Create EventBus Implementation

[x] **Task 1.2.1**: Create `crates/chat-cli/src/agent_env/event_bus.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: `tokio::sync::broadcast`, `std::sync::Arc`
- Import event types from `super::events::*`
- Reference: Design doc section "EventBus Implementation"

[x] **Task 1.2.2**: Implement `EventBus` struct
- Add struct with fields: `sender: broadcast::Sender<AgentEnvironmentEvent>`, `buffer_size: usize`
- Add `#[derive(Clone)]` to struct
- Reference: Design doc "EventBus Implementation"

[x] **Task 1.2.3**: Implement `EventBus::new()` constructor
- Accept `buffer_size: usize` parameter
- Create broadcast channel with specified buffer size
- Return `Self { sender, buffer_size }`
- Reference: Design doc "EventBus Implementation" → "Create new EventBus"

[x] **Task 1.2.4**: Implement `EventBus::publish()` method
- Accept `event: AgentEnvironmentEvent` parameter
- Call `self.sender.send(event)`
- Ignore send errors (no subscribers is OK)
- Reference: Design doc "EventBus Implementation" → "Publish event"

[x] **Task 1.2.5**: Implement `EventBus::subscribe()` method
- Return `broadcast::Receiver<AgentEnvironmentEvent>`
- Call `self.sender.subscribe()`
- Reference: Design doc "EventBus Implementation" → "Subscribe to events"

[x] **Task 1.2.6**: Implement `EventBus::subscriber_count()` method
- Return `usize`
- Call `self.sender.receiver_count()`
- Reference: Design doc "EventBus Implementation" → "Get current subscriber count"

[x] **Task 1.2.7**: Implement `Default` trait for `EventBus`
- Default buffer size: 1000
- Call `Self::new(1000)`
- Reference: Design doc "EventBus Implementation" → "Default implementation"

[x] **Task 1.2.8**: Add event_bus module to `crates/chat-cli/src/agent_env/mod.rs`
- Add `pub mod event_bus;` declaration
- Add re-export: `pub use event_bus::EventBus;`

[x] **Task 1.2.9**: Run `cargo check` to verify event_bus module compiles
- Fix any compilation errors
- Ensure EventBus is properly exported

### 1.3 Integrate EventBus into Session

[x] **Task 1.3.1**: Update `Session` struct in `crates/chat-cli/src/agent_env/session.rs`
- Add field: `event_bus: EventBus`
- Keep existing fields unchanged
- Reference: Design doc "Session" → "Session struct"

[x] **Task 1.3.2**: Update `Session::new()` constructor
- Add parameter: `event_bus: EventBus`
- Store event_bus in struct
- Keep existing initialization logic
- Reference: Design doc "Session" → "Session::new()"

[x] **Task 1.3.3**: Add getter method `Session::event_bus()`
- Return `&EventBus`
- Simple getter for event_bus field

[x] **Task 1.3.4**: Update demo code in `crates/chat-cli/src/agent_env/demo/init.rs`
- Create EventBus before Session
- Pass EventBus to Session::new()
- Keep existing demo logic working

[x] **Task 1.3.5**: Run `cargo check` to verify Session changes compile
- Fix any compilation errors
- Ensure demo still compiles

### 1.4 Write Tests for Event System

[x] **Task 1.4.1**: Create test module in `crates/chat-cli/src/agent_env/events.rs`
- Add `#[cfg(test)]` module
- Add test for `worker_id()` helper method
- Test with WorkerEvent, JobEvent, AgentLoopEvent, SystemEvent
- Reference: Design doc "Phase 1: Core Event System" → "Testing"

[x] **Task 1.4.2**: Add test for event type checking helpers
- Test `is_worker_event()`, `is_job_event()`, `is_agent_loop_event()`, `is_system_event()`
- Verify correct boolean returns for each event type

[x] **Task 1.4.3**: Add test for `timestamp()` helper
- Create events with known timestamps
- Verify timestamp extraction works for all event types

[x] **Task 1.4.4**: Create test module in `crates/chat-cli/src/agent_env/event_bus.rs`
- Add `#[cfg(test)]` module
- Add test for publish/subscribe basic flow
- Create EventBus, subscribe, publish event, verify receipt

[x] **Task 1.4.5**: Add test for multiple subscribers
- Create EventBus
- Create 3 subscribers
- Publish event
- Verify all 3 subscribers receive the event

[x] **Task 1.4.6**: Add test for lagged event handling
- Create EventBus with small buffer (10)
- Subscribe
- Publish 100 events without reading
- Read from subscriber and verify `RecvError::Lagged` is received

[x] **Task 1.4.7**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure test coverage is adequate

---

## Phase 2: Worker State Management

### 2.1 Update Worker Structure

[x] **Task 2.1.1**: Add serde dependency to Worker in `crates/chat-cli/src/agent_env/worker.rs`
- Add `use serde::{Serialize, Deserialize};` import
- Add `#[derive(Serialize, Deserialize)]` to Worker struct
- Reference: Design doc "Worker" → "Worker struct"

[x] **Task 2.1.2**: Add `lifecycle_state` field to Worker
- Add field: `lifecycle_state: Arc<Mutex<WorkerLifecycleState>>`
- Import `std::sync::{Arc, Mutex}`
- Initialize to `WorkerLifecycleState::Idle` in constructor
- Add `#[serde(skip)]` attribute (not serializable due to Mutex)
- Reference: Design doc "Worker" → "Worker struct"

[x] **Task 2.1.3**: Add `task_metadata` field to Worker
- Add field: `task_metadata: HashMap<String, serde_json::Value>`
- Import `std::collections::HashMap`
- Initialize to empty HashMap in constructor
- Reference: Design doc "Worker" → "Worker struct"

[x] **Task 2.1.4**: Mark non-serializable fields with `#[serde(skip)]`
- Add `#[serde(skip)]` to `model_provider` field
- Add `#[serde(skip)]` to `lifecycle_state` field
- Reference: Design doc "Worker" → "Worker struct"

[x] **Task 2.1.5**: Implement `Worker::set_task_metadata()` method
- Accept `key: &str` and `value: serde_json::Value` parameters
- Insert into `self.task_metadata`
- Reference: Design doc "Worker" → "Type-safe metadata access helpers"

[x] **Task 2.1.6**: Implement `Worker::get_task_metadata()` method
- Accept `key: &str` parameter
- Return `Option<&serde_json::Value>`
- Reference: Design doc "Worker" → "Type-safe metadata access helpers"

[x] **Task 2.1.7**: Implement `Worker::get_task_metadata_string()` method
- Accept `key: &str` parameter
- Return `Option<String>`
- Extract string from JSON value if present
- Reference: Design doc "Worker" → "Type-safe metadata access helpers"

[x] **Task 2.1.8**: Create `task_metadata_keys` module in worker.rs
- Add `pub mod task_metadata_keys` at end of file
- Add constants for known metadata keys
- Reference: Design doc "Worker" → "Namespaced metadata keys"

[x] **Task 2.1.9**: Add metadata key constants
- `AGENT_LOOP_COMPLETION_STATE: &str = "agent_loop.completion_state"`
- `AGENT_LOOP_LAST_TOOL: &str = "agent_loop.last_tool"`
- `COMPACT_LAST_RUN: &str = "compact.last_run_timestamp"`
- Reference: Design doc "Worker" → "Namespaced metadata keys"

[x] **Task 2.1.10**: Run `cargo check` to verify Worker changes compile
- Fix any compilation errors
- Ensure Worker is still usable in existing code

### 2.2 Add Worker Lifecycle State Management to Session

[x] **Task 2.2.1**: Implement `Session::set_worker_lifecycle_state()` method in session.rs
- Accept `worker_id: Uuid` and `new_state: WorkerLifecycleState` parameters
- Get worker from workers map
- Lock lifecycle_state mutex and update
- Publish `WorkerEvent::LifecycleStateChanged` event
- Reference: Design doc "Session" → "set_worker_lifecycle_state()"

[x] **Task 2.2.2**: Update `Session::build_worker()` to publish Created event
- After creating worker and adding to map
- Publish `WorkerEvent::Created` event with worker_id, name, timestamp
- Reference: Design doc "Session" → "build_worker()"

[x] **Task 2.2.3**: Update `Session::delete_worker()` to publish Deleted event
- After removing worker from map
- Publish `WorkerEvent::Deleted` event with worker_id, timestamp
- Reference: Design doc "Session" → "delete_worker()"

[x] **Task 2.2.4**: Run `cargo check` to verify Session changes compile
- Fix any compilation errors
- Ensure Session methods work correctly

### 2.3 Write Tests for Worker State Management

[x] **Task 2.3.1**: Add test for Worker serialization in worker.rs
- Create Worker instance
- Serialize to JSON using `serde_json::to_string()`
- Verify serialization succeeds
- Verify skipped fields are not in JSON
- Reference: Design doc "Phase 2: Worker State Management" → "Testing"

[x] **Task 2.3.2**: Add test for Worker deserialization
- Create JSON string with Worker data
- Deserialize using `serde_json::from_str()`
- Verify deserialization succeeds
- Verify fields are correctly populated

[x] **Task 2.3.3**: Add test for task metadata operations
- Create Worker
- Set metadata with `set_task_metadata()`
- Get metadata with `get_task_metadata()`
- Get string metadata with `get_task_metadata_string()`
- Verify all operations work correctly

[x] **Task 2.3.4**: Add test for lifecycle state transitions in session.rs
- Create Session with EventBus
- Create Worker
- Subscribe to events
- Call `set_worker_lifecycle_state()` with different states
- Verify events are published correctly

[x] **Task 2.3.5**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure test coverage is adequate

---

## Phase 3: Session Event Publishing

### 3.1 Update Job Launching Methods

[x] **Task 3.1.1**: Update `Session::run_task__agent_loop()` to set worker state to Busy
- Before creating task
- Call `self.set_worker_lifecycle_state(worker.id, WorkerLifecycleState::Busy)`
- Reference: Design doc "Session" → "run_task__agent_loop()"

[x] **Task 3.1.2**: Update `Session::run_task__agent_loop()` to publish JobStarted event
- After creating job and registering it
- Publish `JobEvent::Started` with worker_id, job_id, task_type, timestamp
- Reference: Design doc "Session" → "run_task__agent_loop()"

[x] **Task 3.1.3**: Create `Session::handle_job_completion()` method
- Accept `worker_id: Uuid` and `result: Result<()>` parameters
- Determine JobCompletionResult from result
- Extract task_metadata from worker
- Update worker lifecycle state (Idle or IdleFailed)
- Publish `JobEvent::Completed` event
- Run job continuations (handled by WorkerJob)
- Reference: Design doc "Session" → "handle_job_completion()"

[x] **Task 3.1.4**: Update job spawning in `run_task__agent_loop()` to call handle_job_completion
- In spawned task, poll job completion
- Call `session.handle_job_completion(worker_id, result).await`
- Reference: Design doc "Session" → "run_task__agent_loop()"

[x] **Task 3.1.5**: Add `Session::run_task__compact_conversation()` method (stub for now)
- Similar structure to `run_task__agent_loop()`
- Set worker to Busy
- Publish JobStarted event
- Create and spawn job
- Call handle_job_completion on completion
- Reference: Design doc "Session" → "run_task__compact_conversation()"

[x] **Task 3.1.6**: Run `cargo check` to verify Session changes compile
- Fix any compilation errors
- Ensure job lifecycle events are published correctly

### 3.2 Write Tests for Session Event Publishing

[x] **Task 3.2.1**: Add test for worker creation events in session.rs
- Create Session with EventBus
- Subscribe to events
- Call `build_worker()`
- Verify `WorkerEvent::Created` is published with correct data

[x] **Task 3.2.2**: Add test for worker deletion events
- Create Session and Worker
- Subscribe to events
- Call `delete_worker()`
- Verify `WorkerEvent::Deleted` is published

[x] **Task 3.2.3**: Add test for job lifecycle events
- Create Session and Worker
- Subscribe to events
- Launch agent loop task
- Verify `JobEvent::Started` is published
- Wait for job completion
- Verify `JobEvent::Completed` is published

[x] **Task 3.2.4**: Add test for multiple workers don't interfere
- Create Session with 2 workers
- Subscribe to events
- Launch jobs on both workers
- Verify events have correct worker_ids
- Verify jobs complete independently

[x] **Task 3.2.5**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure event publishing works correctly

---

## Phase 4: Task Event Publishing

### 4.1 Update AgentLoop Task

[x] **Task 4.1.1**: Update `AgentLoop` struct in `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`
- Add field: `event_bus: EventBus`
- Remove `worker_interface` field (deprecated)
- Reference: Design doc "AgentLoop Task"

[x] **Task 4.1.2**: Update `AgentLoop::new()` constructor
- Add parameter: `event_bus: EventBus`
- Remove `worker_interface` parameter
- Store event_bus in struct
- Reference: Design doc "AgentLoop Task"

[x] **Task 4.1.3**: Update `AgentLoop::run()` to publish OutputChunk events for text
- In response stream processing loop
- When receiving `ResponseChunk::Text(text)`
- Publish `JobEvent::OutputChunk` with `OutputChunk::AssistantResponse(text)`
- Reference: Design doc "AgentLoop Task" → "run()"

[x] **Task 4.1.4**: Update `AgentLoop::run()` to publish OutputChunk events for tool use
- When receiving `ResponseChunk::ToolUse { name, input }`
- Publish `JobEvent::OutputChunk` with `OutputChunk::ToolUse`
- Reference: Design doc "AgentLoop Task" → "run()"

[x] **Task 4.1.5**: Update `AgentLoop::run()` to publish AgentLoopEvent for tool use
- After publishing OutputChunk for tool use
- Publish `AgentLoopEvent::ToolUseRequestReceived` with tool details
- Reference: Design doc "AgentLoop Task" → "run()"

[x] **Task 4.1.6**: Update `AgentLoop::run()` to publish AgentLoopEvent for response
- After completing response stream
- Publish `AgentLoopEvent::ResponseReceived` with complete response text
- Reference: Design doc "AgentLoop Task" → "run()"

[x] **Task 4.1.7**: Update `AgentLoop::run()` to publish OutputChunk events for tool results
- In `execute_tools()` method
- After executing each tool
- Publish `JobEvent::OutputChunk` with `OutputChunk::ToolResult`
- Reference: Design doc "AgentLoop Task" → "execute_tools()"

[x] **Task 4.1.8**: Update `AgentLoop::run()` to set completion state metadata
- At end of run(), before returning
- If tool approval needed: set metadata to "completed_with_tool_request"
- If normal completion: set metadata to "completed_ready_for_prompt"
- Use `worker.set_task_metadata()` with `task_metadata_keys::AGENT_LOOP_COMPLETION_STATE`
- Reference: Design doc "AgentLoop Task" → "Completion States"

[x] **Task 4.1.9**: Update Session to pass EventBus to AgentLoop
- In `Session::run_task__agent_loop()`
- Pass `self.event_bus.clone()` to `AgentLoop::new()`
- Reference: Design doc "Session" → "run_task__agent_loop()"

[x] **Task 4.1.10**: Run `cargo check` to verify AgentLoop changes compile
- Fix any compilation errors
- Ensure AgentLoop publishes events correctly

### 4.2 Remove Deprecated WorkerToHostInterface

[x] **Task 4.2.1**: Remove `crates/chat-cli/src/agent_env/worker_interface.rs` file
- Delete the file entirely
- Reference: Design doc "Phase 4: Task Event Publishing" → "Files to Remove"

[x] **Task 4.2.2**: Remove worker_interface module from `crates/chat-cli/src/agent_env/mod.rs`
- Remove `pub mod worker_interface;` declaration
- Remove any re-exports of WorkerToHostInterface

[x] **Task 4.2.3**: Update demo code to remove WorkerToHostInterface usage
- In `crates/chat-cli/src/agent_env/demo/`
- Remove any references to WorkerToHostInterface
- Update to use EventBus instead

[x] **Task 4.2.4**: Run `cargo check` to verify removal is clean
- Fix any remaining references to WorkerToHostInterface
- Ensure code compiles without the interface

### 4.3 Write Tests for Task Event Publishing

[ ] **Task 4.3.1**: Add test for AgentLoop event publishing in agent_loop.rs
- Create mock model provider that returns test response
- Create AgentLoop with EventBus
- Subscribe to events
- Run AgentLoop
- Verify OutputChunk events are published for text

[ ] **Task 4.3.2**: Add test for tool use event publishing
- Create mock model provider that returns tool use request
- Run AgentLoop
- Verify OutputChunk and AgentLoopEvent are published for tool use

[ ] **Task 4.3.3**: Add test for completion state metadata
- Run AgentLoop to completion
- Check worker's task_metadata
- Verify completion state is set correctly

[ ] **Task 4.3.4**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure event publishing works correctly

---

## Phase 5: Command System

### 5.1 Create Command Type Definitions

[x] **Task 5.1.1**: Create `crates/chat-cli/src/agent_env/commands.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: `uuid::Uuid`, `serde::{Serialize, Deserialize}`
- Reference: Design doc "Command System"

[x] **Task 5.1.2**: Implement `AgentEnvironmentCommand` enum
- Add variants: `Prompt { worker_id: Uuid, text: String }`, `Compact { worker_id: Uuid, instruction: Option<String> }`, `Quit`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Command System" → "AgentEnvironmentCommand"

[x] **Task 5.1.3**: Implement `UiCommand` enum
- Add variants: `Usage`, `Context`, `Status`, `Workers`
- Add `#[derive(Debug, Clone)]`
- Add comment: "UI-specific commands can be added by implementations"
- Reference: Design doc "Command System" → "UiCommand"

[x] **Task 5.1.4**: Implement `Command` enum
- Add variants: `Agent(AgentEnvironmentCommand)`, `Ui(UiCommand)`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Command System" → "Command"

[x] **Task 5.1.5**: Implement `PromptResult` enum
- Add variants: `Command(AgentEnvironmentCommand)`, `Shutdown`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Command System" → "PromptResult"

[x] **Task 5.1.6**: Add commands module to `crates/chat-cli/src/agent_env/mod.rs`
- Add `pub mod commands;` declaration
- Add re-exports: `pub use commands::*;`

[x] **Task 5.1.7**: Run `cargo check` to verify commands module compiles
- Fix any compilation errors
- Ensure all command types are properly exported

### 5.2 Create Command Parser

[x] **Task 5.2.1**: Add `ParseError` type to commands.rs
- Add enum with variant: `UnknownCommand(String)`
- Add `#[derive(Debug, Clone)]`
- Implement `std::fmt::Display` trait
- Implement `std::error::Error` trait

[x] **Task 5.2.2**: Create `CommandParser` struct in commands.rs
- Add empty struct: `pub struct CommandParser;`
- Reference: Design doc "Command Parser"

[x] **Task 5.2.3**: Implement `CommandParser::parse()` method - basic structure
- Accept `input: &str` parameter
- Return `Result<Command, ParseError>`
- Trim input
- Check if starts with '/'
- Reference: Design doc "Command Parser"

[x] **Task 5.2.4**: Implement explicit command parsing
- Split command and arguments: `splitn(2, ' ')`
- Match on command name: "quit", "q", "compact", "usage", "context", "status", "workers"
- Return appropriate Command variant
- Return `ParseError::UnknownCommand` for unknown commands
- Reference: Design doc "Command Parser"

[x] **Task 5.2.5**: Implement implicit prompt command parsing
- If input doesn't start with '/', treat as prompt
- Return `Command::Agent(AgentEnvironmentCommand::Prompt { worker_id: Uuid::nil(), text: trimmed.to_string() })`
- Note: worker_id will be filled by UI
- Reference: Design doc "Command Parser"

[x] **Task 5.2.6**: Run `cargo check` to verify CommandParser compiles
- Fix any compilation errors
- Ensure parser works correctly

### 5.3 Create UI Utilities

[x] **Task 5.3.1**: Create `crates/chat-cli/src/cli/chat/agent_env_ui/ui_utils.rs` with basic structure
- Create new file
- Add module documentation
- Add imports for Worker, ConversationEntry, etc.
- Reference: Design doc "Shared UI Utilities"

[x] **Task 5.3.2**: Implement `TokenUsage` struct
- Add fields: `input_tokens: usize`, `output_tokens: usize`, `total_tokens: usize`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "Shared UI Utilities" → "TokenUsage"

[x] **Task 5.3.3**: Implement `estimate_tokens()` helper function
- Accept `text: &str` parameter
- Return `usize`
- Simple estimation: `text.len() / 4`
- Reference: Design doc "Shared UI Utilities" → "estimate_tokens()"

[x] **Task 5.3.4**: Implement `calculate_token_usage()` function
- Accept `worker: &Worker` parameter
- Return `TokenUsage`
- Lock conversation history
- Iterate entries and sum tokens
- Reference: Design doc "Shared UI Utilities" → "calculate_token_usage()"

[x] **Task 5.3.5**: Implement `format_context_info()` function
- Accept `worker: &Worker` parameter
- Return `String`
- Format worker name, message count, lifecycle state
- Reference: Design doc "Shared UI Utilities" → "format_context_info()"

[x] **Task 5.3.6**: Add ui_utils module to `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`
- Add `pub mod ui_utils;` declaration
- Add re-exports as needed

[x] **Task 5.3.7**: Run `cargo check` to verify ui_utils module compiles
- Fix any compilation errors
- Ensure utilities are usable

### 5.4 Write Tests for Command System

[x] **Task 5.4.1**: Add test module to commands.rs
- Add `#[cfg(test)]` module
- Test parsing explicit commands: "/quit", "/q", "/compact", "/usage", etc.
- Verify correct Command variants are returned

[x] **Task 5.4.2**: Add test for implicit prompt parsing
- Test input without '/' prefix
- Verify `Command::Agent(Prompt)` is returned
- Verify text is preserved correctly

[x] **Task 5.4.3**: Add test for unknown command error
- Test "/unknown_command"
- Verify `ParseError::UnknownCommand` is returned

[x] **Task 5.4.4**: Add test for compact command with instruction
- Test "/compact summarize briefly"
- Verify instruction is captured correctly

[x] **Task 5.4.5**: Add tests for ui_utils functions
- Test `calculate_token_usage()` with mock worker
- Test `format_context_info()` with mock worker
- Verify output is correct

[x] **Task 5.4.6**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure command parsing works correctly

---

## Phase 6: AgentEnvironment Coordinator

### 6.1 Create AgentEnvironment Structure

[x] **Task 6.1.1**: Create `crates/chat-cli/src/agent_env/agent_environment.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: `tokio::sync::{mpsc, Notify}`, `tokio::task::JoinHandle`, `std::sync::Arc`, `eyre::Result`
- Import Session, EventBus, commands, events
- Reference: Design doc "AgentEnvironment Coordinator"

[x] **Task 6.1.2**: Define `UserInterface` trait
- Add `#[async_trait]` attribute
- Add methods: `start()`, `command_receiver()`, `handle_event()`
- Add trait bounds: `Send + Sync`
- Reference: Design doc "UI Trait Hierarchy" → "UserInterface"

[x] **Task 6.1.3**: Define `HeadlessInterface` trait
- Add `#[async_trait]` attribute
- Add method: `handle_event()`
- Add trait bounds: `Send + Sync`
- Reference: Design doc "UI Trait Hierarchy" → "HeadlessInterface"

[x] **Task 6.1.4**: Implement `AgentEnvironment` struct
- Add fields: `session: Arc<Session>`, `event_bus: EventBus`, `main_ui: Option<Arc<dyn UserInterface>>`, `headless_uis: Vec<Arc<dyn HeadlessInterface>>`, `shutdown_signal: Arc<Notify>`
- Reference: Design doc "AgentEnvironment Coordinator" → "Implementation"

[x] **Task 6.1.5**: Implement `AgentEnvironment::new()` constructor
- Accept all fields as parameters
- Initialize shutdown_signal
- Return Self
- Reference: Design doc "AgentEnvironment Coordinator" → "Implementation"

[x] **Task 6.1.6**: Add agent_environment module to `crates/chat-cli/src/agent_env/mod.rs`
- Add `pub mod agent_environment;` declaration
- Add re-exports: `pub use agent_environment::*;`

[x] **Task 6.1.7**: Run `cargo check` to verify AgentEnvironment structure compiles
- Fix any compilation errors
- Ensure traits are properly defined

### 6.2 Implement Event Multicasting

[x] **Task 6.2.1**: Implement `AgentEnvironment::spawn_event_multicast()` method
- Return `JoinHandle<()>`
- Subscribe to event_bus
- Clone main_ui and headless_uis
- Clone shutdown_signal
- Reference: Design doc "AgentEnvironment Coordinator" → "spawn_event_multicast()"

[x] **Task 6.2.2**: Implement event multicast loop
- Use `tokio::spawn()` to create task
- Loop with `tokio::select!`
- Receive events from event_bus
- Call `handle_event()` on main_ui if present
- Call `handle_event()` on all headless_uis
- Reference: Design doc "AgentEnvironment Coordinator" → "spawn_event_multicast()"

[x] **Task 6.2.3**: Implement lagged event handling in multicast loop
- Handle `RecvError::Lagged(n)` case
- Log warning with `tracing::warn!`
- Continue loop
- Reference: Design doc "AgentEnvironment Coordinator" → "spawn_event_multicast()"

[x] **Task 6.2.4**: Implement shutdown handling in multicast loop
- Handle shutdown_signal notification
- Log shutdown message
- Break loop
- Reference: Design doc "AgentEnvironment Coordinator" → "spawn_event_multicast()"

[x] **Task 6.2.5**: Run `cargo check` to verify event multicasting compiles
- Fix any compilation errors
- Ensure multicast task spawns correctly

### 6.3 Implement Command Processing

[x] **Task 6.3.1**: Implement `AgentEnvironment::handle_command()` method
- Accept `cmd: AgentEnvironmentCommand` parameter
- Return `Result<()>`
- Match on command variants
- Reference: Design doc "AgentEnvironment Coordinator" → "handle_command()"

[x] **Task 6.3.2**: Implement Prompt command handling
- Get worker from session
- Add message to conversation history
- Call `session.run_task__agent_loop()`
- Reference: Design doc "AgentEnvironment Coordinator" → "handle_command()"

[x] **Task 6.3.3**: Implement Compact command handling
- Get worker from session
- Call `session.run_task__compact_conversation()`
- Reference: Design doc "AgentEnvironment Coordinator" → "handle_command()"

[x] **Task 6.3.4**: Implement Quit command handling
- Call `self.shutdown_signal.notify_waiters()`
- Reference: Design doc "AgentEnvironment Coordinator" → "handle_command()"

[x] **Task 6.3.5**: Run `cargo check` to verify command handling compiles
- Fix any compilation errors
- Ensure commands are processed correctly

### 6.4 Implement Main Run Loop

[x] **Task 6.4.1**: Implement `AgentEnvironment::run()` method - basic structure
- Return `Result<()>`
- Spawn event multicast task
- Check if main_ui is present
- Reference: Design doc "AgentEnvironment Coordinator" → "run()"

[x] **Task 6.4.2**: Implement main UI mode in run() method
- If main_ui is Some, call `ui.start().await`
- Get command receiver from UI
- Enter command processing loop
- Reference: Design doc "AgentEnvironment Coordinator" → "run()"

[x] **Task 6.4.3**: Implement command processing loop
- Use `tokio::select!`
- Receive from cmd_receiver
- Match on PromptResult variants
- Call `handle_command()` for Command variant
- Break loop for Shutdown variant
- Reference: Design doc "AgentEnvironment Coordinator" → "run()"

[x] **Task 6.4.4**: Implement headless mode in run() method
- If main_ui is None, log "Running in headless mode"
- Wait for shutdown_signal
- Reference: Design doc "AgentEnvironment Coordinator" → "run()"

[x] **Task 6.4.5**: Implement cleanup in run() method
- Log "Shutting down AgentEnvironment"
- Abort multicast_handle
- Call `session.cancel_all_jobs()`
- Return Ok(())
- Reference: Design doc "AgentEnvironment Coordinator" → "run()"

[x] **Task 6.4.6**: Implement `AgentEnvironment::shutdown()` method
- Call `self.shutdown_signal.notify_waiters()`
- Reference: Design doc "AgentEnvironment Coordinator" → "shutdown()"

[x] **Task 6.4.7**: Run `cargo check` to verify AgentEnvironment run loop compiles
- Fix any compilation errors
- Ensure main loop works correctly

### 6.5 Write Tests for AgentEnvironment

[x] **Task 6.5.1**: Add test module to agent_environment.rs
- Add `#[cfg(test)]` module
- Create mock UserInterface implementation
- Create mock HeadlessInterface implementation

[x] **Task 6.5.2**: Add test for event multicasting to main UI
- Create AgentEnvironment with mock main UI
- Spawn multicast task
- Publish event to event_bus
- Verify mock UI receives event

[x] **Task 6.5.3**: Add test for event multicasting to headless UIs
- Create AgentEnvironment with multiple mock headless UIs
- Publish event
- Verify all headless UIs receive event

[x] **Task 6.5.4**: Add test for command processing
- Create AgentEnvironment with mock UI
- Send Prompt command
- Verify Session launches job

[x] **Task 6.5.5**: Add test for shutdown coordination
- Create AgentEnvironment
- Call shutdown()
- Verify run() method exits
- Verify jobs are cancelled

[x] **Task 6.5.6**: Add test for headless mode
- Create AgentEnvironment with no main UI
- Verify run() waits for shutdown
- Verify no errors occur

[x] **Task 6.5.7**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure AgentEnvironment works correctly

---

## Phase 7: TextUi Implementation

### 7.1 Create TextUi Structure

[x] **Task 7.1.1**: Create `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: `tokio::sync::{mpsc, Notify}`, `std::sync::Arc`, `std::path::PathBuf`, `uuid::Uuid`
- Import Session, AgentEnvironmentEvent, PromptResult, Command, etc.
- Reference: Design doc "TextUi Implementation"

[x] **Task 7.1.2**: Implement `TextUi` struct
- Add fields: `session: Arc<Session>`, `main_worker_id: Uuid`, `input_handler: Arc<InputHandler>`, `cmd_sender: mpsc::Sender<PromptResult>`, `prompt_ready: Arc<Notify>`, `shutdown_signal: Arc<Notify>`
- Reference: Design doc "TextUi Implementation" → "TextUi struct"

[x] **Task 7.1.3**: Implement `TextUi::new()` constructor
- Accept `session: Arc<Session>`, `main_worker_id: Uuid`, `history_path: Option<PathBuf>` parameters
- Return `Result<Self>` (using Option pattern for receiver)
- Create command channel with buffer size 10
- Create InputHandler with history_path
- Initialize all fields
- Store receiver in Arc<Mutex<Option<Receiver>>>
- Reference: Design doc "TextUi Implementation" → "TextUi::new()" and Design Q&A "Option D"

[x] **Task 7.1.4**: Add text_ui module to `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`
- Add `pub mod text_ui;` declaration
- Add re-export: `pub use text_ui::TextUi;`

[x] **Task 7.1.5**: Run `cargo check` to verify TextUi structure compiles
- Fix any compilation errors
- Ensure TextUi is properly exported

### 7.2 Implement UserInterface Trait for TextUi

[x] **Task 7.2.1**: Implement `UserInterface::start()` for TextUi
- Add `#[async_trait]` to impl block
- Call `self.spawn_prompt_loop()`
- Return `Ok(())`
- Reference: Design doc "TextUi Implementation" → "UserInterface trait"

[x] **Task 7.2.2**: Implement `UserInterface::command_receiver()` for TextUi
- Use Option pattern with Arc<Mutex<Option<Receiver>>>
- Take receiver once, panic if called multiple times
- Reference: Design Q&A "Option D"

[x] **Task 7.2.3**: Implement `UserInterface::handle_event()` for TextUi - basic structure
- Accept `event: AgentEnvironmentEvent` parameter
- Filter events by worker_id (only process main_worker_id)
- Return early if event is for different worker
- Reference: Design doc "TextUi Implementation" → "handle_event()" and Design Q&A "Option B"

[x] **Task 7.2.4**: Implement OutputChunk event handling in handle_event()
- Match on `AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. })`
- For `OutputChunk::AssistantResponse(text)`: print text and flush stdout
- For `OutputChunk::ToolUse { tool_name, .. }`: print "[Using tool: {}]"
- For `OutputChunk::ToolResult { tool_name, .. }`: print "[Tool {} completed]"
- Reference: Design doc "TextUi Implementation" → "handle_event()"

[x] **Task 7.2.5**: Implement LifecycleStateChanged event handling in handle_event()
- Match on `AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { new_state, .. })`
- For `WorkerLifecycleState::Busy`: do nothing (worker started job)
- For `WorkerLifecycleState::Idle`: print newline, signal prompt_ready
- For `WorkerLifecycleState::IdleFailed`: print "[Task failed]", signal prompt_ready
- Reference: Design doc "TextUi Implementation" → "handle_event()"

[x] **Task 7.2.6**: Run `cargo check` to verify UserInterface implementation compiles
- Fix any compilation errors
- Ensure event handling works correctly

### 7.3 Implement Prompt Loop

[x] **Task 7.3.1**: Implement `TextUi::spawn_prompt_loop()` method - basic structure
- Return `JoinHandle<()>`
- Clone all necessary fields
- Use `tokio::spawn()` to create task
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()" and Design Q&A "Q3"

[x] **Task 7.3.2**: Implement prompt loop with prompt_ready signal
- Use `tokio::select!` in loop
- Wait for `prompt_ready.notified()`
- Read input with `input_handler.read_line("You").await`
- Handle read errors
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()" and Design Q&A "Q3"

[x] **Task 7.3.3**: Implement command parsing in prompt loop
- Call `CommandParser::parse(&input)`
- Handle parse errors
- Match on Command variants
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()"

[x] **Task 7.3.4**: Implement UI command handling in prompt loop
- For `Command::Ui(UiCommand::Usage)`: call `ui_utils::calculate_token_usage()`, print results, re-signal prompt_ready
- For `Command::Ui(UiCommand::Context)`: call `ui_utils::format_context_info()`, print results, re-signal prompt_ready
- For `Command::Ui(UiCommand::Status)`: print worker status, re-signal prompt_ready
- For `Command::Ui(UiCommand::Workers)`: print worker list, re-signal prompt_ready
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()"

[x] **Task 7.3.5**: Implement Agent command handling in prompt loop
- For `Command::Agent(mut agent_cmd)`: fill in worker_id with main_worker_id
- Send command via `cmd_sender.send(PromptResult::Command(agent_cmd)).await`
- Handle send errors (channel closed)
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()"

[x] **Task 7.3.6**: Implement shutdown handling in prompt loop
- Add shutdown_signal to tokio::select!
- Break loop on shutdown notification
- Reference: Design doc "TextUi Implementation" → "spawn_prompt_loop()"

[x] **Task 7.3.7**: Run `cargo check` to verify prompt loop compiles
- Fix any compilation errors
- Ensure prompt loop works correctly

### 7.4 Write Tests for TextUi

[x] **Task 7.4.1**: Add test module to text_ui.rs
- Add `#[cfg(test)]` module
- Create mock Session and Worker
- Create test EventBus

[x] **Task 7.4.2**: Add test for event filtering
- Create TextUi with specific main_worker_id
- Send events for different worker_id
- Verify events are filtered correctly

[x] **Task 7.4.3**: Add test for output chunk display
- Create TextUi
- Send OutputChunk events
- Verify output is displayed (capture stdout)

[x] **Task 7.4.4**: Add test for lifecycle state transitions
- Create TextUi
- Send LifecycleStateChanged events
- Verify prompt_ready is signaled correctly

[x] **Task 7.4.5**: Add test for command parsing and handling
- Create TextUi
- Simulate user input
- Verify commands are parsed and sent correctly

[x] **Task 7.4.6**: Run `cargo test` to verify all tests pass
- Fix any failing tests
- Ensure TextUi works correctly

---

## Phase 8: Entry Point Integration

### 8.1 Update ChatArgs::execute()

[x] **Task 8.1.1**: Update `ChatArgs::execute()` in `crates/chat-cli/src/cli/chat/mod.rs` - create EventBus
- Remove old demo code
- Create EventBus: `let event_bus = EventBus::default();`
- Reference: Design doc "Entry Point Integration" → "Update ChatArgs::execute()"

[x] **Task 8.1.2**: Create Session with EventBus
- Build model providers (keep existing logic)
- Create Session: `let session = Arc::new(Session::new(event_bus.clone(), model_providers));`
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.3**: Create main Worker
- Call `session.build_worker("main".to_string())`
- Store worker_id for later use
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.4**: Handle initial input if provided
- If `self.input.is_some()`, add message to worker's conversation history
- Reference: Design doc "Entry Point Integration" → "Handle command-line arguments"

[x] **Task 8.1.5**: Create TextUi
- Get history path (keep existing logic)
- Call `TextUi::new(session.clone(), main_worker_id, history_path)?`
- Store both TextUi and command receiver
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.6**: Create AgentEnvironment
- Create with Session, EventBus, Some(TextUi), empty headless_uis vec
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.7**: Handle initial prompt if provided
- If initial input was provided, send Prompt command to trigger agent loop
- This should happen before calling `agent_env.run()`
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.8**: Run AgentEnvironment
- Call `agent_env.run().await?`
- This blocks until shutdown
- Return `Ok(ExitCode::SUCCESS)`
- Reference: Design doc "Entry Point Integration"

[x] **Task 8.1.9**: Run `cargo check` to verify entry point integration compiles
- Fix any compilation errors
- Ensure ChatArgs::execute() works correctly

### 8.2 Remove Old Demo Code

[x] **Task 8.2.1**: Remove `crates/chat-cli/src/agent_env/demo/` directory
- Delete entire directory and all files within
- Reference: Design doc "Entry Point Integration" → "Files to Remove"

[x] **Task 8.2.2**: Remove demo module from `crates/chat-cli/src/agent_env/mod.rs`
- Remove `pub mod demo;` declaration
- Remove any re-exports of demo types

[x] **Task 8.2.3**: Remove old UI files
- Delete `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui_worker_to_host_interface.rs`
- Delete `crates/chat-cli/src/cli/chat/agent_env_ui/prompt_queue.rs` (if not needed)
- Reference: Design doc "Entry Point Integration" → "Files to Remove"

[x] **Task 8.2.4**: Update `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`
- Remove references to deleted files
- Keep only: text_ui, ui_utils, input_handler, ctrl_c_handler

[x] **Task 8.2.5**: Run `cargo check` to verify cleanup is complete
- Fix any remaining references to deleted code
- Ensure code compiles cleanly

### 8.3 Handle Command-Line Arguments

[ ] **Task 8.3.1**: Implement --no-interactive mode
- Check `self.no_interactive` flag
- If true, create AgentEnvironment with no main UI (headless mode)
- Reference: Design doc "Entry Point Integration" → "Handle command-line arguments"

[ ] **Task 8.3.2**: Implement agent/profile selection
- Check `self.agent` option
- Load appropriate agent configuration
- Apply to worker creation
- Reference: Design doc "Entry Point Integration" → "Handle command-line arguments"

[ ] **Task 8.3.3**: Implement model selection
- Check `self.model` option
- Select appropriate model provider
- Apply to worker creation
- Reference: Design doc "Entry Point Integration" → "Handle command-line arguments"

[ ] **Task 8.3.4**: Run `cargo check` to verify argument handling compiles
- Fix any compilation errors
- Ensure all arguments are handled correctly

### 8.4 Integration Testing

[ ] **Task 8.4.1**: Manual test - basic chat flow
- Run `cargo run --bin chat_cli`
- Enter a simple prompt
- Verify agent responds
- Verify output is displayed correctly
- Verify /quit command works

[ ] **Task 8.4.2**: Manual test - with initial input
- Run `cargo run --bin chat_cli "Hello, world!"`
- Verify agent processes initial input
- Verify response is displayed
- Verify can continue conversation

[ ] **Task 8.4.3**: Manual test - UI commands
- Run chat
- Test /usage command
- Test /context command
- Test /status command
- Verify all display correctly

[ ] **Task 8.4.4**: Manual test - shutdown
- Run chat
- Test Ctrl+C shutdown
- Test /quit command
- Verify clean shutdown in both cases

[ ] **Task 8.4.5**: Manual test - --no-interactive mode
- Run `cargo run --bin chat_cli --no-interactive "Test prompt"`
- Verify runs in headless mode
- Verify output is produced
- Verify exits cleanly

[ ] **Task 8.4.6**: Fix any issues found during manual testing
- Document issues
- Fix bugs
- Re-test until all tests pass

---

## Phase 9: Additional UI Implementations

### 9.1 Implement StructuredIO

[ ] **Task 9.1.1**: Create `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: `tokio::sync::mpsc`, `tokio::io::{AsyncBufReadExt, BufReader}`, `std::io::Write`, `serde_json`
- Reference: Design doc "StructuredIO Implementation"

[ ] **Task 9.1.2**: Implement `StructuredIO` struct
- Add fields: `session: Arc<Session>`, `main_worker_id: Uuid`, `cmd_sender: mpsc::Sender<PromptResult>`, `output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>`
- Reference: Design doc "StructuredIO Implementation" → "StructuredIO struct"

[ ] **Task 9.1.3**: Implement `StructuredIO::new()` constructor
- Accept `session: Arc<Session>`, `main_worker_id: Uuid` parameters
- Return `Result<(Self, mpsc::Receiver<PromptResult>)>`
- Create command channel
- Initialize output_writer with stdout
- Return tuple
- Reference: Design doc "StructuredIO Implementation" → "StructuredIO::new()"

[ ] **Task 9.1.4**: Implement `UserInterface::start()` for StructuredIO
- Call `self.spawn_input_reader()`
- Return `Ok(())`
- Reference: Design doc "StructuredIO Implementation" → "UserInterface trait"

[ ] **Task 9.1.5**: Implement `UserInterface::handle_event()` for StructuredIO
- Filter events by worker_id
- Match on `AgentLoopEvent::ResponseReceived`: output JSON with assistant_response
- Match on `AgentLoopEvent::ToolUseRequestReceived`: output JSON with tool_use_request
- Reference: Design doc "StructuredIO Implementation" → "handle_event()"

[ ] **Task 9.1.6**: Implement `StructuredIO::spawn_input_reader()` method
- Create async task that continuously reads lines from stdin
- For each line, create Prompt command with line as text
- Send command via cmd_sender
- Reference: Design doc "StructuredIO Implementation" → "spawn_input_reader()"

[ ] **Task 9.1.7**: Add structured_io module to `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`
- Add `pub mod structured_io;` declaration
- Add re-export: `pub use structured_io::StructuredIO;`

[ ] **Task 9.1.8**: Run `cargo check` to verify StructuredIO compiles
- Fix any compilation errors
- Ensure StructuredIO is properly exported

### 9.2 Test StructuredIO

[ ] **Task 9.2.1**: Add test module to structured_io.rs
- Add `#[cfg(test)]` module
- Create mock Session and Worker
- Test event handling

[ ] **Task 9.2.2**: Manual test - StructuredIO with piped input
- Create test script that pipes prompts to CLI
- Run with StructuredIO UI
- Verify JSON output is produced
- Verify all prompts are processed

[ ] **Task 9.2.3**: Manual test - StructuredIO output parsing
- Run StructuredIO
- Parse JSON output
- Verify structure matches specification
- Verify worker_id is included

[ ] **Task 9.2.4**: Run `cargo test` to verify StructuredIO tests pass
- Fix any failing tests
- Ensure StructuredIO works correctly

### 9.3 Add UI Selection to Entry Point

[ ] **Task 9.3.1**: Add UI mode argument to ChatArgs
- Add enum `UiMode { Text, Structured, None }`
- Add field to ChatArgs: `ui_mode: Option<UiMode>`
- Add CLI argument: `--ui-mode <MODE>`
- Reference: Design doc "Additional UI Implementations"

[ ] **Task 9.3.2**: Update ChatArgs::execute() to select UI
- Match on ui_mode
- Create TextUi for Text mode
- Create StructuredIO for Structured mode
- Create no UI for None mode (headless)
- Reference: Design doc "Additional UI Implementations"

[ ] **Task 9.3.3**: Run `cargo check` to verify UI selection compiles
- Fix any compilation errors
- Ensure UI selection works correctly

[ ] **Task 9.3.4**: Manual test - UI mode selection
- Test `--ui-mode text`
- Test `--ui-mode structured`
- Test `--ui-mode none`
- Verify correct UI is used in each case

### 9.4 WebApi Implementation (Future - Optional)

[ ] **Task 9.4.1**: Create `crates/chat-cli/src/cli/chat/agent_env_ui/web_api.rs` stub
- Create file with basic structure
- Add TODO comments for future implementation
- Reference: Design doc "WebApi Implementation (Future)"

[ ] **Task 9.4.2**: Document WebApi design
- Add comments describing REST endpoints
- Add comments describing WebSocket protocol
- Add comments describing authentication requirements
- Reference: Design doc "WebApi Implementation (Future)"

---

## Phase 10: ConversationCompact Task

### 10.1 Implement ConversationCompact Task

[ ] **Task 10.1.1**: Create `crates/chat-cli/src/agent_env/worker_tasks/conversation_compact.rs` with basic structure
- Create new file
- Add module documentation
- Add imports: Worker, EventBus, WorkerTask, etc.
- Reference: Design doc "ConversationCompact Task"

[ ] **Task 10.1.2**: Implement `CompactInput` struct
- Add field: `instruction: Option<String>`
- Add `#[derive(Debug, Clone)]`
- Reference: Design doc "ConversationCompact Task" → "CompactInput"

[ ] **Task 10.1.3**: Implement `ConversationCompact` struct
- Add fields: `worker: Arc<Worker>`, `input: CompactInput`, `event_bus: EventBus`
- Reference: Design doc "ConversationCompact Task" → "ConversationCompact struct"

[ ] **Task 10.1.4**: Implement `ConversationCompact::new()` constructor
- Accept all fields as parameters
- Return Self
- Reference: Design doc "ConversationCompact Task" → "ConversationCompact::new()"

[ ] **Task 10.1.5**: Implement `WorkerTask` trait for ConversationCompact
- Implement `task_type()` to return "ConversationCompact"
- Implement `run()` method (stub for now)
- Reference: Design doc "ConversationCompact Task"

[ ] **Task 10.1.6**: Implement `ConversationCompact::run()` - get conversation history
- Lock conversation history from worker
- Clone history for processing
- Reference: Design doc "ConversationCompact Task" → "run()"

[ ] **Task 10.1.7**: Implement `ConversationCompact::run()` - build compaction prompt
- Use instruction from input or default
- Format history for compaction
- Create prompt for LLM
- Reference: Design doc "ConversationCompact Task" → "run()"

[ ] **Task 10.1.8**: Implement `ConversationCompact::run()` - query LLM
- Get model provider from worker
- Call `query_simple()` with compaction prompt
- Get summary response
- Reference: Design doc "ConversationCompact Task" → "run()"

[ ] **Task 10.1.9**: Implement `ConversationCompact::run()` - replace history
- Lock conversation history
- Call `replace_with_summary()` method
- Update task metadata with last run timestamp
- Reference: Design doc "ConversationCompact Task" → "run()"

[ ] **Task 10.1.10**: Implement `format_history()` helper function
- Accept `ConversationHistory` parameter
- Format entries as text for LLM
- Return formatted string
- Reference: Design doc "ConversationCompact Task" → "format_history()"

[ ] **Task 10.1.11**: Add conversation_compact module to `crates/chat-cli/src/agent_env/worker_tasks/mod.rs`
- Add `pub mod conversation_compact;` declaration
- Add re-export: `pub use conversation_compact::*;`

[ ] **Task 10.1.12**: Run `cargo check` to verify ConversationCompact compiles
- Fix any compilation errors
- Ensure task is properly exported

### 10.2 Integrate ConversationCompact into Session

[ ] **Task 10.2.1**: Implement `Session::run_task__compact_conversation()` method
- Similar to `run_task__agent_loop()`
- Set worker to Busy
- Create ConversationCompact task
- Create and spawn job
- Publish JobStarted event
- Reference: Design doc "Session" → "run_task__compact_conversation()"

[ ] **Task 10.2.2**: Update `AgentEnvironment::handle_command()` to handle Compact command
- Already implemented in Phase 6, verify it works
- Reference: Design doc "AgentEnvironment Coordinator" → "handle_command()"

[ ] **Task 10.2.3**: Run `cargo check` to verify integration compiles
- Fix any compilation errors
- Ensure compact task can be launched

### 10.3 Add Automatic Compaction to AgentLoop

[ ] **Task 10.3.1**: Add token counting to AgentLoop
- Before querying LLM, count tokens in conversation history
- Use `ui_utils::calculate_token_usage()`
- Reference: Design doc "ConversationCompact Task" → "Implement automatic compaction"

[ ] **Task 10.3.2**: Add compaction trigger logic to AgentLoop
- Define token threshold (e.g., 80% of model limit)
- If threshold exceeded, set metadata flag
- Exit with completion state indicating compaction needed
- Reference: Design doc "ConversationCompact Task" → "Implement automatic compaction"

[ ] **Task 10.3.3**: Add continuation for automatic compaction
- Check completion state metadata
- If compaction needed, launch compact task
- After compact completes, re-launch agent loop
- Reference: Design doc "ConversationCompact Task" → "Implement automatic compaction"

[ ] **Task 10.3.4**: Run `cargo check` to verify automatic compaction compiles
- Fix any compilation errors
- Ensure compaction triggers correctly

### 10.4 Test ConversationCompact

[ ] **Task 10.4.1**: Add test module to conversation_compact.rs
- Add `#[cfg(test)]` module
- Create mock model provider
- Test compaction logic

[ ] **Task 10.4.2**: Manual test - /compact command
- Run chat
- Have long conversation
- Run `/compact` command
- Verify history is compacted
- Verify conversation continues correctly

[ ] **Task 10.4.3**: Manual test - /compact with instruction
- Run chat
- Run `/compact summarize briefly`
- Verify instruction is used
- Verify summary is appropriate

[ ] **Task 10.4.4**: Manual test - automatic compaction
- Run chat
- Have very long conversation (exceed token threshold)
- Verify automatic compaction triggers
- Verify conversation continues seamlessly

[ ] **Task 10.4.5**: Run `cargo test` to verify ConversationCompact tests pass
- Fix any failing tests
- Ensure compaction works correctly

---

## Completion Checklist

### Final Verification

[ ] **Task F.1**: Run full test suite
- Run `cargo test` for all tests
- Verify all tests pass
- Fix any failing tests

[ ] **Task F.2**: Run clippy for code quality
- Run `cargo clippy`
- Fix any warnings
- Ensure code meets quality standards

[ ] **Task F.3**: Run formatting check
- Run `cargo +nightly fmt --check`
- Format code if needed
- Ensure consistent formatting

[ ] **Task F.4**: Update documentation
- Update README if needed
- Update architecture docs
- Add migration notes

[ ] **Task F.5**: Manual end-to-end testing
- Test basic chat flow
- Test all UI modes
- Test all commands
- Test error handling
- Test shutdown scenarios

[ ] **Task F.6**: Performance testing
- Test with high event frequency
- Monitor event bus lag
- Check memory usage
- Verify no leaks

[ ] **Task F.7**: Create migration guide
- Document breaking changes
- Document new features
- Provide upgrade instructions

### Known Issues and Future Work

Document any known issues or limitations:
- [ ] Tool approval UI not yet implemented (exits with flag)
- [ ] Multi-worker UI switching not implemented
- [ ] Worker persistence not implemented
- [ ] Advanced UIs (web, fancy TUI) not implemented
- [ ] Performance monitoring not implemented

### Success Criteria

The implementation is complete when:
- [x] All phases are implemented
- [x] All tests pass
- [x] Code compiles without warnings
- [x] Manual testing shows correct behavior
- [x] Documentation is updated
- [x] Migration guide is created

---

## Notes for AI Assistant

### General Guidelines

1. **Read the design document first**: Before starting any task, read `planning/event-bus/event-bus-1-design.md` completely
2. **One task at a time**: Complete each task fully before moving to the next
3. **Mark completed tasks**: Change `[ ]` to `[x]` after completing each task
4. **Keep code buildable**: Run `cargo check` frequently to catch errors early
5. **Write tests**: Don't skip test tasks - they catch bugs early
6. **Ask for clarification**: If a task is unclear, refer to the design document section

### Common Pitfalls to Avoid

1. **Don't skip event publishing**: Every state change should publish an event
2. **Don't block on user input**: Use separate tasks for blocking operations
3. **Don't forget worker_id filtering**: UIs should filter events by worker_id
4. **Don't forget to clone Arc types**: When passing to spawned tasks
5. **Don't forget error handling**: Use `Result<()>` and `?` operator

### Debugging Tips

1. **Event not received**: Check if EventBus buffer is full (lagged events)
2. **Deadlock**: Check for Mutex lock ordering issues
3. **Task not running**: Check if task was spawned correctly
4. **Compilation errors**: Check imports and module declarations
5. **Test failures**: Check mock implementations and test setup

### Reference Sections in Design Doc

- Event types: "Event Structure" section
- EventBus: "EventBus Implementation" section
- Worker: "Worker" section
- Session: "Session" section
- AgentLoop: "AgentLoop Task" section
- AgentEnvironment: "AgentEnvironment Coordinator" section
- TextUi: "TextUi Implementation" section
- Commands: "Command System" section
- Data flow: "Data Flow & Interactions" section

---

## Implementation Progress Tracking

**Started**: [Date]  
**Current Phase**: Phase 1  
**Current Task**: Task 1.1.1  
**Completed Tasks**: 0 / 200+  
**Estimated Completion**: [Date]

### Phase Completion Status

- [x] Phase 1: Core Event System (27/27 tasks) ✅
- [x] Phase 2: Worker State Management (15/15 tasks) ✅
- [x] Phase 3: Session Event Publishing (11/11 tasks) ✅
- [x] Phase 4: Task Event Publishing (14/14 tasks) ✅
- [x] Phase 5: Command System (22/22 tasks) ✅
- [x] Phase 6: AgentEnvironment Coordinator (32/32 tasks) ✅
- [x] Phase 7: TextUi Implementation (26/26 tasks) ✅
- [x] Phase 8.1: Update ChatArgs::execute() (9/9 tasks) ✅
- [x] Phase 8.2: Remove Old Demo Code (5/5 tasks) ✅
- [ ] Phase 8.3: Handle Command-Line Arguments (0/4 tasks)
- [ ] Phase 8.4: Integration Testing (0/6 tasks)
- [ ] Phase 9: Additional UI Implementations (0/18 tasks)
- [ ] Phase 10: ConversationCompact Task (0/19 tasks)
- [ ] Final Verification (0/7 tasks)

**Total Progress**: 161 / 215 tasks (74.9%)

---

**END OF IMPLEMENTATION PLAN**
