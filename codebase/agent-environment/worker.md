# Worker - Agent State Container

## Overview

The **Worker** represents a complete AI agent state container - "agent config + conversation history + lifecycle state + task metadata + LLM access" bundled into a single unit. Each Worker is an independent agent that can execute tasks and maintains its own state.

## Implementation

**File**: `crates/chat-cli/src/agent_env/worker.rs`

## Structure

```rust
#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    
    #[serde(skip, default = "default_lifecycle_state")]
    pub lifecycle_state: Arc<Mutex<WorkerLifecycleState>>,
    
    pub task_metadata: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    
    pub context_container: ContextContainer,
    
    #[serde(skip, default = "default_model_provider")]
    pub model_provider: Option<Arc<dyn ModelProvider>>,
    
    #[serde(skip, default = "default_state")]
    pub state: Arc<Mutex<WorkerStates>>,
    
    #[serde(skip, default = "default_last_failure")]
    pub last_failure: Arc<Mutex<Option<String>>>,
}
```

### Fields

#### Serializable Fields
- **id**: Unique identifier for the worker (UUID)
- **name**: Human-readable name for identification
- **task_metadata**: Extensible storage for task-specific state (Arc<Mutex<HashMap>>)
- **context_container**: Conversation history and context

#### Non-Serializable Fields (Runtime Only)
- **lifecycle_state**: Worker lifecycle state managed by Session (Arc<Mutex<WorkerLifecycleState>>)
- **model_provider**: LLM provider for making requests (Option<Arc<dyn ModelProvider>>)
- **state**: Internal task execution state (Arc<Mutex<WorkerStates>>)
- **last_failure**: Storage for last error message (Arc<Mutex<Option<String>>>)

## Lifecycle States

**Managed by Session**, published as events:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WorkerLifecycleState {
    #[default]
    Idle,
    Busy,
    IdleFailed,
}
```

### State Descriptions

- **Idle**: Worker ready for new task (default state)
- **Busy**: Worker executing task
- **IdleFailed**: Worker idle after task failure

### State Transitions

```
Idle → Busy (when task starts)
Busy → Idle (when task completes successfully)
Busy → IdleFailed (when task fails)
IdleFailed → Busy (when new task starts)
```

## Task Metadata

Workers provide extensible metadata storage for task-specific state:

```rust
pub task_metadata: Arc<Mutex<HashMap<String, serde_json::Value>>>
```

### API

```rust
// Set metadata
worker.set_task_metadata(key, value);

// Get metadata
let value: Option<serde_json::Value> = worker.get_task_metadata(key);

// Get string metadata
let text: Option<String> = worker.get_task_metadata_string(key);
```

### Predefined Metadata Keys

```rust
pub mod task_metadata_keys {
    pub const AGENT_LOOP_COMPLETION_STATE: &str = "agent_loop.completion_state";
    pub const AGENT_LOOP_LAST_TOOL: &str = "agent_loop.last_tool";
    pub const COMPACT_LAST_RUN: &str = "compact.last_run_timestamp";
}
```

### Usage Example

```rust
use crate::agent_env::worker::task_metadata_keys;

// Set completion state
worker.set_task_metadata(
    task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
    serde_json::Value::String("completed_ready_for_prompt".to_string()),
);

// Get completion state
if let Some(state) = worker.get_task_metadata_string(
    task_metadata_keys::AGENT_LOOP_COMPLETION_STATE
) {
    match state.as_str() {
        "completed_ready_for_prompt" => { /* ready for user input */ }
        "completed_with_tool_request" => { /* needs tool approval */ }
        _ => {}
    }
}
```

## Internal Task States

**Managed by Tasks**, not published as events:

```rust
pub enum WorkerStates {
    Inactive,
    Working,
    Requesting,
    Receiving,
    Waiting,
    UsingTool,
    InactiveFailed,
}
```

These states are internal to task execution and not exposed via events. Use lifecycle_state for external state tracking.

## Key Methods

### Constructor

```rust
pub fn new(
    id: Uuid,
    name: String,
    model_provider: Arc<dyn ModelProvider>
) -> Self
```

Creates a new Worker with:
- Provided UUID and name
- Model provider for LLM access
- Lifecycle state initialized to Idle
- Empty task metadata
- Empty context container
- Internal state initialized to Inactive

### Metadata Methods

```rust
pub fn set_task_metadata(&self, key: &str, value: serde_json::Value)
```

Sets task metadata. Thread-safe via Arc<Mutex<>>.

```rust
pub fn get_task_metadata(&self, key: &str) -> Option<serde_json::Value>
```

Gets task metadata. Returns cloned value.

```rust
pub fn get_task_metadata_string(&self, key: &str) -> Option<String>
```

Gets string metadata. Convenience method for string values.

### Internal State Methods

```rust
pub fn set_state(&self, new_state: WorkerStates)
```

Updates internal task state. Thread-safe.

```rust
pub fn get_state(&self) -> WorkerStates
```

Returns current internal state. Thread-safe.

### Error Tracking

```rust
pub fn set_failure(&self, error: String)
```

Records error message for failed tasks.

## Serialization

Workers are fully serializable for persistence:

```rust
// Serialize to JSON
let json = serde_json::to_string(&worker)?;

// Deserialize from JSON
let worker: Worker = serde_json::from_str(&json)?;
```

**Note**: Non-serializable fields (lifecycle_state, model_provider, state, last_failure) are skipped during serialization and restored with default values on deserialization.

## Usage Patterns

### Pattern 1: Create and Configure Worker

```rust
// Create worker
let worker = session.build_worker("main".to_string());

// Add initial message
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_input_message("Hello!".to_string());

// Launch task
session.run_task__agent_loop(worker, AgentLoopInput {})?;
```

### Pattern 2: Check Completion State

```rust
// After job completes, check metadata
if let Some(state) = worker.get_task_metadata_string(
    task_metadata_keys::AGENT_LOOP_COMPLETION_STATE
) {
    match state.as_str() {
        "completed_ready_for_prompt" => {
            // Ready for next user input
            prompt_user();
        }
        "completed_with_tool_request" => {
            // Needs tool approval
            show_tool_approval_ui();
        }
        _ => {}
    }
}
```

### Pattern 3: Persist Worker State

```rust
// Serialize worker
let json = serde_json::to_string(&worker)?;
std::fs::write("worker.json", json)?;

// Later: restore worker
let json = std::fs::read_to_string("worker.json")?;
let mut worker: Worker = serde_json::from_str(&json)?;

// Restore runtime dependencies
worker.model_provider = Some(model_provider);
```

## Design Notes

### Lifecycle State vs Internal State

- **lifecycle_state**: High-level state (Idle/Busy/IdleFailed) managed by Session, published as events
- **state**: Low-level task execution state (Working/Requesting/etc.) managed by Tasks, not published

Use lifecycle_state for UI updates and external coordination. Use internal state for task-specific logic.

### Task Metadata Design

- **Extensible**: Tasks can add custom metadata without changing Worker struct
- **Serializable**: Metadata persists with worker state
- **Type-safe helpers**: Convenience methods for common types (string, etc.)
- **Namespaced keys**: Prevents conflicts between different task types

### Thread Safety

All mutable fields use Arc<Mutex<>> for interior mutability:
- lifecycle_state: Updated by Session
- task_metadata: Updated by Tasks
- state: Updated by Tasks
- last_failure: Updated by Tasks

This allows Worker to be shared across tasks without &mut self.

## Integration with Other Components

### With Session
Session manages Worker lifecycle:
- Creates workers via `build_worker()`
- Updates lifecycle_state via `set_worker_lifecycle_state()`
- Publishes lifecycle events

### With Tasks
Tasks use Worker for:
- Reading conversation history
- Updating task metadata
- Accessing model provider
- Setting internal state

### With ContextContainer
Worker owns ContextContainer:
- Stores conversation history
- Manages context files
- Provides serializable context

## Related Documentation

- [Session](./session.md) - Worker lifecycle management
- [ContextContainer](./context-container.md) - Conversation history
- [Tasks](./tasks.md) - Task implementations that use Workers
- [EventBus](./event-bus.md) - Lifecycle event publishing
