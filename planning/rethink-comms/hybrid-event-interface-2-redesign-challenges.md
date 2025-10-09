# Hybrid Event Interface - Redesign Challenges Analysis

**Date**: 2025-10-09  
**Context**: Analysis of proposed vision for re-architecting agent environment with EventBus + direct interfaces

## Executive Summary

This document analyzes the proposed architecture vision against the current implementation and the hybrid design document. It identifies critical inconsistencies, design issues, and provides recommendations for resolution.

## Proposed vision

Note - _Consider_ things marked as 'Future:', how they would be implemented in this architecture in future iterations.

- Entry Point does the following:
    - Creates EventBus
    - Creates Session (using EventBus)
    - Default 'main' worker (depends on ChatArgs) (future: do not create when started in --web --headless mode)
        - this worker should already pulish events to EventBus
    - selected UI implementation core (start with one, future: choose class based on --ui=??, or don't create at all when --headless)
        - uses Session (_maybe_ default worker if available)
    - (future: selected headless UIs - web, debug tracker)
    - AgentEnvironment (using EventBus, Session, main worker (if available), UI implementation(if available)) ('main loop', responsible for )
    - executes await agentEnvironment.run()
    - proceeds with cleanup and shutdown
- EventBus- implemented close enough to the suggested architecure
    - AgentEvent is AgentEnvironmentEvent, and I want to see them hierarchical of sort (up to reasobnable Rust support):
        - `AgentEnvironmentEvent(WorkerEvent(StatusChanged(workerId, WorkerStatus.Idle, WorkerStatus.Busy)))`
        - `AgentEnvironmentEvent(JobEvent(JobStarted(jobId, workerId, jobType)))`
        - `AgentEnvironmentEvent(JobOutputEvent(jobId, workerId,...)`
        - The goal here is to let subscribers use more transparent event matching, filtering _only_ to WorkerEvents, for example
- Session 
    - takes provided EventBus, passes it down to created Jobs (not Workers!)
    - When the job is launched - sets worker status to Busy and sends message
    - When the job is complete - sets worker status to Idle or IdleFailed
    - Passes EventBus to .run method of the launched task
- Worker
    - let's add some 'taskData' hash map, where Tasks can store extra flags as needed. `AgentLoopTask` can store flags like "completed_with_tool_request".
- WorkerTask trait
    - run method has to accept EventBus so the implementation can send own messages
        - bonus if can provide 'limited' sender version, which would only accept messages of the types declared by the WorkerTask implementation
            - i.e. `impl WorkerTask for AgentLoopTask { fun eventBusEventTypes() => [AgentLoopEvents, JobOutputEvent]; fun run(..., eventSender)}` - Session would use eventBusEventTypes result to make sure the task does not send anything else (like `JobEvent/JobStarted`, for example)
- AgentEnvironment
    - monitors interruption and shutdown signals
    - if main worker and TUI are provided
        - (future: we can potentially have TUI with no main worker, that would display summary of Session state or something like that)
        - listens to JobOutputEvents, filters to workerId=mainWorkrId, forwards to TUI
        - listens to JobEvent/JobCompleted, filters to workerId=mainWorkrId, launches TUI.prompt _not blocking the main loop_, with "callback" or completion
            - callback takes prompt_result that identifies the next task to execute
                - Two tasks initially: AgentLoopTask, and ConversationCompactTask (the second just uses a predefined prompt to compact the conversation history)
                    - Question - do we need TUI.prompt to actually provide extra payload for the task? I.e. ConversationCompactTask can take extra prompt to finetune the result
            - we assume here that TUI will configure the worker properly before task launch: add new prompt to ConversationHistory or update open tool use requests
- BasicTextUi (the first implementation of text UI)
    - provides TUI.prompt method, in the way that AgentEnvironment needs to not block its main loop
        - the method must provide an option to pass control back to AgentEnvironment to spawn next task
        - first implementation can be similar to what we have in demo, just simple readline
        - future implementations will verify more specific state of the worker - did it complete normally, does it have open tool use approval requests, check for job flags etc
        - future implementations will also provide options to run sub-commands (/quit and /compact at first, then existing /usage, /context, etc)
            - note about sub-commands that can be launched from TUI (not just for BasicTextUi)
                - there are two kinds of commands - those that spawn a task in Session, and those that just functions on Worker or Session
                - you can think about basic prompt like an implicit command /prompt, taht spawns task AgentLoopTask; explicit command /compact spawns task ConversationCompactTask

## Critical Issues

### 1. AgentEnvironment Component - Unclear Responsibility

**Proposed Vision**:
- Entry Point creates AgentEnvironment (using EventBus, Session, main worker, UI)
- AgentEnvironment monitors shutdown signals
- AgentEnvironment listens to events and forwards to TUI
- AgentEnvironment launches TUI.prompt

**Problem**: 
- Currently, `AgentEnvTextUi` IS the main loop
- The hybrid design doesn't mention AgentEnvironment as a separate component
- Creates confusing ownership: Entry Point → AgentEnvironment → UI
- AgentEnvironment becomes a thin event forwarder with no clear value

**Recommendation**: 
- **Option A** (Preferred): Remove AgentEnvironment entirely. Let UI subscribe to EventBus directly and own the main loop
- **Option B**: AgentEnvironment IS the UI implementation (rename `AgentEnvTextUi` to `AgentEnvironment`)
- **Option C**: Make AgentEnvironment a minimal coordinator that only handles shutdown

**Rationale**: Option A is simpler, matches the hybrid design better, and eliminates unnecessary indirection.


**DM Response**: AgentEnvironment is future-proofing the system. I want to be able to quickly add and plug in another 
UI implementation ('structured I/O for scripting') AND add headless UIs (web API). This layer would be responsible for
coordinating events with user-facing UI and multicasting them to headless UIs.

**Response**: Valid use case. Revised recommendation:

**AgentEnvironment as Event Coordinator**:
```rust
pub struct AgentEnvironment {
    session: Arc<Session>,
    event_bus: EventBus,
    main_ui: Option<Arc<dyn UserInterface>>,
    headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    shutdown_signal: Arc<Notify>,
}

impl AgentEnvironment {
    pub async fn run(&self) -> Result<(), eyre::Error> {
        // Spawn event multicast task
        let multicast_task = self.spawn_event_multicast();
        
        // Run main UI if present
        if let Some(ui) = &self.main_ui {
            ui.run().await?;
        } else {
            // Headless mode - wait for shutdown
            self.shutdown_signal.notified().await;
        }
        
        multicast_task.abort();
        Ok(())
    }
    
    async fn spawn_event_multicast(&self) -> JoinHandle<()> {
        let mut receiver = self.event_bus.subscribe();
        let headless_uis = self.headless_uis.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                // Multicast to all headless UIs
                for ui in &headless_uis {
                    ui.handle_event(event.clone()).await;
                }
            }
        })
    }
}
```

This design:
- Coordinates between main UI and headless UIs
- Multicasts events to all subscribers
- Supports headless mode (no main UI)
- Owns the top-level run loop

**Resolved** with revised architecture.

---

### 2. Hierarchical Event Structure - Not Idiomatic Rust

**Proposed Vision**:
```rust
AgentEnvironmentEvent(WorkerEvent(StatusChanged(workerId, WorkerStatus.Idle, WorkerStatus.Busy)))
AgentEnvironmentEvent(JobEvent(JobStarted(jobId, workerId, jobType)))
AgentEnvironmentEvent(JobOutputEvent(jobId, workerId,...))
```

**Problem**: 
- Rust doesn't support this nested syntax naturally
- Would require nested enums with verbose pattern matching:
  ```rust
  match event {
      AgentEnvironmentEvent::Worker(WorkerEvent::StatusChanged { .. }) => { .. }
      AgentEnvironmentEvent::Job(JobEvent::Started { .. }) => { .. }
  }
  ```
- The hybrid design uses flat enums which is more idiomatic and cleaner

**Current Hybrid Design** (Better):
```rust
enum AgentEvent {
    WorkerStateChanged { worker_id, old_state, new_state, timestamp },
    JobStarted { worker_id, job_id, task_type, timestamp },
    ResponseChunkReceived { worker_id, chunk, timestamp },
}
```

**Recommendation**: Use flat enum structure. For filtering, add helper methods:
```rust
impl AgentEvent {
    pub fn worker_id(&self) -> Option<Uuid> {
        match self {
            Self::WorkerStateChanged { worker_id, .. } => Some(*worker_id),
            Self::JobStarted { worker_id, .. } => Some(*worker_id),
            _ => None,
        }
    }
    
    pub fn is_worker_event(&self) -> bool {
        matches!(self, Self::WorkerStateChanged { .. } | Self::WorkerDeleted { .. })
    }
    
    pub fn is_job_event(&self) -> bool {
        matches!(self, Self::JobStarted { .. } | Self::JobCompleted { .. })
    }
}
```

**Rationale**: Flat enums are idiomatic Rust, easier to pattern match, and helper methods provide filtering without complexity.

**DM Response**: I don't care, I like it.

**Response**: Understood. Here's how to implement it idiomatically in Rust:

```rust
// Nested enum structure
pub enum AgentEnvironmentEvent {
    Worker(WorkerEvent),
    Job(JobEvent),
    System(SystemEvent),
}

pub enum WorkerEvent {
    Created { worker_id: Uuid, name: String },
    Deleted { worker_id: Uuid },
    StateChanged { worker_id: Uuid, old_state: WorkerStates, new_state: WorkerStates },
}

pub enum JobEvent {
    Started { worker_id: Uuid, job_id: Uuid, task_type: String },
    Completed { worker_id: Uuid, job_id: Uuid, result: JobResult },
    OutputChunk { worker_id: Uuid, job_id: Uuid, chunk: OutputChunk },
}

pub enum SystemEvent {
    ShutdownInitiated { reason: String },
}

// Helper methods for filtering
impl AgentEnvironmentEvent {
    pub fn worker_id(&self) -> Option<Uuid> {
        match self {
            Self::Worker(WorkerEvent::Created { worker_id, .. }) => Some(*worker_id),
            Self::Worker(WorkerEvent::StateChanged { worker_id, .. }) => Some(*worker_id),
            Self::Job(JobEvent::Started { worker_id, .. }) => Some(*worker_id),
            Self::Job(JobEvent::OutputChunk { worker_id, .. }) => Some(*worker_id),
            _ => None,
        }
    }
    
    pub fn is_worker_event(&self) -> bool {
        matches!(self, Self::Worker(_))
    }
}

// Pattern matching usage
match event {
    AgentEnvironmentEvent::Worker(WorkerEvent::StateChanged { worker_id, new_state, .. }) => {
        println!("Worker {} changed to {:?}", worker_id, new_state);
    }
    AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
        print!("{}", chunk);
    }
    _ => {}
}
```

**Resolved** - will implement with nested enums.

---

### 3. State Management Responsibility - Ambiguous

**Proposed Vision**:
- "When the job is launched - sets worker status to Busy and sends message"
- "When the job is complete - sets worker status to Idle or IdleFailed"
- "AgentLoopTask can store flags like 'completed_with_tool_request'"

**Problem**: Who owns worker state transitions?
- Vision says Session sets state on job start/complete
- But tasks need to signal internal transitions (Working → Requesting → Receiving → UsingTool)
- Current code has tasks calling `worker.set_state()` directly
- If both Session and Tasks set state, race conditions are possible

**Current Implementation**:
```rust
// In AgentLoop::run()
self.worker.set_state(WorkerStates::Working, interface);
self.worker.set_state(WorkerStates::Requesting, interface);
// ... etc
```

**Recommendation**: Split responsibilities clearly:

| Component | Responsibility | States |
|-----------|---------------|--------|
| **Session** | Job lifecycle only | Inactive → Working (on launch)<br>Working → Inactive (on completion) |
| **Tasks** | Internal transitions | Working → Requesting<br>Requesting → Receiving<br>Receiving → UsingTool<br>UsingTool → Working |
| **Both** | Publish events | All state changes publish to EventBus |

**Implementation Pattern**:
```rust
// Session.run() - job lifecycle
worker.set_state(WorkerStates::Working);
event_bus.publish(AgentEvent::JobStarted { ... });

// AgentLoop.run() - internal transitions
self.worker.set_state(WorkerStates::Requesting);
self.event_bus.publish(AgentEvent::WorkerStateChanged { ... });
```

**Rationale**: Clear ownership prevents race conditions and makes state transitions predictable.

**DM Response**: Those are different "states". Worker state Idle/Busy/IdleFailed only indicates that there's a job running using this Worker.
'completed_with_tool_request' flag by AgentLoopTask is just a helper signal that would help UI implementation draw proper controls for the Worker.
NOTE: this flag is just an example, and specifically this "state" would be identifiable based on ConversationState data.

**Response**: Clarified. Two separate concepts:

1. **Worker Lifecycle State** (managed by Session):
   - `Idle` - No job running
   - `Busy` - Job is running
   - `IdleFailed` - Last job failed

2. **Task-Specific Flags** (managed by Task, stored in Worker):
   - `completed_with_tool_request` - AgentLoop stopped waiting for tool approval
   - `completed_ready_for_prompt` - AgentLoop completed normally
   - Other task-specific state

Revised implementation:
```rust
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub lifecycle_state: Arc<Mutex<WorkerLifecycleState>>,  // Session manages this
    pub task_flags: Arc<Mutex<HashMap<String, serde_json::Value>>>,  // Tasks manage this
    // ... other fields
}

pub enum WorkerLifecycleState {
    Idle,
    Busy,
    IdleFailed { error: String },
}
```

**Resolved** - clear separation of concerns.

---

### 4. Worker.taskData HashMap - Vague and Untyped

**Proposed Vision**:
> "let's add some 'taskData' hash map, where Tasks can store extra flags as needed. AgentLoopTask can store flags like 'completed_with_tool_request'."

**Problem**:
- Untyped data (`HashMap<String, ???>`) leads to runtime errors
- Unclear ownership (who reads/writes? when?)
- No type safety or compile-time checking
- Better alternatives exist

**Recommendation**: Avoid generic HashMap. Instead:

**Option A** (Preferred): Tasks store their own state internally
```rust
pub struct AgentLoop {
    worker: Arc<Worker>,
    event_bus: EventBus,
    // Task-specific state
    completed_with_tool_request: Arc<AtomicBool>,
    last_tool_name: Arc<Mutex<Option<String>>>,
}
```

**Option B**: Use typed fields in Worker for specific known state
```rust
pub struct Worker {
    // ... existing fields
    pub pending_tool_approval: Arc<Mutex<Option<ToolApprovalRequest>>>,
}
```

**Option C**: If really needed, use typed metadata
```rust
pub struct Worker {
    // ... existing fields
    pub metadata: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}
```

**Rationale**: Type safety prevents bugs. Tasks should own their state. Worker should only store state that needs to persist across tasks.

**DM Response**: I want to be able to serialize the Worker state and de-serialize it back. At the same time I want to be able to safely expand
the list of available tasks, and keep Worker generally abstratced from them. I can't predict right now what flags some future tasks can require.
Does this information change suggested options?

**Response**: Yes, serialization requirement changes the recommendation. Revised approach:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub lifecycle_state: WorkerLifecycleState,
    
    // Task-specific metadata - serializable, extensible
    pub task_metadata: HashMap<String, serde_json::Value>,
    
    // Non-serializable runtime state
    #[serde(skip)]
    pub model_provider: Arc<dyn ModelProvider>,
    #[serde(skip)]
    pub context_container: ContextContainer,
}

// Task-specific metadata keys (namespaced to avoid conflicts)
pub mod task_metadata_keys {
    pub const AGENT_LOOP_COMPLETION_STATE: &str = "agent_loop.completion_state";
    pub const AGENT_LOOP_LAST_TOOL: &str = "agent_loop.last_tool";
    pub const COMPACT_LAST_RUN: &str = "compact.last_run_timestamp";
}

// Helper methods for type-safe access
impl Worker {
    pub fn set_completion_state(&self, state: &str) {
        self.task_metadata.insert(
            task_metadata_keys::AGENT_LOOP_COMPLETION_STATE.to_string(),
            serde_json::Value::String(state.to_string())
        );
    }
    
    pub fn get_completion_state(&self) -> Option<String> {
        self.task_metadata
            .get(task_metadata_keys::AGENT_LOOP_COMPLETION_STATE)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}
```

**Benefits**:
- Fully serializable with serde
- Extensible - new tasks can add metadata without changing Worker struct
- Namespaced keys prevent conflicts
- Type-safe helper methods for common access patterns
- Can version metadata format if needed

**Resolved** - HashMap with serde_json::Value is appropriate for this use case.

---

### 5. WorkerTask.run EventBus Parameter - Wrong Pattern

**Proposed Vision**:
> "run method has to accept EventBus so the implementation can send own messages"

```rust
trait WorkerTask {
    async fn run(&self, event_bus: EventBus) -> Result<(), eyre::Error>;
}
```

**Problem**: 
- The hybrid design stores EventBus in the task struct, not as a parameter
- Passing to `run()` requires changing trait signature, affecting all implementations
- Less flexible than storing in struct

**Current Hybrid Design** (Better):
```rust
impl AgentLoop {
    pub fn new(
        worker: Arc<Worker>,
        input: AgentLoopInput,
        event_bus: EventBus,  // Stored in struct
        tool_approver: Arc<dyn ToolApprover>,
        error_handler: Arc<dyn ErrorHandler>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self { event_bus, ... }
    }
}

#[async_trait]
impl WorkerTask for AgentLoop {
    async fn run(&self) -> Result<(), eyre::Error> {
        // Use self.event_bus throughout
        self.event_bus.publish(AgentEvent::...);
    }
}
```

**Recommendation**: Store EventBus in task struct during construction. This:
- Keeps trait simple and stable
- Allows tasks to publish events throughout lifecycle
- Matches Rust ownership patterns
- More flexible (tasks can store other dependencies too)

**Regarding "limited sender"**: The idea of restricting which events a task can send is interesting but adds significant complexity in Rust's type system:
```rust
pub struct TypedEventBus<T> {
    inner: EventBus,
    _phantom: PhantomData<T>,
}
```
This requires complex trait bounds and provides unclear benefit. Skip for now.

**Rationale**: Storing dependencies in struct is idiomatic Rust. Trait parameters should be minimal.

**DM Response**: 'affecting all implementations' - there are no implementations right now, we are working on proof-of concept and can throw everything away.
Otherwise it makes sense.

**Response**: **Resolved** - storing EventBus in task struct is the right approach.

---

### 6. AgentEnvironment Forwarding Events to TUI - Unnecessary Layer

**Proposed Vision**:
> "listens to JobOutputEvents, filters to workerId=mainWorkrId, forwards to TUI"

**Problem**: 
- Why have AgentEnvironment forward events when TUI can subscribe directly?
- Adds unnecessary indirection
- TUI can filter events itself
- Hybrid design shows TUI subscribing directly to EventBus

**Current Hybrid Design** (Better):
```rust
impl TextUi {
    async fn event_loop(&self) {
        let mut receiver = self.event_bus.subscribe();
        
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    // Filter directly
                    if let Some(wid) = event.worker_id() {
                        if wid == self.main_worker_id {
                            self.handle_event(event).await;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("Warning: Event bus lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
```

**Recommendation**: Remove forwarding layer. Let UI subscribe to EventBus and filter events itself.

**Rationale**: Direct subscription is simpler, more efficient, and eliminates unnecessary component.

**DM Response**: see response to "1. AgentEnvironment Component - Unclear Responsibility". There going to be more 'downstreams' for this layer.

**Response**: **Resolved** - AgentEnvironment multicasts events to multiple UIs (see revised architecture in #1).

---

### 7. TUI.prompt Callback Pattern - Current Design is Better

**Proposed Vision**:
> "launches TUI.prompt not blocking the main loop, with 'callback' or completion"
> "the method must provide an option to pass control back to AgentEnvironment to spawn next task"

**Implied Design**:
```rust
ui.prompt(|prompt_result| {
    // callback to spawn next task
    session.run_agent_loop(...);
});
```

**Problem**: 
- Callback-based design is less idiomatic in Rust than continuation pattern
- Current implementation already solves this well
- Callbacks complicate error handling and cancellation

**Current Design** (Better):
```rust
// Launch job
let job = session.run_agent_loop(worker.clone(), AgentLoopInput {}, ui_interface)?;

// Set up continuation to re-queue prompt when done
let continuation = ui.create_agent_completion_continuation();
job.worker_job_continuations.add_or_run_now(
    "re_queue_prompt",
    continuation,
    worker,
).await;

// Continuation implementation
pub fn create_agent_completion_continuation(&self) -> WorkerJobContinuationFn {
    let prompt_queue = self.prompt_queue.clone();
    
    Continuations::boxed(move |worker, completion_type, error_msg| {
        let prompt_queue = prompt_queue.clone();
        async move {
            // Handle completion
            prompt_queue.enqueue(worker).await;
        }
    })
}
```

**Recommendation**: Keep current PromptQueue + continuation pattern. It's cleaner, more flexible, and already works well.

**Rationale**: Continuations are more composable, easier to test, and handle errors/cancellation better than callbacks.

**DM Response**: UI Prompt is NOT a Session's Task, period. They don't mix and don't overlam for a plethora of reasons.
"Callback" is just a suggested behaviour. Existing 'continuation' mechanism is the same thing idealogically. I'm open to
have continuations on UI.prompt and other user-waiting methods in any Rust-friendly shape.
My main concern with specifically this flow is the contract on ui.prompt result that would let AgentEnvironment launch 
varous Tasks based on it (AgentLoopTask vs ConversationCompactTask for example). Suggestion #8 below answers that partially.

**Response**: Understood. Here's a revised design for UI.prompt that returns a command to AgentEnvironment:

```rust
// UI.prompt returns a command for AgentEnvironment to execute
pub enum PromptResult {
    Command(Command),
    Shutdown,
}

pub enum Command {
    Prompt { worker_id: Uuid, text: String },
    Compact { worker_id: Uuid, instruction: Option<String> },
    // ... other task-spawning commands
}

// In AgentEnvironment main loop
impl AgentEnvironment {
    async fn run(&self) -> Result<(), eyre::Error> {
        loop {
            // UI.prompt blocks until user provides input
            let result = self.main_ui.prompt().await?;
            
            match result {
                PromptResult::Command(cmd) => {
                    // AgentEnvironment decides which task to spawn
                    match cmd {
                        Command::Prompt { worker_id, text } => {
                            let worker = self.session.get_worker(worker_id)?;
                            worker.context_container.add_message(text);
                            self.session.run_agent_loop(worker, AgentLoopInput {})?;
                        }
                        Command::Compact { worker_id, instruction } => {
                            let worker = self.session.get_worker(worker_id)?;
                            self.session.run_compact_task(worker, CompactInput { instruction })?;
                        }
                    }
                }
                PromptResult::Shutdown => break,
            }
        }
        Ok(())
    }
}

// UI implementation
impl TextUi {
    pub async fn prompt(&self) -> Result<PromptResult, eyre::Error> {
        let input = self.input_handler.read_line("You").await?;
        
        // Parse command
        let command = CommandParser::parse(&input)?;
        
        // Handle UI-specific commands internally
        match command {
            Command::Usage => {
                self.display_usage();
                return self.prompt().await; // Re-prompt
            }
            Command::Context => {
                self.display_context();
                return self.prompt().await; // Re-prompt
            }
            // Task-spawning commands return to AgentEnvironment
            cmd @ (Command::Prompt { .. } | Command::Compact { .. }) => {
                Ok(PromptResult::Command(cmd))
            }
            Command::Quit => Ok(PromptResult::Shutdown),
        }
    }
}
```

**Key points**:
- UI.prompt is NOT a task, it's a blocking call that returns a command
- AgentEnvironment owns task spawning logic
- UI handles UI-specific commands (Usage, Context) internally
- Task-spawning commands (Prompt, Compact) are returned to AgentEnvironment
- Clear separation: UI handles display, AgentEnvironment handles orchestration

**Resolved** with revised interface design.

**DM Concern**: Waiting for UI input should not block listening to the main EventBus - AgentEnvironment could still need to forward events to headless UIs. Preferably it shouldn't block UI implementation's main loop, as it could need to display some widgets based on other workers status change (for some more advanced UI implementation, for example). And in general, UI still has to deplete events from the event bus, received through AgentEnvironment.

**Revised Non-Blocking Design**:

```rust
// UI provides a channel for commands instead of blocking call
pub trait UserInterface: Send + Sync {
    /// Get receiver for commands from this UI
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    
    /// Start UI event processing loop (non-blocking)
    async fn start(&self) -> Result<(), eyre::Error>;
}

impl AgentEnvironment {
    async fn run(&self) -> Result<(), eyre::Error> {
        // Spawn event multicast task (always running)
        let multicast_handle = self.spawn_event_multicast();
        
        // Start main UI if present (spawns its own tasks)
        if let Some(ui) = &self.main_ui {
            ui.start().await?;
            let mut cmd_receiver = ui.command_receiver();
            
            // Main loop: process commands without blocking events
            loop {
                tokio::select! {
                    // Process UI commands
                    Some(result) = cmd_receiver.recv() => {
                        match result {
                            PromptResult::Command(cmd) => {
                                self.handle_command(cmd).await?;
                            }
                            PromptResult::Shutdown => break,
                        }
                    }
                    
                    // Handle shutdown signal
                    _ = self.shutdown_signal.notified() => break,
                }
            }
        } else {
            // Headless mode - just wait for shutdown
            self.shutdown_signal.notified().await;
        }
        
        multicast_handle.abort();
        Ok(())
    }
    
    async fn spawn_event_multicast(&self) -> JoinHandle<()> {
        let mut receiver = self.event_bus.subscribe();
        let headless_uis = self.headless_uis.clone();
        let main_ui = self.main_ui.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                // Forward to headless UIs
                for ui in &headless_uis {
                    ui.handle_event(event.clone()).await;
                }
                
                // Forward to main UI
                if let Some(ui) = &main_ui {
                    ui.handle_event(event.clone()).await;
                }
            }
        })
    }
}

// TextUi implementation with non-blocking design
impl TextUi {
    pub fn new(event_bus: EventBus, history_path: Option<PathBuf>) -> Result<Self, eyre::Error> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        
        Ok(Self {
            event_bus,
            input_handler: InputHandler::new(history_path)?,
            cmd_sender,
            cmd_receiver: Arc::new(Mutex::new(cmd_receiver)),
            shutdown_signal: Arc::new(Notify::new()),
        })
    }
}

impl UserInterface for TextUi {
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
        self.cmd_receiver.lock().unwrap().clone()
    }
    
    async fn start(&self) -> Result<(), eyre::Error> {
        // Spawn event processing task (non-blocking)
        let event_task = self.spawn_event_processor();
        
        // Spawn prompt task (non-blocking)
        let prompt_task = self.spawn_prompt_loop();
        
        Ok(())
    }
    
    fn spawn_event_processor(&self) -> JoinHandle<()> {
        let mut receiver = self.event_bus.subscribe();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(event) = receiver.recv() => {
                        // Process event (update display, etc.)
                        match event {
                            AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
                                print!("{}", chunk);
                                io::stdout().flush().unwrap();
                            }
                            AgentEnvironmentEvent::Worker(WorkerEvent::StateChanged { .. }) => {
                                // Update status display
                            }
                            _ => {}
                        }
                    }
                    _ = shutdown.notified() => break,
                }
            }
        })
    }
    
    fn spawn_prompt_loop(&self) -> JoinHandle<()> {
        let input_handler = self.input_handler.clone();
        let cmd_sender = self.cmd_sender.clone();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Read input (blocking, but in separate task)
                    Ok(input) = input_handler.read_line("You") => {
                        let command = CommandParser::parse(&input).unwrap();
                        
                        match command {
                            // UI-specific commands - handle internally
                            Command::Usage => {
                                println!("Token usage: ...");
                                continue; // Re-prompt
                            }
                            
                            // Task-spawning commands - send to AgentEnvironment
                            cmd @ (Command::Prompt { .. } | Command::Compact { .. }) => {
                                cmd_sender.send(PromptResult::Command(cmd)).await.unwrap();
                            }
                            
                            Command::Quit => {
                                cmd_sender.send(PromptResult::Shutdown).await.unwrap();
                                break;
                            }
                        }
                    }
                    _ = shutdown.notified() => break,
                }
            }
        })
    }
}
```

**Benefits of Non-Blocking Design**:
- ✅ AgentEnvironment always processes events (multicast never blocks)
- ✅ UI event processing runs independently (can update widgets)
- ✅ UI prompt loop runs independently (can wait for input)
- ✅ Multiple concurrent operations without blocking
- ✅ Advanced UIs can spawn additional tasks for complex widgets

**Architecture Flow**:
```
AgentEnvironment.run()
├─ Spawns event_multicast task (always running)
│  └─ Forwards events to all UIs
├─ Starts main UI (spawns tasks, returns immediately)
│  ├─ Spawns event_processor task
│  │  └─ Receives events, updates display
│  └─ Spawns prompt_loop task
│     └─ Reads input, sends commands via channel
└─ Main loop: receives commands from UI channel
   └─ Spawns tasks based on commands
```


---

## Design Clarifications Needed

### 8. Command System - Good Idea, Needs Explicit Design

**Proposed Vision**:
> "there are two kinds of commands - those that spawn a task in Session, and those that just functions on Worker or Session"
> "you can think about basic prompt like an implicit command /prompt, that spawns task AgentLoopTask; explicit command /compact spawns task ConversationCompactTask"

**This is a good insight** but needs explicit design before implementation.

**Recommended Design**:

```rust
/// Commands that can be executed from UI
pub enum Command {
    // Task-spawning commands (async, create jobs)
    Prompt(String),                    // Spawns AgentLoopTask
    Compact(Option<String>),           // Spawns ConversationCompactTask
    
    // Immediate commands (sync, no jobs)
    Quit,                              // Trigger shutdown
    Usage,                             // Display token usage
    Context,                           // Display context info
    Status,                            // Display worker status
    Workers,                           // List all workers
}

/// Handles command execution
pub trait CommandHandler: Send + Sync {
    async fn handle(
        &self,
        cmd: Command,
        worker: Arc<Worker>,
        session: Arc<Session>,
    ) -> Result<CommandResult>;
}

/// Result of command execution
pub enum CommandResult {
    TaskSpawned(Arc<WorkerJob>),       // Job was created
    Immediate(String),                 // Immediate result to display
    Shutdown,                          // Shutdown requested
}

/// Parse user input into command
pub struct CommandParser;

impl CommandParser {
    pub fn parse(input: &str) -> Result<Command, ParseError> {
        let trimmed = input.trim();
        
        if trimmed.starts_with('/') {
            // Explicit command
            let parts: Vec<&str> = trimmed[1..].splitn(2, ' ').collect();
            match parts[0] {
                "quit" => Ok(Command::Quit),
                "compact" => Ok(Command::Compact(parts.get(1).map(|s| s.to_string()))),
                "usage" => Ok(Command::Usage),
                "context" => Ok(Command::Context),
                "status" => Ok(Command::Status),
                "workers" => Ok(Command::Workers),
                _ => Err(ParseError::UnknownCommand(parts[0].to_string())),
            }
        } else {
            // Implicit /prompt command
            Ok(Command::Prompt(trimmed.to_string()))
        }
    }
}
```

**Implementation in UI**:
```rust
// In prompt_loop
let input = self.input_handler.read_line(&worker.name).await?;

// Parse command
let command = CommandParser::parse(&input)?;

// Handle command
let result = self.command_handler.handle(command, worker.clone(), self.session.clone()).await?;

match result {
    CommandResult::TaskSpawned(job) => {
        // Set up continuation to re-queue prompt
        let continuation = self.create_agent_completion_continuation();
        job.worker_job_continuations.add_or_run_now("re_prompt", continuation, worker).await;
    }
    CommandResult::Immediate(output) => {
        println!("{}", output);
        // Re-queue prompt immediately
        self.prompt_queue.enqueue(worker).await;
    }
    CommandResult::Shutdown => {
        self.shutdown_signal.notify_waiters();
    }
}
```

**Questions to Answer**:
1. How does ConversationCompactTask know when to trigger automatically?
2. Should commands have access to UI state (e.g., for /status)?
3. How do task-spawning commands pass parameters to tasks?
4. Should there be a help command that lists available commands?

**Recommendation**: Design this explicitly with clear interfaces before implementation.

**DM Response**: Look back to AgentEnvironment comments. This is going to be a required layer, that can support _some_ commands,
but the rest of commands could be specific to given UI implementation.
For example, Prompt, Compact, and Quit from your example have to be executed by AgentEnvironment (therefore passed back from ui.prompt()).
SimpleTextUi can implement Usage and Context - this is going to happen _inside_ of UI, not involving calling back to AgentEnvironment.
And then some AdvancedFancyUi can also implement a dozen other commands that do some fancy widget stuff.
_Some_ of the code will have to be shared between SimpleTextUi and AdvancedFancyUi (calculating topkens numbers for Usage), but the actual "display
the command result to the user" will be specific to the UI implementation.

**Response**: Excellent clarification. Revised command architecture:

```rust
// Commands that AgentEnvironment handles (task-spawning)
pub enum AgentEnvironmentCommand {
    Prompt { worker_id: Uuid, text: String },
    Compact { worker_id: Uuid, instruction: Option<String> },
    Quit,
}

// Commands that UI handles internally (display-only)
pub enum UiCommand {
    Usage,
    Context,
    Status,
    Workers,
    // AdvancedFancyUi can add more:
    // SplitPane, SwitchWorker, etc.
}

// Result from UI.prompt
pub enum PromptResult {
    AgentCommand(AgentEnvironmentCommand),  // Pass to AgentEnvironment
    Handled,                                 // UI handled internally, re-prompt
}

// Shared utilities for UI implementations
pub mod ui_utils {
    pub fn calculate_token_usage(worker: &Worker) -> TokenUsage {
        // Shared logic for calculating tokens
    }
    
    pub fn format_context_info(worker: &Worker) -> String {
        // Shared logic for formatting context
    }
}

// UI implementation
impl TextUi {
    pub async fn prompt(&self) -> Result<PromptResult, eyre::Error> {
        let input = self.input_handler.read_line("You").await?;
        let command = CommandParser::parse(&input)?;
        
        match command {
            // UI-specific commands - handle internally
            Command::Ui(UiCommand::Usage) => {
                let usage = ui_utils::calculate_token_usage(&self.worker);
                println!("Token usage: {}", usage);  // TextUi-specific display
                Ok(PromptResult::Handled)
            }
            Command::Ui(UiCommand::Context) => {
                let info = ui_utils::format_context_info(&self.worker);
                println!("{}", info);  // TextUi-specific display
                Ok(PromptResult::Handled)
            }
            
            // AgentEnvironment commands - pass through
            Command::Agent(agent_cmd) => {
                Ok(PromptResult::AgentCommand(agent_cmd))
            }
        }
    }
}

// AdvancedFancyUi can handle more commands
impl AdvancedFancyUi {
    pub async fn prompt(&self) -> Result<PromptResult, eyre::Error> {
        let command = self.widget_manager.get_command().await?;
        
        match command {
            Command::Ui(UiCommand::Usage) => {
                let usage = ui_utils::calculate_token_usage(&self.worker);
                self.widget_manager.display_usage_widget(usage);  // Fancy display
                Ok(PromptResult::Handled)
            }
            Command::Ui(UiCommand::SplitPane) => {
                self.widget_manager.split_pane();  // UI-specific feature
                Ok(PromptResult::Handled)
            }
            Command::Agent(agent_cmd) => {
                Ok(PromptResult::AgentCommand(agent_cmd))
            }
        }
    }
}
```

**Key principles**:
- **AgentEnvironment commands**: Task-spawning (Prompt, Compact, Quit)
- **UI commands**: Display-only (Usage, Context, Status)
- **Shared utilities**: Common logic (token calculation, formatting)
- **UI-specific display**: Each UI renders results differently
- **Extensible**: UIs can add their own commands

**Resolved** with clear separation of concerns.

---

## Potential Future Issues

### 9. Event Ordering with Multiple Workers

**Issue**: Multiple workers publishing events concurrently could cause UI display issues if events arrive out of order.

**Example**:
- Worker 1 publishes `ResponseChunkReceived` at T=100ms
- Worker 2 publishes `ResponseChunkReceived` at T=101ms
- Events arrive at UI in reverse order due to scheduling

**Mitigation**: 
- Include timestamps in events (hybrid design already does this)
- UI can reorder events if needed for display
- For most cases, slight reordering is acceptable

**DM Response**: Not a concern. 
Basic text UI will always only show the output from the 'main' worker. 
Structured embeddable UI will output accumulated chunks as JSON with worker ID, synchronized.
More adwanced UIs will implement panel switch or something like that. 
Then headless WebUI will not care about overlaps at all, as it will simply push them to different websockets.

**Response**: **Resolved** - different UI implementations handle multi-worker output differently.

---

### 10. Event Buffer Overflow

**Issue**: High-frequency events (streaming chunks) could overflow the broadcast channel buffer.

**Example**:
- LLM streams 100 chunks/second
- UI processes at 50 chunks/second
- Buffer (size 1000) fills in 10 seconds
- Events start getting dropped

**Mitigation**: 
- Tune buffer size appropriately (hybrid design uses 1000)
- Handle `RecvError::Lagged` gracefully (log warning, continue)
- Consider separate channels for high-frequency vs. low-frequency events
- UI should process events quickly (just update display state, don't block)

**DM Response**: This is a significant one, but can be addressed with proper choice of the buffer size, or
even letting user choose it through CLI arguments according to their needs (--events-buffer=10_000 if I expect
to spawn 10s of jobs in this session)

**Response**: **Resolved** - configurable buffer size addresses this concern.

---

### 11. Worker State Race Conditions

**Issue**: If both Session and Tasks set worker state without coordination, race conditions are possible.

**Example**:
- Session sets state to Inactive (job completed)
- Task sets state to Requesting (still running)
- Final state is inconsistent

**Mitigation**: Clear ownership rules (see Issue #3 recommendation)

**DM Response**: Same comment, those are different states, and this is why I suggested extra flags data on Worker.
Session and only Session can set top-level state on the worker itself; specific Task and only that specific task can 
set its specific state at the worker.
Example: AgentLoopTask can set custom state/flag "AgentLoopReceiving", and ConversationCompactTask can set custom 
state/flag "ConversationCompactReceiving"

**Response**: **Resolved** - clear separation: Session owns lifecycle state, Tasks own task-specific flags (see #3).

---

### 12. Prompt Queue Fairness

**Issue**: With multiple workers, how do you prioritize which worker gets prompted next?

**Current**: FIFO queue (first enqueued, first prompted)

**Alternatives**:
- **Priority-based**: Main worker always first
- **Round-robin**: Alternate between workers
- **User-selectable**: User chooses which worker to interact with
- **Smart**: Prompt worker that's been waiting longest

**Recommendation**: Start with FIFO, add priority/selection later if needed.

**DM Response**: This is what I want to delegate to specufic UI implementation:
Basic text UI will only show the prompt for the 'main' worker. 
Structured embeddable UI will accept JSON with with worker ID.
More adwanced UIs will show multiple input widgets or switch between them. 
Then headless WebUI, like structured, will receive POST or PUT requests with worker ID.

**Response**: **Resolved** - UI-specific concern, each implementation handles differently.

---

### 13. Tool Approval Blocking

**Issue**: If tool approval is synchronous and blocks task execution, it could cause deadlocks or poor UX.

**Example**:
- Worker 1 requests tool approval
- Worker 2 requests tool approval
- UI can only show one approval prompt at a time
- Worker 2 blocks indefinitely

**Mitigation**: 
- Tool approval should be async with cancellation support (hybrid design does this)
- UI should queue approval requests
- Provide timeout for approval requests
- Allow cancelling pending approvals

**DM Response**: My original vision was to provide some sort of interface to let a running task block itself for
user-waiting requests. But I shift towards the idea that Tasks must NOT handle such requests and just complete when they
can't continue without user input.
There are currently two scenarios like that: 
- AgentLoopTask received response from LLM with no tool use requests and considers the analyse complete -> exits with flag/state "completed_ready_for_new_prompt" 
- AgentLoopTask received response from LLM with tool use requests that can not be auto-approved -> exits with flag/state "completed_with_tool_request"
With those flags (and data in worker's ConversationHistory) UI implementation can show proper prompt request or widget.
This approach means Jobs always run end-to-end with no need to call back to UI to obtain more data.

**Response**: This is a significant design shift. **Implications**:

**Pros**:
- Jobs are simpler - no blocking on user input
- No deadlock concerns
- Clear job lifecycle (start → run → complete)
- UI can inspect completion state and decide what to show

**Cons**:
- Tool approval becomes a multi-step process:
  1. Job completes with "completed_with_tool_request" flag
  2. UI shows tool approval prompt
  3. User approves/denies
  4. UI updates Worker's conversation history with approval
  5. UI spawns new job to continue
- More complex state management in Worker
- Conversation history needs to track pending tool approvals

**Recommendation**: This works but requires careful design of:
1. How tool approval state is stored in Worker
2. How UI knows which tools are pending approval
3. How to resume execution after approval

Example:
```rust
// Worker stores pending tool requests
pub struct Worker {
    pub pending_tool_requests: Arc<Mutex<Vec<ToolRequest>>>,
    pub task_metadata: HashMap<String, serde_json::Value>,
}

// AgentLoop completes with pending tools
impl AgentLoop {
    async fn run(&self) -> Result<(), eyre::Error> {
        let response = self.query_llm().await?;
        
        if !response.tool_requests.is_empty() {
            // Store pending tools in worker
            *self.worker.pending_tool_requests.lock().unwrap() = response.tool_requests;
            
            // Set completion flag
            self.worker.set_task_metadata("completion_state", "completed_with_tool_request");
            
            // Exit - UI will handle approval
            return Ok(());
        }
        
        // Normal completion
        self.worker.set_task_metadata("completion_state", "completed_ready_for_prompt");
        Ok(())
    }
}

// UI handles tool approval
impl TextUi {
    async fn handle_job_completion(&self, worker: Arc<Worker>) {
        let completion_state = worker.get_task_metadata("completion_state");
        
        match completion_state.as_deref() {
            Some("completed_with_tool_request") => {
                // Show tool approval UI
                let pending_tools = worker.pending_tool_requests.lock().unwrap();
                for tool in pending_tools.iter() {
                    let approved = self.prompt_tool_approval(tool).await?;
                    if approved {
                        // Add approval to conversation history
                        worker.context_container.add_tool_approval(tool);
                    } else {
                        // Add denial to conversation history
                        worker.context_container.add_tool_denial(tool);
                    }
                }
                
                // Spawn new job to continue
                self.session.run_agent_loop(worker, AgentLoopInput {})?;
            }
            Some("completed_ready_for_prompt") => {
                // Normal completion - show prompt
                self.prompt_queue.enqueue(worker).await;
            }
            _ => {}
        }
    }
}
```

**Resolved** with design implications noted.

---

### 14. Error Recovery Strategy

**Issue**: How does UI know what recovery options to offer when a task fails?

**Example**:
- Task fails with "Rate limit exceeded"
- Should UI offer: Retry? Retry with different model? Skip? Abort?

**Mitigation**: 
- ErrorHandler interface provides recovery decisions (hybrid design has this)
- Tasks can provide context about error type
- UI can implement smart recovery based on error patterns

**DM Response**: UI has only one option here - show failure message and ask for new prompt
Task itself (or ModelProvider) will handle API retries.

**Response**: **Resolved** - simplified error handling. Tasks/ModelProvider handle retries internally, UI just displays errors.

---

### 15. Shutdown Coordination

**Issue**: Multiple workers with active jobs - how to ensure clean shutdown?

**Example**:
- User presses Ctrl+C
- Worker 1 is streaming response
- Worker 2 is executing tool
- How to cancel both gracefully?

**Mitigation**:
- ShutdownCoordinator interface (hybrid design has this)
- Session.cancel_all_jobs() method
- Session.wait_for_all_jobs() method
- Cancellation tokens for all jobs

**DM Response**: Correct. Current implementation uses cancellation tokens extensively. But in the end
it's going to be running tasks' responsibility to react on it and stop gracefully.
AgentEnvironment can impose hard timeout (1s, for example) for active tasks to close and then just hard 
stop the application.

**Response**: **Resolved** - cancellation tokens + hard timeout is a good approach.

---

### 16. Context Window Management

**Issue**: ConversationCompactTask needs to know when to trigger automatically.

**Triggers**:
- Context approaching token limit
- User explicitly requests compaction
- After N turns of conversation

**Recommendation**: 
- Worker tracks token count
- Publishes event when approaching limit
- UI or Session can trigger compact task automatically
- User can also trigger with /compact command

**DM Response**: No, automatic compact is a part of responsibility of AgentLoopTask.
Actual implememtation can share some code with ConversationCompactTask implementation,
but the _Tasks_ are separate and must not depend on each other.

**Response**: **Resolved** - AgentLoopTask handles automatic compaction internally. ConversationCompactTask is for explicit user-requested compaction.

---

## Recommended Architecture (Revised)

Based on resolved discussions, here's the updated architecture:

```
Entry Point (ChatArgs::execute):
├─ Creates EventBus
├─ Creates Session (with EventBus)
├─ Creates main Worker
├─ Creates UI implementation(s)
│  ├─ Main UI (TextUi, AdvancedFancyUi, or None for headless)
│  └─ Headless UIs (WebApi, StructuredIO, etc.)
├─ Creates AgentEnvironment (with EventBus, Session, UIs)
└─ Runs AgentEnvironment.run() (blocks until shutdown)

AgentEnvironment.run():
├─ Spawns event multicast task
│  └─ Broadcasts events to all headless UIs
├─ If main UI exists:
│  └─ Runs main UI loop (blocks)
├─ Else (headless mode):
│  └─ Waits for shutdown signal
└─ Cleanup on shutdown

Main UI Loop (e.g., TextUi):
├─ Spawns event subscriber task
│  ├─ Subscribes to EventBus
│  ├─ Filters events for main worker
│  └─ Updates display state
├─ Runs prompt loop (main thread)
│  ├─ Calls ui.prompt() (blocks for user input)
│  ├─ Parses command
│  ├─ Handles UI-specific commands internally (Usage, Context)
│  ├─ Returns AgentEnvironment commands (Prompt, Compact, Quit)
│  └─ AgentEnvironment spawns tasks via Session
└─ Cleanup on shutdown

Headless UI (e.g., WebApi):
├─ Subscribes to EventBus
├─ Pushes events to websockets
├─ Receives commands via REST API
└─ Returns commands to AgentEnvironment
```

**Key Principles** (Revised):
- **AgentEnvironment coordinates** - Owns top-level loop, multicasts events, spawns tasks
- **Nested event structure** - `AgentEnvironmentEvent(WorkerEvent(...))` for clear categorization
- **Clear state separation** - Session owns lifecycle state, Tasks own task-specific flags
- **Serializable Worker** - `task_metadata: HashMap<String, serde_json::Value>` for extensibility
- **UI.prompt returns commands** - AgentEnvironment decides which tasks to spawn
- **Two-tier command system** - AgentEnvironment commands vs. UI-specific commands
- **Jobs complete without blocking** - Tasks exit with flags when they need user input
- **Shared UI utilities** - Common logic (token calculation) with UI-specific display

**Architecture Layers**:

1. **Entry Point** - Setup and initialization
2. **AgentEnvironment** - Event coordination, task orchestration, multi-UI support
3. **Session** - Worker/job lifecycle management
4. **UI Implementations** - Display and user interaction (pluggable)
5. **Tasks** - Executable work units (AgentLoop, ConversationCompact, etc.)
6. **Workers** - State and context containers

---

## Implementation Checklist

### Phase 1: Core Infrastructure
- [ ] Create flat `AgentEvent` enum with all event types
- [ ] Implement `EventBus` with broadcast channel
- [ ] Add helper methods to `AgentEvent` (worker_id, is_worker_event, etc.)
- [ ] Add EventBus to Session constructor
- [ ] Add EventBus to Worker constructor
- [ ] Update Worker::set_state() to publish events

### Phase 2: Task Updates
- [ ] Add EventBus parameter to AgentLoop constructor (store in struct)
- [ ] Update AgentLoop to publish events for state changes
- [ ] Update AgentLoop to publish events for streaming chunks
- [ ] Remove WorkerToHostInterface parameter from AgentLoop
- [ ] Add tool_approver and error_handler parameters to AgentLoop

### Phase 3: UI Refactoring
- [ ] Add event_loop() method to UI (subscribes to EventBus)
- [ ] Update prompt_loop() to use command system
- [ ] Implement CommandParser
- [ ] Implement CommandHandler
- [ ] Remove event forwarding (if any)
- [ ] Update entry point to remove AgentEnvironment

### Phase 4: Interface Implementations
- [ ] Implement PromptProvider for UI
- [ ] Implement ToolApprover for UI
- [ ] Implement ErrorHandler for UI
- [ ] Implement ShutdownCoordinator for UI

### Phase 5: Testing & Documentation
- [ ] Test single worker with events
- [ ] Test multiple workers with events
- [ ] Test command system
- [ ] Test shutdown coordination
- [ ] Update architecture documentation
- [ ] Add code examples

---

## Conclusion (Revised)

After discussion and resolution of design issues, the architecture is clarified:

**Confirmed Design Decisions**:
- **AgentEnvironment** - Required layer for multi-UI coordination and event multicasting
- **Nested event structure** - `AgentEnvironmentEvent(WorkerEvent(...))` for clear categorization
- **Serializable Worker** - `task_metadata: HashMap<String, serde_json::Value>` for extensibility
- **Two-tier command system** - AgentEnvironment commands (task-spawning) vs. UI commands (display-only)
- **Jobs complete without blocking** - Tasks exit with flags when they need user input
- **EventBus in task struct** - Stored during construction, not passed to run()

**Key Architecture Components**:

1. **AgentEnvironment**:
   - Coordinates between main UI and headless UIs
   - Multicasts events to all subscribers
   - Owns task spawning logic
   - Supports headless mode

2. **Event System**:
   - Nested enum structure for clear categorization
   - Broadcast channel for efficient multicast
   - Configurable buffer size

3. **Worker State**:
   - Lifecycle state (Idle/Busy/IdleFailed) - managed by Session
   - Task-specific metadata - managed by Tasks
   - Fully serializable for persistence

4. **Command System**:
   - AgentEnvironment commands: Prompt, Compact, Quit
   - UI-specific commands: Usage, Context, Status, etc.
   - Shared utilities for common logic
   - UI-specific display implementations

5. **UI Implementations**:
   - Main UI (TextUi, AdvancedFancyUi) - interactive
   - Headless UIs (WebApi, StructuredIO) - programmatic
   - Pluggable and extensible
   - Each handles events and commands differently

**Next Steps**:
1. Implement core EventBus with nested enum structure
2. Implement AgentEnvironment coordinator
3. Update Worker with serializable task_metadata
4. Implement two-tier command system
5. Update Tasks to exit with completion flags
6. Implement TextUi with new architecture
7. Test multi-UI scenarios

The architecture now supports the vision of multiple UI implementations while maintaining clean separation of concerns.
