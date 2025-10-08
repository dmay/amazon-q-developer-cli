# Unified Event Stream Architecture - Detailed Design

## 1. Event Type Definitions

### 1.1 Core Event Enum

```rust
// crates/chat-cli/src/agent_env/events.rs

use uuid::Uuid;
use std::sync::Arc;

/// All events that can occur in the agent environment
#[derive(Debug, Clone)]
pub enum AgentEnvEvent {
    Session(SessionEvent),
    Worker(WorkerEvent),
    Job(JobEvent),
    Task(TaskEvent),
    Stream(StreamEvent),
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    WorkerCreated { worker_id: Uuid, name: String },
    WorkerDeleted { worker_id: Uuid },
    JobStarted { worker_id: Uuid, job_id: Uuid, task_type: String },
    JobCompleted { worker_id: Uuid, job_id: Uuid, completion: JobCompletion },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum WorkerEvent {
    StateChanged { worker_id: Uuid, state: WorkerStates },
    FailureRecorded { worker_id: Uuid, error: String },
}

#[derive(Debug, Clone)]
pub enum JobEvent {
    StateChanged { job_id: Uuid, worker_id: Uuid, state: JobState },
    Cancelled { job_id: Uuid, worker_id: Uuid },
}

#[derive(Debug, Clone)]
pub enum TaskEvent {
    AgentLoop(AgentLoopEvent),
    // Future: CompactHistory, CustomTask, etc.
}

#[derive(Debug, Clone)]
pub enum AgentLoopEvent {
    StatusChanged { worker_id: Uuid, status: AgentLoopStatus },
    ToolExecutionStarted { worker_id: Uuid, tool_name: String },
    ToolExecutionCompleted { worker_id: Uuid, tool_name: String, success: bool },
}

#[derive(Debug, Clone)]
pub enum AgentLoopStatus {
    Preparing,
    SendingRequest,
    ReceivingResponse,
    ExecutingTool,
    Complete,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Chunk of text from LLM response
    ResponseChunk { worker_id: Uuid, chunk: String },
    /// Complete message received
    MessageComplete { worker_id: Uuid },
    /// Tool use block received
    ToolUseReceived { worker_id: Uuid, tool_use: ToolUseBlock },
}

#[derive(Debug, Clone)]
pub struct ToolUseBlock {
    pub tool_name: String,
    pub tool_input: String,
}

#[derive(Debug, Clone)]
pub enum JobCompletion {
    Normal,
    Cancelled,
    Failed { error: String },
}
```

### 1.2 Synchronous Request Types

```rust
// crates/chat-cli/src/agent_env/sync_requests.rs

use uuid::Uuid;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Synchronous requests that require UI response
#[derive(Debug)]
pub enum SyncRequest {
    ToolApproval(ToolApprovalRequest),
    UserPrompt(UserPromptRequest),
}

#[derive(Debug)]
pub struct ToolApprovalRequest {
    pub worker_id: Uuid,
    pub tool_name: String,
    pub tool_input: String,
    pub response_tx: oneshot::Sender<ToolApprovalResponse>,
    pub cancellation_token: CancellationToken,
}

#[derive(Debug)]
pub enum ToolApprovalResponse {
    Approved,
    Denied,
    Cancelled,
}

#[derive(Debug)]
pub struct UserPromptRequest {
    pub worker_id: Uuid,
    pub worker_name: String,
    pub response_tx: oneshot::Sender<UserPromptResponse>,
}

#[derive(Debug)]
pub enum UserPromptResponse {
    Input(String),
    Command(String), // e.g., "/quit"
    Cancelled,
}
```

## 2. Event Publisher Implementation

### 2.1 EventBus

```rust
// crates/chat-cli/src/agent_env/event_bus.rs

use tokio::sync::broadcast;
use super::events::AgentEnvEvent;

const EVENT_CHANNEL_CAPACITY: usize = 1000;

pub struct EventBus {
    tx: broadcast::Sender<AgentEnvEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { tx }
    }
    
    pub fn publish(&self, event: AgentEnvEvent) {
        // Ignore send errors (no subscribers is OK)
        let _ = self.tx.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEnvEvent> {
        self.tx.subscribe()
    }
    
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
```

### 2.2 Session Integration

```rust
// Modifications to crates/chat-cli/src/agent_env/session.rs

use super::event_bus::EventBus;
use super::events::{AgentEnvEvent, SessionEvent};
use super::sync_requests::{SyncRequest, ToolApprovalRequest, UserPromptRequest};

pub struct Session {
    model_providers: Vec<Arc<dyn ModelProvider>>,
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>,
    event_bus: EventBus,
    sync_request_tx: mpsc::UnboundedSender<SyncRequest>,
}

impl Session {
    pub fn new(
        model_providers: Vec<Arc<dyn ModelProvider>>,
        event_bus: EventBus,
        sync_request_tx: mpsc::UnboundedSender<SyncRequest>,
    ) -> Self {
        Self {
            model_providers,
            workers: Arc::new(Mutex::new(Vec::new())),
            jobs: Arc::new(Mutex::new(Vec::new())),
            event_bus,
            sync_request_tx,
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
        self.event_bus.publish(AgentEnvEvent::Session(
            SessionEvent::WorkerCreated {
                worker_id: worker.id,
                name,
            }
        ));
        
        worker
    }
    
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
    
    pub fn sync_request_sender(&self) -> mpsc::UnboundedSender<SyncRequest> {
        self.sync_request_tx.clone()
    }
}
```

### 2.3 Worker Integration

```rust
// Modifications to crates/chat-cli/src/agent_env/worker.rs

use super::event_bus::EventBus;
use super::events::{AgentEnvEvent, WorkerEvent};

pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub context_container: ContextContainer,
    pub model_provider: Arc<dyn ModelProvider>,
    pub state: Arc<Mutex<WorkerStates>>,
    pub last_failure: Arc<Mutex<Option<String>>>,
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
        {
            let mut state = self.state.lock().unwrap();
            *state = new_state;
        }
        
        self.event_bus.publish(AgentEnvEvent::Worker(
            WorkerEvent::StateChanged {
                worker_id: self.id,
                state: new_state,
            }
        ));
    }
    
    pub fn set_failure(&self, error: String) {
        {
            let mut failure = self.last_failure.lock().unwrap();
            *failure = Some(error.clone());
        }
        
        self.event_bus.publish(AgentEnvEvent::Worker(
            WorkerEvent::FailureRecorded {
                worker_id: self.id,
                error,
            }
        ));
    }
}
```

## 3. Event Subscriber Pattern

### 3.1 EventSubscriber Trait

```rust
// crates/chat-cli/src/agent_env/event_subscriber.rs

use super::events::AgentEnvEvent;

#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Handle an event. Return false to stop receiving events.
    async fn handle_event(&mut self, event: AgentEnvEvent) -> bool;
    
    /// Called when event stream ends or subscriber stops
    async fn on_shutdown(&mut self) {}
}

/// Helper to run subscriber in background task
pub fn spawn_subscriber<S: EventSubscriber + 'static>(
    mut subscriber: S,
    mut event_rx: broadcast::Receiver<AgentEnvEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if !subscriber.handle_event(event).await {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("Subscriber lagged, skipped {} events", skipped);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
        subscriber.on_shutdown().await;
    })
}
```

### 3.2 Example: Simple Text Output Subscriber

```rust
// Example subscriber that prints events to stdout

use super::event_subscriber::EventSubscriber;
use super::events::*;

pub struct TextOutputSubscriber {
    worker_filter: Option<Uuid>,
}

impl TextOutputSubscriber {
    pub fn new(worker_filter: Option<Uuid>) -> Self {
        Self { worker_filter }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for TextOutputSubscriber {
    async fn handle_event(&mut self, event: AgentEnvEvent) -> bool {
        match event {
            AgentEnvEvent::Stream(StreamEvent::ResponseChunk { worker_id, chunk }) => {
                if self.worker_filter.map_or(true, |id| id == worker_id) {
                    print!("{}", chunk);
                    let _ = std::io::stdout().flush();
                }
            }
            AgentEnvEvent::Worker(WorkerEvent::StateChanged { worker_id, state }) => {
                if self.worker_filter.map_or(true, |id| id == worker_id) {
                    println!("\n[Worker {} state: {:?}]", worker_id, state);
                }
            }
            AgentEnvEvent::Session(SessionEvent::Shutdown) => {
                return false; // Stop processing
            }
            _ => {}
        }
        true
    }
}
```

## 4. Synchronous Operations Handling

### 4.1 SyncRequestHandler

```rust
// crates/chat-cli/src/agent_env/sync_request_handler.rs

use tokio::sync::mpsc;
use super::sync_requests::*;

/// Handles synchronous requests from workers
pub struct SyncRequestHandler {
    rx: mpsc::UnboundedReceiver<SyncRequest>,
}

impl SyncRequestHandler {
    pub fn new(rx: mpsc::UnboundedReceiver<SyncRequest>) -> Self {
        Self { rx }
    }
    
    /// Get next synchronous request (blocking)
    pub async fn next_request(&mut self) -> Option<SyncRequest> {
        self.rx.recv().await
    }
}
```

### 4.2 Tool Approval in AgentLoop

```rust
// Modifications to crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs

async fn request_tool_approval(
    &self,
    tool_name: String,
    tool_input: String,
) -> Result<ToolApprovalResponse, eyre::Error> {
    let (response_tx, response_rx) = oneshot::channel();
    
    let request = SyncRequest::ToolApproval(ToolApprovalRequest {
        worker_id: self.worker.id,
        tool_name,
        tool_input,
        response_tx,
        cancellation_token: self.cancellation_token.clone(),
    });
    
    // Send request through session's sync channel
    self.sync_request_tx.send(request)?;
    
    // Wait for response or cancellation
    tokio::select! {
        response = response_rx => {
            response.map_err(|_| eyre::eyre!("Approval request cancelled"))
        }
        _ = self.cancellation_token.cancelled() => {
            Ok(ToolApprovalResponse::Cancelled)
        }
    }
}
```

## 5. Main Application Loop Structure

### 5.1 Application Entry Point

```rust
// crates/chat-cli/src/cli/chat/mod.rs (ChatArgs::execute)

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Create event bus and sync request channel
        let event_bus = EventBus::new();
        let (sync_tx, sync_rx) = mpsc::unbounded_channel();
        
        // Build session
        let model_providers = vec![/* ... */];
        let session = Arc::new(Session::new(
            model_providers,
            event_bus.clone(),
            sync_tx,
        ));
        
        // Create UI controller
        let mut ui = TextUiController::new(
            session.clone(),
            event_bus.clone(),
            sync_rx,
            history_path,
        )?;
        
        // Create initial worker if input provided
        if let Some(input) = self.input {
            let worker = session.build_worker("Main".to_string());
            worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(input);
            
            session.run_agent_loop(
                worker,
                AgentLoopInput {},
            )?;
        }
        
        // Run UI (blocks until shutdown)
        ui.run().await?;
        
        // Cleanup
        session.cancel_all_jobs();
        event_bus.publish(AgentEnvEvent::Session(SessionEvent::Shutdown));
        
        Ok(ExitCode::SUCCESS)
    }
}
```

### 5.2 UI Controller Structure

```rust
// crates/chat-cli/src/cli/chat/text_ui_controller.rs

use tokio::sync::Notify;
use super::event_bus::EventBus;
use super::sync_requests::*;

pub struct TextUiController {
    session: Arc<Session>,
    event_bus: EventBus,
    sync_handler: SyncRequestHandler,
    input_handler: InputHandler,
    shutdown_signal: Arc<Notify>,
    active_workers: HashMap<Uuid, Arc<Worker>>,
}

impl TextUiController {
    pub fn new(
        session: Arc<Session>,
        event_bus: EventBus,
        sync_rx: mpsc::UnboundedReceiver<SyncRequest>,
        history_path: Option<PathBuf>,
    ) -> Result<Self, eyre::Error> {
        Ok(Self {
            session,
            event_bus,
            sync_handler: SyncRequestHandler::new(sync_rx),
            input_handler: InputHandler::new(history_path)?,
            shutdown_signal: Arc::new(Notify::new()),
            active_workers: HashMap::new(),
        })
    }
    
    pub async fn run(&mut self) -> Result<(), eyre::Error> {
        // Spawn event subscribers
        let output_subscriber = TextOutputSubscriber::new(None);
        let _output_handle = spawn_subscriber(
            output_subscriber,
            self.event_bus.subscribe(),
        );
        
        // Setup Ctrl+C handler
        let shutdown = self.shutdown_signal.clone();
        let session = self.session.clone();
        tokio::spawn(async move {
            let _ = ctrl_c().await;
            session.cancel_all_jobs();
            shutdown.notify_waiters();
        });
        
        // Main loop
        loop {
            tokio::select! {
                _ = self.shutdown_signal.notified() => {
                    break;
                }
                
                Some(sync_request) = self.sync_handler.next_request() => {
                    self.handle_sync_request(sync_request).await?;
                }
            }
        }
        
        self.input_handler.save_history()?;
        Ok(())
    }
    
    async fn handle_sync_request(&mut self, request: SyncRequest) -> Result<(), eyre::Error> {
        match request {
            SyncRequest::ToolApproval(req) => {
                self.handle_tool_approval(req).await
            }
            SyncRequest::UserPrompt(req) => {
                self.handle_user_prompt(req).await
            }
        }
    }
    
    async fn handle_tool_approval(&mut self, req: ToolApprovalRequest) -> Result<(), eyre::Error> {
        println!("\n[Tool Approval Required]");
        println!("Tool: {}", req.tool_name);
        println!("Input: {}", req.tool_input);
        print!("Approve? (y/n): ");
        std::io::stdout().flush()?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        let response = match input.trim().to_lowercase().as_str() {
            "y" | "yes" => ToolApprovalResponse::Approved,
            _ => ToolApprovalResponse::Denied,
        };
        
        let _ = req.response_tx.send(response);
        Ok(())
    }
    
    async fn handle_user_prompt(&mut self, req: UserPromptRequest) -> Result<(), eyre::Error> {
        let input = self.input_handler.read_line(&req.worker_name).await?;
        
        if input.trim() == "/quit" {
            let _ = req.response_tx.send(UserPromptResponse::Command("/quit".to_string()));
            self.shutdown_signal.notify_waiters();
            return Ok(());
        }
        
        let _ = req.response_tx.send(UserPromptResponse::Input(input));
        Ok(())
    }
}
```

## 6. Example UI Implementations

### 6.1 Multi-Worker Colored Output

```rust
// crates/chat-cli/src/cli/chat/colored_output_subscriber.rs

pub struct ColoredOutputSubscriber {
    worker_colors: HashMap<Uuid, &'static str>,
}

impl ColoredOutputSubscriber {
    pub fn new() -> Self {
        Self {
            worker_colors: HashMap::new(),
        }
    }
    
    fn get_color(&mut self, worker_id: Uuid) -> &'static str {
        let colors = ["\x1b[32m", "\x1b[36m", "\x1b[33m", "\x1b[35m"];
        let idx = self.worker_colors.len() % colors.len();
        *self.worker_colors.entry(worker_id).or_insert(colors[idx])
    }
}

#[async_trait::async_trait]
impl EventSubscriber for ColoredOutputSubscriber {
    async fn handle_event(&mut self, event: AgentEnvEvent) -> bool {
        match event {
            AgentEnvEvent::Session(SessionEvent::WorkerCreated { worker_id, name }) => {
                let color = self.get_color(worker_id);
                println!("{}[Worker {} created: {}]\x1b[0m", color, worker_id, name);
            }
            AgentEnvEvent::Stream(StreamEvent::ResponseChunk { worker_id, chunk }) => {
                let color = self.get_color(worker_id);
                print!("{}{}\x1b[0m", color, chunk);
                let _ = std::io::stdout().flush();
            }
            AgentEnvEvent::Session(SessionEvent::Shutdown) => {
                return false;
            }
            _ => {}
        }
        true
    }
}
```

### 6.2 Web API Event Stream

```rust
// Example: WebSocket event broadcaster

pub struct WebSocketSubscriber {
    clients: Arc<Mutex<Vec<WebSocketSender>>>,
}

#[async_trait::async_trait]
impl EventSubscriber for WebSocketSubscriber {
    async fn handle_event(&mut self, event: AgentEnvEvent) -> bool {
        let json = serde_json::to_string(&event).unwrap();
        let clients = self.clients.lock().unwrap();
        
        for client in clients.iter() {
            let _ = client.send(json.clone()).await;
        }
        
        true
    }
}
```

### 6.3 Logging Subscriber

```rust
// Example: Event logger for debugging

pub struct LoggingSubscriber {
    log_file: Arc<Mutex<File>>,
}

#[async_trait::async_trait]
impl EventSubscriber for LoggingSubscriber {
    async fn handle_event(&mut self, event: AgentEnvEvent) -> bool {
        let mut file = self.log_file.lock().unwrap();
        let timestamp = chrono::Utc::now();
        writeln!(file, "[{}] {:?}", timestamp, event).ok();
        true
    }
}
```

## 7. Error Handling and Edge Cases

### 7.1 Event Channel Backpressure

```rust
// In EventBus implementation

impl EventBus {
    pub fn publish(&self, event: AgentEnvEvent) {
        match self.tx.send(event) {
            Ok(_) => {}
            Err(_) => {
                // No subscribers - this is OK, events are fire-and-forget
                tracing::trace!("Event published with no subscribers");
            }
        }
    }
}

// In subscriber spawn helper

pub fn spawn_subscriber<S: EventSubscriber + 'static>(
    mut subscriber: S,
    mut event_rx: broadcast::Receiver<AgentEnvEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if !subscriber.handle_event(event).await {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    // Subscriber too slow - log and continue
                    tracing::warn!("Subscriber lagged, skipped {} events", skipped);
                    // Could also: notify subscriber, drop subscriber, etc.
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
        subscriber.on_shutdown().await;
    })
}
```

### 7.2 Synchronous Request Timeout

```rust
// In AgentLoop tool approval

async fn request_tool_approval(
    &self,
    tool_name: String,
    tool_input: String,
) -> Result<ToolApprovalResponse, eyre::Error> {
    let (response_tx, response_rx) = oneshot::channel();
    
    let request = SyncRequest::ToolApproval(ToolApprovalRequest {
        worker_id: self.worker.id,
        tool_name,
        tool_input,
        response_tx,
        cancellation_token: self.cancellation_token.clone(),
    });
    
    self.sync_request_tx.send(request)?;
    
    // Add timeout
    tokio::select! {
        response = response_rx => {
            response.map_err(|_| eyre::eyre!("Approval request cancelled"))
        }
        _ = self.cancellation_token.cancelled() => {
            Ok(ToolApprovalResponse::Cancelled)
        }
        _ = tokio::time::sleep(Duration::from_secs(300)) => {
            Err(eyre::eyre!("Tool approval timeout"))
        }
    }
}
```

### 7.3 Worker Deletion Cleanup

```rust
// In Session

impl Session {
    pub fn delete_worker(&self, worker_id: Uuid) -> Result<(), eyre::Error> {
        // Cancel all jobs for this worker
        let jobs = self.jobs.lock().unwrap();
        for job in jobs.iter() {
            if job.worker.id == worker_id {
                job.cancel();
            }
        }
        drop(jobs);
        
        // Remove from workers list
        let mut workers = self.workers.lock().unwrap();
        workers.retain(|w| w.id != worker_id);
        
        // Publish event
        self.event_bus.publish(AgentEnvEvent::Session(
            SessionEvent::WorkerDeleted { worker_id }
        ));
        
        Ok(())
    }
}
```

### 7.4 Graceful Shutdown

```rust
// In TextUiController

impl TextUiController {
    pub async fn shutdown(&mut self) -> Result<(), eyre::Error> {
        // Cancel all jobs
        self.session.cancel_all_jobs();
        
        // Wait for jobs to complete (with timeout)
        tokio::select! {
            _ = self.session.wait_for_all_jobs() => {}
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                tracing::warn!("Shutdown timeout, forcing exit");
            }
        }
        
        // Publish shutdown event
        self.event_bus.publish(AgentEnvEvent::Session(SessionEvent::Shutdown));
        
        // Give subscribers time to process shutdown
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Save history
        self.input_handler.save_history()?;
        
        Ok(())
    }
}
```

## 8. Migration Path

### Phase 1: Add Event System (Non-Breaking)

1. Add event types and EventBus to codebase
2. Keep existing WorkerToHostInterface
3. Publish events alongside interface calls
4. No changes to existing UI code

```rust
// In Worker::set_state
pub fn set_state(&self, new_state: WorkerStates, interface: &dyn WorkerToHostInterface) {
    // Existing code
    {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }
    interface.worker_state_change(self.id, new_state);
    
    // NEW: Also publish event
    self.event_bus.publish(AgentEnvEvent::Worker(
        WorkerEvent::StateChanged {
            worker_id: self.id,
            state: new_state,
        }
    ));
}
```

### Phase 2: Create New UI Implementation

1. Implement TextUiController using events
2. Run both UIs in parallel for testing
3. Verify feature parity

```rust
// In ChatArgs::execute
pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
    // ... setup ...
    
    // OLD UI (for comparison)
    let old_ui = AgentEnvTextUi::new(session.clone(), history_path.clone())?;
    
    // NEW UI (event-based)
    let new_ui = TextUiController::new(
        session.clone(),
        event_bus.clone(),
        sync_rx,
        history_path,
    )?;
    
    // Run new UI
    new_ui.run().await?;
    
    Ok(ExitCode::SUCCESS)
}
```

### Phase 3: Remove WorkerToHostInterface

1. Remove interface parameter from Worker methods
2. Remove interface implementations
3. Update all call sites
4. Delete old UI code

```rust
// Worker::set_state becomes:
pub fn set_state(&self, new_state: WorkerStates) {
    {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }
    
    self.event_bus.publish(AgentEnvEvent::Worker(
        WorkerEvent::StateChanged {
            worker_id: self.id,
            state: new_state,
        }
    ));
}
```

### Phase 4: Add Advanced Features

1. Multiple simultaneous UIs
2. Web API with WebSocket events
3. Fancy TUI with widgets
4. Event recording/replay for testing

## 9. Benefits Summary

### Flexibility
- Swap UI implementations without touching core
- Run multiple UIs simultaneously (terminal + web)
- Easy to add new event types

### Testability
- Record event streams for replay
- Mock subscribers for testing
- Verify event sequences

### Performance
- Broadcast channel is efficient
- Subscribers can lag without blocking core
- Fire-and-forget event publishing

### Maintainability
- Clear separation of concerns
- Single source of truth for events
- Type-safe event handling

## 10. Alternative Considerations

### Alternative: Callback-Based Interface (Current)
- **Pros**: Simple, direct, synchronous
- **Cons**: Tight coupling, hard to add multiple UIs, testing difficult

### Alternative: Actor Model (Actix/Tokio Actors)
- **Pros**: Message passing, isolation, supervision
- **Cons**: More complex, heavier weight, learning curve

### Alternative: Channels Per Component
- **Pros**: Fine-grained control, backpressure per channel
- **Cons**: Complex wiring, many channels to manage, harder to add subscribers

### Why Unified Event Stream Wins
- Simplest to implement and understand
- Rust's broadcast channel is purpose-built for this
- Easy to add/remove subscribers
- Natural fit for UI event handling
- Minimal changes to existing code
