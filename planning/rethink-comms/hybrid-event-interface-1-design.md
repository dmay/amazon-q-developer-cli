# Hybrid Event Bus + Direct Interfaces - Detailed Design

## Executive Summary

This design combines event-driven architecture for asynchronous notifications with direct trait-based interfaces for synchronous interactions. It provides loose coupling for display updates while maintaining clear contracts for critical user interactions.

**Key Innovation**: Right tool for the right job - events for fire-and-forget notifications, interfaces for request-response interactions.

## Design Goals

1. **Flexible UI Swapping**: Easy to replace or combine UI implementations
2. **Clean Separation**: Core logic independent of UI presentation
3. **Parallel Workers**: Support multiple concurrent agents
4. **Streaming Support**: Efficient handling of LLM response streams
5. **Clear Contracts**: Explicit interfaces for critical interactions
6. **Loose Coupling**: UI can subscribe/unsubscribe from events dynamically

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Loop                         │
│  - Creates Session with UI interfaces                        │
│  - Spawns event subscriber tasks                             │
│  - Coordinates shutdown                                       │
└────────────┬────────────────────────────────────────────────┘
             │
             ├──────────────┬──────────────┬──────────────┐
             ▼              ▼              ▼              ▼
      ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
      │ Session  │   │ EventBus │   │ Prompt   │   │   Tool   │
      │          │   │          │   │ Provider │   │ Approver │
      └────┬─────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘
           │              │              │              │
           │              │              │              │
      ┌────▼──────────────▼──────────────▼──────────────▼─────┐
      │                  UI Implementation                      │
      │  - Subscribes to events                                 │
      │  - Implements provider interfaces                       │
      │  - Manages display state                                │
      └─────────────────────────────────────────────────────────┘
```

### Communication Patterns

**Event Bus (Async, Fire-and-Forget)**:
- Worker state changes
- Streaming response chunks
- Job lifecycle events
- Progress updates
- Error notifications

**Direct Interfaces (Sync/Async, Request-Response)**:
- `PromptProvider`: Get user input
- `ToolApprover`: Approve/deny tool execution
- `ErrorHandler`: Handle errors, decide recovery
- `ShutdownCoordinator`: Coordinate graceful shutdown

## Core Components

### 1. Event Bus

Central event distribution system using tokio broadcast channels.

#### Event Types

```rust
/// All events that can be published to the event bus
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // Worker lifecycle
    WorkerCreated {
        worker_id: Uuid,
        worker_name: String,
    },
    WorkerDeleted {
        worker_id: Uuid,
    },
    
    // Worker state changes
    WorkerStateChanged {
        worker_id: Uuid,
        old_state: WorkerStates,
        new_state: WorkerStates,
        timestamp: Instant,
    },
    
    // Job lifecycle
    JobStarted {
        worker_id: Uuid,
        job_id: Uuid,
        task_type: String,
        timestamp: Instant,
    },
    JobCompleted {
        worker_id: Uuid,
        job_id: Uuid,
        completion_type: WorkerJobCompletionType,
        error_message: Option<String>,
        duration: Duration,
        timestamp: Instant,
    },
    
    // Streaming output
    ResponseChunkReceived {
        worker_id: Uuid,
        chunk: ModelResponseChunk,
        timestamp: Instant,
    },
    
    // Tool execution
    ToolExecutionStarted {
        worker_id: Uuid,
        tool_name: String,
        parameters: String,
        timestamp: Instant,
    },
    ToolExecutionCompleted {
        worker_id: Uuid,
        tool_name: String,
        success: bool,
        duration: Duration,
        timestamp: Instant,
    },
    
    // Progress updates
    ProgressUpdate {
        worker_id: Uuid,
        message: String,
        percentage: Option<u8>,
        timestamp: Instant,
    },
    
    // System events
    SystemShutdownInitiated {
        reason: String,
        timestamp: Instant,
    },
}
```

#### EventBus Implementation

```rust
use tokio::sync::broadcast;
use std::sync::Arc;

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    /// Publish event to all subscribers
    pub fn publish(&self, event: AgentEvent) {
        // Ignore send errors (no subscribers is OK)
        let _ = self.sender.send(event);
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
    
    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
```

### 2. Direct Interfaces

Trait-based interfaces for synchronous interactions requiring responses.

#### PromptProvider Interface

```rust
use uuid::Uuid;
use tokio_util::sync::CancellationToken;

/// Provides user input when requested by workers
#[async_trait::async_trait]
pub trait PromptProvider: Send + Sync {
    /// Request user input for a specific worker
    /// 
    /// # Arguments
    /// * `worker_id` - ID of worker requesting input
    /// * `worker_name` - Display name of worker
    /// * `context` - Optional context about what input is needed
    /// * `cancellation_token` - Token to cancel the request
    /// 
    /// # Returns
    /// User input string, or error if cancelled/failed
    async fn get_prompt(
        &self,
        worker_id: Uuid,
        worker_name: &str,
        context: Option<&str>,
        cancellation_token: CancellationToken,
    ) -> Result<String, PromptError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("Prompt cancelled")]
    Cancelled,
    #[error("Input error: {0}")]
    InputError(String),
    #[error("Shutdown requested")]
    Shutdown,
}
```

#### ToolApprover Interface

```rust
/// Approves or denies tool execution requests
#[async_trait::async_trait]
pub trait ToolApprover: Send + Sync {
    /// Request approval for tool execution
    /// 
    /// # Arguments
    /// * `worker_id` - ID of worker requesting approval
    /// * `request` - Details about the tool execution
    /// * `cancellation_token` - Token to cancel the request
    /// 
    /// # Returns
    /// Approval decision
    async fn approve_tool(
        &self,
        worker_id: Uuid,
        request: ToolApprovalRequest,
        cancellation_token: CancellationToken,
    ) -> Result<ToolApprovalDecision, ToolApprovalError>;
}

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub description: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,      // Read-only operations
    Medium,   // Write operations, reversible
    High,     // Destructive operations, irreversible
}

#[derive(Debug, Clone)]
pub enum ToolApprovalDecision {
    Approved,
    ApprovedOnce,
    ApprovedAlways { remember_for_session: bool },
    Denied,
    DeniedAlways,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolApprovalError {
    #[error("Approval cancelled")]
    Cancelled,
    #[error("Approval timeout")]
    Timeout,
    #[error("Shutdown requested")]
    Shutdown,
}
```

#### ErrorHandler Interface

```rust
/// Handles errors and decides recovery strategy
#[async_trait::async_trait]
pub trait ErrorHandler: Send + Sync {
    /// Handle an error and decide recovery strategy
    /// 
    /// # Arguments
    /// * `worker_id` - ID of worker that encountered error
    /// * `error` - The error that occurred
    /// * `context` - Context about what was happening
    /// 
    /// # Returns
    /// Recovery decision
    async fn handle_error(
        &self,
        worker_id: Uuid,
        error: &eyre::Error,
        context: ErrorContext,
    ) -> ErrorRecoveryDecision;
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub phase: ExecutionPhase,
    pub retry_count: u32,
    pub last_successful_state: Option<WorkerStates>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    Initialization,
    LlmRequest,
    LlmStreaming,
    ToolExecution,
    Finalization,
}

#[derive(Debug, Clone)]
pub enum ErrorRecoveryDecision {
    Retry { delay: Duration },
    RetryWithDifferentModel,
    Skip,
    Abort,
    PromptUser { message: String },
}
```

#### ShutdownCoordinator Interface

```rust
/// Coordinates graceful shutdown
#[async_trait::async_trait]
pub trait ShutdownCoordinator: Send + Sync {
    /// Notify that shutdown has been requested
    async fn shutdown_requested(&self, reason: ShutdownReason);
    
    /// Check if shutdown is in progress
    fn is_shutting_down(&self) -> bool;
    
    /// Wait for shutdown to complete
    async fn wait_for_shutdown(&self);
}

#[derive(Debug, Clone)]
pub enum ShutdownReason {
    UserRequest,
    CtrlC,
    Error(String),
    Completion,
}
```

## Session Integration

### Modified Session Structure

```rust
pub struct Session {
    model_providers: Vec<Arc<dyn ModelProvider>>,
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>,
    
    // NEW: Event bus for async notifications
    event_bus: EventBus,
    
    // NEW: Direct interfaces for sync interactions
    prompt_provider: Arc<dyn PromptProvider>,
    tool_approver: Arc<dyn ToolApprover>,
    error_handler: Arc<dyn ErrorHandler>,
    shutdown_coordinator: Arc<dyn ShutdownCoordinator>,
}

impl Session {
    pub fn new(
        model_providers: Vec<Arc<dyn ModelProvider>>,
        event_bus: EventBus,
        prompt_provider: Arc<dyn PromptProvider>,
        tool_approver: Arc<dyn ToolApprover>,
        error_handler: Arc<dyn ErrorHandler>,
        shutdown_coordinator: Arc<dyn ShutdownCoordinator>,
    ) -> Self {
        Self {
            model_providers,
            workers: Arc::new(Mutex::new(Vec::new())),
            jobs: Arc::new(Mutex::new(Vec::new())),
            event_bus,
            prompt_provider,
            tool_approver,
            error_handler,
            shutdown_coordinator,
        }
    }
    
    pub fn build_worker(&self, name: String) -> Arc<Worker> {
        let model_provider = self.model_providers.first()
            .expect("At least one model provider required")
            .clone();
        
        let worker = Arc::new(Worker::new(
            name.clone(),
            model_provider,
            self.event_bus.clone(),
        ));
        
        self.workers.lock().unwrap().push(worker.clone());
        
        // Publish event
        self.event_bus.publish(AgentEvent::WorkerCreated {
            worker_id: worker.id,
            worker_name: name,
        });
        
        worker
    }
    
    pub fn run_agent_loop(
        &self,
        worker: Arc<Worker>,
        input: AgentLoopInput,
    ) -> Result<Arc<WorkerJob>, eyre::Error> {
        let cancellation_token = CancellationToken::new();
        
        let agent_loop = Arc::new(AgentLoop::new(
            worker.clone(),
            input,
            self.event_bus.clone(),
            self.tool_approver.clone(),
            self.error_handler.clone(),
            cancellation_token.clone(),
        ));
        
        self.run(worker, agent_loop, cancellation_token)
    }
    
    fn run(
        &self,
        worker: Arc<Worker>,
        worker_task: Arc<dyn WorkerTask>,
        cancellation_token: CancellationToken,
    ) -> Result<Arc<WorkerJob>, eyre::Error> {
        let job_id = Uuid::new_v4();
        let task_type = worker_task.task_type().to_string();
        
        let mut job = WorkerJob::new(
            job_id,
            worker.clone(),
            worker_task,
            cancellation_token,
            self.event_bus.clone(),
        );
        
        // Publish event
        self.event_bus.publish(AgentEvent::JobStarted {
            worker_id: worker.id,
            job_id,
            task_type,
            timestamp: Instant::now(),
        });
        
        job.launch();
        
        let job = Arc::new(job);
        self.jobs.lock().unwrap().push(job.clone());
        Ok(job)
    }
    
    pub fn get_prompt_provider(&self) -> Arc<dyn PromptProvider> {
        self.prompt_provider.clone()
    }
    
    pub fn get_event_bus(&self) -> EventBus {
        self.event_bus.clone()
    }
}
```

### Modified Worker Structure

```rust
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub context_container: ContextContainer,
    pub model_provider: Arc<dyn ModelProvider>,
    pub state: Arc<Mutex<WorkerStates>>,
    pub last_failure: Arc<Mutex<Option<String>>>,
    
    // NEW: Event bus for publishing state changes
    event_bus: EventBus,
}

impl Worker {
    pub fn new(
        name: String,
        model_provider: Arc<dyn ModelProvider>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            context_container: ContextContainer::new(),
            model_provider,
            state: Arc::new(Mutex::new(WorkerStates::Inactive)),
            last_failure: Arc::new(Mutex::new(None)),
            event_bus,
        }
    }
    
    pub fn set_state(&self, new_state: WorkerStates) {
        let old_state = {
            let mut state = self.state.lock().unwrap();
            let old = *state;
            *state = new_state;
            old
        };
        
        // Publish event
        self.event_bus.publish(AgentEvent::WorkerStateChanged {
            worker_id: self.id,
            old_state,
            new_state,
            timestamp: Instant::now(),
        });
    }
    
    pub fn get_state(&self) -> WorkerStates {
        *self.state.lock().unwrap()
    }
}
```

### Modified WorkerJob Structure

```rust
pub struct WorkerJob {
    pub id: Uuid,
    pub worker: Arc<Worker>,
    pub worker_task: Arc<dyn WorkerTask>,
    pub cancellation_token: CancellationToken,
    pub task_handle: Option<tokio::task::JoinHandle<Result<(), eyre::Error>>>,
    pub worker_job_continuations: Arc<Continuations>,
    
    // NEW: Event bus and start time for metrics
    event_bus: EventBus,
    start_time: Instant,
}

impl WorkerJob {
    pub fn new(
        id: Uuid,
        worker: Arc<Worker>,
        worker_task: Arc<dyn WorkerTask>,
        cancellation_token: CancellationToken,
        event_bus: EventBus,
    ) -> Self {
        Self {
            id,
            worker,
            worker_task,
            cancellation_token,
            task_handle: None,
            worker_job_continuations: Arc::new(Continuations::new()),
            event_bus,
            start_time: Instant::now(),
        }
    }
    
    pub fn launch(&mut self) {
        let worker_task_clone = self.worker_task.clone();
        let continuations = Arc::clone(&self.worker_job_continuations);
        let worker = Arc::clone(&self.worker);
        let cancellation_token = self.cancellation_token.clone();
        let event_bus = self.event_bus.clone();
        let job_id = self.id;
        let start_time = self.start_time;
        
        let task_handle = tokio::spawn(async move {
            let result = worker_task_clone.run().await;
            
            // Publish completion event
            let duration = start_time.elapsed();
            let (completion_type, error_message) = match &result {
                Ok(_) => (WorkerJobCompletionType::Normal, None),
                Err(e) if cancellation_token.is_cancelled() => {
                    (WorkerJobCompletionType::Cancelled, None)
                }
                Err(e) => (WorkerJobCompletionType::Failed, Some(e.to_string())),
            };
            
            event_bus.publish(AgentEvent::JobCompleted {
                worker_id: worker.id,
                job_id,
                completion_type,
                error_message,
                duration,
                timestamp: Instant::now(),
            });
            
            continuations.complete(result, worker, &cancellation_token).await;
            Ok(())
        });
        
        self.task_handle = Some(task_handle);
    }
}
```

## Modified AgentLoop Task

```rust
pub struct AgentLoop {
    worker: Arc<Worker>,
    cancellation_token: CancellationToken,
    event_bus: EventBus,
    tool_approver: Arc<dyn ToolApprover>,
    error_handler: Arc<dyn ErrorHandler>,
}

impl AgentLoop {
    pub fn new(
        worker: Arc<Worker>,
        _input: AgentLoopInput,
        event_bus: EventBus,
        tool_approver: Arc<dyn ToolApprover>,
        error_handler: Arc<dyn ErrorHandler>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            worker,
            event_bus,
            tool_approver,
            error_handler,
            cancellation_token,
        }
    }
    
    async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
        self.check_cancellation()?;
        
        // Get prompt from worker's context
        let prompt = self.get_prompt_from_history()?;
        let request = ModelRequest { prompt };
        
        self.worker.set_state(WorkerStates::Requesting);
        
        let worker_id = self.worker.id;
        let event_bus = self.event_bus.clone();
        let event_bus2 = self.event_bus.clone();
        
        let response = self.worker.model_provider.request(
            request,
            Box::new(move || {
                // Transition to receiving state
                event_bus.publish(AgentEvent::WorkerStateChanged {
                    worker_id,
                    old_state: WorkerStates::Requesting,
                    new_state: WorkerStates::Receiving,
                    timestamp: Instant::now(),
                });
            }),
            Box::new(move |chunk| {
                // Publish streaming chunk
                event_bus2.publish(AgentEvent::ResponseChunkReceived {
                    worker_id,
                    chunk,
                    timestamp: Instant::now(),
                });
            }),
            self.cancellation_token.clone(),
        ).await.map_err(|e| {
            if !self.cancellation_token.is_cancelled() {
                let error_msg = format!("LLM request failed: {}", e);
                self.worker.set_failure(error_msg);
                self.worker.set_state(WorkerStates::InactiveFailed);
            } else {
                self.worker.set_state(WorkerStates::Inactive);
            }
            e
        })?;
        
        Ok(response)
    }
    
    async fn execute_tools(
        &self,
        tool_requests: Vec<ToolRequest>,
    ) -> Result<Vec<ToolResult>, eyre::Error> {
        let mut results = Vec::new();
        
        for tool_req in tool_requests {
            // Request approval
            let approval_request = ToolApprovalRequest {
                tool_name: tool_req.name.clone(),
                parameters: tool_req.parameters.clone(),
                description: tool_req.description.clone(),
                risk_level: self.assess_risk(&tool_req),
            };
            
            let decision = self.tool_approver.approve_tool(
                self.worker.id,
                approval_request,
                self.cancellation_token.clone(),
            ).await?;
            
            match decision {
                ToolApprovalDecision::Approved
                | ToolApprovalDecision::ApprovedOnce
                | ToolApprovalDecision::ApprovedAlways { .. } => {
                    // Execute tool
                    self.event_bus.publish(AgentEvent::ToolExecutionStarted {
                        worker_id: self.worker.id,
                        tool_name: tool_req.name.clone(),
                        parameters: serde_json::to_string(&tool_req.parameters)
                            .unwrap_or_default(),
                        timestamp: Instant::now(),
                    });
                    
                    let start = Instant::now();
                    let result = self.execute_single_tool(&tool_req).await;
                    let duration = start.elapsed();
                    
                    self.event_bus.publish(AgentEvent::ToolExecutionCompleted {
                        worker_id: self.worker.id,
                        tool_name: tool_req.name.clone(),
                        success: result.is_ok(),
                        duration,
                        timestamp: Instant::now(),
                    });
                    
                    results.push(result?);
                }
                ToolApprovalDecision::Denied | ToolApprovalDecision::DeniedAlways => {
                    results.push(ToolResult {
                        tool_name: tool_req.name,
                        success: false,
                        output: "Tool execution denied by user".to_string(),
                    });
                }
            }
        }
        
        Ok(results)
    }
}

#[async_trait::async_trait]
impl WorkerTask for AgentLoop {
    fn task_type(&self) -> &str {
        "AgentLoop"
    }
    
    fn get_worker(&self) -> &Worker {
        &self.worker
    }
    
    async fn run(&self) -> Result<(), eyre::Error> {
        self.check_cancellation()?;
        self.worker.set_state(WorkerStates::Working);
        
        let response = self.query_llm().await?;
        
        // Handle tool requests if any
        if !response.tool_requests.is_empty() {
            self.worker.set_state(WorkerStates::UsingTool);
            let _tool_results = self.execute_tools(response.tool_requests).await?;
            // Tool results would be added to context and loop continues
        }
        
        // Add response to history
        let assistant_message = AssistantMessage::new_response(
            None,
            response.content.clone(),
        );
        self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_assistant_message(assistant_message);
        
        self.worker.set_state(WorkerStates::Inactive);
        Ok(())
    }
}
```

## UI Implementation Example

### TextUI with Hybrid Pattern

```rust
pub struct TextUi {
    session: Arc<Session>,
    event_bus: EventBus,
    input_handler: InputHandler,
    shutdown_signal: Arc<Notify>,
    
    // State tracking from events
    worker_states: Arc<Mutex<HashMap<Uuid, WorkerStates>>>,
    active_jobs: Arc<Mutex<HashSet<Uuid>>>,
    
    // Prompt queue for managing input requests
    prompt_queue: Arc<PromptQueue>,
}

impl TextUi {
    pub fn new(
        session: Arc<Session>,
        history_path: Option<PathBuf>,
    ) -> Result<Self, eyre::Error> {
        let event_bus = session.get_event_bus();
        
        Ok(Self {
            session,
            event_bus,
            input_handler: InputHandler::new(history_path)?,
            shutdown_signal: Arc::new(Notify::new()),
            worker_states: Arc::new(Mutex::new(HashMap::new())),
            active_jobs: Arc::new(Mutex::new(HashSet::new())),
            prompt_queue: Arc::new(PromptQueue::new()),
        })
    }
    
    /// Build and return all interface implementations
    pub fn build_interfaces(self: &Arc<Self>) -> SessionInterfaces {
        SessionInterfaces {
            prompt_provider: Arc::new(TextUiPromptProvider {
                ui: self.clone(),
            }),
            tool_approver: Arc::new(TextUiToolApprover {
                ui: self.clone(),
            }),
            error_handler: Arc::new(TextUiErrorHandler {
                ui: self.clone(),
            }),
            shutdown_coordinator: Arc::new(TextUiShutdownCoordinator {
                ui: self.clone(),
            }),
        }
    }
    
    pub async fn run(self: Arc<Self>) -> Result<(), eyre::Error> {
        // Spawn event subscriber task
        let event_subscriber = self.clone();
        let event_task = tokio::spawn(async move {
            event_subscriber.event_loop().await
        });
        
        // Spawn Ctrl+C handler
        let ctrl_c_ui = self.clone();
        tokio::spawn(async move {
            if ctrl_c().await.is_ok() {
                ctrl_c_ui.shutdown_signal.notify_waiters();
            }
        });
        
        // Main prompt loop
        self.prompt_loop().await?;
        
        // Cleanup
        event_task.abort();
        Ok(())
    }
    
    /// Event subscriber loop - processes all events from event bus
    async fn event_loop(&self) {
        let mut receiver = self.event_bus.subscribe();
        
        loop {
            tokio::select! {
                _ = self.shutdown_signal.notified() => {
                    break;
                }
                event = receiver.recv() => {
                    match event {
                        Ok(event) => self.handle_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("Warning: Event bus lagged by {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    }
    
    /// Handle individual events
    async fn handle_event(&self, event: AgentEvent) {
        match event {
            AgentEvent::WorkerStateChanged { worker_id, new_state, .. } => {
                self.worker_states.lock().unwrap().insert(worker_id, new_state);
                
                match new_state {
                    WorkerStates::Working => {
                        println!("\n[Worker {} thinking...]", worker_id);
                    }
                    WorkerStates::Requesting => {
                        print!("🤔 ");
                        io::stdout().flush().unwrap();
                    }
                    WorkerStates::Receiving => {
                        // Start of streaming
                    }
                    WorkerStates::Inactive => {
                        println!();
                    }
                    _ => {}
                }
            }
            
            AgentEvent::ResponseChunkReceived { chunk, .. } => {
                match chunk {
                    ModelResponseChunk::AssistantMessage(text) => {
                        print!("{}", text);
                        io::stdout().flush().unwrap();
                    }
                    ModelResponseChunk::ToolUseRequest { tool_name, parameters } => {
                        println!("\n[Tool: {} with params: {}]", tool_name, parameters);
                    }
                }
            }
            
            AgentEvent::JobStarted { worker_id, job_id, .. } => {
                self.active_jobs.lock().unwrap().insert(job_id);
                tracing::debug!("Job {} started for worker {}", job_id, worker_id);
            }
            
            AgentEvent::JobCompleted { job_id, completion_type, error_message, .. } => {
                self.active_jobs.lock().unwrap().remove(&job_id);
                
                match completion_type {
                    WorkerJobCompletionType::Failed => {
                        if let Some(msg) = error_message {
                            eprintln!("\n❌ Error: {}", msg);
                        }
                    }
                    WorkerJobCompletionType::Cancelled => {
                        println!("\n⚠️  Task cancelled");
                    }
                    WorkerJobCompletionType::Normal => {}
                }
            }
            
            AgentEvent::ToolExecutionStarted { tool_name, .. } => {
                println!("\n🔧 Executing tool: {}", tool_name);
            }
            
            AgentEvent::ToolExecutionCompleted { tool_name, success, duration, .. } => {
                if success {
                    println!("✅ Tool {} completed in {:?}", tool_name, duration);
                } else {
                    println!("❌ Tool {} failed", tool_name);
                }
            }
            
            AgentEvent::SystemShutdownInitiated { reason, .. } => {
                println!("\nShutting down: {}", reason);
                self.shutdown_signal.notify_waiters();
            }
            
            _ => {}
        }
    }
    
    /// Main prompt loop - waits for prompt requests and handles input
    async fn prompt_loop(&self) -> Result<(), eyre::Error> {
        loop {
            tokio::select! {
                _ = self.shutdown_signal.notified() => {
                    break;
                }
                _ = self.prompt_queue.wait_for_items() => {
                    // Process prompt request
                    if let Some(request) = self.prompt_queue.dequeue().await {
                        self.handle_prompt_request(request).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_prompt_request(
        &self,
        request: PromptRequest,
    ) -> Result<(), eyre::Error> {
        // Read input
        let input = self.input_handler
            .read_line(&request.worker_name)
            .await?;
        
        if input.trim().is_empty() {
            // Re-queue
            self.prompt_queue.enqueue(request).await;
            return Ok(());
        }
        
        // Handle commands
        if input.trim() == "/quit" {
            self.shutdown_signal.notify_waiters();
            return Ok(());
        }
        
        // Add to history
        request.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_input_message(input);
        
        // Launch agent loop
        let job = self.session.run_agent_loop(
            request.worker.clone(),
            AgentLoopInput {},
        )?;
        
        // Set up continuation to re-queue prompt
        let prompt_queue = self.prompt_queue.clone();
        let continuation = Continuations::boxed(move |worker, _, _| {
            let prompt_queue = prompt_queue.clone();
            async move {
                prompt_queue.enqueue(PromptRequest { worker }).await;
            }
        });
        
        job.worker_job_continuations.add_or_run_now(
            "re_queue_prompt",
            continuation,
            request.worker,
        ).await;
        
        Ok(())
    }
}

/// Container for all interface implementations
pub struct SessionInterfaces {
    pub prompt_provider: Arc<dyn PromptProvider>,
    pub tool_approver: Arc<dyn ToolApprover>,
    pub error_handler: Arc<dyn ErrorHandler>,
    pub shutdown_coordinator: Arc<dyn ShutdownCoordinator>,
}

/// PromptProvider implementation for TextUI
struct TextUiPromptProvider {
    ui: Arc<TextUi>,
}

#[async_trait::async_trait]
impl PromptProvider for TextUiPromptProvider {
    async fn get_prompt(
        &self,
        worker_id: Uuid,
        worker_name: &str,
        _context: Option<&str>,
        cancellation_token: CancellationToken,
    ) -> Result<String, PromptError> {
        // Find worker
        let worker = {
            let workers = self.ui.session.workers.lock().unwrap();
            workers.iter()
                .find(|w| w.id == worker_id)
                .cloned()
                .ok_or(PromptError::InputError("Worker not found".to_string()))?
        };
        
        // Create prompt request
        let request = PromptRequest {
            worker,
            worker_name: worker_name.to_string(),
            cancellation_token: cancellation_token.clone(),
        };
        
        // Enqueue and wait for response
        self.ui.prompt_queue.enqueue(request).await;
        
        // Wait for input to be processed
        // (In real implementation, would use a response channel)
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                Err(PromptError::Cancelled)
            }
            _ = self.ui.shutdown_signal.notified() => {
                Err(PromptError::Shutdown)
            }
        }
    }
}

/// ToolApprover implementation for TextUI
struct TextUiToolApprover {
    ui: Arc<TextUi>,
}

#[async_trait::async_trait]
impl ToolApprover for TextUiToolApprover {
    async fn approve_tool(
        &self,
        _worker_id: Uuid,
        request: ToolApprovalRequest,
        cancellation_token: CancellationToken,
    ) -> Result<ToolApprovalDecision, ToolApprovalError> {
        // For now, auto-approve low risk, prompt for others
        match request.risk_level {
            RiskLevel::Low => Ok(ToolApprovalDecision::Approved),
            RiskLevel::Medium | RiskLevel::High => {
                // TODO: Implement interactive approval
                // For now, auto-approve
                Ok(ToolApprovalDecision::Approved)
            }
        }
    }
}

/// ErrorHandler implementation for TextUI
struct TextUiErrorHandler {
    ui: Arc<TextUi>,
}

#[async_trait::async_trait]
impl ErrorHandler for TextUiErrorHandler {
    async fn handle_error(
        &self,
        _worker_id: Uuid,
        error: &eyre::Error,
        context: ErrorContext,
    ) -> ErrorRecoveryDecision {
        // Simple strategy: retry once, then abort
        if context.retry_count < 1 {
            ErrorRecoveryDecision::Retry {
                delay: Duration::from_secs(1),
            }
        } else {
            eprintln!("Error: {}", error);
            ErrorRecoveryDecision::Abort
        }
    }
}

/// ShutdownCoordinator implementation for TextUI
struct TextUiShutdownCoordinator {
    ui: Arc<TextUi>,
}

#[async_trait::async_trait]
impl ShutdownCoordinator for TextUiShutdownCoordinator {
    async fn shutdown_requested(&self, reason: ShutdownReason) {
        self.ui.event_bus.publish(AgentEvent::SystemShutdownInitiated {
            reason: format!("{:?}", reason),
            timestamp: Instant::now(),
        });
        self.ui.shutdown_signal.notify_waiters();
    }
    
    fn is_shutting_down(&self) -> bool {
        // Check if shutdown signal has been triggered
        // (Would need additional state tracking in real implementation)
        false
    }
    
    async fn wait_for_shutdown(&self) {
        self.ui.shutdown_signal.notified().await;
    }
}
```

## Application Loop and Entry Point

### Main Entry Point (ChatArgs::execute)

```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // 1. Create event bus
        let event_bus = EventBus::new(1000); // 1000 event capacity
        
        // 2. Build model providers
        let model_providers = vec![
            Arc::new(BedrockConverseStreamProvider::new()?) as Arc<dyn ModelProvider>
        ];
        
        // 3. Create UI
        let history_path = directories::chat_cli_bash_history_path(os).ok();
        let ui = Arc::new(TextUi::new_placeholder(event_bus.clone(), history_path)?);
        
        // 4. Build interface implementations from UI
        let interfaces = ui.build_interfaces();
        
        // 5. Create session with all dependencies
        let session = Arc::new(Session::new(
            model_providers,
            event_bus,
            interfaces.prompt_provider,
            interfaces.tool_approver,
            interfaces.error_handler,
            interfaces.shutdown_coordinator,
        ));
        
        // 6. Complete UI initialization with session
        let ui = ui.with_session(session.clone());
        
        // 7. Create initial worker(s)
        let worker = session.build_worker("Assistant".to_string());
        
        // 8. Handle initial input if provided
        if let Some(input) = self.input {
            worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(input);
            
            // Launch agent loop
            let job = session.run_agent_loop(worker.clone(), AgentLoopInput {})?;
            
            // Set up continuation to queue prompt when done
            let prompt_queue = ui.prompt_queue.clone();
            let continuation = Continuations::boxed(move |worker, _, _| {
                let prompt_queue = prompt_queue.clone();
                async move {
                    prompt_queue.enqueue(PromptRequest {
                        worker,
                        worker_name: "Assistant".to_string(),
                        cancellation_token: CancellationToken::new(),
                    }).await;
                }
            });
            
            job.worker_job_continuations.add_or_run_now(
                "initial_prompt",
                continuation,
                worker,
            ).await;
        } else {
            // No initial input - queue prompt immediately
            ui.prompt_queue.enqueue(PromptRequest {
                worker,
                worker_name: "Assistant".to_string(),
                cancellation_token: CancellationToken::new(),
            }).await;
        }
        
        // 9. Run UI (blocks until shutdown)
        ui.run().await?;
        
        // 10. Cleanup
        session.cancel_all_jobs();
        session.wait_for_all_jobs().await;
        
        println!("Goodbye!");
        Ok(ExitCode::SUCCESS)
    }
}
```

### Simplified Application Loop Structure

```rust
/// High-level application loop structure
pub struct ApplicationLoop {
    session: Arc<Session>,
    ui: Arc<dyn UserInterface>,
    shutdown_signal: Arc<Notify>,
}

impl ApplicationLoop {
    pub async fn run(&self) -> Result<(), eyre::Error> {
        // Spawn event subscriber
        let ui_clone = self.ui.clone();
        let shutdown_clone = self.shutdown_signal.clone();
        let event_task = tokio::spawn(async move {
            ui_clone.event_loop(shutdown_clone).await
        });
        
        // Spawn Ctrl+C handler
        let shutdown_clone = self.shutdown_signal.clone();
        tokio::spawn(async move {
            if ctrl_c().await.is_ok() {
                shutdown_clone.notify_waiters();
            }
        });
        
        // Run main UI loop
        tokio::select! {
            result = self.ui.main_loop() => {
                result?;
            }
            _ = self.shutdown_signal.notified() => {
                // Shutdown requested
            }
        }
        
        // Cleanup
        event_task.abort();
        self.session.cancel_all_jobs();
        self.session.wait_for_all_jobs().await;
        
        Ok(())
    }
}

/// Trait for UI implementations
#[async_trait::async_trait]
pub trait UserInterface: Send + Sync {
    /// Main UI loop (prompt handling, etc.)
    async fn main_loop(&self) -> Result<(), eyre::Error>;
    
    /// Event subscriber loop
    async fn event_loop(&self, shutdown: Arc<Notify>) -> Result<(), eyre::Error>;
    
    /// Build interface implementations
    fn build_interfaces(&self) -> SessionInterfaces;
}
```

## Event Flow Diagrams

### User Input Flow

```
User Input
    │
    ▼
InputHandler.read_line()
    │
    ▼
Add to Worker.context_container
    │
    ▼
Session.run_agent_loop()
    │
    ├─► EventBus.publish(JobStarted)
    │
    ▼
AgentLoop.run()
    │
    ├─► Worker.set_state(Working)
    │   └─► EventBus.publish(WorkerStateChanged)
    │
    ├─► Worker.set_state(Requesting)
    │   └─► EventBus.publish(WorkerStateChanged)
    │
    ├─► ModelProvider.request()
    │   │
    │   ├─► on_start_streaming()
    │   │   └─► EventBus.publish(WorkerStateChanged: Receiving)
    │   │
    │   └─► on_chunk()
    │       └─► EventBus.publish(ResponseChunkReceived)
    │
    ├─► Worker.set_state(Inactive)
    │   └─► EventBus.publish(WorkerStateChanged)
    │
    └─► EventBus.publish(JobCompleted)
        │
        ▼
    Continuation runs
        │
        ▼
    PromptQueue.enqueue()
        │
        ▼
    Back to User Input
```

### Tool Execution Flow

```
AgentLoop detects tool requests
    │
    ▼
ToolApprover.approve_tool()
    │
    ├─► User approves
    │   │
    │   ├─► EventBus.publish(ToolExecutionStarted)
    │   │
    │   ├─► Execute tool
    │   │
    │   └─► EventBus.publish(ToolExecutionCompleted)
    │
    └─► User denies
        │
        └─► Return denial result
```

### Shutdown Flow

```
Ctrl+C pressed
    │
    ▼
ShutdownCoordinator.shutdown_requested()
    │
    ├─► EventBus.publish(SystemShutdownInitiated)
    │
    └─► shutdown_signal.notify_waiters()
        │
        ├─► UI event loop exits
        │
        ├─► UI main loop exits
        │
        └─► Session.cancel_all_jobs()
            │
            └─► Session.wait_for_all_jobs()
                │
                └─► Application exits
```

## File Structure

```
crates/chat-cli/src/
├── agent_env/
│   ├── mod.rs
│   ├── session.rs                    # Modified: add event_bus, interfaces
│   ├── worker.rs                     # Modified: add event_bus
│   ├── worker_job.rs                 # Modified: add event_bus, metrics
│   ├── worker_task.rs                # Modified: add task_type()
│   │
│   ├── events/                       # NEW
│   │   ├── mod.rs
│   │   ├── event_bus.rs             # EventBus implementation
│   │   └── agent_event.rs           # AgentEvent enum
│   │
│   ├── interfaces/                   # NEW
│   │   ├── mod.rs
│   │   ├── prompt_provider.rs       # PromptProvider trait
│   │   ├── tool_approver.rs         # ToolApprover trait
│   │   ├── error_handler.rs         # ErrorHandler trait
│   │   └── shutdown_coordinator.rs  # ShutdownCoordinator trait
│   │
│   ├── worker_tasks/
│   │   ├── mod.rs
│   │   └── agent_loop.rs            # Modified: use event_bus, interfaces
│   │
│   └── ...
│
└── cli/chat/
    ├── mod.rs                        # Modified: new entry point
    │
    └── ui/                           # NEW (renamed from agent_env_ui)
        ├── mod.rs
        ├── text_ui.rs                # Main TextUI implementation
        ├── text_ui_prompt.rs         # PromptProvider impl
        ├── text_ui_tool.rs           # ToolApprover impl
        ├── text_ui_error.rs          # ErrorHandler impl
        ├── text_ui_shutdown.rs       # ShutdownCoordinator impl
        ├── prompt_queue.rs           # Modified: add response channel
        ├── input_handler.rs          # Unchanged
        └── session_interfaces.rs     # SessionInterfaces struct
```

## Migration Path

### Phase 1: Add Event Bus Infrastructure (Non-Breaking)

**Goal**: Introduce event bus alongside existing WorkerToHostInterface

**Changes**:
1. Add `events/` module with EventBus and AgentEvent
2. Add EventBus to Session (optional parameter for backward compatibility)
3. Publish events in parallel with existing interface calls
4. No changes to existing UI code

**Code Example**:
```rust
// In Worker::set_state()
pub fn set_state(&self, new_state: WorkerStates, interface: &dyn WorkerToHostInterface) {
    let old_state = {
        let mut state = self.state.lock().unwrap();
        let old = *state;
        *state = new_state;
        old
    };
    
    // Existing interface call
    interface.worker_state_change(self.id, new_state);
    
    // NEW: Also publish event if event bus available
    if let Some(event_bus) = &self.event_bus {
        event_bus.publish(AgentEvent::WorkerStateChanged {
            worker_id: self.id,
            old_state,
            new_state,
            timestamp: Instant::now(),
        });
    }
}
```

**Testing**: Run existing demo, verify events are published (add logging subscriber)

### Phase 2: Add Interface Traits (Parallel Implementation)

**Goal**: Introduce new interface traits, implement alongside existing code

**Changes**:
1. Add `interfaces/` module with all trait definitions
2. Create TextUI implementations of new interfaces
3. Keep existing WorkerToHostInterface implementations
4. Both systems run in parallel

**Code Example**:
```rust
// New TextUI can implement both old and new interfaces
impl TextUi {
    pub fn build_interfaces(&self) -> SessionInterfaces {
        SessionInterfaces {
            prompt_provider: Arc::new(TextUiPromptProvider { ui: self.clone() }),
            tool_approver: Arc::new(TextUiToolApprover { ui: self.clone() }),
            error_handler: Arc::new(TextUiErrorHandler { ui: self.clone() }),
            shutdown_coordinator: Arc::new(TextUiShutdownCoordinator { ui: self.clone() }),
        }
    }
    
    // Also implements old WorkerToHostInterface
    pub fn as_worker_interface(&self) -> Arc<dyn WorkerToHostInterface> {
        Arc::new(TextUiWorkerToHostInterface::new(None))
    }
}
```

**Testing**: Create new demo using new interfaces, verify both systems work

### Phase 3: Migrate AgentLoop to New Pattern

**Goal**: Update AgentLoop to use events and new interfaces

**Changes**:
1. Remove WorkerToHostInterface parameter from AgentLoop
2. Add EventBus and interface parameters
3. Update all communication to use new pattern
4. Keep old demo working with compatibility layer

**Code Example**:
```rust
// Old constructor (deprecated)
impl AgentLoop {
    #[deprecated(note = "Use new() with event_bus and interfaces")]
    pub fn new_legacy(
        worker: Arc<Worker>,
        input: AgentLoopInput,
        host_interface: Arc<dyn WorkerToHostInterface>,
        cancellation_token: CancellationToken,
    ) -> Self {
        // Convert to new pattern internally
        let event_bus = EventBus::new(100);
        // ... create default interfaces
        Self::new(worker, input, event_bus, tool_approver, error_handler, cancellation_token)
    }
    
    // New constructor
    pub fn new(
        worker: Arc<Worker>,
        input: AgentLoopInput,
        event_bus: EventBus,
        tool_approver: Arc<dyn ToolApprover>,
        error_handler: Arc<dyn ErrorHandler>,
        cancellation_token: CancellationToken,
    ) -> Self {
        // ...
    }
}
```

**Testing**: Update demos to use new pattern, verify functionality

### Phase 4: Remove Old Interface (Breaking Change)

**Goal**: Clean up deprecated code

**Changes**:
1. Remove WorkerToHostInterface trait
2. Remove compatibility layers
3. Update all code to use new pattern exclusively
4. Update documentation

**Testing**: Full regression test suite

### Phase 5: Add Advanced Features

**Goal**: Leverage new architecture for enhanced functionality

**Possible Enhancements**:
1. Multiple event subscribers (logging, metrics, debugging UI)
2. Event replay for debugging
3. Advanced tool approval with history
4. Error recovery strategies
5. Multi-worker coordination

## Benefits and Trade-offs

### Benefits

#### 1. Loose Coupling for Display
- UI can subscribe/unsubscribe from events dynamically
- Multiple UIs can observe same events (e.g., TUI + web dashboard)
- Events don't block worker execution
- Easy to add logging, metrics, debugging subscribers

#### 2. Clear Contracts for Interactions
- Explicit interfaces for critical operations
- Type-safe request-response patterns
- Easy to mock for testing
- Clear separation of concerns

#### 3. Flexible UI Swapping
- UI implementations are independent modules
- Can mix and match components (e.g., CLI input + web output)
- Easy to add new UI types without changing core
- Can run headless with minimal interface implementations

#### 4. Better Testability
- Can test core logic with mock interfaces
- Can verify events are published correctly
- Can test UI independently with mock event sources
- Clear boundaries for unit tests

#### 5. Performance
- Events are fire-and-forget (no blocking)
- Broadcast channel is efficient for multiple subscribers
- Can tune event buffer size per use case
- Streaming chunks don't block worker

#### 6. Observability
- All state changes are events (easy to log)
- Can add metrics subscriber without changing code
- Can record events for replay/debugging
- Clear audit trail of system behavior

### Trade-offs

#### 1. Complexity
- **Cost**: Two communication patterns to understand
- **Mitigation**: Clear documentation, examples, consistent patterns
- **When it matters**: Onboarding new developers
- **Verdict**: Acceptable - complexity is localized and well-structured

#### 2. Event Ordering
- **Cost**: Events may arrive out of order under high load
- **Mitigation**: Include timestamps, sequence numbers if needed
- **When it matters**: High-frequency events from multiple workers
- **Verdict**: Acceptable - UI can handle minor reordering

#### 3. Memory Usage
- **Cost**: Event buffer holds recent events in memory
- **Mitigation**: Configurable buffer size, events are small
- **When it matters**: Long-running sessions with many events
- **Verdict**: Minimal - events are lightweight, buffer is bounded

#### 4. Debugging
- **Cost**: Async event flow harder to trace than direct calls
- **Mitigation**: Comprehensive logging, event replay tools
- **When it matters**: Debugging race conditions or timing issues
- **Verdict**: Acceptable - benefits outweigh debugging complexity

#### 5. Interface Coordination
- **Cost**: Need to coordinate state between events and interface calls
- **Mitigation**: Clear ownership rules, immutable events
- **When it matters**: Complex interactions with multiple state updates
- **Verdict**: Acceptable - clear patterns prevent issues

### Comparison with Alternatives

#### vs. Pure Event Bus
- **Advantage**: Clear contracts for request-response interactions
- **Advantage**: Type-safe interfaces, compile-time checking
- **Advantage**: No need for request/response event pairs
- **Disadvantage**: Slightly more code (traits + implementations)

#### vs. Pure Direct Interfaces
- **Advantage**: Loose coupling for display updates
- **Advantage**: Multiple subscribers without code changes
- **Advantage**: Non-blocking notifications
- **Disadvantage**: Two patterns instead of one

#### vs. Current WorkerToHostInterface
- **Advantage**: Separation of concerns (display vs. interaction)
- **Advantage**: Multiple UI support
- **Advantage**: Better testability
- **Disadvantage**: Migration effort required

## Implementation Checklist

### Core Infrastructure
- [ ] Create `events/` module
  - [ ] `event_bus.rs` - EventBus implementation
  - [ ] `agent_event.rs` - AgentEvent enum with all event types
  - [ ] Unit tests for EventBus
  
- [ ] Create `interfaces/` module
  - [ ] `prompt_provider.rs` - PromptProvider trait
  - [ ] `tool_approver.rs` - ToolApprover trait
  - [ ] `error_handler.rs` - ErrorHandler trait
  - [ ] `shutdown_coordinator.rs` - ShutdownCoordinator trait
  - [ ] `mod.rs` - SessionInterfaces struct

### Session/Worker Updates
- [ ] Add EventBus to Session
- [ ] Add interfaces to Session
- [ ] Add EventBus to Worker
- [ ] Update Worker::set_state() to publish events
- [ ] Update WorkerJob to publish lifecycle events
- [ ] Add task_type() to WorkerTask trait

### AgentLoop Updates
- [ ] Add EventBus parameter to AgentLoop
- [ ] Add interface parameters to AgentLoop
- [ ] Update state changes to use events
- [ ] Update streaming to use events
- [ ] Update tool execution to use ToolApprover
- [ ] Update error handling to use ErrorHandler

### UI Implementation
- [ ] Create `ui/` module structure
- [ ] Implement TextUI with event subscriber
- [ ] Implement TextUiPromptProvider
- [ ] Implement TextUiToolApprover
- [ ] Implement TextUiErrorHandler
- [ ] Implement TextUiShutdownCoordinator
- [ ] Update PromptQueue with response channels
- [ ] Update entry point (ChatArgs::execute)

### Testing
- [ ] Unit tests for EventBus
- [ ] Unit tests for each interface implementation
- [ ] Integration test: single worker with events
- [ ] Integration test: multiple workers with events
- [ ] Integration test: tool approval flow
- [ ] Integration test: error handling flow
- [ ] Integration test: shutdown flow
- [ ] Performance test: high-frequency events

### Documentation
- [ ] Update architecture README
- [ ] Document event types and when they're published
- [ ] Document interface contracts and implementations
- [ ] Create migration guide
- [ ] Add code examples for common patterns
- [ ] Update demo documentation

### Migration
- [ ] Phase 1: Add event bus (non-breaking)
- [ ] Phase 2: Add interfaces (parallel)
- [ ] Phase 3: Migrate AgentLoop
- [ ] Phase 4: Remove old interface
- [ ] Phase 5: Add advanced features

## Future Enhancements

### Event Replay System
```rust
pub struct EventRecorder {
    events: Arc<Mutex<Vec<(Instant, AgentEvent)>>>,
}

impl EventRecorder {
    pub fn subscribe_to(&self, event_bus: &EventBus) {
        let mut receiver = event_bus.subscribe();
        let events = self.events.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                events.lock().unwrap().push((Instant::now(), event));
            }
        });
    }
    
    pub fn replay(&self, event_bus: &EventBus) {
        let events = self.events.lock().unwrap();
        for (_, event) in events.iter() {
            event_bus.publish(event.clone());
        }
    }
}
```

### Metrics Subscriber
```rust
pub struct MetricsSubscriber {
    worker_state_durations: HashMap<Uuid, HashMap<WorkerStates, Duration>>,
    job_durations: HashMap<Uuid, Duration>,
    tool_execution_counts: HashMap<String, usize>,
}

impl MetricsSubscriber {
    pub async fn subscribe_to(&mut self, event_bus: &EventBus) {
        let mut receiver = event_bus.subscribe();
        
        while let Ok(event) = receiver.recv().await {
            match event {
                AgentEvent::WorkerStateChanged { worker_id, new_state, .. } => {
                    // Track state durations
                }
                AgentEvent::JobCompleted { duration, .. } => {
                    // Track job durations
                }
                AgentEvent::ToolExecutionCompleted { tool_name, .. } => {
                    // Track tool usage
                }
                _ => {}
            }
        }
    }
    
    pub fn report(&self) -> MetricsReport {
        // Generate metrics report
    }
}
```

### Multi-UI Support
```rust
// Run TUI and web API simultaneously
let event_bus = EventBus::new(1000);

// TUI subscriber
let tui = Arc::new(TextUi::new(event_bus.clone(), history_path)?);
tokio::spawn(async move {
    tui.event_loop().await
});

// Web API subscriber
let web_api = Arc::new(WebApi::new(event_bus.clone(), port)?);
tokio::spawn(async move {
    web_api.event_loop().await
});

// Both UIs receive same events, can provide different interfaces
```

## Conclusion

The Hybrid Event Bus + Direct Interfaces approach provides the best balance of flexibility, clarity, and performance for the Q CLI agent environment. It enables:

1. **Easy UI swapping** - Implement new UIs by subscribing to events and providing interfaces
2. **Multiple concurrent UIs** - TUI, web API, logging, metrics all from same event stream
3. **Clear contracts** - Type-safe interfaces for critical interactions
4. **Loose coupling** - Events don't block workers, UIs are independent
5. **Testability** - Mock interfaces and event sources for comprehensive testing
6. **Observability** - All state changes are events, easy to log and monitor

The migration path is incremental and non-breaking, allowing gradual adoption while maintaining existing functionality. The architecture is extensible for future enhancements like event replay, advanced metrics, and multi-worker coordination.
