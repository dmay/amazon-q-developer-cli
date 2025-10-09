# Agent Environment Architecture Design v2
## Hybrid Event Interface with Multi-UI Support

**Date**: 2025-10-09  
**Status**: Design Specification  
**Based On**: 
- Current implementation in `crates/chat-cli/src/agent_env/`
- Analysis in `hybrid-event-interface-2-redesign-challenges.md`
- Resolved design discussions

---

## Executive Summary

This document specifies the architecture for the Agent Environment system, which enables multiple AI agents to run in parallel with flexible UI implementations. The design supports:

- **Multiple concurrent workers** executing tasks independently
- **Pluggable UI implementations** (text-based, structured I/O, web API, headless)
- **Event-driven communication** via centralized EventBus
- **Clean separation of concerns** between orchestration, execution, and presentation
- **Extensible task system** for different agent behaviors

### Key Design Principles

1. **Non-blocking execution**: All components operate asynchronously without blocking each other
2. **Event-driven coordination**: Components communicate via events, not direct calls
3. **UI independence**: Core logic has no knowledge of UI implementations
4. **Serializable state**: Workers can be persisted and restored
5. **Type safety**: Strong typing with minimal runtime errors

---

## Architecture Overview

### Component Hierarchy

```
Entry Point (ChatArgs::execute)
├─ EventBus (broadcast channel)
├─ Session (worker/job lifecycle manager)
│  ├─ Workers (state containers)
│  └─ Jobs (running tasks)
├─ UI Implementations
│  ├─ Main UI (TextUi, AdvancedFancyUi, or None)
│  └─ Headless UIs (WebApi, StructuredIO, etc.)
└─ AgentEnvironment (coordinator)
   ├─ Event multicast task
   ├─ Main UI loop (if present)
   └─ Command processing loop
```

### Code Organization

```
crates/chat-cli/src/
├─ agent_env/                           # Core agent environment
│  ├─ mod.rs                           # Module exports
│  ├─ event_bus.rs                     # NEW: EventBus implementation
│  ├─ events.rs                        # NEW: Event type definitions
│  ├─ session.rs                       # Session orchestrator (UPDATED)
│  ├─ worker.rs                        # Worker state container (UPDATED)
│  ├─ worker_job.rs                    # Job execution (UPDATED)
│  ├─ worker_job_continuations.rs     # Job completion callbacks
│  ├─ worker_task.rs                   # WorkerTask trait
│  ├─ agent_environment.rs             # NEW: AgentEnvironment coordinator
│  ├─ commands.rs                      # NEW: Command system
│  ├─ context_container/               # Context management
│  │  ├─ context_container.rs
│  │  ├─ conversation_history.rs
│  │  └─ conversation_entry.rs
│  ├─ model_providers/                 # LLM abstractions
│  │  ├─ model_provider.rs
│  │  └─ bedrock_converse_stream.rs
│  └─ worker_tasks/                    # Task implementations
│     ├─ agent_loop.rs                # Main agent loop (UPDATED)
│     └─ conversation_compact.rs      # NEW: Conversation compaction
│
└─ cli/chat/
   ├─ mod.rs                           # Entry point (UPDATED)
   └─ agent_env_ui/                    # UI implementations
      ├─ mod.rs                        # NEW: UI trait definitions
      ├─ text_ui.rs                    # NEW: Basic text UI
      ├─ ui_utils.rs                   # NEW: Shared UI utilities
      ├─ input_handler.rs              # Input handling
      └─ ctrl_c_handler.rs             # Signal handling
```

---

## Event System Design

### Event Structure

Events use a **nested enum structure** for clear categorization and filtering:

```rust
/// Top-level event envelope
#[derive(Debug, Clone)]
pub enum AgentEnvironmentEvent {
    Worker(WorkerEvent),
    Job(JobEvent),
    AgentLoop(AgentLoopEvent),
    System(SystemEvent),
}

/// Worker lifecycle and state events
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    Created {
        worker_id: Uuid,
        name: String,
        timestamp: Instant,
    },
    Deleted {
        worker_id: Uuid,
        timestamp: Instant,
    },
    LifecycleStateChanged {
        worker_id: Uuid,
        old_state: WorkerLifecycleState,
        new_state: WorkerLifecycleState,
        timestamp: Instant,
    },
}

/// Job execution events
#[derive(Debug, Clone)]
pub enum JobEvent {
    Started {
        worker_id: Uuid,
        job_id: Uuid,
        task_type: String,
        timestamp: Instant,
    },
    Completed {
        worker_id: Uuid,
        job_id: Uuid,
        result: JobCompletionResult,
        timestamp: Instant,
    },
    OutputChunk {
        worker_id: Uuid,
        job_id: Uuid,
        chunk: OutputChunk,
        timestamp: Instant,
    },
}

/// AgentLoop-specific events
#[derive(Debug, Clone)]
pub enum AgentLoopEvent {
    ResponseReceived {
        worker_id: Uuid,
        job_id: Uuid,
        text: String,
        timestamp: Instant,
    },
    ToolUseRequestReceived {
        worker_id: Uuid,
        job_id: Uuid,
        tool_name: String,
        tool_input: serde_json::Value,
        timestamp: Instant,
    },
}

/// System-level events
#[derive(Debug, Clone)]
pub enum SystemEvent {
    ShutdownInitiated {
        reason: String,
        timestamp: Instant,
    },
}

/// Worker lifecycle states (managed by Session)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerLifecycleState {
    Idle,
    Busy,
    IdleFailed,
}

/// Job completion results
#[derive(Debug, Clone)]
pub enum JobCompletionResult {
    Success {
        task_metadata: HashMap<String, serde_json::Value>,
    },
    Cancelled,
    Failed {
        error: String,
    },
}

/// Output chunk types
#[derive(Debug, Clone)]
pub enum OutputChunk {
    AssistantResponse(String),
    ToolUse {
        tool_name: String,
        tool_input: serde_json::Value,
    },
    ToolResult {
        tool_name: String,
        result: String,
    },
}
```

### Event Helper Methods

```rust
impl AgentEnvironmentEvent {
    /// Extract worker_id from any event that has one
    pub fn worker_id(&self) -> Option<Uuid> {
        match self {
            Self::Worker(WorkerEvent::Created { worker_id, .. }) => Some(*worker_id),
            Self::Worker(WorkerEvent::Deleted { worker_id, .. }) => Some(*worker_id),
            Self::Worker(WorkerEvent::LifecycleStateChanged { worker_id, .. }) => Some(*worker_id),
            Self::Job(JobEvent::Started { worker_id, .. }) => Some(*worker_id),
            Self::Job(JobEvent::Completed { worker_id, .. }) => Some(*worker_id),
            Self::Job(JobEvent::OutputChunk { worker_id, .. }) => Some(*worker_id),
            Self::AgentLoop(AgentLoopEvent::ResponseReceived { worker_id, .. }) => Some(*worker_id),
            Self::AgentLoop(AgentLoopEvent::ToolUseRequestReceived { worker_id, .. }) => Some(*worker_id),
            Self::System(_) => None,
        }
    }
    
    /// Check if this is a worker-related event
    pub fn is_worker_event(&self) -> bool {
        matches!(self, Self::Worker(_))
    }
    
    /// Check if this is a job-related event
    pub fn is_job_event(&self) -> bool {
        matches!(self, Self::Job(_))
    }
    
    /// Check if this is an agent loop event
    pub fn is_agent_loop_event(&self) -> bool {
        matches!(self, Self::AgentLoop(_))
    }
    
    /// Check if this is a system event
    pub fn is_system_event(&self) -> bool {
        matches!(self, Self::System(_))
    }
    
    /// Get timestamp from any event
    pub fn timestamp(&self) -> Instant {
        match self {
            Self::Worker(WorkerEvent::Created { timestamp, .. }) => *timestamp,
            Self::Worker(WorkerEvent::Deleted { timestamp, .. }) => *timestamp,
            Self::Worker(WorkerEvent::LifecycleStateChanged { timestamp, .. }) => *timestamp,
            Self::Job(JobEvent::Started { timestamp, .. }) => *timestamp,
            Self::Job(JobEvent::Completed { timestamp, .. }) => *timestamp,
            Self::Job(JobEvent::OutputChunk { timestamp, .. }) => *timestamp,
            Self::AgentLoop(AgentLoopEvent::ResponseReceived { timestamp, .. }) => *timestamp,
            Self::AgentLoop(AgentLoopEvent::ToolUseRequestReceived { timestamp, .. }) => *timestamp,
            Self::System(SystemEvent::ShutdownInitiated { timestamp, .. }) => *timestamp,
        }
    }
}
```

### EventBus Implementation

```rust
use tokio::sync::broadcast;
use std::sync::Arc;

/// Central event distribution system
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEnvironmentEvent>,
    buffer_size: usize,
}

impl EventBus {
    /// Create new EventBus with specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self { sender, buffer_size }
    }
    
    /// Publish event to all subscribers
    pub fn publish(&self, event: AgentEnvironmentEvent) {
        // Ignore send errors (no subscribers is OK)
        let _ = self.sender.send(event);
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEnvironmentEvent> {
        self.sender.subscribe()
    }
    
    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000) // Default buffer size
    }
}
```

### Event Filtering Patterns

```rust
// Filter by worker ID
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    if let Some(wid) = event.worker_id() {
        if wid == target_worker_id {
            handle_event(event).await;
        }
    }
}

// Filter by event type
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    match event {
        AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
            print!("{}", chunk);
        }
        AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { .. }) => {
            update_status_display();
        }
        _ => {}
    }
}

// Handle lagged events
let mut receiver = event_bus.subscribe();
loop {
    match receiver.recv().await {
        Ok(event) => handle_event(event).await,
        Err(broadcast::error::RecvError::Lagged(n)) => {
            eprintln!("Warning: Event bus lagged by {} events", n);
        }
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```

---

## Core Components

### Worker

**Purpose**: Container for agent state, context, and configuration. Fully serializable for persistence.

**Location**: `crates/chat-cli/src/agent_env/worker.rs`

```rust
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    
    /// Lifecycle state (managed by Session)
    pub lifecycle_state: WorkerLifecycleState,
    
    /// Task-specific metadata (managed by Tasks)
    /// Extensible storage for task-specific flags and state
    pub task_metadata: HashMap<String, serde_json::Value>,
    
    /// Conversation context
    pub context_container: ContextContainer,
    
    /// Non-serializable runtime dependencies
    #[serde(skip)]
    pub model_provider: Option<Arc<dyn ModelProvider>>,
}

impl Worker {
    pub fn new(id: Uuid, name: String, model_provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            id,
            name,
            lifecycle_state: WorkerLifecycleState::Idle,
            task_metadata: HashMap::new(),
            context_container: ContextContainer::new(),
            model_provider: Some(model_provider),
        }
    }
    
    /// Type-safe metadata access helpers
    pub fn set_task_metadata(&mut self, key: &str, value: serde_json::Value) {
        self.task_metadata.insert(key.to_string(), value);
    }
    
    pub fn get_task_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.task_metadata.get(key)
    }
    
    pub fn get_task_metadata_string(&self, key: &str) -> Option<String> {
        self.task_metadata
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Namespaced metadata keys to avoid conflicts
pub mod task_metadata_keys {
    pub const AGENT_LOOP_COMPLETION_STATE: &str = "agent_loop.completion_state";
    pub const AGENT_LOOP_LAST_TOOL: &str = "agent_loop.last_tool";
    pub const COMPACT_LAST_RUN: &str = "compact.last_run_timestamp";
}
```

**Key Responsibilities**:
- Store conversation history and context
- Maintain lifecycle state (Idle/Busy/IdleFailed)
- Provide extensible task metadata storage
- Support serialization for persistence

**State Management**:
- **Lifecycle state**: Managed exclusively by Session
- **Task metadata**: Managed by individual Tasks
- **Context**: Managed by Tasks (add messages, update history)

---

### Session

**Purpose**: Central orchestrator for worker and job lifecycle management. Owns EventBus and coordinates all execution.

**Location**: `crates/chat-cli/src/agent_env/session.rs`

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use eyre::Result;

pub struct Session {
    /// Event distribution system
    event_bus: EventBus,
    
    /// All workers managed by this session
    workers: Arc<Mutex<HashMap<Uuid, Arc<Worker>>>>,
    
    /// All active jobs
    jobs: Arc<Mutex<HashMap<Uuid, Arc<WorkerJob>>>>,
    
    /// Model providers available for workers
    model_providers: Vec<Arc<dyn ModelProvider>>,
}

impl Session {
    pub fn new(event_bus: EventBus, model_providers: Vec<Arc<dyn ModelProvider>>) -> Self {
        Self {
            event_bus,
            workers: Arc::new(Mutex::new(HashMap::new())),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            model_providers,
        }
    }
    
    /// Create new worker
    pub fn build_worker(&self, name: String) -> Arc<Worker> {
        let worker_id = Uuid::new_v4();
        let model_provider = self.model_providers[0].clone(); // TODO: selection logic
        
        let worker = Arc::new(Worker::new(worker_id, name.clone(), model_provider));
        
        // Register worker
        self.workers.lock().unwrap().insert(worker_id, worker.clone());
        
        // Publish event
        self.event_bus.publish(AgentEnvironmentEvent::Worker(
            WorkerEvent::Created {
                worker_id,
                name,
                timestamp: Instant::now(),
            }
        ));
        
        worker
    }
    
    /// Delete worker
    pub fn delete_worker(&self, worker_id: Uuid) -> Result<()> {
        // Cancel any active jobs for this worker
        self.cancel_worker_jobs(worker_id)?;
        
        // Remove worker
        self.workers.lock().unwrap().remove(&worker_id);
        
        // Publish event
        self.event_bus.publish(AgentEnvironmentEvent::Worker(
            WorkerEvent::Deleted {
                worker_id,
                timestamp: Instant::now(),
            }
        ));
        
        Ok(())
    }
    
    /// Get worker by ID
    pub fn get_worker(&self, worker_id: Uuid) -> Option<Arc<Worker>> {
        self.workers.lock().unwrap().get(&worker_id).cloned()
    }
    
    /// Launch agent loop task
    pub fn run_task__agent_loop(
        &self,
        worker: Arc<Worker>,
        input: AgentLoopInput,
    ) -> Result<Arc<WorkerJob>> {
        // Set worker to Busy
        self.set_worker_lifecycle_state(
            worker.id,
            WorkerLifecycleState::Busy,
        );
        
        // Create task
        let task = AgentLoop::new(
            worker.clone(),
            input,
            self.event_bus.clone(),
        );
        
        // Create and launch job
        let job = WorkerJob::new(
            worker.clone(),
            Box::new(task),
            self.event_bus.clone(),
        );
        
        let job = Arc::new(job);
        let job_id = job.id;
        
        // Register job
        self.jobs.lock().unwrap().insert(job_id, job.clone());
        
        // Publish event
        self.event_bus.publish(AgentEnvironmentEvent::Job(
            JobEvent::Started {
                worker_id: worker.id,
                job_id,
                task_type: "AgentLoop".to_string(),
                timestamp: Instant::now(),
            }
        ));
        
        // Spawn job execution
        let session = Arc::new(self.clone());
        tokio::spawn(async move {
            let result = job.run().await;
            session.handle_job_completion(job.clone(), result).await;
        });
        
        Ok(job)
    }
    
    /// Launch conversation compact task
    pub fn run_task__compact_conversation(
        &self,
        worker: Arc<Worker>,
        input: CompactInput,
    ) -> Result<Arc<WorkerJob>> {
        // Set worker to Busy
        self.set_worker_lifecycle_state(
            worker.id,
            WorkerLifecycleState::Busy,
        );
        
        // Create task
        let task = ConversationCompact::new(
            worker.clone(),
            input,
            self.event_bus.clone(),
        );
        
        // Create and launch job
        let job = WorkerJob::new(
            worker.clone(),
            Box::new(task),
            self.event_bus.clone(),
        );
        
        let job = Arc::new(job);
        let job_id = job.id;
        
        // Register job
        self.jobs.lock().unwrap().insert(job_id, job.clone());
        
        // Publish event
        self.event_bus.publish(AgentEnvironmentEvent::Job(
            JobEvent::Started {
                worker_id: worker.id,
                job_id,
                task_type: "ConversationCompact".to_string(),
                timestamp: Instant::now(),
            }
        ));
        
        // Spawn job execution
        let session = Arc::new(self.clone());
        tokio::spawn(async move {
            let result = job.run().await;
            session.handle_job_completion(job.clone(), result).await;
        });
        
        Ok(job)
    }
    
    /// Handle job completion
    async fn handle_job_completion(
        &self,
        job: Arc<WorkerJob>,
        result: Result<()>,
    ) {
        let worker_id = job.worker.id;
        let job_id = job.id;
        
        // Determine completion result
        let completion_result = match result {
            Ok(_) => {
                // Extract task metadata from worker
                let task_metadata = job.worker.task_metadata.clone();
                JobCompletionResult::Success { task_metadata }
            }
            Err(e) => JobCompletionResult::Failed {
                error: e.to_string(),
            },
        };
        
        // Update worker lifecycle state
        let new_state = match &completion_result {
            JobCompletionResult::Success { .. } => WorkerLifecycleState::Idle,
            JobCompletionResult::Failed { .. } => WorkerLifecycleState::IdleFailed,
            JobCompletionResult::Cancelled => WorkerLifecycleState::Idle,
        };
        
        self.set_worker_lifecycle_state(worker_id, new_state);
        
        // Remove job from active jobs
        self.jobs.lock().unwrap().remove(&job_id);
        
        // Publish completion event
        self.event_bus.publish(AgentEnvironmentEvent::Job(
            JobEvent::Completed {
                worker_id,
                job_id,
                result: completion_result,
                timestamp: Instant::now(),
            }
        ));
        
        // Run continuations
        job.run_continuations().await;
    }
    
    /// Set worker lifecycle state and publish event
    fn set_worker_lifecycle_state(
        &self,
        worker_id: Uuid,
        new_state: WorkerLifecycleState,
    ) {
        if let Some(worker) = self.get_worker(worker_id) {
            let old_state = worker.lifecycle_state;
            
            // Update state (requires interior mutability)
            // TODO: Add Arc<Mutex<WorkerLifecycleState>> to Worker
            
            // Publish event
            self.event_bus.publish(AgentEnvironmentEvent::Worker(
                WorkerEvent::LifecycleStateChanged {
                    worker_id,
                    old_state,
                    new_state,
                    timestamp: Instant::now(),
                }
            ));
        }
    }
    
    /// Cancel all jobs for a worker
    pub fn cancel_worker_jobs(&self, worker_id: Uuid) -> Result<()> {
        let jobs: Vec<Arc<WorkerJob>> = self.jobs
            .lock()
            .unwrap()
            .values()
            .filter(|job| job.worker.id == worker_id)
            .cloned()
            .collect();
        
        for job in jobs {
            job.cancel();
        }
        
        Ok(())
    }
    
    /// Cancel all jobs
    pub fn cancel_all_jobs(&self) {
        let jobs: Vec<Arc<WorkerJob>> = self.jobs
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        
        for job in jobs {
            job.cancel();
        }
    }
}
```

**Key Responsibilities**:
- Create and manage workers
- Launch and track jobs
- Manage worker lifecycle state transitions
- Publish lifecycle events to EventBus
- Coordinate job completion and cleanup

---

### WorkerJob

**Purpose**: Running instance of a task with lifecycle management and cancellation support.

**Location**: `crates/chat-cli/src/agent_env/worker_job.rs`

```rust
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct WorkerJob {
    pub id: Uuid,
    pub worker: Arc<Worker>,
    pub task: Box<dyn WorkerTask>,
    pub event_bus: EventBus,
    pub cancellation_token: CancellationToken,
    pub continuations: Arc<WorkerJobContinuations>,
}

impl WorkerJob {
    pub fn new(
        worker: Arc<Worker>,
        task: Box<dyn WorkerTask>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            worker,
            task,
            event_bus,
            cancellation_token: CancellationToken::new(),
            continuations: Arc::new(WorkerJobContinuations::new()),
        }
    }
    
    /// Execute the task
    pub async fn run(&self) -> Result<()> {
        // Run task with cancellation support
        tokio::select! {
            result = self.task.run() => result,
            _ = self.cancellation_token.cancelled() => {
                Err(eyre::eyre!("Job cancelled"))
            }
        }
    }
    
    /// Cancel this job
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
    
    /// Run all registered continuations
    pub async fn run_continuations(&self) {
        self.continuations.run_all(self.worker.clone()).await;
    }
}
```

**Key Responsibilities**:
- Execute task with cancellation support
- Manage task lifecycle
- Run completion continuations
- Provide cancellation mechanism

---

### WorkerTask Trait

**Purpose**: Interface for executable work units (agent loops, commands, etc.)

**Location**: `crates/chat-cli/src/agent_env/worker_task.rs`

```rust
use async_trait::async_trait;
use eyre::Result;

#[async_trait]
pub trait WorkerTask: Send + Sync {
    /// Get task type name for logging/events
    fn task_type(&self) -> &str;
    
    /// Execute the task
    async fn run(&self) -> Result<()>;
}
```

**Key Responsibilities**:
- Define task execution interface
- Support async execution
- Provide task type identification

**Implementation Notes**:
- Tasks store EventBus in their struct (passed during construction)
- Tasks publish events throughout their lifecycle
- Tasks update Worker's task_metadata as needed
- Tasks should NOT block on user input - exit with completion flags instead

---
## AgentEnvironment Coordinator

**Purpose**: Top-level coordinator that manages event multicasting, UI coordination, and command processing. Supports multiple concurrent UIs (one main interactive UI + multiple headless UIs).

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

### Architecture

```
AgentEnvironment
├─ Event Multicast Task (always running)
│  └─ Broadcasts events to all UIs
├─ Main UI (optional, interactive)
│  ├─ Event processor task
│  ├─ Prompt loop task
│  └─ Command channel
└─ Headless UIs (multiple, non-interactive)
   └─ Event processor tasks
```

### Implementation

```rust
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use std::sync::Arc;

pub struct AgentEnvironment {
    session: Arc<Session>,
    event_bus: EventBus,
    main_ui: Option<Arc<dyn UserInterface>>,
    headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    shutdown_signal: Arc<Notify>,
}

impl AgentEnvironment {
    pub fn new(
        session: Arc<Session>,
        event_bus: EventBus,
        main_ui: Option<Arc<dyn UserInterface>>,
        headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    ) -> Self {
        Self {
            session,
            event_bus,
            main_ui,
            headless_uis,
            shutdown_signal: Arc::new(Notify::new()),
        }
    }
    
    /// Main execution loop
    pub async fn run(&self) -> Result<()> {
        // Spawn event multicast task (always running)
        let multicast_handle = self.spawn_event_multicast();
        
        // Run main UI if present
        if let Some(ui) = &self.main_ui {
            // Start UI (spawns its own tasks, returns immediately)
            ui.start().await?;
            
            // Get command receiver from UI
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
                            PromptResult::Shutdown => {
                                tracing::info!("Shutdown requested by UI");
                                break;
                            }
                        }
                    }
                    
                    // Handle shutdown signal
                    _ = self.shutdown_signal.notified() => {
                        tracing::info!("Shutdown signal received");
                        break;
                    }
                }
            }
        } else {
            // Headless mode - just wait for shutdown
            tracing::info!("Running in headless mode");
            self.shutdown_signal.notified().await;
        }
        
        // Cleanup
        tracing::info!("Shutting down AgentEnvironment");
        multicast_handle.abort();
        self.session.cancel_all_jobs();
        
        Ok(())
    }
    
    /// Spawn event multicast task
    fn spawn_event_multicast(&self) -> JoinHandle<()> {
        let mut receiver = self.event_bus.subscribe();
        let headless_uis = self.headless_uis.clone();
        let main_ui = self.main_ui.clone();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(event) = receiver.recv() => {
                        // Forward to main UI
                        if let Some(ui) = &main_ui {
                            ui.handle_event(event.clone()).await;
                        }
                        
                        // Forward to headless UIs
                        for headless_ui in &headless_uis {
                            headless_ui.handle_event(event.clone()).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Event bus lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Event bus closed");
                        break;
                    }
                    _ = shutdown.notified() => {
                        tracing::info!("Event multicast shutting down");
                        break;
                    }
                }
            }
        })
    }
    
    /// Handle command from UI
    async fn handle_command(&self, cmd: AgentEnvironmentCommand) -> Result<()> {
        match cmd {
            AgentEnvironmentCommand::Prompt { worker_id, text } => {
                let worker = self.session.get_worker(worker_id)
                    .ok_or_else(|| eyre::eyre!("Worker not found: {}", worker_id))?;
                
                // Add message to conversation history
                worker.context_container
                    .conversation_history
                    .lock()
                    .unwrap()
                    .push_input_message(text);
                
                // Launch agent loop
                self.session.run_task__agent_loop(worker, AgentLoopInput {})?;
            }
            
            AgentEnvironmentCommand::Compact { worker_id, instruction } => {
                let worker = self.session.get_worker(worker_id)
                    .ok_or_else(|| eyre::eyre!("Worker not found: {}", worker_id))?;
                
                // Launch compact task
                self.session.run_task__compact_conversation(worker, CompactInput { instruction })?;
            }
            
            AgentEnvironmentCommand::Quit => {
                self.shutdown_signal.notify_waiters();
            }
        }
        
        Ok(())
    }
    
    /// Trigger shutdown
    pub fn shutdown(&self) {
        self.shutdown_signal.notify_waiters();
    }
}
```

### Key Responsibilities

1. **Event Multicasting**:
   - Subscribe to EventBus
   - Forward events to all registered UIs (main + headless)
   - Handle lagged events gracefully
   - Never block event processing

2. **UI Coordination**:
   - Start main UI (if present)
   - Receive commands from UI via channel
   - Support headless mode (no main UI)
   - Coordinate multiple headless UIs

3. **Command Processing**:
   - Receive commands from UI
   - Spawn appropriate tasks via Session
   - Handle shutdown requests
   - Non-blocking command processing

4. **Lifecycle Management**:
   - Coordinate startup
   - Handle shutdown signals
   - Cancel all jobs on shutdown
   - Clean resource cleanup

### Design Benefits

- **Non-blocking**: Event multicast runs independently of command processing
- **Flexible**: Supports any combination of main UI + headless UIs
- **Scalable**: Can add new UI types without changing core logic
- **Testable**: Can run without any UI (headless mode)
- **Resilient**: Handles lagged events and UI failures gracefully

---
## UI System Design

### UI Trait Hierarchy

```rust
/// Main interactive UI interface
#[async_trait]
pub trait UserInterface: Send + Sync {
    /// Start UI (spawns tasks, returns immediately)
    async fn start(&self) -> Result<()>;
    
    /// Get receiver for commands from this UI
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    
    /// Handle event from EventBus (called by AgentEnvironment)
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}

/// Headless UI interface (non-interactive)
#[async_trait]
pub trait HeadlessInterface: Send + Sync {
    /// Handle event from EventBus
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

### Command System

```rust
/// Result from UI prompt
pub enum PromptResult {
    Command(AgentEnvironmentCommand),  // Pass to AgentEnvironment
    Shutdown,                           // Shutdown requested
}

/// Commands that AgentEnvironment handles (task-spawning)
#[derive(Debug, Clone)]
pub enum AgentEnvironmentCommand {
    Prompt {
        worker_id: Uuid,
        text: String,
    },
    Compact {
        worker_id: Uuid,
        instruction: Option<String>,
    },
    Quit,
}

/// Commands that UI handles internally (display-only)
#[derive(Debug, Clone)]
pub enum UiCommand {
    Usage,
    Context,
    Status,
    Workers,
    // UI-specific commands can be added by implementations
}

/// Parsed command from user input
#[derive(Debug, Clone)]
pub enum Command {
    Agent(AgentEnvironmentCommand),  // Forward to AgentEnvironment
    Ui(UiCommand),                   // Handle in UI
}
```

### Command Parser

```rust
pub struct CommandParser;

impl CommandParser {
    pub fn parse(input: &str) -> Result<Command, ParseError> {
        let trimmed = input.trim();
        
        if trimmed.starts_with('/') {
            // Explicit command
            let parts: Vec<&str> = trimmed[1..].splitn(2, ' ').collect();
            match parts[0] {
                "quit" | "q" => Ok(Command::Agent(AgentEnvironmentCommand::Quit)),
                "compact" => {
                    let instruction = parts.get(1).map(|s| s.to_string());
                    Ok(Command::Agent(AgentEnvironmentCommand::Compact {
                        worker_id: Uuid::nil(), // Will be filled by UI
                        instruction,
                    }))
                }
                "usage" => Ok(Command::Ui(UiCommand::Usage)),
                "context" => Ok(Command::Ui(UiCommand::Context)),
                "status" => Ok(Command::Ui(UiCommand::Status)),
                "workers" => Ok(Command::Ui(UiCommand::Workers)),
                _ => Err(ParseError::UnknownCommand(parts[0].to_string())),
            }
        } else {
            // Implicit /prompt command
            Ok(Command::Agent(AgentEnvironmentCommand::Prompt {
                worker_id: Uuid::nil(), // Will be filled by UI
                text: trimmed.to_string(),
            }))
        }
    }
}
```

### Shared UI Utilities

```rust
/// Shared utilities for UI implementations
pub mod ui_utils {
    use super::*;
    
    /// Calculate token usage for a worker
    pub fn calculate_token_usage(worker: &Worker) -> TokenUsage {
        let history = worker.context_container
            .conversation_history
            .lock()
            .unwrap();
        
        let mut input_tokens = 0;
        let mut output_tokens = 0;
        
        for entry in history.entries() {
            match entry {
                ConversationEntry::User { content, .. } => {
                    input_tokens += estimate_tokens(content);
                }
                ConversationEntry::Assistant { content, .. } => {
                    output_tokens += estimate_tokens(content);
                }
            }
        }
        
        TokenUsage {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }
    
    /// Format context information for display
    pub fn format_context_info(worker: &Worker) -> String {
        let history = worker.context_container
            .conversation_history
            .lock()
            .unwrap();
        
        format!(
            "Worker: {}\nMessages: {}\nState: {:?}",
            worker.name,
            history.len(),
            worker.lifecycle_state
        )
    }
    
    /// Estimate token count for text
    fn estimate_tokens(text: &str) -> usize {
        // Simple estimation: ~4 chars per token
        text.len() / 4
    }
}

pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}
```

---

### TextUi Implementation

**Purpose**: Basic text-based UI with readline-style input and streaming output.

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

```rust
use tokio::sync::{mpsc, Notify};
use std::sync::Arc;
use std::path::PathBuf;

pub struct TextUi {
    session: Arc<Session>,
    main_worker_id: Uuid,
    input_handler: Arc<InputHandler>,
    cmd_sender: mpsc::Sender<PromptResult>,
    prompt_ready: Arc<Notify>,
    shutdown_signal: Arc<Notify>,
}

impl TextUi {
    pub fn new(
        session: Arc<Session>,
        main_worker_id: Uuid,
        history_path: Option<PathBuf>,
    ) -> Result<(Self, mpsc::Receiver<PromptResult>)> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        
        let ui = Self {
            session,
            main_worker_id,
            input_handler: Arc::new(InputHandler::new(history_path)?),
            cmd_sender,
            prompt_ready: Arc::new(Notify::new()),
            shutdown_signal: Arc::new(Notify::new()),
        };
        
        Ok((ui, cmd_receiver))
    }
}

#[async_trait]
impl UserInterface for TextUi {
    async fn start(&self) -> Result<()> {
        // Spawn prompt loop task
        self.spawn_prompt_loop();
        Ok(())
    }
    
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Filter to main worker
        if let Some(wid) = event.worker_id() {
            if wid != self.main_worker_id {
                return;
            }
        }
        
        // Handle event
        match event {
            AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
                match chunk {
                    OutputChunk::AssistantResponse(text) => {
                        print!("{}", text);
                        io::stdout().flush().unwrap();
                    }
                    OutputChunk::ToolUse { tool_name, .. } => {
                        println!("\n[Using tool: {}]", tool_name);
                    }
                    OutputChunk::ToolResult { tool_name, .. } => {
                        println!("[Tool {} completed]", tool_name);
                    }
                }
            }
            AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { new_state, .. }) => {
                match new_state {
                    WorkerLifecycleState::Busy => {
                        // Worker started job - don't prompt
                    }
                    WorkerLifecycleState::Idle => {
                        println!(); // New line after completion
                        // Signal prompt loop to read input
                        self.prompt_ready.notify_one();
                    }
                    WorkerLifecycleState::IdleFailed => {
                        println!("\n[Task failed]");
                        // Still allow prompt
                        self.prompt_ready.notify_one();
                    }
                }
            }
            _ => {}
        }
    }
}

impl TextUi {
    /// Spawn prompt loop task
    fn spawn_prompt_loop(&self) -> JoinHandle<()> {
        let input_handler = self.input_handler.clone();
        let cmd_sender = self.cmd_sender.clone();
        let session = self.session.clone();
        let main_worker_id = self.main_worker_id;
        let prompt_ready = self.prompt_ready.clone();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Wait for worker to be ready for input
                    _ = prompt_ready.notified() => {
                        // Read input (blocking, but in separate task)
                        let input = match input_handler.read_line("You").await {
                            Ok(input) => input,
                            Err(e) => {
                                eprintln!("Error reading input: {}", e);
                                continue;
                            }
                        };
                        
                        // Parse command
                        let command = match CommandParser::parse(&input) {
                            Ok(cmd) => cmd,
                            Err(e) => {
                                eprintln!("Error parsing command: {}", e);
                                continue;
                            }
                        };
                        
                        match command {
                            // UI-specific commands - handle internally
                            Command::Ui(UiCommand::Usage) => {
                                if let Some(worker) = session.get_worker(main_worker_id) {
                                    let usage = ui_utils::calculate_token_usage(&worker);
                                    println!("Token usage:");
                                    println!("  Input:  {}", usage.input_tokens);
                                    println!("  Output: {}", usage.output_tokens);
                                    println!("  Total:  {}", usage.total_tokens);
                                }
                                // Re-signal for next prompt
                                prompt_ready.notify_one();
                                continue;
                            }
                            
                            Command::Ui(UiCommand::Context) => {
                                if let Some(worker) = session.get_worker(main_worker_id) {
                                    let info = ui_utils::format_context_info(&worker);
                                    println!("{}", info);
                                }
                                // Re-signal for next prompt
                                prompt_ready.notify_one();
                                continue;
                            }
                            
                            // Task-spawning commands - send to AgentEnvironment
                            Command::Agent(mut agent_cmd) => {
                                // Fill in worker_id
                                match &mut agent_cmd {
                                    AgentEnvironmentCommand::Prompt { worker_id, .. } => {
                                        *worker_id = main_worker_id;
                                    }
                                    AgentEnvironmentCommand::Compact { worker_id, .. } => {
                                        *worker_id = main_worker_id;
                                    }
                                    AgentEnvironmentCommand::Quit => {}
                                }
                                
                                // Send command
                                if cmd_sender.send(PromptResult::Command(agent_cmd)).await.is_err() {
                                    break; // Channel closed
                                }
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

**Key Changes from Original Design**:
- **No `spawn_event_processor()`**: Events come via `handle_event()` from AgentEnvironment
- **Prompt queue pattern**: Only reads input when `prompt_ready` is signaled
- **Worker state tracking**: Listens to `LifecycleStateChanged` events to know when to prompt
- **Constructor returns tuple**: `(TextUi, Receiver)` for clear ownership transfer
- **UI commands re-signal**: After handling Usage/Context, notify prompt loop to read again

---

### StructuredIO Implementation

**Purpose**: Structured JSON I/O for scripting and automation. Reads single-line prompts from stdin, outputs structured JSON events.

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

```rust
use tokio::sync::mpsc;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::io::Write;

pub struct StructuredIO {
    session: Arc<Session>,
    main_worker_id: Uuid,
    cmd_sender: mpsc::Sender<PromptResult>,
    output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>,
}

impl StructuredIO {
    pub fn new(
        session: Arc<Session>,
        main_worker_id: Uuid,
    ) -> Result<(Self, mpsc::Receiver<PromptResult>)> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        
        let ui = Self {
            session,
            main_worker_id,
            cmd_sender,
            output_writer: Arc::new(TokioMutex::new(Box::new(io::stdout()))),
        };
        
        Ok((ui, cmd_receiver))
    }
}

#[async_trait]
impl UserInterface for StructuredIO {
    async fn start(&self) -> Result<()> {
        // Spawn input reading task (always reading)
        self.spawn_input_reader();
        Ok(())
    }
    
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Filter to main worker
        if let Some(wid) = event.worker_id() {
            if wid != self.main_worker_id {
                return;
            }
        }
        
        // Handle AgentLoop-specific events
        match event {
            AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived { 
                worker_id, 
                text, 
                .. 
            }) => {
                let json = serde_json::json!({
                    "worker_id": worker_id,
                    "assistant_response": text,
                });
                
                let mut writer = self.output_writer.lock().await;
                writeln!(writer, "{}", json).unwrap();
                writer.flush().unwrap();
            }
            AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ToolUseRequestReceived {
                worker_id,
                tool_name,
                tool_input,
                ..
            }) => {
                let json = serde_json::json!({
                    "worker_id": worker_id,
                    "tool_use_request": {
                        "tool_name": tool_name,
                        "tool_input": tool_input,
                    }
                });
                
                let mut writer = self.output_writer.lock().await;
                writeln!(writer, "{}", json).unwrap();
                writer.flush().unwrap();
            }
            _ => {}
        }
    }
}

impl StructuredIO {
    /// Spawn input reader task (always reading)
    fn spawn_input_reader(&self) -> JoinHandle<()> {
        let cmd_sender = self.cmd_sender.clone();
        let main_worker_id = self.main_worker_id;
        
        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let reader = BufReader::new(stdin);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                
                if line.is_empty() {
                    continue;
                }
                
                // Single-line prompt - send as Prompt command
                let cmd = AgentEnvironmentCommand::Prompt {
                    worker_id: main_worker_id,
                    text: line.to_string(),
                };
                
                if cmd_sender.send(PromptResult::Command(cmd)).await.is_err() {
                    break; // Channel closed
                }
            }
        })
    }
}
```

**Key Changes from Original Design**:
- **No `spawn_event_processor()`**: Events come via `handle_event()` from AgentEnvironment
- **Always reading**: Input loop continuously reads lines without waiting for worker state
- **Constructor returns tuple**: `(StructuredIO, Receiver)` for clear ownership transfer
- **Simplified**: No event bus subscription, no prompt queue - just read and forward

**Key Behaviors**:
- Reads single-line prompts from stdin (no JSON parsing)
- Listens to `AgentLoopEvent::ResponseReceived` and `AgentLoopEvent::ToolUseRequestReceived`
- Outputs structured JSON with worker_id and response/tool data
- Non-blocking: input reading runs in separate task
- Suitable for scripting where commands may be piped in

---

### WebApi Implementation (Future)

**Purpose**: REST API + WebSocket for web-based UIs.

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/web_api.rs`

```rust
pub struct WebApi {
    event_bus: EventBus,
    websocket_connections: Arc<TokioMutex<HashMap<Uuid, WebSocketSender>>>,
    http_server: Arc<HttpServer>,
}

#[async_trait]
impl HeadlessInterface for WebApi {
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Broadcast event to all connected websockets
        let connections = self.websocket_connections.lock().await;
        let json = serde_json::to_string(&event).unwrap();
        
        for (_, sender) in connections.iter() {
            let _ = sender.send(json.clone()).await;
        }
    }
}

// REST endpoints:
// POST /workers - Create worker
// DELETE /workers/:id - Delete worker
// POST /workers/:id/prompt - Send prompt
// POST /workers/:id/compact - Compact conversation
// GET /workers/:id - Get worker state
// WS /events - WebSocket for events
```

---
## Task Implementations

### AgentLoop Task

**Purpose**: Main agent loop that queries LLM, handles tool use, and manages conversation flow.

**Location**: `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

```rust
pub struct AgentLoop {
    worker: Arc<Worker>,
    input: AgentLoopInput,
    event_bus: EventBus,
}

pub struct AgentLoopInput {
    // Empty for now - context comes from worker
}

impl AgentLoop {
    pub fn new(
        worker: Arc<Worker>,
        input: AgentLoopInput,
        event_bus: EventBus,
    ) -> Self {
        Self {
            worker,
            input,
            event_bus,
        }
    }
}

#[async_trait]
impl WorkerTask for AgentLoop {
    fn task_type(&self) -> &str {
        "AgentLoop"
    }
    
    async fn run(&self) -> Result<()> {
        // Get conversation history from worker
        let history = self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .clone();
        
        // Query LLM
        let model_provider = self.worker.model_provider
            .as_ref()
            .ok_or_else(|| eyre::eyre!("No model provider"))?;
        
        let mut stream = model_provider.query_stream(history).await?;
        
        // Process response stream
        let mut response_text = String::new();
        let mut tool_requests = Vec::new();
        
        while let Some(chunk) = stream.next().await {
            match chunk? {
                ResponseChunk::Text(text) => {
                    response_text.push_str(&text);
                    
                    // Publish output chunk event (for streaming display)
                    self.event_bus.publish(AgentEnvironmentEvent::Job(
                        JobEvent::OutputChunk {
                            worker_id: self.worker.id,
                            job_id: Uuid::nil(), // TODO: get from context
                            chunk: OutputChunk::AssistantResponse(text),
                            timestamp: Instant::now(),
                        }
                    ));
                }
                ResponseChunk::ToolUse { name, input } => {
                    tool_requests.push(ToolRequest { name: name.clone(), input: input.clone() });
                    
                    // Publish tool use event
                    self.event_bus.publish(AgentEnvironmentEvent::Job(
                        JobEvent::OutputChunk {
                            worker_id: self.worker.id,
                            job_id: Uuid::nil(),
                            chunk: OutputChunk::ToolUse {
                                tool_name: name.clone(),
                                tool_input: input.clone(),
                            },
                            timestamp: Instant::now(),
                        }
                    ));
                    
                    // Publish AgentLoop-specific event
                    self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
                        AgentLoopEvent::ToolUseRequestReceived {
                            worker_id: self.worker.id,
                            job_id: Uuid::nil(),
                            tool_name: name,
                            tool_input: input,
                            timestamp: Instant::now(),
                        }
                    ));
                }
            }
        }
        
        // Publish complete response event
        self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
            AgentLoopEvent::ResponseReceived {
                worker_id: self.worker.id,
                job_id: Uuid::nil(),
                text: response_text.clone(),
                timestamp: Instant::now(),
            }
        ));
        
        // Add assistant response to conversation history
        self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_assistant_message(response_text);
        
        // Check if we need tool approval
        if !tool_requests.is_empty() {
            // Check if tools can be auto-approved
            let auto_approved = self.check_auto_approval(&tool_requests);
            
            if auto_approved {
                // Execute tools and continue
                self.execute_tools(tool_requests).await?;
                
                // Recursively call agent loop to continue
                return self.run().await;
            } else {
                // Exit with flag indicating tool approval needed
                self.worker.set_task_metadata(
                    task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
                    serde_json::Value::String("completed_with_tool_request".to_string()),
                );
                
                // Store pending tool requests in worker
                // (requires adding pending_tool_requests field to Worker)
                
                return Ok(());
            }
        }
        
        // Normal completion - ready for new prompt
        self.worker.set_task_metadata(
            task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
            serde_json::Value::String("completed_ready_for_prompt".to_string()),
        );
        
        Ok(())
    }
}

impl AgentLoop {
    fn check_auto_approval(&self, tool_requests: &[ToolRequest]) -> bool {
        // Check worker's tool trust settings
        // TODO: implement trust checking logic
        false
    }
    
    async fn execute_tools(&self, tool_requests: Vec<ToolRequest>) -> Result<()> {
        for tool_request in tool_requests {
            // Execute tool
            let result = self.execute_tool(&tool_request).await?;
            
            // Add tool result to conversation history
            self.worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_tool_result(tool_request.name.clone(), result.clone());
            
            // Publish tool result event
            self.event_bus.publish(AgentEnvironmentEvent::Job(
                JobEvent::OutputChunk {
                    worker_id: self.worker.id,
                    job_id: Uuid::nil(),
                    chunk: OutputChunk::ToolResult {
                        tool_name: tool_request.name,
                        result,
                    },
                    timestamp: Instant::now(),
                }
            ));
        }
        
        Ok(())
    }
    
    async fn execute_tool(&self, tool_request: &ToolRequest) -> Result<String> {
        // TODO: implement tool execution
        Ok("Tool result".to_string())
    }
}
```

**Key Behaviors**:
- Queries LLM with conversation history
- Streams response chunks as events
- Handles tool use requests
- Auto-approves tools if configured
- Exits with completion flag if tool approval needed
- Recursively continues after tool execution
- Updates conversation history

**Completion States**:
- `completed_ready_for_prompt`: Normal completion, ready for user input
- `completed_with_tool_request`: Stopped waiting for tool approval

---

### ConversationCompact Task

**Purpose**: Compact conversation history to reduce token usage.

**Location**: `crates/chat-cli/src/agent_env/worker_tasks/conversation_compact.rs`

```rust
pub struct ConversationCompact {
    worker: Arc<Worker>,
    input: CompactInput,
    event_bus: EventBus,
}

pub struct CompactInput {
    pub instruction: Option<String>,
}

impl ConversationCompact {
    pub fn new(
        worker: Arc<Worker>,
        input: CompactInput,
        event_bus: EventBus,
    ) -> Self {
        Self {
            worker,
            input,
            event_bus,
        }
    }
}

#[async_trait]
impl WorkerTask for ConversationCompact {
    fn task_type(&self) -> &str {
        "ConversationCompact"
    }
    
    async fn run(&self) -> Result<()> {
        // Get conversation history
        let history = self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .clone();
        
        // Build compaction prompt
        let instruction = self.input.instruction
            .as_deref()
            .unwrap_or("Summarize the conversation history concisely, preserving key information.");
        
        let compact_prompt = format!(
            "{}\n\nConversation history:\n{}",
            instruction,
            format_history(&history)
        );
        
        // Query LLM for summary
        let model_provider = self.worker.model_provider
            .as_ref()
            .ok_or_else(|| eyre::eyre!("No model provider"))?;
        
        let summary = model_provider.query_simple(compact_prompt).await?;
        
        // Replace conversation history with summary
        self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .replace_with_summary(summary);
        
        // Update metadata
        self.worker.set_task_metadata(
            task_metadata_keys::COMPACT_LAST_RUN,
            serde_json::Value::String(Instant::now().to_string()),
        );
        
        // Publish completion event (via normal job completion)
        
        Ok(())
    }
}

fn format_history(history: &ConversationHistory) -> String {
    // Format history for compaction
    // TODO: implement formatting
    String::new()
}
```

**Key Behaviors**:
- Reads conversation history from worker
- Queries LLM for summary
- Replaces history with compacted version
- Updates last run timestamp
- Can be triggered manually or automatically by AgentLoop

---

## Data Flow & Interactions

### Startup Flow

```
1. Entry Point (ChatArgs::execute)
   ├─ Create EventBus
   ├─ Create Session (with EventBus)
   ├─ Create main Worker
   ├─ Create TextUi (with EventBus, Session, main_worker_id)
   ├─ Create AgentEnvironment (with Session, EventBus, TextUi)
   └─ Run AgentEnvironment.run() (blocks)

2. AgentEnvironment.run()
   ├─ Spawn event multicast task
   ├─ Start TextUi
   │  ├─ Spawn event processor task
   │  └─ Spawn prompt loop task
   └─ Enter command processing loop

3. TextUi event processor
   ├─ Subscribe to EventBus
   ├─ Filter events for main_worker_id
   └─ Display output chunks

4. TextUi prompt loop
   ├─ Read user input
   ├─ Parse command
   ├─ Handle UI commands internally (Usage, Context)
   └─ Send AgentEnvironment commands via channel
```

### User Prompt Flow

```
1. User enters text in TextUi
   └─ TextUi.prompt_loop reads input

2. TextUi parses command
   ├─ If UI command (Usage, Context):
   │  ├─ Handle internally
   │  └─ Re-prompt
   └─ If Agent command (Prompt):
      └─ Send to AgentEnvironment via channel

3. AgentEnvironment receives command
   ├─ Get worker from Session
   ├─ Add message to worker's conversation history
   └─ Call Session.run_agent_loop()

4. Session.run_agent_loop()
   ├─ Set worker lifecycle state to Busy
   ├─ Publish JobStarted event
   ├─ Create AgentLoop task
   ├─ Create WorkerJob
   ├─ Spawn job execution
   └─ Return job handle

5. AgentLoop.run()
   ├─ Query LLM with conversation history
   ├─ Stream response chunks
   │  └─ Publish OutputChunk events for each chunk
   ├─ Add assistant response to conversation history
   ├─ Check for tool requests
   │  ├─ If auto-approved: execute tools and continue
   │  └─ If needs approval: exit with flag
   └─ Set completion state metadata

6. Job completes
   ├─ Session.handle_job_completion()
   ├─ Set worker lifecycle state to Idle
   ├─ Publish JobCompleted event
   └─ Run continuations

7. Continuation re-queues prompt
   └─ TextUi.prompt_loop reads next input
```

### Event Flow

```
Component publishes event
   ↓
EventBus.publish()
   ↓
broadcast::Sender.send()
   ↓
All subscribers receive event
   ├─ AgentEnvironment.event_multicast
   │  ├─ Forward to main UI
   │  └─ Forward to headless UIs
   ├─ TextUi.event_processor
   │  ├─ Filter by worker_id
   │  └─ Display output
   └─ StructuredIO
      └─ Write JSON to stdout
```

### Tool Approval Flow (Future)

```
1. AgentLoop receives tool use request from LLM
   ├─ Check auto-approval rules
   └─ If not auto-approved:
      ├─ Store pending tool requests in worker
      ├─ Set completion state to "completed_with_tool_request"
      └─ Exit

2. Job completes
   ├─ Session publishes JobCompleted event
   └─ Run continuations

3. Continuation checks completion state
   ├─ If "completed_with_tool_request":
   │  └─ Queue tool approval prompt (not regular prompt)
   └─ If "completed_ready_for_prompt":
      └─ Queue regular prompt

4. TextUi shows tool approval prompt
   ├─ Display tool details
   ├─ Ask for approval (y/n)
   └─ User responds

5. User approves/denies
   ├─ Add approval/denial to conversation history
   └─ Send Prompt command to AgentEnvironment

6. AgentEnvironment launches new AgentLoop
   ├─ AgentLoop reads approval from history
   ├─ If approved: execute tool
   ├─ Add tool result to history
   └─ Continue agent loop
```

---
## Implementation Plan

### Phase 1: Core Event System (Week 1)

**Goal**: Implement EventBus and event types

**Tasks**:
1. Create `events.rs` with nested enum structure
   - `AgentEnvironmentEvent`, `WorkerEvent`, `JobEvent`, `SystemEvent`
   - Helper methods (worker_id, is_worker_event, timestamp)
   - Derive Clone, Debug for all event types

2. Create `event_bus.rs` with broadcast channel
   - EventBus struct with configurable buffer size
   - publish() and subscribe() methods
   - Default implementation with 1000 buffer size

3. Add EventBus to Session
   - Update Session constructor to accept EventBus
   - Store EventBus in Session struct
   - Update existing code to use Session's EventBus

4. Add EventBus to Worker (optional)
   - Consider if Worker needs direct EventBus access
   - Or if all events should go through Session

**Testing**:
- Unit tests for event helper methods
- Integration test: publish events, verify subscribers receive them
- Test lagged event handling

**Files to Create**:
- `crates/chat-cli/src/agent_env/events.rs`
- `crates/chat-cli/src/agent_env/event_bus.rs`

**Files to Modify**:
- `crates/chat-cli/src/agent_env/session.rs`
- `crates/chat-cli/src/agent_env/mod.rs`

---

### Phase 2: Worker State Management (Week 1-2)

**Goal**: Update Worker with lifecycle state and task metadata

**Tasks**:
1. Update Worker struct
   - Add `lifecycle_state: Arc<Mutex<WorkerLifecycleState>>`
   - Add `task_metadata: HashMap<String, serde_json::Value>`
   - Add serde Serialize/Deserialize derives
   - Mark non-serializable fields with `#[serde(skip)]`

2. Add Worker helper methods
   - `set_task_metadata(key, value)`
   - `get_task_metadata(key) -> Option<&Value>`
   - `get_task_metadata_string(key) -> Option<String>`

3. Create task_metadata_keys module
   - Define namespaced constants for known metadata keys
   - Document metadata key conventions

4. Update Session to manage Worker lifecycle state
   - `set_worker_lifecycle_state()` method
   - Publish WorkerEvent::LifecycleStateChanged events
   - Update state on job start/completion

**Testing**:
- Test Worker serialization/deserialization
- Test metadata get/set operations
- Test lifecycle state transitions
- Test event publishing on state changes

**Files to Modify**:
- `crates/chat-cli/src/agent_env/worker.rs`
- `crates/chat-cli/src/agent_env/session.rs`

---

### Phase 3: Session Event Publishing (Week 2)

**Goal**: Make Session publish events for all lifecycle changes

**Tasks**:
1. Update `build_worker()` to publish WorkerEvent::Created
2. Update `delete_worker()` to publish WorkerEvent::Deleted
3. Update `run_agent_loop()` to:
   - Set worker state to Busy
   - Publish JobEvent::Started
4. Create `handle_job_completion()` method to:
   - Set worker state to Idle/IdleFailed
   - Publish JobEvent::Completed
   - Run continuations
5. Update job spawning to call handle_job_completion

**Testing**:
- Test event publishing for each lifecycle transition
- Test event data correctness (worker_id, timestamps, etc.)
- Test multiple workers don't interfere with each other

**Files to Modify**:
- `crates/chat-cli/src/agent_env/session.rs`

---

### Phase 4: Task Event Publishing (Week 2-3)

**Goal**: Update AgentLoop to publish output events

**Tasks**:
1. Update AgentLoop constructor to accept EventBus
   - Store EventBus in struct
   - Remove WorkerToHostInterface parameter

2. Update AgentLoop.run() to publish events
   - Publish OutputChunk events for text chunks
   - Publish OutputChunk events for tool use
   - Publish OutputChunk events for tool results

3. Update task completion to set metadata
   - Set completion_state metadata
   - Store pending tool requests if needed

4. Remove WorkerToHostInterface trait (deprecated)

**Testing**:
- Test event publishing during LLM streaming
- Test event data correctness
- Test completion state metadata

**Files to Modify**:
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Files to Remove**:
- `crates/chat-cli/src/agent_env/worker_interface.rs` (deprecated)

---

### Phase 5: Command System (Week 3)

**Goal**: Implement command parsing and handling

**Tasks**:
1. Create `commands.rs` with command types
   - `AgentEnvironmentCommand` enum
   - `UiCommand` enum
   - `Command` enum
   - `PromptResult` enum

2. Create `CommandParser`
   - Parse explicit commands (/quit, /compact, etc.)
   - Parse implicit prompt command
   - Return appropriate Command variant

3. Create `ui_utils` module
   - `calculate_token_usage()`
   - `format_context_info()`
   - Other shared utilities

**Testing**:
- Test command parsing for all command types
- Test implicit prompt parsing
- Test error handling for unknown commands
- Test utility functions

**Files to Create**:
- `crates/chat-cli/src/agent_env/commands.rs`
- `crates/chat-cli/src/cli/chat/agent_env_ui/ui_utils.rs`

---

### Phase 6: AgentEnvironment Coordinator (Week 3-4)

**Goal**: Implement AgentEnvironment coordinator

**Tasks**:
1. Create `agent_environment.rs`
   - AgentEnvironment struct
   - Constructor accepting Session, EventBus, UIs
   - `run()` method with main loop
   - `spawn_event_multicast()` method
   - `handle_command()` method

2. Define UI traits
   - `UserInterface` trait
   - `HeadlessInterface` trait

3. Implement shutdown coordination
   - Shutdown signal handling
   - Cancel all jobs on shutdown
   - Clean resource cleanup

**Testing**:
- Test event multicasting to multiple UIs
- Test command processing
- Test shutdown coordination
- Test headless mode (no main UI)

**Files to Create**:
- `crates/chat-cli/src/agent_env/agent_environment.rs`

**Files to Modify**:
- `crates/chat-cli/src/agent_env/mod.rs`
- `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`

---

### Phase 7: TextUi Implementation (Week 4)

**Goal**: Implement TextUi with new architecture

**Tasks**:
1. Create new `text_ui.rs`
   - TextUi struct
   - Implement UserInterface trait
   - `spawn_event_processor()` method
   - `spawn_prompt_loop()` method

2. Refactor command_receiver pattern
   - Consider using oneshot channel to send receiver
   - Or refactor trait to avoid Mutex issue

3. Implement event handling
   - Filter events by worker_id
   - Display output chunks
   - Update status display

4. Implement prompt loop
   - Read user input
   - Parse commands
   - Handle UI commands internally
   - Send Agent commands to AgentEnvironment

**Testing**:
- Test event filtering and display
- Test command parsing and handling
- Test non-blocking operation
- Test with multiple workers

**Files to Create**:
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Files to Modify**:
- `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs`

---

### Phase 8: Entry Point Integration (Week 4-5)

**Goal**: Update ChatArgs::execute to use new architecture

**Tasks**:
1. Update ChatArgs::execute()
   - Create EventBus
   - Create Session with EventBus
   - Create main Worker
   - Create TextUi
   - Create AgentEnvironment
   - Run AgentEnvironment

2. Remove old demo code
   - Remove demo module
   - Remove old UI implementation

3. Handle command-line arguments
   - --no-interactive mode
   - Initial input handling
   - Agent/profile selection

**Testing**:
- Test basic chat flow
- Test with initial input
- Test --no-interactive mode
- Test shutdown (Ctrl+C, /quit)

**Files to Modify**:
- `crates/chat-cli/src/cli/chat/mod.rs`

**Files to Remove**:
- `crates/chat-cli/src/agent_env/demo/` (entire directory)
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui_worker_to_host_interface.rs`
- `crates/chat-cli/src/cli/chat/agent_env_ui/prompt_queue.rs` (if not needed)

---

### Phase 9: Additional UI Implementations (Week 5+)

**Goal**: Implement additional UI types

**Tasks**:
1. Implement StructuredIO
   - HeadlessInterface implementation
   - JSON output for events
   - Synchronized output with worker_id

2. Implement WebApi (future)
   - REST endpoints for worker management
   - WebSocket for event streaming
   - Authentication and authorization

3. Implement AdvancedFancyUi (future)
   - Full-screen TUI with widgets
   - Multiple panes for multiple workers
   - Status displays and visualizations

**Testing**:
- Test each UI implementation independently
- Test multiple UIs running simultaneously
- Test event delivery to all UIs

**Files to Create**:
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
- `crates/chat-cli/src/cli/chat/agent_env_ui/web_api.rs` (future)
- `crates/chat-cli/src/cli/chat/agent_env_ui/advanced_fancy_ui.rs` (future)

---

### Phase 10: ConversationCompact Task (Week 5+)

**Goal**: Implement conversation compaction task

**Tasks**:
1. Create ConversationCompact task
   - Read conversation history
   - Query LLM for summary
   - Replace history with summary
   - Update metadata

2. Add Session.run_compact_task() method

3. Add /compact command to CommandParser

4. Implement automatic compaction in AgentLoop
   - Check token count
   - Trigger compact when approaching limit

**Testing**:
- Test manual compaction via /compact command
- Test automatic compaction
- Test conversation history replacement

**Files to Create**:
- `crates/chat-cli/src/agent_env/worker_tasks/conversation_compact.rs`

**Files to Modify**:
- `crates/chat-cli/src/agent_env/session.rs`
- `crates/chat-cli/src/agent_env/commands.rs`
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

---

## Migration Strategy

### Approach: Incremental Migration

**Strategy**: Build new architecture alongside existing code, then switch over.

**Steps**:

1. **Phase 1-4**: Build core infrastructure without breaking existing code
   - New files don't affect existing functionality
   - Can test new components in isolation

2. **Phase 5-7**: Build new UI implementation
   - Keep existing UI working
   - New UI can be tested with feature flag

3. **Phase 8**: Switch entry point to new architecture
   - Single commit that switches ChatArgs::execute
   - Remove old code in same commit
   - Minimize time with both implementations

4. **Phase 9-10**: Add new features
   - Additional UIs
   - New tasks
   - Enhanced functionality

### Rollback Plan

If issues arise during migration:

1. **Before Phase 8**: No rollback needed (new code not in use)
2. **During Phase 8**: Revert single commit
3. **After Phase 8**: Fix forward (new architecture is simpler)

### Testing Strategy

1. **Unit Tests**: Test each component in isolation
2. **Integration Tests**: Test component interactions
3. **End-to-End Tests**: Test full user flows
4. **Manual Testing**: Test with real LLM interactions

### Risk Mitigation

**Risks**:
- Event buffer overflow with high-frequency events
- Race conditions in state management
- UI blocking on slow operations
- Memory leaks from unclosed subscriptions

**Mitigations**:
- Configurable buffer size with monitoring
- Clear ownership rules for state
- All UI operations in separate tasks
- Proper cleanup on shutdown

---

## Open Questions & Future Considerations

### Open Questions

1. **Worker Serialization**: Where to store serialized workers?
   - File system? Database? In-memory only?
   - When to serialize? On every change? Periodically?

2. **Tool Approval UI**: How to show tool details?
   - Simple y/n prompt? Rich display with syntax highlighting?
   - Batch approval for multiple tools?

3. **Multi-Worker UI**: How to switch between workers in TextUi?
   - Command to switch? Tab completion? Status display?

4. **Event Buffer Size**: What's the right default?
   - 1000? 10000? Configurable via CLI?
   - Monitor and adjust based on usage?

5. **Error Recovery**: How to recover from task failures?
   - Retry? Skip? Abort? User choice?

### Future Enhancements

1. **Worker Persistence**:
   - Save/load worker state
   - Resume conversations across sessions
   - Export/import workers

2. **Advanced Tool Approval**:
   - Trust rules (always approve X, never approve Y)
   - Approval history and learning
   - Batch approval UI

3. **Multi-Worker Coordination**:
   - Workers can communicate with each other
   - Delegate tasks between workers
   - Shared context between workers

4. **Performance Monitoring**:
   - Event bus metrics (lag, throughput)
   - Task execution metrics (duration, success rate)
   - Resource usage (memory, CPU)

5. **Advanced UIs**:
   - Full-screen TUI with multiple panes
   - Web-based UI with rich interactions
   - Voice interface
   - IDE integration

6. **Task Marketplace**:
   - Plugin system for custom tasks
   - Share tasks between users
   - Task composition and chaining

---

## Conclusion

This architecture provides a solid foundation for the Agent Environment system with:

- **Clean separation of concerns** between orchestration, execution, and presentation
- **Event-driven communication** that scales to multiple workers and UIs
- **Extensible design** that supports new tasks and UI types
- **Type-safe implementation** with minimal runtime errors
- **Non-blocking execution** for responsive user experience

The phased implementation plan allows for incremental development and testing, with clear milestones and rollback points.

The design has been validated through discussion and addresses all identified issues from the challenges analysis document.

---

## Appendix: Key Design Decisions

### Why Nested Event Enums?

**Decision**: Use nested enum structure (`AgentEnvironmentEvent(WorkerEvent(...))`)

**Rationale**:
- Clear categorization of event types
- Easy filtering with helper methods
- Extensible (add new event categories)
- Type-safe pattern matching

**Alternative Considered**: Flat enum structure
- Simpler but less organized
- Harder to filter by category
- Less extensible

### Why EventBus in Task Struct?

**Decision**: Store EventBus in task struct during construction

**Rationale**:
- Keeps WorkerTask trait simple
- Tasks can publish events throughout lifecycle
- Matches Rust ownership patterns
- More flexible than passing to run()

**Alternative Considered**: Pass EventBus to run() method
- Requires changing trait signature
- Affects all implementations
- Less flexible

### Why AgentEnvironment Layer?

**Decision**: Add AgentEnvironment coordinator between entry point and UIs

**Rationale**:
- Supports multiple concurrent UIs (main + headless)
- Multicasts events to all subscribers
- Coordinates command processing
- Enables headless mode

**Alternative Considered**: UIs subscribe directly to EventBus
- Simpler but less flexible
- No coordination between UIs
- No headless mode support

### Why Jobs Exit on Tool Approval?

**Decision**: Tasks exit with completion flag when tool approval needed

**Rationale**:
- Simpler job lifecycle (no blocking)
- No deadlock concerns
- Clear state management
- UI controls approval flow

**Alternative Considered**: Tasks block waiting for approval
- More complex state management
- Potential deadlocks
- Harder to test

### Why Task Metadata HashMap?

**Decision**: Use `HashMap<String, serde_json::Value>` for task metadata

**Rationale**:
- Fully serializable
- Extensible without changing Worker struct
- Supports unknown future tasks
- Type-safe helpers for common access patterns

**Alternative Considered**: Typed fields in Worker
- Type-safe but not extensible
- Requires Worker changes for new tasks
- Not serializable for custom types

---

## Design Q&A

### Q1: Why does TextUi have both `handle_event()` and `spawn_event_processor()`?

**Short Answer**: `handle_event()` is part of the trait interface but unused. `spawn_event_processor()` does the actual work.

**Detailed Explanation**:

The current design has a mismatch:
- `UserInterface` trait defines `handle_event()` because AgentEnvironment calls it
- But TextUi subscribes directly to EventBus in `spawn_event_processor()`
- This means `handle_event()` is never actually used

**Better Design Options**:

**Option A** (Recommended): Remove `handle_event()` from trait, UIs subscribe directly
```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn start(&self) -> Result<()>;
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    // No handle_event() - UIs subscribe to EventBus themselves
}
```

**Option B**: AgentEnvironment calls `handle_event()`, UIs don't subscribe directly
```rust
// In AgentEnvironment.spawn_event_multicast()
if let Some(ui) = &main_ui {
    ui.handle_event(event.clone()).await;  // Actually used
}

// In TextUi - NO spawn_event_processor(), just implement handle_event()
impl UserInterface for TextUi {
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Filter and display
        if let Some(wid) = event.worker_id() {
            if wid == self.main_worker_id {
                // Display event
            }
        }
    }
}
```

**Recommendation**: Use Option A. It's simpler and gives UIs more control over event filtering and buffering.

**DM Response**: Apply option B for all three UIs in this doc - TextUi, StructuredIO, WebApi

**Resolved** - Will implement Option B. This means:
- Remove direct EventBus subscription from UI implementations
- Remove `spawn_event_processor()` methods
- Implement `handle_event()` to process events passed from AgentEnvironment
- AgentEnvironment's `spawn_event_multicast()` calls `ui.handle_event()` for each event
- Simpler design: single event flow path through AgentEnvironment

**Implementation Note**: Update TextUi and StructuredIO sections to remove `spawn_event_processor()` and implement `handle_event()` instead.

---

### Q2: What options for `TextUi.command_receiver()` to avoid Mutex issue?

**The Problem**: 
```rust
cmd_receiver: Arc<TokioMutex<mpsc::Receiver<PromptResult>>>,

fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
    // Can't return receiver from behind Mutex!
}
```

**Option A** (Recommended): Use `OnceCell` to transfer receiver once
```rust
use tokio::sync::OnceCell;

pub struct TextUi {
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver_cell: Arc<OnceCell<mpsc::Receiver<PromptResult>>>,
}

impl TextUi {
    pub fn new(...) -> Result<Self> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        let cmd_receiver_cell = Arc::new(OnceCell::new());
        cmd_receiver_cell.set(cmd_receiver).unwrap();
        
        Ok(Self {
            cmd_sender,
            cmd_receiver_cell,
        })
    }
}

impl UserInterface for TextUi {
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
        self.cmd_receiver_cell
            .take()
            .expect("command_receiver() called more than once")
    }
}
```

**Option B**: Return sender, AgentEnvironment creates receiver
```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn start(&self) -> Result<()>;
    fn command_sender(&self) -> mpsc::Sender<PromptResult>;  // Changed
}

// In AgentEnvironment
let cmd_sender = ui.command_sender();
let mut cmd_receiver = /* how to get receiver from sender? Can't! */
```
This doesn't work - can't get receiver from sender.

**Option C**: Return channel pair from constructor
```rust
impl TextUi {
    pub fn new(...) -> Result<(Self, mpsc::Receiver<PromptResult>)> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        
        let ui = Self {
            cmd_sender,
            // No cmd_receiver stored
        };
        
        Ok((ui, cmd_receiver))
    }
}

// In entry point
let (text_ui, cmd_receiver) = TextUi::new(...)?;
let agent_env = AgentEnvironment::new(..., text_ui, cmd_receiver);
```

**Option D**: Use `Option<Receiver>` and `take()`
```rust
pub struct TextUi {
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<Mutex<Option<mpsc::Receiver<PromptResult>>>>,
}

impl UserInterface for TextUi {
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
        self.cmd_receiver
            .lock()
            .unwrap()
            .take()
            .expect("command_receiver() called more than once")
    }
}
```

**Recommendation**: Use Option C (return from constructor) or Option A (OnceCell). Option C is clearest about ownership transfer.

**DM Response**: Apply Option C

**Resolved** - Will implement Option C. UI constructors return `(Self, mpsc::Receiver<PromptResult>)`. Entry point passes receiver to AgentEnvironment. Clear ownership transfer, no Mutex complexity.

**Implementation Note**: Update all UI constructor signatures and entry point code.

---

### Q3: How does `spawn_prompt_loop()` work? Does it always read input?

**Yes, it always reads input** - that's the point! Let me explain the flow:

```rust
fn spawn_prompt_loop(&self) -> JoinHandle<()> {
    let input_handler = self.input_handler.clone();
    let cmd_sender = self.cmd_sender.clone();
    
    tokio::spawn(async move {
        loop {
            // This BLOCKS waiting for user input
            // But it's in a separate task, so it doesn't block anything else
            let input = input_handler.read_line("You").await?;
            
            // Parse and send command
            let command = CommandParser::parse(&input)?;
            cmd_sender.send(PromptResult::Command(command)).await?;
            
            // Loop back to read next input
        }
    })
}
```

**Key Points**:

1. **Runs in separate task**: `tokio::spawn()` creates independent task
2. **Blocks on input**: `read_line()` blocks until user presses Enter
3. **Doesn't block main loop**: AgentEnvironment continues processing events
4. **Sends commands via channel**: Non-blocking send to AgentEnvironment

**Detailed Flow**:

```
Time 0: spawn_prompt_loop() called
  └─ Spawns task, returns immediately
  
Time 1: Prompt task starts
  └─ Calls input_handler.read_line("You")
  └─ BLOCKS waiting for user input
  
Time 2-100: User thinking, typing...
  └─ Prompt task still blocked
  └─ AgentEnvironment continues running
  └─ Events still being processed
  └─ Other workers can run
  
Time 101: User presses Enter
  └─ read_line() returns with input
  └─ Parse command
  └─ Send to cmd_sender channel
  └─ Loop back to read_line() (blocks again)
  
Time 102: AgentEnvironment receives command
  └─ Processes command
  └─ Spawns task
  └─ Continues main loop
```

**How `input_handler.read_line()` works**:

```rust
pub struct InputHandler {
    editor: rustyline::Editor<()>,
}

impl InputHandler {
    pub async fn read_line(&self, prompt: &str) -> Result<String> {
        // rustyline is synchronous, so we run it in blocking task
        let prompt = prompt.to_string();
        let editor = self.editor.clone();
        
        tokio::task::spawn_blocking(move || {
            editor.readline(&format!("{}: ", prompt))
        })
        .await?
    }
}
```

**Why this works**:
- `spawn_blocking()` runs synchronous code without blocking async runtime
- User can type while events are being processed
- Multiple workers can run while waiting for input
- Clean separation: input reading is independent of event processing

**Alternative Design** (if you want to stop reading during job execution):

```rust
fn spawn_prompt_loop(&self) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Wait for "ready for input" signal
            prompt_queue.wait_for_prompt().await;
            
            // Now read input
            let input = input_handler.read_line("You").await?;
            
            // Send command
            cmd_sender.send(PromptResult::Command(command)).await?;
        }
    })
}
```

But the simpler design (always reading) works fine because:
- User can type ahead
- Commands are queued
- AgentEnvironment processes commands sequentially

**DM Response**: Make TextUi stop reading during job execution (use this alternative design), but keep 'always reading' in StructuredIO

**Resolved** - Will implement different behaviors:
- **TextUi**: Use prompt queue pattern - only read input when worker is ready (Idle state). Prevents confusing UX where user types while agent is working.
- **StructuredIO**: Always read input (simpler pattern). Suitable for scripting where commands may be piped in.

**Implementation Note**: 
- TextUi needs `PromptQueue` or similar mechanism to signal when to read input
- Listen to `WorkerEvent::LifecycleStateChanged` to know when worker becomes Idle
- StructuredIO keeps simple always-reading loop

---

**End of Design Document**

