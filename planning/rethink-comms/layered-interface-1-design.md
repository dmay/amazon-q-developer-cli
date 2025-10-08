# Layered Interface with UI Adapters - Detailed Design

## Executive Summary

This design proposes a **layered adapter architecture** where the core application loop owns the Session and actively routes events to UI implementations through well-defined adapter traits. UI implementations provide concrete adapters that handle output, input, interactions, and lifecycle events.

**Key Principle**: Core loop is the orchestrator; UI adapters are passive receivers of events and providers of input.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                   UI Implementation                      │
│  (TextUI, WebUI, FancyTUI, CompositeUI)                │
│                                                          │
│  Provides concrete implementations of:                  │
│  - OutputAdapter                                        │
│  - InputAdapter                                         │
│  - InteractionAdapter                                   │
│  - LifecycleAdapter                                     │
└────────────────┬────────────────────────────────────────┘
                 │ implements traits
                 ▼
┌─────────────────────────────────────────────────────────┐
│              Adapter Trait Definitions                   │
│  (OutputAdapter, InputAdapter, InteractionAdapter, etc) │
└────────────────┬────────────────────────────────────────┘
                 │ called by
                 ▼
┌─────────────────────────────────────────────────────────┐
│              Core Application Loop                       │
│  - Owns Session                                         │
│  - Polls for state changes                              │
│  - Routes events to adapters                            │
│  - Coordinates worker lifecycle                         │
└────────────────┬────────────────────────────────────────┘
                 │ manages
                 ▼
┌─────────────────────────────────────────────────────────┐
│                      Session                             │
│  - Workers, Jobs, Model Providers                       │
│  - State tracking                                       │
│  - Job execution                                        │
└─────────────────────────────────────────────────────────┘
```

## Design Goals

1. **Explicit Contracts**: Clear trait boundaries define what UI must implement
2. **Type Safety**: Compiler enforces interface compliance
3. **Composability**: Mix and match adapter implementations
4. **Testability**: Mock adapters for testing core loop
5. **Flexibility**: Support synchronous and asynchronous UI patterns
6. **Migration Path**: Incremental adoption from current implementation

---

## 1. Adapter Trait Definitions

### 1.1 OutputAdapter

Handles all output from workers: streaming responses, state changes, errors.

```rust
/// Adapter for handling output from workers
#[async_trait::async_trait]
pub trait OutputAdapter: Send + Sync {
    /// Worker state changed
    async fn worker_state_changed(&self, worker_id: Uuid, new_state: WorkerStates);
    
    /// Streaming chunk received from LLM
    async fn response_chunk_received(&self, worker_id: Uuid, chunk: ResponseChunk);
    
    /// Worker completed a response
    async fn response_completed(&self, worker_id: Uuid);
    
    /// Worker encountered an error
    async fn worker_error(&self, worker_id: Uuid, error: String);
    
    /// Tool execution started
    async fn tool_execution_started(&self, worker_id: Uuid, tool_name: String, tool_input: String);
    
    /// Tool execution completed
    async fn tool_execution_completed(&self, worker_id: Uuid, tool_name: String, result: ToolExecutionResult);
    
    /// Job state changed
    async fn job_state_changed(&self, worker_id: Uuid, job_id: Uuid, new_state: JobState);
}

/// Response chunk types
#[derive(Debug, Clone)]
pub enum ResponseChunk {
    Text(String),
    ToolUse { name: String, input: String },
    Metadata(HashMap<String, String>),
}

/// Tool execution result
#[derive(Debug, Clone)]
pub enum ToolExecutionResult {
    Success(String),
    Error(String),
    Cancelled,
}
```

### 1.2 InputAdapter

Provides user input to the core loop.

```rust
/// Adapter for handling user input
#[async_trait::async_trait]
pub trait InputAdapter: Send + Sync {
    /// Request input from user for a specific worker
    /// Returns None if user wants to quit
    async fn request_input(&self, worker_id: Uuid, worker_name: &str) -> Result<Option<String>, InputError>;
    
    /// Check if input is available without blocking
    async fn poll_input(&self) -> Option<PendingInput>;
    
    /// Cancel any pending input requests
    async fn cancel_input_requests(&self);
}

/// Pending input from user
#[derive(Debug, Clone)]
pub struct PendingInput {
    pub worker_id: Option<Uuid>,  // None = any worker
    pub content: String,
}

/// Input errors
#[derive(Debug, Error)]
pub enum InputError {
    #[error("User cancelled input")]
    Cancelled,
    #[error("Input handler closed")]
    Closed,
    #[error("IO error: {0}")]
    Io(String),
}
```

### 1.3 InteractionAdapter

Handles interactive confirmations and approvals.

```rust
/// Adapter for interactive confirmations
#[async_trait::async_trait]
pub trait InteractionAdapter: Send + Sync {
    /// Request tool execution approval
    /// Returns true if approved, false if denied
    async fn request_tool_approval(
        &self,
        worker_id: Uuid,
        tool_name: &str,
        tool_input: &str,
        cancellation_token: CancellationToken,
    ) -> Result<bool, InteractionError>;
    
    /// Display a notification to user
    async fn notify(&self, message: &str, level: NotificationLevel);
    
    /// Request confirmation for an action
    async fn confirm(&self, message: &str) -> Result<bool, InteractionError>;
}

/// Notification levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// Interaction errors
#[derive(Debug, Error)]
pub enum InteractionError {
    #[error("User cancelled interaction")]
    Cancelled,
    #[error("Interaction timeout")]
    Timeout,
    #[error("Interaction handler closed")]
    Closed,
}
```

### 1.4 LifecycleAdapter

Manages application lifecycle events.

```rust
/// Adapter for application lifecycle events
#[async_trait::async_trait]
pub trait LifecycleAdapter: Send + Sync {
    /// Application starting up
    async fn on_startup(&self) -> Result<(), eyre::Error>;
    
    /// Worker created
    async fn on_worker_created(&self, worker_id: Uuid, worker_name: &str);
    
    /// Worker destroyed
    async fn on_worker_destroyed(&self, worker_id: Uuid);
    
    /// Job started
    async fn on_job_started(&self, worker_id: Uuid, job_id: Uuid, task_type: &str);
    
    /// Job completed
    async fn on_job_completed(
        &self,
        worker_id: Uuid,
        job_id: Uuid,
        completion_type: WorkerJobCompletionType,
        error: Option<String>,
    );
    
    /// Application shutting down
    async fn on_shutdown(&self) -> Result<(), eyre::Error>;
    
    /// Shutdown requested (e.g., Ctrl+C)
    async fn on_shutdown_requested(&self);
}
```

### 1.5 UiAdapterSet

Composite struct holding all adapters.

```rust
/// Complete set of UI adapters
pub struct UiAdapterSet {
    pub output: Arc<dyn OutputAdapter>,
    pub input: Arc<dyn InputAdapter>,
    pub interaction: Arc<dyn InteractionAdapter>,
    pub lifecycle: Arc<dyn LifecycleAdapter>,
}

impl UiAdapterSet {
    pub fn new(
        output: Arc<dyn OutputAdapter>,
        input: Arc<dyn InputAdapter>,
        interaction: Arc<dyn InteractionAdapter>,
        lifecycle: Arc<dyn LifecycleAdapter>,
    ) -> Self {
        Self { output, input, interaction, lifecycle }
    }
}
```

---

## 2. Core Application Loop Structure

The core application loop is responsible for:
1. Owning the Session
2. Managing worker lifecycle
3. Routing events from Session/Workers/Jobs to UI adapters
4. Coordinating input and output
5. Handling shutdown

### 2.1 CoreLoop Structure

```rust
/// Core application loop
pub struct CoreLoop {
    session: Arc<Session>,
    adapters: UiAdapterSet,
    shutdown_signal: Arc<Notify>,
    active_workers: Arc<Mutex<HashMap<Uuid, WorkerContext>>>,
}

/// Context for each active worker
struct WorkerContext {
    worker: Arc<Worker>,
    last_state: WorkerStates,
    pending_prompt: bool,
}

impl CoreLoop {
    pub fn new(session: Arc<Session>, adapters: UiAdapterSet) -> Self {
        Self {
            session,
            adapters,
            shutdown_signal: Arc::new(Notify::new()),
            active_workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Main loop execution
    pub async fn run(&mut self) -> Result<(), eyre::Error> {
        // Startup
        self.adapters.lifecycle.on_startup().await?;
        
        // Setup Ctrl+C handler
        self.setup_shutdown_handler();
        
        // Main event loop
        loop {
            tokio::select! {
                _ = self.shutdown_signal.notified() => {
                    break;
                }
                
                // Poll for input
                input = self.adapters.input.poll_input() => {
                    if let Some(input) = input {
                        self.handle_input(input).await?;
                    }
                }
                
                // Poll for state changes (every 50ms)
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    self.poll_state_changes().await?;
                }
            }
        }
        
        // Shutdown
        self.shutdown().await?;
        Ok(())
    }
    
    /// Handle user input
    async fn handle_input(&mut self, input: PendingInput) -> Result<(), eyre::Error> {
        // Implementation in next section
    }
    
    /// Poll for state changes in workers and jobs
    async fn poll_state_changes(&mut self) -> Result<(), eyre::Error> {
        // Implementation in next section
    }
    
    /// Shutdown sequence
    async fn shutdown(&mut self) -> Result<(), eyre::Error> {
        self.adapters.lifecycle.on_shutdown_requested().await;
        self.session.cancel_all_jobs();
        self.session.wait_for_all_jobs().await;
        self.adapters.lifecycle.on_shutdown().await?;
        Ok(())
    }
    
    /// Setup Ctrl+C handler
    fn setup_shutdown_handler(&self) {
        let shutdown_signal = self.shutdown_signal.clone();
        tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                shutdown_signal.notify_one();
            }
        });
    }
}
```

### 2.2 Input Handling

```rust
impl CoreLoop {
    async fn handle_input(&mut self, input: PendingInput) -> Result<(), eyre::Error> {
        // Handle commands
        if input.content.trim() == "/quit" {
            self.shutdown_signal.notify_one();
            return Ok(());
        }
        
        // Determine target worker
        let worker = if let Some(worker_id) = input.worker_id {
            // Input for specific worker
            self.active_workers
                .lock()
                .unwrap()
                .get(&worker_id)
                .map(|ctx| ctx.worker.clone())
        } else {
            // Input for any available worker - pick first idle
            self.find_idle_worker()
        };
        
        let worker = match worker {
            Some(w) => w,
            None => {
                self.adapters.interaction.notify(
                    "No available worker to handle input",
                    NotificationLevel::Warning,
                ).await;
                return Ok(());
            }
        };
        
        // Add input to conversation history
        worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_input_message(input.content);
        
        // Launch agent loop
        self.launch_agent_loop(worker).await?;
        
        Ok(())
    }
    
    fn find_idle_worker(&self) -> Option<Arc<Worker>> {
        let workers = self.active_workers.lock().unwrap();
        workers
            .values()
            .find(|ctx| ctx.last_state == WorkerStates::Inactive && !ctx.pending_prompt)
            .map(|ctx| ctx.worker.clone())
    }
}
```

### 2.3 State Polling

```rust
impl CoreLoop {
    async fn poll_state_changes(&mut self) -> Result<(), eyre::Error> {
        let mut workers = self.active_workers.lock().unwrap();
        
        for (worker_id, ctx) in workers.iter_mut() {
            let current_state = ctx.worker.get_state();
            
            // Check if state changed
            if current_state != ctx.last_state {
                self.adapters.output.worker_state_changed(*worker_id, current_state).await;
                ctx.last_state = current_state;
                
                // If worker became inactive, check if we need to prompt
                if current_state == WorkerStates::Inactive && !ctx.pending_prompt {
                    ctx.pending_prompt = true;
                    self.request_prompt_for_worker(ctx.worker.clone()).await?;
                }
            }
        }
        
        // Cleanup old jobs
        self.session.cleanup_inactive_jobs();
        
        Ok(())
    }
    
    async fn request_prompt_for_worker(&self, worker: Arc<Worker>) -> Result<(), eyre::Error> {
        let adapters = self.adapters.clone();
        let worker_id = worker.id;
        let worker_name = worker.name.clone();
        
        // Spawn task to request input
        tokio::spawn(async move {
            match adapters.input.request_input(worker_id, &worker_name).await {
                Ok(Some(input)) => {
                    // Input will be picked up by poll_input in main loop
                }
                Ok(None) => {
                    // User wants to quit
                }
                Err(e) => {
                    tracing::error!("Input request failed: {}", e);
                }
            }
        });
        
        Ok(())
    }
}
```

### 2.4 Job Launching

```rust
impl CoreLoop {
    async fn launch_agent_loop(&self, worker: Arc<Worker>) -> Result<(), eyre::Error> {
        // Create UI interface that routes to output adapter
        let ui_interface = self.create_worker_interface(worker.id);
        
        // Launch job
        let job = self.session.run_agent_loop(
            worker.clone(),
            AgentLoopInput {},
            ui_interface,
        )?;
        
        // Notify lifecycle
        self.adapters.lifecycle.on_job_started(
            worker.id,
            job.worker.id,  // Using worker.id as job identifier for now
            "AgentLoop",
        ).await;
        
        // Setup completion continuation
        let adapters = self.adapters.clone();
        let worker_id = worker.id;
        let continuation = Continuations::boxed(move |worker, completion_type, error| {
            let adapters = adapters.clone();
            async move {
                adapters.lifecycle.on_job_completed(
                    worker_id,
                    worker.id,
                    completion_type,
                    error,
                ).await;
            }
        });
        
        job.worker_job_continuations.add_or_run_now(
            "core_loop_completion",
            continuation,
            worker,
        ).await;
        
        Ok(())
    }
    
    fn create_worker_interface(&self, worker_id: Uuid) -> Arc<dyn WorkerToHostInterface> {
        Arc::new(AdapterBridgeInterface::new(
            worker_id,
            self.adapters.output.clone(),
        ))
    }
}
```

### 2.5 Worker Management

```rust
impl CoreLoop {
    /// Create a new worker
    pub async fn create_worker(&mut self, name: String) -> Arc<Worker> {
        let worker = self.session.build_worker(name.clone());
        
        self.active_workers.lock().unwrap().insert(
            worker.id,
            WorkerContext {
                worker: worker.clone(),
                last_state: WorkerStates::Inactive,
                pending_prompt: false,
            },
        );
        
        self.adapters.lifecycle.on_worker_created(worker.id, &name).await;
        
        worker
    }
    
    /// Remove a worker
    pub async fn destroy_worker(&mut self, worker_id: Uuid) -> Result<(), eyre::Error> {
        self.active_workers.lock().unwrap().remove(&worker_id);
        self.adapters.lifecycle.on_worker_destroyed(worker_id).await;
        Ok(())
    }
}
```

---

## 3. Event Routing Mechanism

### 3.1 Bridge Interface

The bridge interface connects WorkerToHostInterface (used by tasks) to OutputAdapter (used by UI).

```rust
/// Bridge between WorkerToHostInterface and OutputAdapter
struct AdapterBridgeInterface {
    worker_id: Uuid,
    output_adapter: Arc<dyn OutputAdapter>,
}

impl AdapterBridgeInterface {
    fn new(worker_id: Uuid, output_adapter: Arc<dyn OutputAdapter>) -> Self {
        Self { worker_id, output_adapter }
    }
}

#[async_trait::async_trait]
impl WorkerToHostInterface for AdapterBridgeInterface {
    fn worker_state_change(&self, worker_id: Uuid, new_state: WorkerStates) {
        let adapter = self.output_adapter.clone();
        tokio::spawn(async move {
            adapter.worker_state_changed(worker_id, new_state).await;
        });
    }
    
    fn response_chunk_received(&self, worker_id: Uuid, chunk: String) {
        let adapter = self.output_adapter.clone();
        tokio::spawn(async move {
            adapter.response_chunk_received(
                worker_id,
                ResponseChunk::Text(chunk),
            ).await;
        });
    }
    
    async fn get_tool_confirmation(
        &self,
        worker_id: Uuid,
        tool_name: String,
        tool_input: String,
        cancellation_token: CancellationToken,
    ) -> Result<bool, eyre::Error> {
        // This would need InteractionAdapter access
        // For now, return error indicating not implemented
        Err(eyre::eyre!("Tool confirmation not implemented in bridge"))
    }
}
```

### 3.2 Event Flow Diagram

```
User Input Flow:
User → InputAdapter → CoreLoop.poll_input() → CoreLoop.handle_input() 
  → Worker.context_container → CoreLoop.launch_agent_loop() → Job

Worker Output Flow:
Task → WorkerToHostInterface → AdapterBridgeInterface → OutputAdapter → UI

State Change Flow:
Worker.set_state() → CoreLoop.poll_state_changes() → OutputAdapter → UI

Lifecycle Flow:
CoreLoop → LifecycleAdapter → UI
```

---

## 4. UI Adapter Implementation Strategy

### 4.1 TextUI Implementation

```rust
/// Simple text-based UI implementation
pub struct TextUi {
    output: Arc<TextOutputAdapter>,
    input: Arc<TextInputAdapter>,
    interaction: Arc<TextInteractionAdapter>,
    lifecycle: Arc<TextLifecycleAdapter>,
}

impl TextUi {
    pub fn new(history_path: Option<PathBuf>) -> Result<Self, eyre::Error> {
        Ok(Self {
            output: Arc::new(TextOutputAdapter::new()),
            input: Arc::new(TextInputAdapter::new(history_path)?),
            interaction: Arc::new(TextInteractionAdapter::new()),
            lifecycle: Arc::new(TextLifecycleAdapter::new()),
        })
    }
    
    pub fn into_adapter_set(self) -> UiAdapterSet {
        UiAdapterSet::new(
            self.output,
            self.input,
            self.interaction,
            self.lifecycle,
        )
    }
}
```

### 4.2 TextOutputAdapter

```rust
struct TextOutputAdapter {
    worker_colors: Mutex<HashMap<Uuid, &'static str>>,
}

impl TextOutputAdapter {
    fn new() -> Self {
        Self {
            worker_colors: Mutex::new(HashMap::new()),
        }
    }
    
    fn get_worker_color(&self, worker_id: Uuid) -> &'static str {
        let mut colors = self.worker_colors.lock().unwrap();
        colors.entry(worker_id).or_insert("\x1b[32m").clone()
    }
}

#[async_trait::async_trait]
impl OutputAdapter for TextOutputAdapter {
    async fn worker_state_changed(&self, worker_id: Uuid, new_state: WorkerStates) {
        let color = self.get_worker_color(worker_id);
        println!("{}[Worker {}] State: {:?}\x1b[0m", color, worker_id, new_state);
    }
    
    async fn response_chunk_received(&self, worker_id: Uuid, chunk: ResponseChunk) {
        let color = self.get_worker_color(worker_id);
        match chunk {
            ResponseChunk::Text(text) => {
                print!("{}{}\x1b[0m", color, text);
                std::io::stdout().flush().unwrap();
            }
            ResponseChunk::ToolUse { name, input } => {
                println!("{}[Tool: {}]\x1b[0m", color, name);
            }
            ResponseChunk::Metadata(_) => {}
        }
    }
    
    async fn response_completed(&self, worker_id: Uuid) {
        println!();  // Newline after response
    }
    
    async fn worker_error(&self, worker_id: Uuid, error: String) {
        eprintln!("\x1b[31m[Worker {}] Error: {}\x1b[0m", worker_id, error);
    }
    
    async fn tool_execution_started(&self, worker_id: Uuid, tool_name: String, _input: String) {
        let color = self.get_worker_color(worker_id);
        println!("{}[Executing: {}]\x1b[0m", color, tool_name);
    }
    
    async fn tool_execution_completed(&self, worker_id: Uuid, tool_name: String, result: ToolExecutionResult) {
        let color = self.get_worker_color(worker_id);
        match result {
            ToolExecutionResult::Success(_) => {
                println!("{}[Completed: {}]\x1b[0m", color, tool_name);
            }
            ToolExecutionResult::Error(e) => {
                eprintln!("\x1b[31m[Failed: {}] {}\x1b[0m", tool_name, e);
            }
            ToolExecutionResult::Cancelled => {
                println!("{}[Cancelled: {}]\x1b[0m", color, tool_name);
            }
        }
    }
    
    async fn job_state_changed(&self, worker_id: Uuid, _job_id: Uuid, new_state: JobState) {
        // Optional: could display job state changes
    }
}
```

### 4.3 TextInputAdapter

```rust
struct TextInputAdapter {
    editor: Mutex<rustyline::DefaultEditor>,
    input_queue: Arc<Mutex<VecDeque<PendingInput>>>,
}

impl TextInputAdapter {
    fn new(history_path: Option<PathBuf>) -> Result<Self, eyre::Error> {
        let mut editor = rustyline::DefaultEditor::new()?;
        if let Some(path) = history_path {
            let _ = editor.load_history(&path);
        }
        
        Ok(Self {
            editor: Mutex::new(editor),
            input_queue: Arc::new(Mutex::new(VecDeque::new())),
        })
    }
}

#[async_trait::async_trait]
impl InputAdapter for TextInputAdapter {
    async fn request_input(&self, worker_id: Uuid, worker_name: &str) -> Result<Option<String>, InputError> {
        let prompt = format!("{}> ", worker_name);
        
        let mut editor = self.editor.lock().unwrap();
        match editor.readline(&prompt) {
            Ok(line) => {
                editor.add_history_entry(&line).ok();
                
                // Add to queue
                self.input_queue.lock().unwrap().push_back(PendingInput {
                    worker_id: Some(worker_id),
                    content: line.clone(),
                });
                
                Ok(Some(line))
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                Err(InputError::Cancelled)
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                Ok(None)
            }
            Err(e) => {
                Err(InputError::Io(e.to_string()))
            }
        }
    }
    
    async fn poll_input(&self) -> Option<PendingInput> {
        self.input_queue.lock().unwrap().pop_front()
    }
    
    async fn cancel_input_requests(&self) {
        self.input_queue.lock().unwrap().clear();
    }
}
```

### 4.4 TextInteractionAdapter

```rust
struct TextInteractionAdapter {
    editor: Mutex<rustyline::DefaultEditor>,
}

impl TextInteractionAdapter {
    fn new() -> Self {
        Self {
            editor: Mutex::new(rustyline::DefaultEditor::new().unwrap()),
        }
    }
}

#[async_trait::async_trait]
impl InteractionAdapter for TextInteractionAdapter {
    async fn request_tool_approval(
        &self,
        _worker_id: Uuid,
        tool_name: &str,
        tool_input: &str,
        _cancellation_token: CancellationToken,
    ) -> Result<bool, InteractionError> {
        println!("\n\x1b[33mTool approval requested:\x1b[0m");
        println!("  Tool: {}", tool_name);
        println!("  Input: {}", tool_input);
        
        let mut editor = self.editor.lock().unwrap();
        loop {
            match editor.readline("Approve? (y/n): ") {
                Ok(line) => {
                    match line.trim().to_lowercase().as_str() {
                        "y" | "yes" => return Ok(true),
                        "n" | "no" => return Ok(false),
                        _ => println!("Please enter 'y' or 'n'"),
                    }
                }
                Err(_) => return Err(InteractionError::Cancelled),
            }
        }
    }
    
    async fn notify(&self, message: &str, level: NotificationLevel) {
        let color = match level {
            NotificationLevel::Info => "\x1b[34m",
            NotificationLevel::Warning => "\x1b[33m",
            NotificationLevel::Error => "\x1b[31m",
        };
        println!("{}[{:?}] {}\x1b[0m", color, level, message);
    }
    
    async fn confirm(&self, message: &str) -> Result<bool, InteractionError> {
        println!("\x1b[33m{}\x1b[0m", message);
        
        let mut editor = self.editor.lock().unwrap();
        loop {
            match editor.readline("Confirm? (y/n): ") {
                Ok(line) => {
                    match line.trim().to_lowercase().as_str() {
                        "y" | "yes" => return Ok(true),
                        "n" | "no" => return Ok(false),
                        _ => println!("Please enter 'y' or 'n'"),
                    }
                }
                Err(_) => return Err(InteractionError::Cancelled),
            }
        }
    }
}
```

### 4.5 TextLifecycleAdapter

```rust
struct TextLifecycleAdapter;

impl TextLifecycleAdapter {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl LifecycleAdapter for TextLifecycleAdapter {
    async fn on_startup(&self) -> Result<(), eyre::Error> {
        println!("Amazon Q CLI - Agent Environment");
        println!("Type /quit to exit\n");
        Ok(())
    }
    
    async fn on_worker_created(&self, worker_id: Uuid, worker_name: &str) {
        println!("\x1b[32m[Created worker: {}]\x1b[0m", worker_name);
    }
    
    async fn on_worker_destroyed(&self, worker_id: Uuid) {
        println!("\x1b[31m[Destroyed worker: {}]\x1b[0m", worker_id);
    }
    
    async fn on_job_started(&self, worker_id: Uuid, _job_id: Uuid, task_type: &str) {
        println!("\x1b[36m[Started {} for worker {}]\x1b[0m", task_type, worker_id);
    }
    
    async fn on_job_completed(
        &self,
        worker_id: Uuid,
        _job_id: Uuid,
        completion_type: WorkerJobCompletionType,
        error: Option<String>,
    ) {
        match completion_type {
            WorkerJobCompletionType::Normal => {
                println!("\x1b[32m[Completed job for worker {}]\x1b[0m", worker_id);
            }
            WorkerJobCompletionType::Cancelled => {
                println!("\x1b[33m[Cancelled job for worker {}]\x1b[0m", worker_id);
            }
            WorkerJobCompletionType::Failed => {
                eprintln!("\x1b[31m[Failed job for worker {}]: {:?}\x1b[0m", worker_id, error);
            }
        }
    }
    
    async fn on_shutdown(&self) -> Result<(), eyre::Error> {
        println!("\nShutting down...");
        Ok(())
    }
    
    async fn on_shutdown_requested(&self) {
        println!("\n\x1b[33mShutdown requested, cleaning up...\x1b[0m");
    }
}
```

---

## 5. Main Application Loop Structure

### 5.1 Entry Point

```rust
// In crates/chat-cli/src/cli/chat/mod.rs

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Build session
        let session = Arc::new(build_session().await?);
        
        // Create UI
        let history_path = directories::chat_cli_bash_history_path(os).ok();
        let text_ui = TextUi::new(history_path)?;
        let adapters = text_ui.into_adapter_set();
        
        // Create core loop
        let mut core_loop = CoreLoop::new(session, adapters);
        
        // Create initial workers
        let worker1 = core_loop.create_worker("Worker#1".to_string()).await;
        let worker2 = core_loop.create_worker("Worker#2".to_string()).await;
        
        // Handle initial input if provided
        if let Some(input) = self.input {
            // Queue input for both workers
            core_loop.handle_input(PendingInput {
                worker_id: Some(worker1.id),
                content: input.clone(),
            }).await?;
            
            core_loop.handle_input(PendingInput {
                worker_id: Some(worker2.id),
                content: input,
            }).await?;
        }
        
        // Run main loop
        core_loop.run().await?;
        
        Ok(ExitCode::SUCCESS)
    }
}
```

### 5.2 Alternative: Composite UI

Example of combining multiple UI implementations:

```rust
pub struct CompositeUi {
    text_output: Arc<TextOutputAdapter>,
    web_output: Arc<WebOutputAdapter>,
    text_input: Arc<TextInputAdapter>,
    text_interaction: Arc<TextInteractionAdapter>,
    lifecycle: Arc<CompositeLifecycleAdapter>,
}

impl CompositeUi {
    pub fn into_adapter_set(self) -> UiAdapterSet {
        // Use composite output that broadcasts to both
        let composite_output = Arc::new(BroadcastOutputAdapter::new(vec![
            self.text_output,
            self.web_output,
        ]));
        
        UiAdapterSet::new(
            composite_output,
            self.text_input,
            self.text_interaction,
            self.lifecycle,
        )
    }
}

/// Output adapter that broadcasts to multiple adapters
struct BroadcastOutputAdapter {
    adapters: Vec<Arc<dyn OutputAdapter>>,
}

#[async_trait::async_trait]
impl OutputAdapter for BroadcastOutputAdapter {
    async fn worker_state_changed(&self, worker_id: Uuid, new_state: WorkerStates) {
        for adapter in &self.adapters {
            adapter.worker_state_changed(worker_id, new_state).await;
        }
    }
    
    // ... implement other methods similarly
}
```

---

## 6. Example UI Implementation Outline

### 6.1 Web API UI

```rust
pub struct WebApiUi {
    output: Arc<WebOutputAdapter>,
    input: Arc<WebInputAdapter>,
    interaction: Arc<WebInteractionAdapter>,
    lifecycle: Arc<WebLifecycleAdapter>,
}

/// Web output adapter using websockets
struct WebOutputAdapter {
    websocket_manager: Arc<WebSocketManager>,
}

#[async_trait::async_trait]
impl OutputAdapter for WebOutputAdapter {
    async fn worker_state_changed(&self, worker_id: Uuid, new_state: WorkerStates) {
        let event = json!({
            "type": "worker_state_changed",
            "worker_id": worker_id,
            "state": format!("{:?}", new_state),
        });
        self.websocket_manager.broadcast(event).await;
    }
    
    async fn response_chunk_received(&self, worker_id: Uuid, chunk: ResponseChunk) {
        let event = json!({
            "type": "response_chunk",
            "worker_id": worker_id,
            "chunk": chunk,
        });
        self.websocket_manager.broadcast(event).await;
    }
    
    // ... other methods
}

/// Web input adapter using REST API
struct WebInputAdapter {
    input_queue: Arc<Mutex<VecDeque<PendingInput>>>,
    http_server: Arc<HttpServer>,
}

#[async_trait::async_trait]
impl InputAdapter for WebInputAdapter {
    async fn request_input(&self, worker_id: Uuid, worker_name: &str) -> Result<Option<String>, InputError> {
        // Send prompt request via websocket
        // Wait for input via HTTP POST endpoint
        // Implementation details...
        todo!()
    }
    
    async fn poll_input(&self) -> Option<PendingInput> {
        self.input_queue.lock().unwrap().pop_front()
    }
    
    async fn cancel_input_requests(&self) {
        self.input_queue.lock().unwrap().clear();
    }
}
```

### 6.2 Fancy TUI with Widgets

```rust
pub struct FancyTui {
    output: Arc<FancyOutputAdapter>,
    input: Arc<FancyInputAdapter>,
    interaction: Arc<FancyInteractionAdapter>,
    lifecycle: Arc<FancyLifecycleAdapter>,
}

/// Fancy output using ratatui
struct FancyOutputAdapter {
    terminal: Arc<Mutex<Terminal<CrosstermBackend<std::io::Stdout>>>>,
    worker_panes: Arc<Mutex<HashMap<Uuid, WorkerPane>>>,
}

struct WorkerPane {
    buffer: Vec<String>,
    scroll_offset: usize,
}

#[async_trait::async_trait]
impl OutputAdapter for FancyOutputAdapter {
    async fn worker_state_changed(&self, worker_id: Uuid, new_state: WorkerStates) {
        // Update worker pane header with state
        let mut panes = self.worker_panes.lock().unwrap();
        if let Some(pane) = panes.get_mut(&worker_id) {
            // Update pane state
        }
        
        // Trigger redraw
        self.redraw().await;
    }
    
    async fn response_chunk_received(&self, worker_id: Uuid, chunk: ResponseChunk) {
        // Append to worker pane buffer
        let mut panes = self.worker_panes.lock().unwrap();
        if let Some(pane) = panes.get_mut(&worker_id) {
            if let ResponseChunk::Text(text) = chunk {
                pane.buffer.push(text);
            }
        }
        
        // Trigger redraw
        self.redraw().await;
    }
    
    // ... other methods
}

impl FancyOutputAdapter {
    async fn redraw(&self) {
        // Redraw terminal using ratatui
        // Layout: split screen into panes for each worker
        // Show state, output buffer, etc.
    }
}
```

---

## 7. Error Handling and Edge Cases

### 7.1 Error Handling Strategy

```rust
/// Errors that can occur in core loop
#[derive(Debug, Error)]
pub enum CoreLoopError {
    #[error("Session error: {0}")]
    Session(#[from] eyre::Error),
    
    #[error("Input error: {0}")]
    Input(#[from] InputError),
    
    #[error("Interaction error: {0}")]
    Interaction(#[from] InteractionError),
    
    #[error("Worker not found: {0}")]
    WorkerNotFound(Uuid),
    
    #[error("No available workers")]
    NoAvailableWorkers,
}

impl CoreLoop {
    /// Handle errors gracefully
    async fn handle_error(&self, error: CoreLoopError) {
        match error {
            CoreLoopError::Input(InputError::Cancelled) => {
                // User cancelled, trigger shutdown
                self.shutdown_signal.notify_one();
            }
            CoreLoopError::NoAvailableWorkers => {
                self.adapters.interaction.notify(
                    "No workers available to handle request",
                    NotificationLevel::Warning,
                ).await;
            }
            _ => {
                self.adapters.interaction.notify(
                    &format!("Error: {}", error),
                    NotificationLevel::Error,
                ).await;
            }
        }
    }
}
```

### 7.2 Edge Cases

**1. Worker dies during execution**
```rust
impl CoreLoop {
    async fn poll_state_changes(&mut self) -> Result<(), eyre::Error> {
        // ... existing code ...
        
        // Check for failed workers
        for (worker_id, ctx) in workers.iter() {
            if let Some(error) = ctx.worker.get_failure() {
                self.adapters.output.worker_error(*worker_id, error).await;
                
                // Reset worker state
                ctx.worker.set_state(WorkerStates::Inactive, &*self.adapters.output);
            }
        }
        
        Ok(())
    }
}
```

**2. Input requested for non-existent worker**
```rust
impl CoreLoop {
    async fn handle_input(&mut self, input: PendingInput) -> Result<(), eyre::Error> {
        if let Some(worker_id) = input.worker_id {
            if !self.active_workers.lock().unwrap().contains_key(&worker_id) {
                return Err(CoreLoopError::WorkerNotFound(worker_id).into());
            }
        }
        
        // ... rest of implementation
    }
}
```

**3. Multiple shutdown signals**
```rust
impl CoreLoop {
    fn setup_shutdown_handler(&self) {
        let shutdown_signal = self.shutdown_signal.clone();
        let shutdown_triggered = Arc::new(AtomicBool::new(false));
        
        tokio::spawn(async move {
            loop {
                if let Ok(()) = tokio::signal::ctrl_c().await {
                    if !shutdown_triggered.swap(true, Ordering::SeqCst) {
                        shutdown_signal.notify_one();
                    }
                }
            }
        });
    }
}
```

**4. Adapter method panics**
```rust
// Wrap adapter calls in catch_unwind or use tokio::spawn with error handling
impl CoreLoop {
    async fn safe_adapter_call<F, Fut>(&self, f: F) 
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        let result = tokio::spawn(f()).await;
        if let Err(e) = result {
            tracing::error!("Adapter call panicked: {:?}", e);
        }
    }
}
```

**5. Deadlock prevention**
```rust
// Use timeout for adapter calls
impl CoreLoop {
    async fn call_adapter_with_timeout<F, Fut>(&self, f: F, timeout: Duration) -> Result<(), eyre::Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        match tokio::time::timeout(timeout, f()).await {
            Ok(_) => Ok(()),
            Err(_) => {
                tracing::error!("Adapter call timed out");
                Err(eyre::eyre!("Adapter timeout"))
            }
        }
    }
}
```

---

## 8. Migration Path from Current Implementation

### Phase 1: Create Adapter Traits (Week 1)
1. Define all adapter traits in new module: `crates/chat-cli/src/agent_env/ui_adapters/`
2. Create trait definitions: `output.rs`, `input.rs`, `interaction.rs`, `lifecycle.rs`
3. Add `UiAdapterSet` struct
4. No changes to existing code yet

### Phase 2: Implement TextUI Adapters (Week 1-2)
1. Create `text_ui/` module with adapter implementations
2. Extract logic from current `agent_env_ui/` into adapters
3. Keep both implementations running in parallel
4. Add feature flag to switch between old and new

### Phase 3: Create CoreLoop (Week 2)
1. Implement `CoreLoop` struct in `crates/chat-cli/src/agent_env/core_loop.rs`
2. Implement basic event loop with polling
3. Add worker management methods
4. Test with TextUI adapters

### Phase 4: Create Bridge Interface (Week 2-3)
1. Implement `AdapterBridgeInterface` to connect `WorkerToHostInterface` to `OutputAdapter`
2. Update job launching to use bridge
3. Test end-to-end flow

### Phase 5: Update Entry Point (Week 3)
1. Modify `ChatArgs::execute()` to use CoreLoop
2. Keep old implementation behind feature flag
3. Test both paths

### Phase 6: Deprecate Old Implementation (Week 4)
1. Remove old `agent_env_ui/` implementation
2. Remove feature flags
3. Update documentation

### Phase 7: Add Alternative UIs (Week 5+)
1. Implement WebApiUi adapters
2. Implement FancyTui adapters
3. Add CLI flags to select UI mode

### Migration Checklist

- [ ] Define adapter traits
- [ ] Implement TextUI adapters
- [ ] Create CoreLoop
- [ ] Create AdapterBridgeInterface
- [ ] Update entry point
- [ ] Test with existing functionality
- [ ] Remove old implementation
- [ ] Add WebApiUi
- [ ] Add FancyTui
- [ ] Update documentation

---

## 9. Benefits and Trade-offs

### Benefits

1. **Clear Contracts**: Adapter traits explicitly define UI requirements
2. **Type Safety**: Compiler enforces interface compliance
3. **Testability**: Easy to mock adapters for testing
4. **Composability**: Mix and match adapter implementations
5. **Flexibility**: Support multiple UI modes simultaneously
6. **Maintainability**: Clear separation of concerns

### Trade-offs

1. **Complexity**: More abstraction layers than current implementation
2. **Boilerplate**: Need to implement multiple traits per UI
3. **Performance**: Additional indirection through trait objects
4. **Learning Curve**: Developers need to understand adapter pattern
5. **Polling Overhead**: Core loop polls for state changes (could use events instead)

### When to Use This Approach

**Good fit:**
- Need to support multiple UI implementations
- Want strong type safety and compile-time guarantees
- Team comfortable with trait-based abstractions
- Performance overhead acceptable (minimal for CLI app)

**Not ideal:**
- Simple single-UI application
- Need maximum performance (avoid trait object overhead)
- Team prefers simpler callback-based approach

---

## 10. Future Enhancements

### 10.1 Event-Based State Changes

Replace polling with event channels:

```rust
pub struct CoreLoop {
    session: Arc<Session>,
    adapters: UiAdapterSet,
    state_change_rx: mpsc::Receiver<StateChangeEvent>,
}

enum StateChangeEvent {
    WorkerStateChanged(Uuid, WorkerStates),
    JobCompleted(Uuid, Uuid, WorkerJobCompletionType),
}
```

### 10.2 Adapter Middleware

Add middleware layer for cross-cutting concerns:

```rust
pub trait AdapterMiddleware: Send + Sync {
    async fn before_output(&self, event: &OutputEvent) -> Result<(), eyre::Error>;
    async fn after_output(&self, event: &OutputEvent) -> Result<(), eyre::Error>;
}

// Example: Logging middleware
struct LoggingMiddleware;

#[async_trait::async_trait]
impl AdapterMiddleware for LoggingMiddleware {
    async fn before_output(&self, event: &OutputEvent) -> Result<(), eyre::Error> {
        tracing::debug!("Output event: {:?}", event);
        Ok(())
    }
}
```

### 10.3 Dynamic Adapter Registration

Allow runtime adapter registration:

```rust
impl CoreLoop {
    pub fn register_output_adapter(&mut self, adapter: Arc<dyn OutputAdapter>) {
        // Add to list of output adapters
    }
    
    pub fn unregister_output_adapter(&mut self, adapter_id: Uuid) {
        // Remove from list
    }
}
```

---

## Conclusion

The Layered Interface with UI Adapters approach provides a robust, type-safe architecture for supporting multiple UI implementations in the Q CLI agent environment. By defining explicit adapter traits and implementing a core application loop that routes events to these adapters, we achieve:

- Clear separation between core logic and UI
- Flexibility to swap or combine UI implementations
- Strong compile-time guarantees
- Testability through adapter mocking

The migration path allows incremental adoption without disrupting existing functionality, and the design supports future enhancements like event-based state changes and adapter middleware.

This approach is well-suited for the Q CLI's requirements of supporting multiple UI modes (text, web, fancy TUI) while maintaining clean architecture and code quality.
