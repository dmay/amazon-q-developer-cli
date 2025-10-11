# MVP Small Wins - Technical Design

## Document Status

**Status**: Draft  
**Created**: 2025-10-11  
**Last Updated**: 2025-10-11

## Table of Contents

1. [Overview and Goals](#overview-and-goals)
2. [Task 1.1: --no-interactive Support](#task-11---no-interactive-support)
3. [Task 1.6: StructuredIO Enhancements](#task-16-structuredio-enhancements)
4. [Implementation Considerations](#implementation-considerations)
5. [Testing Strategy](#testing-strategy)
6. [References](#references)

---

## Overview and Goals

This document provides technical design for two foundational improvements to the agent_env architecture:

1. **Task 1.1: --no-interactive Support** - Enable CLI to run in non-interactive mode, processing a single task and exiting cleanly
2. **Task 1.6: StructuredIO Enhancements** - Improve event handling and quit command responsiveness in structured JSON I/O mode

### Design Principles

- **Minimal Changes**: Leverage existing architecture without major refactoring
- **Clean Separation**: UI concerns remain in UI layer, job tracking in AgentEnvironment
- **Event-Driven**: Use existing EventBus for all communication
- **Extensibility**: Design for future enhancements (multiple initial jobs, additional UIs)

### Success Criteria

**Task 1.1:**
- CLI exits cleanly after processing initial input in non-interactive mode
- No hanging processes or resource leaks
- Clear indication of clean vs non-clean exit states
- Both TextUi and StructuredIO support the flag

**Task 1.6:**
- All worker lifecycle events visible in StructuredIO output
- Quit command responds immediately without blocking
- Events appear in correct chronological order
- Initial worker creation event is captured

---


## Task 1.1: --no-interactive Support

### Problem Statement

The `--no-interactive` flag exists but is not fully implemented. When set, the CLI should:
1. Process only the initial input provided via command line
2. Execute the agent loop once
3. Exit cleanly after task completion
4. Indicate whether the task completed in a "clean" state (waiting for prompt) or "non-clean" state (waiting for tool approval)

Currently, both TextUi and StructuredIO spawn input reading tasks that continue indefinitely, preventing clean exit.

### Architecture Overview

The design involves three layers:

1. **UI Layer**: Conditionally spawn input reading tasks based on `no_interactive` flag
2. **Coordination Layer**: Track initial jobs and trigger shutdown when complete
3. **Event Layer**: Add metadata to indicate job completion state

### Component Designs

#### 1. JobCompletionResult Enhancement

Add a `user_interaction_required` enum to indicate what type of interaction is needed:

```rust
// In: crates/chat-cli/src/agent_env/events.rs

/// Type of user interaction required for task to continue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserInteractionRequired {
    /// No interaction needed - task completed normally
    None,
    /// Task is waiting for tool approval
    ToolApproval,
    // Future: Prompt, FileSelection, etc.
}

pub enum JobCompletionResult {
    Success {
        task_metadata: HashMap<String, serde_json::Value>,
        user_interaction_required: UserInteractionRequired,  // NEW
    },
    Cancelled,
    Failed {
        error: String,
    },
}
```

**Rationale**: Enum is more extensible than boolean. Future tasks can add other interaction types (e.g., `Prompt`, `FileSelection`).

**Usage**:
- AgentLoop sets `UserInteractionRequired::None` when completing normally (response delivered)
- AgentLoop sets `UserInteractionRequired::ToolApproval` when completing while waiting for tool approval
- Other tasks can use additional variants as needed

#### 2. Session Active Jobs Query

Add method to Session to check for active jobs:

```rust
// In: crates/chat-cli/src/agent_env/session.rs

impl Session {
    /// Check if there are any active jobs
    pub fn has_active_jobs(&self) -> bool {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().any(|job| job.is_active())
    }
}
```

**Note**: Jobs remain in the `jobs` Vec after completion until `cleanup_inactive_jobs()` is called. We must check `job.is_active()` rather than Vec emptiness.

#### 3. AgentEnvironment Job Monitoring

Add interactive flag to AgentEnvironment (note: inverted from CLI flag):

```rust
// In: crates/chat-cli/src/agent_env/agent_environment.rs

pub struct AgentEnvironment {
    session: Arc<Session>,
    event_bus: EventBus,
    main_ui: Option<Arc<dyn UserInterface>>,
    headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    shutdown_signal: Arc<Notify>,
    
    // NEW: Interactive mode flag (inverted from --no-interactive)
    interactive: bool,
}

impl AgentEnvironment {
    pub fn new(
        session: Arc<Session>,
        event_bus: EventBus,
        main_ui: Option<Arc<dyn UserInterface>>,
        headless_uis: Vec<Arc<dyn HeadlessInterface>>,
        interactive: bool,  // NEW
    ) -> Self {
        Self {
            session,
            event_bus,
            main_ui,
            headless_uis,
            shutdown_signal: Arc::new(Notify::new()),
            interactive,
        }
    }
}
```

**Job Completion Handling**:

In `AgentEnvironment::run()`, spawn job completion monitor BEFORE launching any jobs:

```rust
fn spawn_job_completion_monitor(&self) -> JoinHandle<()> {
    let mut receiver = self.event_bus.subscribe();
    let session = self.session.clone();
    let shutdown_signal = self.shutdown_signal.clone();
    let interactive = self.interactive;
    
    tokio::spawn(async move {
        if interactive {
            return; // Only monitor in non-interactive mode
        }
        
        loop {
            match receiver.recv().await {
                Ok(AgentEnvironmentEvent::Job(JobEvent::Completed { result, .. })) => {
                    // Check if any jobs remain active
                    if !session.has_active_jobs() {
                        // All jobs complete - check for non-clean exit
                        match result {
                            JobCompletionResult::Success { user_interaction_required, .. } => {
                                match user_interaction_required {
                                    UserInteractionRequired::None => {
                                        // Clean exit
                                    }
                                    UserInteractionRequired::ToolApproval => {
                                        eprintln!("Error: Task completed while waiting for tool approval (non-clean exit)");
                                        // Could set exit code here
                                    }
                                }
                            }
                            JobCompletionResult::Failed { error } => {
                                eprintln!("Error: Task failed: {}", error);
                            }
                            JobCompletionResult::Cancelled => {
                                eprintln!("Error: Task was cancelled");
                            }
                        }
                        
                        // Trigger shutdown
                        shutdown_signal.notify_waiters();
                        break;
                    }
                }
                Ok(_) => {} // Ignore other events
                Err(_) => break,
            }
        }
    })
}
```

**Edge Cases**:
- **No initial jobs**: If no jobs are spawned before run() completes, exit with error in non-interactive mode
- **Job completes before monitoring starts**: Start monitoring BEFORE spawning jobs to avoid race condition
- **Multiple jobs**: Design naturally supports multiple jobs - waits for all to complete

#### 4. TextUi Modifications

Add `interactive` field and conditionally spawn prompt loop:

```rust
// In: crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs

pub struct TextUi {
    session: Arc<Session>,
    main_worker_id: Uuid,
    input_handler: Arc<tokio::sync::Mutex<InputHandler>>,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<std::sync::Mutex<Option<mpsc::Receiver<PromptResult>>>>,
    prompt_ready: Arc<Notify>,
    shutdown_signal: Arc<Notify>,
    interactive: bool,  // NEW
}

impl TextUi {
    pub fn new(
        session: Arc<Session>,
        main_worker_id: Uuid,
        history_path: Option<PathBuf>,
        interactive: bool,  // NEW
    ) -> Result<Self, eyre::Error> {
        // ... existing code ...
        Ok(Self {
            // ... existing fields ...
            interactive,
        })
    }
}

#[async_trait]
impl UserInterface for TextUi {
    async fn start(&self) -> Result<()> {
        // Only spawn prompt loop in interactive mode
        if self.interactive {
            self.spawn_prompt_loop();
        }
        
        // Check if worker is idle and signal prompt_ready (existing logic)
        // ...
        
        Ok(())
    }
    
    // ... rest of implementation unchanged ...
}
```

#### 5. StructuredIO Modifications

Similar changes to StructuredIO:

```rust
// In: crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs

pub struct StructuredIO {
    session: Arc<Session>,
    output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<StdMutex<Option<mpsc::Receiver<PromptResult>>>>,
    shutdown_signal: Arc<Notify>,
    interactive: bool,  // NEW
}

impl StructuredIO {
    pub fn new(
        session: Arc<Session>,
        interactive: bool,  // NEW
    ) -> Result<Self> {
        // ... existing code ...
        Ok(Self {
            // ... existing fields ...
            interactive,
        })
    }
}

#[async_trait]
impl UserInterface for StructuredIO {
    async fn start(&self) -> Result<()> {
        // Only spawn input reader in interactive mode
        if self.interactive {
            self.spawn_input_reader();
        }
        Ok(())
    }
    
    // ... rest of implementation unchanged ...
}
```

#### 6. ChatArgs::execute() Integration

Update ChatArgs::execute() to pass `interactive` flag and create components in correct order:

```rust
// In: crates/chat-cli/src/cli/chat/mod.rs

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Create EventBus and Session
        let event_bus = EventBus::default();
        let session = Arc::new(Session::new(event_bus.clone(), model_providers));
        
        // Invert flag for internal use
        let interactive = !self.no_interactive;
        
        // Create UI BEFORE worker (with placeholder worker_id for now)
        let main_ui: Option<Arc<dyn UserInterface>> = match ui_mode {
            UiMode::Text => {
                let text_ui = TextUi::new(
                    session.clone(),
                    Uuid::nil(),  // Placeholder
                    history_path,
                    interactive,
                )?;
                Some(Arc::new(text_ui))
            }
            UiMode::Structured => {
                let structured_io = StructuredIO::new(
                    session.clone(),
                    interactive,
                )?;
                Some(Arc::new(structured_io))
            }
            UiMode::None => None,
        };
        
        // Create AgentEnvironment
        let agent_env = AgentEnvironment::new(
            session.clone(),
            event_bus.clone(),
            main_ui.clone(),
            vec![],
            interactive,
        );
        
        // NOW create worker (UI is already subscribed to EventBus)
        let main_worker = session.build_worker("main".to_string());
        
        // Update UI with actual worker_id (TextUi only)
        if let Some(ui) = &main_ui {
            if let UiMode::Text = ui_mode {
                // Set worker_id on TextUi
                // (requires adding set_main_worker_id method)
            }
        }
        
        // Add initial input if provided
        if let Some(initial_input) = &self.input {
            main_worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(initial_input.clone());
        } else if !interactive {
            // No input in non-interactive mode - error
            return Err(eyre::eyre!("No input provided for non-interactive mode"));
        }
        
        // Start job completion monitor BEFORE spawning jobs
        if !interactive {
            agent_env.spawn_job_completion_monitor();
        }
        
        // Launch initial job if input provided
        if self.input.is_some() {
            session.run_task__agent_loop(main_worker, AgentLoopInput {})?;
        }
        
        // Check for no jobs in non-interactive mode
        if !interactive && !session.has_active_jobs() {
            return Err(eyre::eyre!("No jobs spawned in non-interactive mode"));
        }
        
        // Run
        agent_env.run().await?;
        
        Ok(ExitCode::SUCCESS)
    }
}
```

### Execution Flow

**Non-Interactive Mode**:

1. User runs: `q chat --no-interactive "What is 2+2?"`
2. ChatArgs::execute() creates EventBus, Session
3. TextUi created with `interactive = false`
4. AgentEnvironment created with `interactive = false`
5. Worker created (UI already subscribed, receives WorkerEvent::Created)
6. Initial input added to conversation history
7. Job completion monitor spawned
8. AgentLoop job launched
9. AgentEnvironment.run() starts:
   - TextUi.start() called - does NOT spawn prompt loop
10. AgentLoop executes, publishes events
11. AgentLoop completes, publishes JobEvent::Completed
12. Job completion monitor receives event:
    - Checks session.has_active_jobs() - returns false
    - Checks user_interaction_required field
    - Triggers shutdown
13. AgentEnvironment exits, cancels any remaining jobs
14. CLI exits with appropriate exit code

**Interactive Mode** (unchanged):

1. User runs: `q chat "What is 2+2?"`
2. Same initialization as above, but `interactive = true`
3. TextUi.start() spawns prompt loop
4. Job completion monitor not spawned
5. After AgentLoop completes, prompt loop waits for next input
6. User can continue conversation or quit manually

### Design Alternatives Considered

#### Alternative 1: Track Initial Jobs Instead of Checking Session

Track specific initial job IDs in AgentEnvironment:

```rust
pub struct AgentEnvironment {
    initial_job_ids: Arc<Mutex<HashSet<Uuid>>>,
    // ...
}

pub fn register_initial_job(&self, job_id: Uuid) {
    self.initial_job_ids.lock().unwrap().insert(job_id);
}
```

**Rejected**: More complex, requires explicit registration. Checking `session.has_active_jobs()` is simpler and handles all jobs automatically.

#### Alternative 2: Use Boolean Flag Instead of Enum

Use `waiting_for_input: bool` instead of `UserInteractionRequired` enum:

```rust
pub enum JobCompletionResult {
    Success {
        task_metadata: HashMap<String, serde_json::Value>,
        waiting_for_input: bool,
    },
    // ...
}
```

**Rejected**: Not extensible. Enum allows for future interaction types (Prompt, FileSelection, etc.).

#### Alternative 3: Separate Exit Codes for Clean/Non-Clean

Use different exit codes to indicate clean vs non-clean exits:

```rust
if user_interaction_required != UserInteractionRequired::None {
    return Ok(ExitCode::from(2)); // Non-clean exit
} else {
    return Ok(ExitCode::SUCCESS); // Clean exit
}
```

**Deferred**: Can be added later if needed. For MVP, error message is sufficient.


## Task 1.6: StructuredIO Enhancements

### Problem Statement

StructuredIO has three issues:

1. **Missing Events**: WorkerEvent::Created, WorkerEvent::Deleted, JobEvent::Started, and JobEvent::Completed are not displayed in JSON output
2. **Event Timing**: Initial WorkerEvent::Created may be missed because StructuredIO subscribes to EventBus AFTER the worker is created
3. **Quit Command Blocking**: The `{"command":"quit"}` may not respond immediately because `lines.next_line()` blocks the entire task

### Architecture Overview

The design addresses three areas:

1. **Event Handlers**: Add handlers for all worker and job lifecycle events, output events for ALL workers (not just main)
2. **Event Timing**: Create StructuredIO before worker to ensure all events are captured
3. **Input Reading**: Use separate reader task pattern to make quit command responsive

### Component Designs

#### 1. Add Event Handlers

Extend StructuredIO::handle_event() to handle all worker and job lifecycle events.

**Remove main_worker_id filtering** - StructuredIO outputs events for ALL workers:

```rust
// In: crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs

pub struct StructuredIO {
    session: Arc<Session>,
    output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<StdMutex<Option<mpsc::Receiver<PromptResult>>>>,
    shutdown_signal: Arc<Notify>,
    interactive: bool,
    // REMOVED: main_worker_id field
}

impl StructuredIO {
    pub fn new(
        session: Arc<Session>,
        interactive: bool,
    ) -> Result<Self> {
        // ... existing code ...
        Ok(Self {
            session,
            output_writer: Arc::new(TokioMutex::new(Box::new(std::io::stdout()))),
            cmd_sender,
            cmd_receiver: Arc::new(StdMutex::new(Some(cmd_receiver))),
            shutdown_signal: Arc::new(Notify::new()),
            interactive,
            // REMOVED: main_worker_id
        })
    }
}

async fn handle_event(&self, event: AgentEnvironmentEvent) {
    // NO FILTERING - output all events from all workers
    
    match event {
        // NEW: Handle WorkerEvent::Created
        AgentEnvironmentEvent::Worker(WorkerEvent::Created { worker_id, name, timestamp }) => {
            let json = json!({
                "event": "worker_created",
                "worker_id": worker_id.to_string(),
                "name": name,
                "timestamp": format!("{:?}", timestamp),
            });
            
            let mut writer = self.output_writer.lock().await;
            writeln!(writer, "{}", json).unwrap();
            writer.flush().unwrap();
        }
        
        // NEW: Handle WorkerEvent::Deleted
        AgentEnvironmentEvent::Worker(WorkerEvent::Deleted { worker_id, timestamp }) => {
            let json = json!({
                "event": "worker_deleted",
                "worker_id": worker_id.to_string(),
                "timestamp": format!("{:?}", timestamp),
            });
            
            let mut writer = self.output_writer.lock().await;
            writeln!(writer, "{}", json).unwrap();
            writer.flush().unwrap();
        }
        
        // NEW: Handle JobEvent::Started
        AgentEnvironmentEvent::Job(JobEvent::Started { worker_id, job_id, task_type, timestamp }) => {
            let json = json!({
                "event": "job_started",
                "worker_id": worker_id.to_string(),
                "job_id": job_id.to_string(),
                "task_type": task_type,
                "timestamp": format!("{:?}", timestamp),
            });
            
            let mut writer = self.output_writer.lock().await;
            writeln!(writer, "{}", json).unwrap();
            writer.flush().unwrap();
        }
        
        // NEW: Handle JobEvent::Completed
        AgentEnvironmentEvent::Job(JobEvent::Completed { worker_id, job_id, result, timestamp }) => {
            let result_str = match result {
                JobCompletionResult::Success { .. } => "success",
                JobCompletionResult::Cancelled => "cancelled",
                JobCompletionResult::Failed { .. } => "failed",
            };
            
            let json = json!({
                "event": "job_completed",
                "worker_id": worker_id.to_string(),
                "job_id": job_id.to_string(),
                "result": result_str,
                "timestamp": format!("{:?}", timestamp),
            });
            
            let mut writer = self.output_writer.lock().await;
            writeln!(writer, "{}", json).unwrap();
            writer.flush().unwrap();
        }
        
        // Existing handlers...
        AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { .. }) => {
            // ... existing code ...
        }
        AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived { .. }) => {
            // ... existing code ...
        }
        AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ToolUseRequestReceived { .. }) => {
            // ... existing code ...
        }
        _ => {}
    }
}
```

**JSON Output Format**:

```json
{"event":"worker_created","worker_id":"550e8400-e29b-41d4-a716-446655440000","name":"main","timestamp":"Instant { ... }"}
{"event":"worker_deleted","worker_id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"Instant { ... }"}
{"event":"job_started","worker_id":"...","job_id":"...","task_type":"agent_loop","timestamp":"..."}
{"event":"job_completed","worker_id":"...","job_id":"...","result":"success","timestamp":"..."}
```

#### 2. Fix Event Timing

**Problem**: StructuredIO subscribes to EventBus in its constructor, but the worker is created BEFORE StructuredIO is instantiated in ChatArgs::execute(). This causes the initial WorkerEvent::Created to be missed.

**Solution**: Create StructuredIO BEFORE creating worker (already covered in Task 1.1 ChatArgs::execute() design):

```rust
// In: crates/chat-cli/src/cli/chat/mod.rs

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Create EventBus and Session
        let event_bus = EventBus::default();
        let session = Arc::new(Session::new(event_bus.clone(), model_providers));
        
        // Create StructuredIO BEFORE worker
        let structured_io = StructuredIO::new(
            session.clone(),
            interactive,
        )?;
        
        // NOW create worker (StructuredIO is already subscribed)
        let main_worker = session.build_worker("main".to_string());
        
        // StructuredIO receives WorkerEvent::Created automatically
        
        // ... rest of initialization ...
    }
}
```

**No additional changes needed** - StructuredIO doesn't need worker_id, so it can be created before any workers exist.

#### 3. Fix Quit Command Blocking

**Problem**: `spawn_input_reader()` uses `lines.next_line().await` which blocks the entire task, even when wrapped in `tokio::select!`. The quit command cannot interrupt this blocking read.

**Solution**: Use separate reader task pattern - spawn a dedicated task for stdin reading that can be aborted:

```rust
// In: crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs

fn spawn_input_reader(&self) -> JoinHandle<()> {
    let cmd_sender = self.cmd_sender.clone();
    let shutdown = self.shutdown_signal.clone();
    let session = self.session.clone();
    
    // Spawn reader task
    let reader_handle = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();
        
        // Channel to send lines from reader to processor
        let (line_tx, mut line_rx) = mpsc::channel::<String>(10);
        
        // Spawn actual stdin reader (this will block on next_line)
        let reader_task = tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                if line_tx.send(line).await.is_err() {
                    break; // Processor closed channel
                }
            }
        });
        
        // Process lines with shutdown signal
        loop {
            tokio::select! {
                // Receive line from reader
                Some(line) = line_rx.recv() => {
                    // Parse command
                    let parsed = match serde_json::from_str::<serde_json::Value>(&line) {
                        Ok(json) => json,
                        Err(e) => {
                            eprintln!("Error parsing JSON: {}", e);
                            continue;
                        }
                    };
                    
                    // Handle command
                    if let Some(serde_json::Value::String(cmd)) = parsed.get("command") {
                        match cmd.as_str() {
                            "quit" => {
                                let _ = cmd_sender.send(PromptResult::Command(
                                    AgentEnvironmentCommand::Quit
                                )).await;
                                shutdown.notify_waiters();
                                break;
                            }
                            "prompt" => {
                                let text = parsed.get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                
                                let worker_id = parsed.get("worker_id")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                    .unwrap_or_else(|| {
                                        // Get first worker from session
                                        session.get_workers().first()
                                            .map(|w| w.id)
                                            .unwrap_or(Uuid::nil())
                                    });
                                
                                let _ = cmd_sender.send(PromptResult::Command(
                                    AgentEnvironmentCommand::Prompt { worker_id, text }
                                )).await;
                            }
                            _ => {
                                eprintln!("Unknown command: {}", cmd);
                            }
                        }
                    }
                }
                
                // Handle shutdown signal
                _ = shutdown.notified() => {
                    break;
                }
            }
        }
        
        // Abort reader task (it may still be blocked on next_line)
        reader_task.abort();
        let _ = reader_task.await; // Swallow JoinError
    });
    
    reader_handle
}
```

**Key Changes**:
- Spawn dedicated reader task that blocks on `next_line()`
- Use channel to send lines from reader to processor
- Processor uses `tokio::select!` to handle shutdown signal
- When shutdown is triggered, abort the reader task immediately

**Alternative Approach** (if above is too complex):

Use `spawn_blocking` similar to InputHandler:

```rust
fn spawn_input_reader(&self) -> JoinHandle<()> {
    let cmd_sender = self.cmd_sender.clone();
    let shutdown = self.shutdown_signal.clone();
    let session = self.session.clone();
    
    tokio::spawn(async move {
        loop {
            // Check shutdown before reading
            if shutdown.is_notified() {
                break;
            }
            
            // Read line in blocking task
            let line_result = tokio::task::spawn_blocking(|| {
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).map(|_| line)
            }).await;
            
            let line = match line_result {
                Ok(Ok(line)) if !line.is_empty() => line,
                _ => break,
            };
            
            // Parse and handle command (same as above)
            // ...
            
            // Check for quit command
            if is_quit {
                shutdown.notify_waiters();
                break;
            }
        }
    })
}
```

**Recommendation**: Use the first approach (reader task + channel) for better responsiveness. The reader task can be aborted immediately when shutdown is triggered.

### Execution Flow

**Event Display Flow**:

1. ChatArgs::execute() creates EventBus
2. ChatArgs::execute() creates Session
3. ChatArgs::execute() creates StructuredIO (subscribes to EventBus)
4. ChatArgs::execute() creates Worker → publishes WorkerEvent::Created
5. StructuredIO misses Created event (timing issue)
6. **Fix**: Manually emit Created event to StructuredIO after creation
7. AgentEnvironment starts, multicasts events to StructuredIO
8. StructuredIO receives all subsequent events and outputs JSON

**Quit Command Flow**:

1. User pipes quit command: `echo '{"command":"quit"}' | q chat --ui-mode=structured`
2. StructuredIO spawns input reader task
3. Input reader reads `{"command":"quit"}` from stdin
4. `tokio::select!` receives line, parses command
5. Quit command sent to AgentEnvironment via channel
6. Shutdown signal notified
7. Input reader task breaks immediately (doesn't wait for more input)
8. AgentEnvironment receives quit command, triggers shutdown
9. CLI exits cleanly

### Design Alternatives Considered

#### Alternative 1: Use tokio::select! Directly

Wrap `next_line()` in `tokio::select!` with shutdown signal:

```rust
tokio::select! {
    result = lines.next_line() => { /* handle */ }
    _ = shutdown.notified() => break,
}
```

**Rejected**: Testing shows `lines.next_line()` blocks everything, even within `tokio::select!`. The future cannot be cancelled once it starts polling stdin.

#### Alternative 2: Send Enter to Stdin

Programmatically send newline to stdin to unblock `next_line()`:

```rust
// When shutdown is triggered
std::io::stdin().write(b"\n")?;
```

**Rejected**: Cannot write to stdin from the same process. Would require external process or platform-specific hacks.

#### Alternative 3: Use spawn_blocking

Use `spawn_blocking` similar to InputHandler:

```rust
let line = tokio::task::spawn_blocking(|| {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map(|_| line)
}).await?;
```

**Rejected**: Still blocks on stdin read. Cannot be interrupted until input arrives. Reader task pattern is better because the blocking task can be aborted.


## Implementation Considerations

### Cross-Task Dependencies

**Task 1.1 and Task 1.6 are independent** and can be implemented in parallel or in any order.

However, both tasks modify the same files:
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
- `crates/chat-cli/src/cli/chat/mod.rs`

**Recommendation**: Implement Task 1.1 first, then Task 1.6. This minimizes merge conflicts.

### Backward Compatibility

**Task 1.1**:
- Adds new parameter to UI constructors - **breaking change** for any external code creating UIs
- Adds new field to `JobCompletionResult::Success` - **breaking change** for code pattern matching on this variant
- Mitigation: This is internal API, no external consumers expected

**Task 1.6**:
- Adds new JSON events to StructuredIO output - **non-breaking** (additive change)
- Changes input reading behavior - **non-breaking** (internal implementation detail)

### Error Handling

**Task 1.1**:

1. **No input in non-interactive mode**:
   ```rust
   if self.no_interactive && self.input.is_none() {
       return Err(eyre::eyre!("No input provided for non-interactive mode"));
   }
   ```

2. **Job completion monitor errors**:
   - EventBus closed: Log and exit monitor task
   - EventBus lagged: Log warning, continue processing

3. **Empty initial_job_ids**:
   ```rust
   if self.no_interactive && self.initial_job_ids.lock().unwrap().is_empty() {
       tracing::warn!("No initial jobs registered in non-interactive mode, exiting immediately");
       return Ok(());
   }
   ```

**Task 1.6**:

1. **JSON parsing errors**:
   ```rust
   let parsed = match serde_json::from_str::<serde_json::Value>(&line) {
       Ok(json) => json,
       Err(e) => {
           eprintln!("Error parsing JSON: {}", e);
           continue; // Skip invalid input
       }
   };
   ```

2. **Stdin read errors**:
   ```rust
   Err(e) => {
       eprintln!("Error reading stdin: {}", e);
       break; // Exit input reader task
   }
   ```

3. **Event output errors**:
   - If `writeln!()` or `flush()` fails, log error but continue processing events
   - Consider: Should we exit on output errors? (Probably yes - if stdout is broken, no point continuing)

### Performance Considerations

**Task 1.1**:
- Job completion monitor subscribes to EventBus - adds one more subscriber
- Minimal overhead: only processes JobEvent::Completed events
- HashSet operations (insert/remove) are O(1) average case

**Task 1.6**:
- `tokio::select!` adds minimal overhead compared to blocking `next_line()`
- Event handlers add two more match arms - negligible impact
- JSON serialization for events is already happening, just adding two more event types

### Testing Considerations

**Task 1.1**:

1. **Unit Tests**:
   - Test `register_initial_job()` with and without `no_interactive`
   - Test job completion monitor with single and multiple jobs
   - Test `waiting_for_input` flag handling

2. **Integration Tests**:
   - Test full flow with TextUi in non-interactive mode
   - Test full flow with StructuredIO in non-interactive mode
   - Test error case: no input provided
   - Test edge case: job completes before monitor starts (race condition)

3. **Manual Tests**:
   ```bash
   q chat --no-interactive "What is 2+2?"
   q chat --no-interactive --ui-mode=structured "What is 2+2?"
   q chat --no-interactive  # Should error
   ```

**Task 1.6**:

1. **Unit Tests**:
   - Test WorkerEvent::Created handler
   - Test WorkerEvent::Deleted handler
   - Test quit command parsing

2. **Integration Tests**:
   - Test event timing fix (Created event is captured)
   - Test quit command responsiveness
   - Test stdin EOF handling

3. **Manual Tests**:
   ```bash
   q chat --ui-mode=structured "hello" | jq 'select(.event == "worker_created")'
   echo '{"command":"quit"}' | q chat --ui-mode=structured
   echo "hello" | q chat --ui-mode=structured  # Test EOF
   ```

### Code Organization

**Files to Modify**:

**Task 1.1**:
1. `crates/chat-cli/src/agent_env/events.rs` - Add `waiting_for_input` field
2. `crates/chat-cli/src/agent_env/agent_environment.rs` - Add job tracking
3. `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs` - Add `no_interactive` field
4. `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - Add `no_interactive` field
5. `crates/chat-cli/src/cli/chat/mod.rs` - Pass flag, register jobs
6. `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - Set `waiting_for_input` flag

**Task 1.6**:
1. `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - Add event handlers, fix input reading
2. `crates/chat-cli/src/cli/chat/mod.rs` - Fix event timing (manually emit Created event)

**Estimated Lines of Code**:
- Task 1.1: ~150 lines (including tests)
- Task 1.6: ~80 lines (including tests)

### Migration Path

**Task 1.1**:

No migration needed - this is a new feature. Existing behavior (interactive mode) remains unchanged.

**Task 1.6**:

No migration needed - additive changes only. Existing StructuredIO consumers will see new events but can ignore them.

### Documentation Updates

**Task 1.1**:
- Update `ChatArgs` documentation for `--no-interactive` flag
- Add examples to README
- Document `waiting_for_input` flag in `JobCompletionResult`

**Task 1.6**:
- Update StructuredIO documentation with new event types
- Add examples of JSON output format
- Document quit command behavior

### Future Enhancements

**Task 1.1**:
1. Support multiple initial prompts: `q chat --no-interactive "prompt1" "prompt2"`
2. Add exit codes for clean vs non-clean exits
3. Add timeout for non-interactive mode: `q chat --no-interactive --timeout=30s "prompt"`
4. Add progress indicator for long-running tasks

**Task 1.6**:
1. Add JobEvent::Started and JobEvent::Completed handlers
2. Add JobEvent::OutputChunk handler for granular output
3. Support filtering events by type: `q chat --ui-mode=structured --events=worker,job`
4. Add event timestamps in ISO 8601 format instead of `Instant`


## Testing Strategy

### Task 1.1: --no-interactive Support

#### Unit Tests

**Test: register_initial_job() in interactive mode**
```rust
#[tokio::test]
async fn test_register_initial_job_interactive_mode() {
    let agent_env = AgentEnvironment::new(
        session, event_bus, None, vec![], false  // no_interactive = false
    );
    
    let job_id = Uuid::new_v4();
    agent_env.register_initial_job(job_id);
    
    // Should NOT add to initial_job_ids in interactive mode
    assert!(agent_env.initial_job_ids.lock().unwrap().is_empty());
}
```

**Test: register_initial_job() in non-interactive mode**
```rust
#[tokio::test]
async fn test_register_initial_job_non_interactive_mode() {
    let agent_env = AgentEnvironment::new(
        session, event_bus, None, vec![], true  // no_interactive = true
    );
    
    let job_id = Uuid::new_v4();
    agent_env.register_initial_job(job_id);
    
    // Should add to initial_job_ids
    assert_eq!(agent_env.initial_job_ids.lock().unwrap().len(), 1);
    assert!(agent_env.initial_job_ids.lock().unwrap().contains(&job_id));
}
```

**Test: Job completion triggers shutdown**
```rust
#[tokio::test]
async fn test_job_completion_triggers_shutdown() {
    let agent_env = AgentEnvironment::new(
        session, event_bus, None, vec![], true
    );
    
    let job_id = Uuid::new_v4();
    agent_env.register_initial_job(job_id);
    
    // Spawn monitor
    let monitor_handle = agent_env.spawn_job_completion_monitor();
    
    // Publish JobEvent::Completed
    event_bus.publish(AgentEnvironmentEvent::Job(JobEvent::Completed {
        worker_id: worker.id,
        job_id,
        result: JobCompletionResult::Success {
            task_metadata: HashMap::new(),
            waiting_for_input: false,
        },
        timestamp: Instant::now(),
    }));
    
    // Wait for shutdown signal
    tokio::time::timeout(
        Duration::from_secs(1),
        agent_env.shutdown_signal.notified()
    ).await.expect("Shutdown signal should be triggered");
    
    monitor_handle.abort();
}
```

**Test: waiting_for_input flag warning**
```rust
#[tokio::test]
async fn test_waiting_for_input_warning() {
    // Capture stderr
    let mut stderr = Vec::new();
    
    // ... setup agent_env ...
    
    // Publish JobEvent::Completed with waiting_for_input = true
    event_bus.publish(AgentEnvironmentEvent::Job(JobEvent::Completed {
        worker_id: worker.id,
        job_id,
        result: JobCompletionResult::Success {
            task_metadata: HashMap::new(),
            waiting_for_input: true,  // Non-clean exit
        },
        timestamp: Instant::now(),
    }));
    
    // Check stderr contains warning
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("Warning: Task completed while waiting for user input"));
}
```

#### Integration Tests

**Test: TextUi non-interactive mode**
```rust
#[tokio::test]
async fn test_text_ui_non_interactive() {
    let text_ui = TextUi::new(
        session.clone(),
        worker_id,
        None,
        true,  // no_interactive = true
    ).unwrap();
    
    text_ui.start().await.unwrap();
    
    // Verify prompt loop was NOT spawned
    // (Check internal state or use timeout to verify no input is requested)
}
```

**Test: Full flow with non-interactive mode**
```rust
#[tokio::test]
async fn test_full_non_interactive_flow() {
    // Setup
    let event_bus = EventBus::default();
    let session = Arc::new(Session::new(event_bus.clone(), vec![model_provider]));
    let worker = session.build_worker("main".to_string());
    
    worker.context_container
        .conversation_history
        .lock()
        .unwrap()
        .push_input_message("Test prompt".to_string());
    
    let text_ui = TextUi::new(session.clone(), worker.id, None, true).unwrap();
    let agent_env = AgentEnvironment::new(
        session.clone(),
        event_bus.clone(),
        Some(Arc::new(text_ui)),
        vec![],
        true,
    );
    
    // Launch job
    let job = session.run_task__agent_loop(worker, AgentLoopInput {}).unwrap();
    agent_env.register_initial_job(job.id);
    
    // Run with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        agent_env.run()
    ).await;
    
    assert!(result.is_ok(), "AgentEnvironment should exit cleanly");
}
```

#### Manual Tests

```bash
# Test 1: TextUi non-interactive with input
q chat --no-interactive "What is 2+2?"
# Expected: Process prompt, display response, exit cleanly

# Test 2: StructuredIO non-interactive with input
q chat --no-interactive --ui-mode=structured "What is 2+2?"
# Expected: Output JSON events, exit cleanly

# Test 3: Non-interactive without input (should error)
q chat --no-interactive
# Expected: Error message "No input provided for non-interactive mode"

# Test 4: Interactive mode still works
q chat "What is 2+2?"
# Expected: Process prompt, display response, show prompt for next input

# Test 5: Non-clean exit (requires tool use scenario)
q chat --no-interactive "Use a tool that requires approval"
# Expected: Warning message about non-clean exit
```

### Task 1.6: StructuredIO Enhancements

#### Unit Tests

**Test: WorkerEvent::Created handler**
```rust
#[tokio::test]
async fn test_worker_created_event_handler() {
    let mut output = Vec::new();
    let structured_io = StructuredIO::new_with_writer(
        session.clone(),
        worker_id,
        false,
        Box::new(&mut output),
    ).unwrap();
    
    // Send WorkerEvent::Created
    structured_io.handle_event(AgentEnvironmentEvent::Worker(
        WorkerEvent::Created {
            worker_id,
            name: "main".to_string(),
            timestamp: Instant::now(),
        }
    )).await;
    
    // Parse output
    let output_str = String::from_utf8(output).unwrap();
    let json: serde_json::Value = serde_json::from_str(&output_str).unwrap();
    
    assert_eq!(json["event"], "worker_created");
    assert_eq!(json["name"], "main");
}
```

**Test: WorkerEvent::Deleted handler**
```rust
#[tokio::test]
async fn test_worker_deleted_event_handler() {
    let mut output = Vec::new();
    let structured_io = StructuredIO::new_with_writer(
        session.clone(),
        worker_id,
        false,
        Box::new(&mut output),
    ).unwrap();
    
    // Send WorkerEvent::Deleted
    structured_io.handle_event(AgentEnvironmentEvent::Worker(
        WorkerEvent::Deleted {
            worker_id,
            timestamp: Instant::now(),
        }
    )).await;
    
    // Parse output
    let output_str = String::from_utf8(output).unwrap();
    let json: serde_json::Value = serde_json::from_str(&output_str).unwrap();
    
    assert_eq!(json["event"], "worker_deleted");
}
```

**Test: Quit command parsing**
```rust
#[tokio::test]
async fn test_quit_command_parsing() {
    let structured_io = StructuredIO::new(session.clone(), worker_id, false).unwrap();
    
    // Simulate stdin with quit command
    let input = r#"{"command":"quit"}"#;
    
    // Parse command (internal method)
    let command = structured_io.parse_command(input).unwrap();
    
    assert!(matches!(command, PromptResult::Command(AgentEnvironmentCommand::Quit)));
}
```

#### Integration Tests

**Test: Event timing fix**
```rust
#[tokio::test]
async fn test_event_timing_worker_created() {
    let event_bus = EventBus::default();
    let session = Arc::new(Session::new(event_bus.clone(), vec![]));
    
    let mut output = Vec::new();
    let structured_io = StructuredIO::new_with_writer(
        session.clone(),
        Uuid::nil(),
        false,
        Box::new(&mut output),
    ).unwrap();
    
    // Create worker AFTER StructuredIO
    let worker = session.build_worker("main".to_string());
    
    // Manually emit Created event (as per design)
    structured_io.handle_event(AgentEnvironmentEvent::Worker(
        WorkerEvent::Created {
            worker_id: worker.id,
            name: "main".to_string(),
            timestamp: Instant::now(),
        }
    )).await;
    
    // Verify event was captured
    let output_str = String::from_utf8(output).unwrap();
    assert!(output_str.contains("worker_created"));
}
```

**Test: Quit command responsiveness**
```rust
#[tokio::test]
async fn test_quit_command_responsiveness() {
    let structured_io = StructuredIO::new(session.clone(), worker_id, false).unwrap();
    
    structured_io.start().await.unwrap();
    
    // Send quit command
    let quit_json = r#"{"command":"quit"}"#;
    // (Simulate stdin input)
    
    // Measure time to shutdown
    let start = Instant::now();
    
    // Wait for shutdown signal
    tokio::time::timeout(
        Duration::from_millis(100),
        structured_io.shutdown_signal.notified()
    ).await.expect("Quit should trigger shutdown quickly");
    
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(100), "Quit should be immediate");
}
```

**Test: Stdin EOF handling**
```rust
#[tokio::test]
async fn test_stdin_eof_handling() {
    // Create pipe with EOF
    let (mut reader, mut writer) = tokio::io::duplex(64);
    writer.write_all(b"hello\n").await.unwrap();
    drop(writer); // Close writer, causing EOF
    
    let structured_io = StructuredIO::new_with_stdin(
        session.clone(),
        worker_id,
        false,
        reader,
    ).unwrap();
    
    structured_io.start().await.unwrap();
    
    // Input reader should exit cleanly on EOF
    // (Verify by checking task handle or timeout)
}
```

#### Manual Tests

```bash
# Test 1: Worker Created event
q chat --ui-mode=structured "hello" | jq 'select(.event == "worker_created")'
# Expected: Output worker_created event with worker_id and name

# Test 2: Quit command responsiveness
echo '{"command":"quit"}' | q chat --ui-mode=structured
# Expected: Exit immediately without hanging

# Test 3: Event ordering
q chat --ui-mode=structured "hello" | jq -c '.event' | head -20
# Expected: Events in correct chronological order (worker_created first)

# Test 4: Stdin EOF handling
echo "hello" | q chat --ui-mode=structured
# Expected: Process prompt, reach EOF, continue until job completes, exit cleanly

# Test 5: Multiple events
q chat --ui-mode=structured "hello" | jq 'select(.event | startswith("worker"))'
# Expected: See worker_created and lifecycle_state events
```

### Acceptance Criteria

**Task 1.1**:
- ✅ `q chat --no-interactive "hello"` runs once and exits cleanly
- ✅ `q chat --no-interactive --ui-mode=structured "hello"` runs once and exits cleanly
- ✅ No input reading occurs in non-interactive mode
- ✅ No hanging processes or prompt loops
- ✅ Proper shutdown with all resources cleaned up
- ✅ Warning displayed for non-clean exits
- ✅ Error displayed when no input provided

**Task 1.6**:
- ✅ WorkerEvent::Created appears in JSON output
- ✅ WorkerEvent::Deleted appears in JSON output (when implemented)
- ✅ Quit command responds immediately
- ✅ No blocking on input reading when quit is issued
- ✅ All events appear in correct chronological order
- ✅ Initial worker creation event is captured


## References

### Related Documents

- **Scope**: `planning/mvp-small-wins/mvp-small-wins-0-scope.md`
- **Research**: `planning/mvp-small-wins/mvp-small-wins-1-research.md`
- **Architecture**: `codebase/agent-environment/README.md`
- **EventBus Design**: `planning/event-bus/event-bus-1-design.md`

### Key Files

**Task 1.1**:
- `crates/chat-cli/src/agent_env/events.rs` - Add UserInteractionRequired enum
- `crates/chat-cli/src/agent_env/session.rs` - Add has_active_jobs() method
- `crates/chat-cli/src/agent_env/agent_environment.rs` - Add job completion monitor
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs` - Add interactive flag, add set_main_worker_id()
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - Add interactive flag, remove main_worker_id
- `crates/chat-cli/src/cli/chat/mod.rs` - Reorder initialization, pass interactive flag
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - Set user_interaction_required field

**Task 1.6**:
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - Add event handlers, refactor input reading
- `crates/chat-cli/src/cli/chat/mod.rs` - Create StructuredIO before worker

### Design Decisions Summary

**Task 1.1**:
1. ✅ Add `UserInteractionRequired` enum to `JobCompletionResult::Success`
2. ✅ Add `Session::has_active_jobs()` method to check for active jobs
3. ✅ Spawn job completion monitor in AgentEnvironment (non-interactive mode only)
4. ✅ Monitor checks for JobEvent::Completed + no active jobs to trigger shutdown
5. ✅ Conditionally spawn input reading tasks in UIs based on `interactive` flag
6. ✅ Use `interactive` flag internally (inverted from `--no-interactive`)
7. ✅ Create worker AFTER UI and AgentEnvironment
8. ✅ Start monitoring BEFORE spawning jobs
9. ✅ Error on non-clean exit or no jobs spawned

**Task 1.6**:
1. ✅ Remove main_worker_id from StructuredIO - output events for ALL workers
2. ✅ Add `WorkerEvent::Created`, `WorkerEvent::Deleted`, `JobEvent::Started`, `JobEvent::Completed` handlers
3. ✅ Create StructuredIO before worker to capture all events (Option A)
4. ✅ Use reader task pattern (spawn + abort) to make quit command responsive
5. ✅ Support worker_id in prompt command JSON: `{"worker_id":"...","prompt":"..."}`

### Implementation Order

1. **Task 1.1 first** (1-2 hours):
   - Modify `JobCompletionResult` in `events.rs`
   - Add job tracking to `AgentEnvironment`
   - Update UI constructors (TextUi, StructuredIO)
   - Update `ChatArgs::execute()`
   - Update `AgentLoop` to set `waiting_for_input` flag
   - Test

2. **Task 1.6 second** (2-3 hours):
   - Add event handlers to StructuredIO
   - Fix event timing in `ChatArgs::execute()`
   - Refactor input reading with `tokio::select!`
   - Test

**Total Estimated Effort**: 3-5 hours

---

## Document Changelog

- **2025-10-11 12:41**: Design review and updates
  - Task 1.1: Changed `waiting_for_input` bool to `UserInteractionRequired` enum
  - Task 1.1: Removed `register_initial_job()`, use `Session::has_active_jobs()` instead
  - Task 1.1: Flipped `no_interactive` to `interactive` internally
  - Task 1.1: Reordered initialization - create worker after UI and AgentEnvironment
  - Task 1.1: Start monitoring before spawning jobs
  - Task 1.6: Removed main_worker_id filtering - output ALL workers
  - Task 1.6: Added JobEvent::Started and JobEvent::Completed handlers
  - Task 1.6: Changed to Option A for event timing (create UI before worker)
  - Task 1.6: Changed quit command solution to reader task pattern (spawn + abort)
  - Task 1.6: Added worker_id support in prompt command JSON
- **2025-10-11**: Initial design document created
  - Task 1.1: --no-interactive support design
  - Task 1.6: StructuredIO enhancements design
  - Implementation considerations and testing strategy

