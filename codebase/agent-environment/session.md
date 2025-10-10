# Session - Worker and Job Orchestrator

## Overview

The **Session** serves as the central orchestrator managing all Workers and Jobs. It provides worker factory methods, job launching, lifecycle management, resource sharing, and **event publishing** for all lifecycle changes.

## Implementation

**File**: `crates/chat-cli/src/agent_env/session.rs`

## Structure

```rust
pub struct Session {
    event_bus: EventBus,
    workers: Arc<Mutex<HashMap<Uuid, Arc<Worker>>>>,
    jobs: Arc<Mutex<HashMap<Uuid, Arc<WorkerJob>>>>,
    model_providers: Vec<Arc<dyn ModelProvider>>,
}
```

### Fields

- **event_bus**: Central event distribution system for publishing lifecycle events
- **workers**: Thread-safe map of all created workers (keyed by worker_id)
- **jobs**: Thread-safe map of all running jobs (keyed by job_id)
- **model_providers**: Shared LLM providers for all workers

## Key Methods

### Constructor

```rust
pub fn new(event_bus: EventBus, model_providers: Vec<Arc<dyn ModelProvider>>) -> Self
```

Creates a new Session with:
- EventBus for publishing events
- Model providers for LLM communication
- Empty worker and job collections

**Example**:
```rust
let event_bus = EventBus::default();
let session = Session::new(event_bus, vec![bedrock_provider]);
```

### Worker Management

#### Create Worker

```rust
pub fn build_worker(&self, name: String) -> Arc<Worker>
```

Creates a new Worker with:
- Unique UUID identifier
- Specified name for identification
- First available model provider
- Lifecycle state initialized to Idle
- Empty task metadata
- Registered in session's worker collection

**Publishes**: `WorkerEvent::Created` with worker_id, name, timestamp

**Example**:
```rust
let worker = session.build_worker("main".to_string());
// Event published: WorkerEvent::Created { worker_id, name: "main", timestamp }
```

#### Get Worker

```rust
pub fn get_worker(&self, worker_id: Uuid) -> Option<Arc<Worker>>
```

Retrieves worker by ID. Returns None if worker doesn't exist.

#### Delete Worker

```rust
pub fn delete_worker(&self, worker_id: Uuid) -> Result<()>
```

Deletes a worker:
1. Cancels all jobs for this worker
2. Removes worker from collection
3. Publishes deletion event

**Publishes**: `WorkerEvent::Deleted` with worker_id, timestamp

### Worker Lifecycle State Management

```rust
fn set_worker_lifecycle_state(
    &self,
    worker_id: Uuid,
    new_state: WorkerLifecycleState,
)
```

Updates worker lifecycle state and publishes event:
- Gets worker by ID
- Reads old state
- Updates to new state
- Publishes state change event

**Publishes**: `WorkerEvent::LifecycleStateChanged` with worker_id, old_state, new_state, timestamp

**Lifecycle States**:
- **Idle**: Worker ready for new task
- **Busy**: Worker executing task
- **IdleFailed**: Worker idle after task failure

### Task Launchers

#### Agent Loop

```rust
pub fn run_task__agent_loop(
    &self,
    worker: Arc<Worker>,
    input: AgentLoopInput,
) -> Result<Arc<WorkerJob>>
```

Launches an agent loop task:
1. Sets worker lifecycle state to Busy
2. Creates AgentLoop task with EventBus
3. Creates WorkerJob
4. Publishes JobStarted event
5. Spawns async task to monitor completion
6. Returns job handle

**Publishes**:
- `WorkerEvent::LifecycleStateChanged` (Idle → Busy)
- `JobEvent::Started` with worker_id, job_id, task_type="AgentLoop", timestamp

**Example**:
```rust
let job = session.run_task__agent_loop(worker, AgentLoopInput {})?;
// Events published:
// 1. WorkerEvent::LifecycleStateChanged { worker_id, old_state: Idle, new_state: Busy, ... }
// 2. JobEvent::Started { worker_id, job_id, task_type: "AgentLoop", ... }
```

#### Compact Conversation

```rust
pub fn run_task__compact_conversation(
    &self,
    worker: Arc<Worker>,
    input: CompactInput,
) -> Result<Arc<WorkerJob>>
```

Launches a conversation compaction task:
1. Sets worker lifecycle state to Busy
2. Creates ConversationCompact task with EventBus
3. Creates WorkerJob
4. Publishes JobStarted event
5. Spawns async task to monitor completion
6. Returns job handle

**Publishes**:
- `WorkerEvent::LifecycleStateChanged` (Idle → Busy)
- `JobEvent::Started` with worker_id, job_id, task_type="ConversationCompact", timestamp

### Job Completion Handling

```rust
async fn handle_job_completion(
    &self,
    job: Arc<WorkerJob>,
    result: Result<()>,
)
```

Handles job completion:
1. Determines JobCompletionResult (Success/Failed/Cancelled)
2. Extracts task_metadata from worker
3. Updates worker lifecycle state (Idle or IdleFailed)
4. Removes job from active jobs
5. Publishes completion event
6. Runs job continuations

**Publishes**:
- `WorkerEvent::LifecycleStateChanged` (Busy → Idle/IdleFailed)
- `JobEvent::Completed` with worker_id, job_id, result, timestamp

**JobCompletionResult**:
- `Success { task_metadata }`: Task completed successfully
- `Failed { error }`: Task failed with error
- `Cancelled`: Task was cancelled

### Job Cancellation

```rust
pub fn cancel_worker_jobs(&self, worker_id: Uuid) -> Result<()>
```

Cancels all jobs for a specific worker.

```rust
pub fn cancel_all_jobs(&self)
```

Cancels all running jobs:
- Iterates through job collection
- Calls `cancel()` on each job
- Jobs complete gracefully via cancellation tokens

## Event Publishing

Session publishes events for all lifecycle changes:

### Worker Events
- **Created**: When `build_worker()` is called
- **Deleted**: When `delete_worker()` is called
- **LifecycleStateChanged**: When worker state changes (Idle ↔ Busy ↔ IdleFailed)

### Job Events
- **Started**: When task is launched
- **Completed**: When task finishes (success, failure, or cancellation)

All events include:
- Relevant IDs (worker_id, job_id)
- Timestamp (Instant::now())
- Event-specific data (state, result, etc.)

## Usage Pattern

```rust
// Create EventBus and Session
let event_bus = EventBus::default();
let session = Arc::new(Session::new(event_bus.clone(), vec![bedrock_provider]));

// Subscribe to events (optional)
let mut receiver = event_bus.subscribe();
tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        println!("Event: {:?}", event);
    }
});

// Create worker
let worker = session.build_worker("main".to_string());
// Event: WorkerEvent::Created { worker_id, name: "main", ... }

// Add message to conversation
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_input_message("Hello!".to_string());

// Launch agent loop
let job = session.run_task__agent_loop(worker.clone(), AgentLoopInput {})?;
// Event: WorkerEvent::LifecycleStateChanged { ..., new_state: Busy, ... }
// Event: JobEvent::Started { ..., task_type: "AgentLoop", ... }

// Job runs asynchronously, publishes events:
// - JobEvent::OutputChunk for each response chunk
// - AgentLoopEvent::ResponseReceived when complete
// - JobEvent::Completed when job finishes
// - WorkerEvent::LifecycleStateChanged { ..., new_state: Idle, ... }

// Cancel all jobs if needed
session.cancel_all_jobs();
```

## Design Notes

### Event-Driven Architecture
- All lifecycle changes publish events
- Components communicate via EventBus
- No direct coupling between Session and UIs
- Enables multiple concurrent UIs

### Resource Sharing
- **Model Providers**: Shared across all workers
- **EventBus**: Single instance for all events
- **Thread Pool**: Tokio runtime shared for all async tasks

### Thread Safety
- Workers and jobs stored in Arc<Mutex<HashMap>>
- Safe concurrent access from multiple tasks
- Lifecycle state in Arc<Mutex<>> for interior mutability

### Lifecycle Management
- Session tracks all workers and jobs
- Cancellation tokens enable graceful shutdown
- Job completion handled asynchronously
- Continuations run after job completes

## Integration with Other Components

### With EventBus
Session publishes events to EventBus for:
- Worker creation/deletion
- Worker lifecycle state changes
- Job start/completion

### With AgentEnvironment
AgentEnvironment calls Session methods to:
- Create workers
- Launch tasks
- Cancel jobs

### With Worker
Session manages Worker lifecycle:
- Creates workers with `build_worker()`
- Updates lifecycle state via `set_worker_lifecycle_state()`
- Accesses worker data for job completion

### With WorkerJob
Session creates and monitors jobs:
- Creates WorkerJob with worker and task
- Spawns async task to monitor completion
- Calls `handle_job_completion()` when done

## Related Documentation

- [EventBus](./event-bus.md) - Event distribution system
- [Worker](./worker.md) - Worker structure with lifecycle state
- [WorkerJob](./job.md) - Job execution and lifecycle
- [AgentEnvironment](./agent-environment.md) - Top-level coordinator
- [Tasks](./tasks.md) - WorkerTask implementations
