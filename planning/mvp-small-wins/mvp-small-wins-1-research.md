# MVP Small Wins - Research and Analysis

## Overview

This document contains research findings for implementing Task 1.1 (--no-interactive support) and Task 1.6 (StructuredIO enhancements) in the agent_env architecture.

---

## Task 1.1: --no-interactive Support

### Current Implementation Analysis

#### ChatArgs Structure
**Location:** `crates/chat-cli/src/cli/chat/mod.rs`

```rust
pub struct ChatArgs {
    #[arg(long, alias = "non-interactive")]
    pub no_interactive: bool,
    
    #[arg(long = "ui-mode", value_enum)]
    pub ui_mode: Option<UiMode>,
    
    pub input: Option<String>,
    // ... other fields
}
```

**Current behavior:**
- `--no-interactive` flag exists but is only used to default `ui_mode` to `UiMode::None`
- Flag is NOT passed to UI constructors
- UIs spawn input reading tasks unconditionally

#### TextUi Implementation
**Location:** `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Key methods:**
- `new()` - Constructor, does not accept no_interactive parameter
- `start()` - Unconditionally calls `spawn_prompt_loop()`
- `spawn_prompt_loop()` - Spawns tokio task that loops forever reading input

**Input reading mechanism:**
```rust
fn spawn_prompt_loop(&self) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = prompt_ready.notified() => {
                    // Read input from InputHandler
                    let input = handler.read_line("You").await;
                    // Process command
                }
                _ = shutdown.notified() => break,
            }
        }
    })
}
```

**Issue:** Loop continues indefinitely, waiting for user input even in non-interactive mode.

#### StructuredIO Implementation
**Location:** `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**Key methods:**
- `new()` - Constructor, does not accept no_interactive parameter
- `start()` - Unconditionally calls `spawn_input_reader()`
- `spawn_input_reader()` - Spawns tokio task that reads stdin continuously

**Input reading mechanism:**
```rust
fn spawn_input_reader(&self) -> JoinHandle<()> {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // Process line as command or prompt
        }
    })
}
```

**Issue:** Continuously reads stdin, blocking on `lines.next_line().await` even when no more input is expected.

#### AgentEnvironment Implementation
**Location:** `crates/chat-cli/src/agent_env/agent_environment.rs`

**Main execution loop:**
```rust
pub async fn run(&self) -> Result<()> {
    // Start UI
    ui.start().await?;
    
    // Get command receiver
    let mut cmd_receiver = ui.command_receiver();
    
    // Main loop
    loop {
        tokio::select! {
            Some(result) = cmd_receiver.recv() => {
                match result {
                    PromptResult::Command(cmd) => {
                        self.handle_command(cmd).await?;
                    }
                    PromptResult::Shutdown => break,
                }
            }
            _ = self.shutdown_signal.notified() => break,
        }
    }
    
    // Cleanup
    self.session.cancel_all_jobs();
    Ok(())
}
```

**Issue:** No mechanism to track job completion and automatically exit when initial jobs complete.

#### Event System
**Location:** `crates/chat-cli/src/agent_env/events.rs`

**Relevant events:**
- `JobEvent::Started { worker_id, job_id, task_type, timestamp }`
- `JobEvent::Completed { worker_id, job_id, result, timestamp }`
- `WorkerEvent::LifecycleStateChanged { worker_id, old_state, new_state, timestamp }`

**Job completion results:**
```rust
pub enum JobCompletionResult {
    Success { task_metadata: HashMap<String, serde_json::Value> },
    Cancelled,
    Failed { error: String },
}
```

### Implementation Requirements

#### 1. Pass no_interactive flag to UI constructors

**TextUi::new() signature change:**
```rust
pub fn new(
    session: Arc<Session>,
    main_worker_id: Uuid,
    history_path: Option<PathBuf>,
    no_interactive: bool,  // NEW
) -> Result<Self, eyre::Error>
```

**StructuredIO::new() signature change:**
```rust
pub fn new(
    session: Arc<Session>,
    main_worker_id: Uuid,
    no_interactive: bool,  // NEW
) -> Result<Self>
```

#### 2. Conditionally spawn input reading tasks

**TextUi::start() modification:**
```rust
async fn start(&self) -> Result<()> {
    if !self.no_interactive {
        self.spawn_prompt_loop();
    }
    
    // Check if worker is idle and signal prompt_ready
    // (existing logic)
    Ok(())
}
```

**StructuredIO::start() modification:**
```rust
async fn start(&self) -> Result<()> {
    if !self.no_interactive {
        self.spawn_input_reader();
    }
    Ok(())
}
```

#### 3. Add job tracking to AgentEnvironment

**AgentEnvironment needs:**
- Track initial job IDs created before `run()` is called
- Subscribe to `JobEvent::Completed` events
- When all initial jobs complete AND no new jobs exist, trigger shutdown

**Proposed approach:**

**Option A: Track initial jobs in AgentEnvironment**
```rust
pub struct AgentEnvironment {
    // ... existing fields
    initial_job_ids: Arc<Mutex<HashSet<Uuid>>>,
    no_interactive: bool,
}

impl AgentEnvironment {
    pub fn register_initial_job(&self, job_id: Uuid) {
        if self.no_interactive {
            self.initial_job_ids.lock().unwrap().insert(job_id);
        }
    }
    
    async fn handle_job_completed(&self, job_id: Uuid) {
        if !self.no_interactive {
            return;
        }
        
        let mut initial_jobs = self.initial_job_ids.lock().unwrap();
        initial_jobs.remove(&job_id);
        
        if initial_jobs.is_empty() {
            // All initial jobs complete - trigger shutdown
            self.shutdown_signal.notify_waiters();
        }
    }
}
```

**Option B: Track active jobs in Session**
- Session already tracks jobs in `jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>`
- AgentEnvironment could query Session for active job count
- When count reaches 0 in non-interactive mode, trigger shutdown

**Recommendation:** Option A is cleaner - AgentEnvironment owns the shutdown logic and doesn't need to poll Session.

#### 4. Update ChatArgs::execute()

**Modifications needed:**
```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // ... create EventBus, Session, Worker ...
        
        // Pass no_interactive to UI constructors
        let main_ui = match ui_mode {
            UiMode::Text => {
                let text_ui = TextUi::new(
                    session.clone(),
                    main_worker_id,
                    history_path,
                    self.no_interactive,  // NEW
                )?;
                Some(Arc::new(text_ui))
            }
            UiMode::Structured => {
                let structured_io = StructuredIO::new(
                    session.clone(),
                    main_worker_id,
                    self.no_interactive,  // NEW
                )?;
                Some(Arc::new(structured_io))
            }
            UiMode::None => None,
        };
        
        // Create AgentEnvironment with no_interactive flag
        let agent_env = AgentEnvironment::new(
            session.clone(),
            event_bus.clone(),
            main_ui,
            vec![],
            self.no_interactive,  // NEW
        );
        
        // Launch initial job if input provided
        if let Some(_) = &self.input {
            let job = session.run_task__agent_loop(main_worker, AgentLoopInput {})?;
            agent_env.register_initial_job(job.id);  // NEW
        }
        
        // Run
        agent_env.run().await?;
        
        Ok(ExitCode::SUCCESS)
    }
}
```

### Potential Pitfalls

#### 1. Race condition: Job completes before AgentEnvironment starts listening

**Scenario:**
1. ChatArgs::execute() launches initial job
2. Job completes immediately (e.g., error)
3. AgentEnvironment.run() hasn't started yet
4. JobEvent::Completed is published but not handled
5. AgentEnvironment never exits

**Mitigation:**
- Register initial jobs BEFORE calling agent_env.run()
- AgentEnvironment subscribes to EventBus in constructor
- Buffer events until run() starts processing

#### 2. Multiple initial jobs

**Scenario:**
- Future enhancement: support multiple initial prompts
- Need to track all initial job IDs
- Only exit when ALL complete

**Mitigation:**
- Use `HashSet<Uuid>` for initial_job_ids
- Remove each job as it completes
- Exit when set is empty

#### 3. Job spawns child jobs

**Scenario:**
- Initial job spawns additional jobs (e.g., tool use spawns sub-tasks)
- Should we wait for child jobs too?

**Current scope:**
- Only track explicitly registered initial jobs
- Child jobs are part of the initial job's lifecycle
- When initial job completes, all its work is done

#### 4. No initial input provided

**Scenario:**
```bash
q chat --no-interactive
```

**Expected behavior:**
- Should error or exit immediately
- No job to track

**Mitigation:**
- Check if initial_job_ids is empty in run()
- If empty and no_interactive, exit immediately

---

## Task 1.6: StructuredIO Enhancements

### Current Implementation Analysis

#### Event Handling
**Location:** `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**Currently handled events:**
```rust
async fn handle_event(&self, event: AgentEnvironmentEvent) {
    // Filter to main worker
    if let Some(wid) = event.worker_id() {
        if wid != self.main_worker_id {
            return;
        }
    }
    
    match event {
        AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { .. }) => {
            // Output JSON
        }
        AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived { .. }) => {
            // Output JSON
        }
        AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ToolUseRequestReceived { .. }) => {
            // Output JSON
        }
        _ => {}  // All other events ignored
    }
}
```

**Missing events:**
- `WorkerEvent::Created` - Not handled
- `WorkerEvent::Deleted` - Not handled
- `JobEvent::Started` - Not handled
- `JobEvent::Completed` - Not handled
- `JobEvent::OutputChunk` - Not handled (but AgentLoopEvent::ResponseReceived is handled)

#### Input Reading Mechanism

**Current implementation:**
```rust
fn spawn_input_reader(&self) -> JoinHandle<()> {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // Parse and send command
            if cmd_sender.send(result).await.is_err() {
                break;
            }
        }
    })
}
```

**Potential issue:** `lines.next_line().await` blocks until input is available. If quit command is sent via EventBus or shutdown signal, the task may not respond immediately.

**Testing needed:**
```bash
echo '{"command":"quit"}' | q chat --ui-mode=structured
```

Does this exit immediately or wait for stdin to close?

#### Event Timing

**Current flow:**
1. ChatArgs::execute() creates EventBus
2. ChatArgs::execute() creates Session
3. ChatArgs::execute() calls session.build_worker() → publishes WorkerEvent::Created
4. ChatArgs::execute() creates StructuredIO
5. StructuredIO constructor subscribes to EventBus
6. ChatArgs::execute() creates AgentEnvironment
7. AgentEnvironment.run() calls ui.start()

**Question:** Does StructuredIO receive the initial WorkerEvent::Created?

**Answer:** NO - StructuredIO subscribes AFTER worker is created. The Created event is already published and missed.

### Implementation Requirements

#### 1. Add WorkerEvent::Created handler

```rust
async fn handle_event(&self, event: AgentEnvironmentEvent) {
    // ... existing filter logic ...
    
    match event {
        AgentEnvironmentEvent::Worker(WorkerEvent::Created { worker_id, name, .. }) => {
            let json = json!({
                "event": "worker_created",
                "worker_id": worker_id,
                "name": name,
            });
            
            let mut writer = self.output_writer.lock().await;
            writeln!(writer, "{}", json).unwrap();
            writer.flush().unwrap();
        }
        // ... existing handlers ...
    }
}
```

#### 2. Add WorkerEvent::Deleted handler

```rust
AgentEnvironmentEvent::Worker(WorkerEvent::Deleted { worker_id, .. }) => {
    let json = json!({
        "event": "worker_deleted",
        "worker_id": worker_id,
    });
    
    let mut writer = self.output_writer.lock().await;
    writeln!(writer, "{}", json).unwrap();
    writer.flush().unwrap();
}
```

#### 3. Fix event timing issue

**Problem:** StructuredIO subscribes to EventBus AFTER worker is created, missing the initial Created event.

**Solution options:**

**Option A: Create StructuredIO before worker**
```rust
// In ChatArgs::execute()
let structured_io = StructuredIO::new(session.clone(), Uuid::nil())?;
let main_worker = session.build_worker("main".to_string());
structured_io.set_main_worker_id(main_worker.id);  // Update worker ID
```

**Option B: Subscribe to EventBus before creating UI**
```rust
// In ChatArgs::execute()
let event_bus = EventBus::default();
let session = Arc::new(Session::new(event_bus.clone(), model_providers));

// Subscribe BEFORE creating worker
let mut event_receiver = event_bus.subscribe();

let main_worker = session.build_worker("main".to_string());

// Now create UI with pre-subscribed receiver
let structured_io = StructuredIO::with_receiver(
    session.clone(),
    main_worker.id,
    event_receiver,
)?;
```

**Option C: Manually emit Created event after UI is ready**
```rust
// In ChatArgs::execute()
let main_worker = session.build_worker("main".to_string());
let structured_io = StructuredIO::new(session.clone(), main_worker.id)?;

// Manually send Created event to UI
structured_io.handle_event(AgentEnvironmentEvent::Worker(
    WorkerEvent::Created {
        worker_id: main_worker.id,
        name: "main".to_string(),
        timestamp: Instant::now(),
    }
)).await;
```

**Recommendation:** Option A is cleanest - create UI first, then worker. UI subscribes to EventBus in constructor and receives all subsequent events.

**Challenge with Option A:** StructuredIO needs worker_id in constructor for event filtering. Can't filter without knowing the ID.

**Revised Option A:**
```rust
// StructuredIO filters events AFTER receiving them
// Constructor doesn't need worker_id upfront
let structured_io = StructuredIO::new(session.clone())?;
let main_worker = session.build_worker("main".to_string());
structured_io.set_main_worker_id(main_worker.id);
```

#### 4. Investigate quit command blocking

**Test case:**
```bash
echo '{"command":"quit"}' | q chat --ui-mode=structured "hello"
```

**Expected behavior:**
- Process "hello" prompt
- Read quit command from stdin
- Exit immediately

**Potential issue:**
- If `lines.next_line().await` blocks waiting for more input after quit is processed
- Quit command is sent via channel, but input reader task continues blocking

**Investigation needed:**
1. Test current behavior
2. If blocking occurs, refactor input reading

**Refactoring approach (if needed):**

**Option A: Use tokio::select! with shutdown signal**
```rust
fn spawn_input_reader(&self) -> JoinHandle<()> {
    let shutdown = self.shutdown_signal.clone();
    
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        loop {
            tokio::select! {
                result = lines.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            // Process line
                            if cmd_sender.send(result).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,  // EOF
                        Err(e) => {
                            eprintln!("Error reading stdin: {}", e);
                            break;
                        }
                    }
                }
                _ = shutdown.notified() => {
                    break;
                }
            }
        }
    })
}
```

**Option B: Close stdin on quit**
- When quit command is processed, close stdin
- Input reader task will receive EOF and exit

**Recommendation:** Test first. If blocking is confirmed, use Option A (tokio::select! with shutdown signal).

### Potential Pitfalls

#### 1. Event ordering

**Scenario:**
- Multiple events published in quick succession
- EventBus uses broadcast channel with buffer
- If buffer overflows, events may be lagged or dropped

**Mitigation:**
- EventBus already handles lagged events (logs warning)
- StructuredIO should handle RecvError::Lagged gracefully
- Consider increasing buffer size if needed

#### 2. JSON output interleaving

**Scenario:**
- Multiple events arrive simultaneously
- Multiple threads try to write to stdout
- JSON lines may interleave

**Current protection:**
- `output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>`
- Each write acquires lock, preventing interleaving

**Verification:** Lock is held for entire `writeln!()` + `flush()` sequence.

#### 3. Worker ID filtering

**Scenario:**
- StructuredIO filters events by main_worker_id
- If worker_id is not set correctly, events may be missed

**Current implementation:**
```rust
if let Some(wid) = event.worker_id() {
    if wid != self.main_worker_id {
        return;  // Filter out
    }
}
```

**Issue:** If main_worker_id is Uuid::nil() or incorrect, all events are filtered.

**Mitigation:** Ensure main_worker_id is set correctly in constructor or via setter.

#### 4. Stdin EOF handling

**Scenario:**
```bash
echo "hello" | q chat --ui-mode=structured
```

**Expected behavior:**
- Process "hello"
- Stdin reaches EOF
- Input reader task exits
- AgentEnvironment continues running until job completes

**Current behavior:**
- `lines.next_line().await` returns `Ok(None)` on EOF
- Loop exits: `while let Ok(Some(line)) = lines.next_line().await`
- Input reader task terminates
- AgentEnvironment continues (good!)

**Verification:** This should work correctly. Test to confirm.

---

## Code Elements Summary

### Files to Modify

#### Task 1.1: --no-interactive Support
1. `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`
   - Add `no_interactive: bool` field
   - Update `new()` signature
   - Conditionally spawn prompt loop in `start()`

2. `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
   - Add `no_interactive: bool` field
   - Update `new()` signature
   - Conditionally spawn input reader in `start()`

3. `crates/chat-cli/src/agent_env/agent_environment.rs`
   - Add `initial_job_ids: Arc<Mutex<HashSet<Uuid>>>` field
   - Add `no_interactive: bool` field
   - Add `register_initial_job()` method
   - Subscribe to `JobEvent::Completed` in event multicast
   - Trigger shutdown when all initial jobs complete

4. `crates/chat-cli/src/cli/chat/mod.rs`
   - Pass `no_interactive` to UI constructors
   - Pass `no_interactive` to AgentEnvironment constructor
   - Register initial jobs after launching

#### Task 1.6: StructuredIO Enhancements
1. `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
   - Add `WorkerEvent::Created` handler
   - Add `WorkerEvent::Deleted` handler
   - Investigate and fix quit command blocking (if needed)

2. `crates/chat-cli/src/cli/chat/mod.rs`
   - Fix event timing: create UI before worker (if needed)
   - Or manually emit Created event after UI is ready

### Critical APIs

#### EventBus
- `subscribe() -> broadcast::Receiver<AgentEnvironmentEvent>`
- `publish(event: AgentEnvironmentEvent)`

#### Session
- `build_worker(name: String) -> Arc<Worker>` - Publishes WorkerEvent::Created
- `run_task__agent_loop(worker, input) -> Result<Arc<WorkerJob>>` - Publishes JobEvent::Started
- `get_worker(id: Uuid) -> Option<Arc<Worker>>`
- `cancel_all_jobs()`

#### AgentEnvironment
- `new(session, event_bus, main_ui, headless_uis) -> Self`
- `run() -> Result<()>` - Main execution loop
- `shutdown()` - Trigger shutdown signal

#### UserInterface trait
- `start() -> Result<()>` - Start UI (spawn tasks)
- `command_receiver() -> mpsc::Receiver<PromptResult>` - Get command channel
- `handle_event(event: AgentEnvironmentEvent)` - Handle event from EventBus

### Event Flow

```
Session::build_worker()
  └─> Publish WorkerEvent::Created
        └─> EventBus
              └─> AgentEnvironment (multicast)
                    └─> StructuredIO::handle_event()

Session::run_task__agent_loop()
  ├─> Set worker to Busy
  │     └─> Publish WorkerEvent::LifecycleStateChanged
  └─> Spawn job
        └─> Publish JobEvent::Started

Job completes
  ├─> Publish JobEvent::Completed
  └─> Set worker to Idle/IdleFailed
        └─> Publish WorkerEvent::LifecycleStateChanged
```

---

## Testing Strategy

### Task 1.1 Tests

#### Test 1: TextUi non-interactive with input
```bash
q chat --no-interactive "What is 2+2?"
```
**Expected:** Process prompt, display response, exit cleanly.

#### Test 2: StructuredIO non-interactive with input
```bash
q chat --no-interactive --ui-mode=structured "What is 2+2?"
```
**Expected:** Output JSON events, exit cleanly.

#### Test 3: Non-interactive without input
```bash
q chat --no-interactive
```
**Expected:** Error or exit immediately (no job to process).

#### Test 4: Interactive mode still works
```bash
q chat "What is 2+2?"
```
**Expected:** Process prompt, display response, show prompt for next input.

#### Test 5: Multiple initial jobs (future)
```bash
q chat --no-interactive "First prompt" "Second prompt"
```
**Expected:** Process both prompts, exit after both complete.

### Task 1.6 Tests

#### Test 1: Worker Created event
```bash
q chat --ui-mode=structured "hello" | jq 'select(.event == "worker_created")'
```
**Expected:** Output worker_created event with worker_id and name.

#### Test 2: Worker Deleted event (future)
```bash
# When worker deletion is implemented
q chat --ui-mode=structured "/delete-worker main" | jq 'select(.event == "worker_deleted")'
```
**Expected:** Output worker_deleted event.

#### Test 3: Quit command responsiveness
```bash
echo '{"command":"quit"}' | q chat --ui-mode=structured
```
**Expected:** Exit immediately without hanging.

#### Test 4: Event ordering
```bash
q chat --ui-mode=structured "hello" | jq -c '.event' | head -20
```
**Expected:** Events in correct chronological order (worker_created, lifecycle_state, etc.).

#### Test 5: Stdin EOF handling
```bash
echo "hello" | q chat --ui-mode=structured
```
**Expected:** Process prompt, reach EOF, continue until job completes, exit cleanly.

---

## Recommendations

### Task 1.1 Implementation Order
1. Add `no_interactive` parameter to TextUi and StructuredIO constructors
2. Conditionally spawn input reading tasks in `start()` methods
3. Add job tracking to AgentEnvironment
4. Update ChatArgs::execute() to pass flag and register initial jobs
5. Test all scenarios

### Task 1.6 Implementation Order
1. Add WorkerEvent::Created and Deleted handlers to StructuredIO
2. Test event output
3. Investigate quit command blocking
4. If blocking confirmed, refactor input reading with tokio::select!
5. Fix event timing issue (create UI before worker or manual emit)
6. Test all scenarios

### Risk Mitigation
- **Race conditions:** Register initial jobs before starting AgentEnvironment
- **Event timing:** Subscribe to EventBus before creating workers
- **Input blocking:** Use tokio::select! with shutdown signal
- **Event ordering:** Rely on EventBus broadcast channel ordering guarantees

---

## Open Questions

1. **Task 1.1:** Should `--no-interactive` without initial input error or exit silently?
   - **Recommendation:** Error with helpful message

2. **Task 1.1:** Should we support multiple initial prompts in the future?
   - **Recommendation:** Design with extensibility in mind (use HashSet for job IDs)

3. **Task 1.6:** Does quit command actually block on stdin?
   - **Action:** Test and confirm before refactoring

4. **Task 1.6:** Should we add handlers for JobEvent::Started and JobEvent::Completed?
   - **Recommendation:** Yes, for completeness. Users may want to track job lifecycle.

5. **Task 1.6:** Should we output JobEvent::OutputChunk in addition to AgentLoopEvent::ResponseReceived?
   - **Recommendation:** No, ResponseReceived is sufficient for now. OutputChunk is more granular.

---

## Conclusion

Both tasks are well-scoped and implementable with the current architecture. Key challenges:

**Task 1.1:**
- Job tracking in AgentEnvironment requires careful event handling
- Race conditions must be avoided by proper initialization order

**Task 1.6:**
- Event timing issue requires creating UI before worker or manual event emission
- Quit command blocking needs investigation and potential refactoring

Estimated effort remains accurate:
- Task 1.1: 1-2 hours
- Task 1.6: 2-3 hours (includes investigation)

Total: 3-5 hours for both tasks.
