# MVP Small Wins - Implementation Plan

## Document Status

**Status**: Ready for Implementation  
**Created**: 2025-10-11  
**Design Reference**: `mvp-small-wins-2-design.md`

## Overview

This plan provides step-by-step implementation tasks for two foundational improvements:
- **Task 1.1**: --no-interactive support for single-execution mode
- **Task 1.6**: StructuredIO enhancements for complete event coverage and responsive quit command

**Implementation Order**: Task 1.1 first, then Task 1.6 (minimizes merge conflicts)

**Estimated Total Effort**: 3-5 hours

---

## Phase 1: Task 1.1 - Core Infrastructure (Events & Session)

### 1.1 Add UserInteractionRequired Enum (Design Doc §2.1.1)

- [x] **Add UserInteractionRequired enum to events.rs** (Design Doc §2.1.1)
  - Create enum with variants: `None`, `ToolApproval`
  - Add to `crates/chat-cli/src/agent_env/events.rs`
  - Derive `Debug, Clone, Copy, PartialEq, Eq`
  - Add documentation explaining each variant

- [x] **Update JobCompletionResult::Success variant** (Design Doc §2.1.1)
  - Add `user_interaction_required: UserInteractionRequired` field to Success variant
  - Update all existing code that constructs `JobCompletionResult::Success` to include the new field
  - Set to `UserInteractionRequired::None` for existing usages (default behavior)

### 1.2 Add Session Active Jobs Query (Design Doc §2.1.2)

- [x] **Implement Session::has_active_jobs() method** (Design Doc §2.1.2)
  - Add public method to `crates/chat-cli/src/agent_env/session.rs`
  - Lock jobs mutex and iterate to check `job.is_active()`
  - Return true if any job is active, false otherwise
  - Add documentation explaining the method checks active state, not Vec emptiness

---

## Phase 2: Task 1.1 - AgentEnvironment Job Monitoring (Design Doc §2.1.3)

### 2.1 Add Interactive Flag to AgentEnvironment

- [x] **Add interactive field to AgentEnvironment struct** (Design Doc §2.1.3)
  - Add `interactive: bool` field to struct in `crates/chat-cli/src/agent_env/agent_environment.rs`
  - Update `new()` constructor to accept `interactive: bool` parameter
  - Store the parameter in the struct

### 2.2 Implement Job Completion Monitor

- [x] **Create spawn_job_completion_monitor() method** (Design Doc §2.1.3)
  - Add method to AgentEnvironment that returns `JoinHandle<()>`
  - Early return if `interactive` is true (only monitor in non-interactive mode)
  - Subscribe to EventBus to receive events
  - Spawn tokio task that loops on event receiver

- [x] **Handle JobEvent::Completed in monitor** (Design Doc §2.1.3)
  - Match on `JobEvent::Completed` events
  - Call `session.has_active_jobs()` to check for remaining jobs
  - If no active jobs remain, check `user_interaction_required` field
  - Print error message if `UserInteractionRequired::ToolApproval` (non-clean exit)
  - Print error message if `JobCompletionResult::Failed` or `Cancelled`
  - Trigger shutdown by calling `shutdown_signal.notify_waiters()`
  - Break from loop after shutdown

- [x] **Call spawn_job_completion_monitor() in AgentEnvironment::run()** (Design Doc §2.1.3)
  - Call method BEFORE any jobs are spawned (in ChatArgs::execute)
  - Store JoinHandle for cleanup (optional)

---

## Phase 3: Task 1.1 - UI Modifications

### 3.1 Update TextUi for Interactive Flag (Design Doc §2.1.4)

- [x] **Add interactive field to TextUi struct** (Design Doc §2.1.4)
  - Add `interactive: bool` field in `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`
  - Update `new()` signature to accept `interactive: bool` parameter
  - Store parameter in struct

- [x] **Conditionally spawn prompt loop in TextUi::start()** (Design Doc §2.1.4)
  - Wrap `self.spawn_prompt_loop()` call in `if self.interactive { ... }`
  - Keep existing worker idle check and prompt_ready signal logic
  - Ensure start() still returns Ok(()) in non-interactive mode

### 3.2 Update StructuredIO for Interactive Flag (Design Doc §2.1.5)

- [x] **Add interactive field to StructuredIO struct** (Design Doc §2.1.5)
  - Add `interactive: bool` field in `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`
  - Update `new()` signature to accept `interactive: bool` parameter
  - Store parameter in struct

- [x] **Conditionally spawn input reader in StructuredIO::start()** (Design Doc §2.1.5)
  - Wrap `self.spawn_input_reader()` call in `if self.interactive { ... }`
  - Ensure start() still returns Ok(()) in non-interactive mode

---

## Phase 4: Task 1.1 - ChatArgs Integration (Design Doc §2.1.6)

### 4.1 Update ChatArgs::execute() Initialization Order

- [x] **Invert no_interactive flag** (Design Doc §2.1.6)
  - Add `let interactive = !self.no_interactive;` at start of execute() method
  - Use `interactive` variable throughout the method

- [x] **Pass interactive flag to UI constructors** (Design Doc §2.1.6)
  - Update TextUi::new() call to pass `interactive` parameter
  - Update StructuredIO::new() call to pass `interactive` parameter
  - Update any other UI constructors if they exist

- [x] **Pass interactive flag to AgentEnvironment::new()** (Design Doc §2.1.6)
  - Add `interactive` parameter to AgentEnvironment::new() call
  - Ensure parameter is passed correctly

### 4.2 Add Job Monitoring Startup

- [x] **Call spawn_job_completion_monitor() before spawning jobs** (Design Doc §2.1.6)
  - Add conditional: `if !interactive { agent_env.spawn_job_completion_monitor(); }`
  - Place AFTER AgentEnvironment creation but BEFORE any job spawning
  - This ensures monitor is ready before jobs complete

### 4.3 Add Error Handling for Non-Interactive Mode

- [x] **Error if no input provided in non-interactive mode** (Design Doc §2.1.6)
  - After checking `self.input`, add: `if self.input.is_none() && !interactive { return Err(...); }`
  - Error message: "No input provided for non-interactive mode"

- [x] **Error if no jobs spawned in non-interactive mode** (Design Doc §2.1.6)
  - After attempting to spawn jobs, check: `if !interactive && !session.has_active_jobs() { return Err(...); }`
  - Error message: "No jobs spawned in non-interactive mode"

---

## Phase 5: Task 1.1 - AgentLoop Update (Design Doc §2.1.6)

### 5.1 Set user_interaction_required in AgentLoop

- [x] **Update AgentLoop completion to set user_interaction_required** (Design Doc §2.1.1)
  - In `crates/chat-cli/src/agent_env/session.rs` (not agent_loop.rs)
  - When completing with normal response, set `user_interaction_required: UserInteractionRequired::None`
  - When completing while waiting for tool approval, set `user_interaction_required: UserInteractionRequired::ToolApproval`
  - Check task_metadata for "agent_loop_completion_state" to determine which case applies
  - Ensure field is set in all Success result paths

---

## Phase 6: Task 1.6 - StructuredIO Event Handlers (Design Doc §3.1)

### 6.1 Remove main_worker_id Filtering

- [x] **Remove main_worker_id field from StructuredIO** (Design Doc §3.1)
  - Remove `main_worker_id: Uuid` field from struct
  - Remove parameter from `new()` signature
  - Remove all filtering logic in `handle_event()` that checks worker_id

### 6.2 Add WorkerEvent Handlers

- [x] **Add WorkerEvent::Created handler** (Design Doc §3.1)
  - Add match arm for `AgentEnvironmentEvent::Worker(WorkerEvent::Created { ... })`
  - Create JSON object with fields: event="worker_created", worker_id, name, timestamp
  - Lock output_writer, write JSON line, flush

- [x] **Add WorkerEvent::Deleted handler** (Design Doc §3.1)
  - Add match arm for `AgentEnvironmentEvent::Worker(WorkerEvent::Deleted { ... })`
  - Create JSON object with fields: event="worker_deleted", worker_id, timestamp
  - Lock output_writer, write JSON line, flush

### 6.3 Add JobEvent Handlers

- [x] **Add JobEvent::Started handler** (Design Doc §3.1)
  - Add match arm for `AgentEnvironmentEvent::Job(JobEvent::Started { ... })`
  - Create JSON object with fields: event="job_started", worker_id, job_id, task_type, timestamp
  - Lock output_writer, write JSON line, flush

- [x] **Add JobEvent::Completed handler** (Design Doc §3.1)
  - Add match arm for `AgentEnvironmentEvent::Job(JobEvent::Completed { ... })`
  - Map result to string: Success→"success", Cancelled→"cancelled", Failed→"failed"
  - Create JSON object with fields: event="job_completed", worker_id, job_id, result, timestamp
  - Lock output_writer, write JSON line, flush

---

## Phase 7: Task 1.6 - Event Timing Fix (Design Doc §3.2)

### 7.1 Reorder Initialization in ChatArgs::execute()

- [x] **Create StructuredIO before worker** (Design Doc §3.2)
  - Move StructuredIO::new() call to BEFORE session.build_worker() call
  - StructuredIO subscribes to EventBus in constructor
  - Worker creation publishes WorkerEvent::Created
  - StructuredIO receives the event automatically

- [x] **Update TextUi initialization order** (Design Doc §3.2)
  - TextUi currently needs worker_id in constructor
  - Create TextUi with placeholder Uuid::nil() before worker creation
  - Add `set_main_worker_id()` method to TextUi
  - Call method after worker is created to update the ID

---

## Phase 8: Task 1.6 - Quit Command Fix (Design Doc §3.3)

### 8.1 Refactor spawn_input_reader() with Reader Task Pattern

- [x] **Create channel for line communication** (Design Doc §3.3)
  - Create `mpsc::channel::<String>(10)` for sending lines from reader to processor
  - Split into `line_tx` (sender) and `line_rx` (receiver)

- [x] **Spawn dedicated stdin reader task** (Design Doc §3.3)
  - Spawn tokio task that creates BufReader from stdin
  - Loop on `lines.next_line().await` (this task will block)
  - Send each line to `line_tx` channel
  - Break on EOF or send error

- [x] **Implement processor loop with tokio::select!** (Design Doc §3.3)
  - Main loop uses `tokio::select!` on two branches
  - Branch 1: `Some(line) = line_rx.recv()` - process incoming lines
  - Branch 2: `_ = shutdown.notified()` - handle shutdown signal
  - Parse JSON and handle commands in Branch 1
  - Break from loop in Branch 2

- [x] **Abort reader task on shutdown** (Design Doc §3.3)
  - Store reader task JoinHandle
  - Call `reader_task.abort()` when processor loop exits
  - Await the handle and ignore JoinError (expected from abort)

### 8.2 Update Command Parsing

- [x] **Parse quit command** (Design Doc §3.3)
  - Match on `{"command":"quit"}`
  - Send `PromptResult::Command(AgentEnvironmentCommand::Quit)` to cmd_sender
  - Call `shutdown.notify_waiters()`
  - Break from processor loop

- [x] **Parse prompt command** (Design Doc §3.3)
  - Match on `{"command":"prompt"}`
  - Extract `text` field (required)
  - Extract `worker_id` field (optional, defaults to first worker from session)
  - Send `PromptResult::Command(AgentEnvironmentCommand::Prompt { worker_id, text })`

---

## Phase 9: Testing

### 9.1 Unit Tests for Task 1.1

- [ ] **Test UserInteractionRequired enum** (Design Doc §5)
  - Test enum variants can be constructed
  - Test PartialEq implementation

- [ ] **Test Session::has_active_jobs()** (Design Doc §5)
  - Test returns false when no jobs exist
  - Test returns false when jobs exist but all inactive
  - Test returns true when at least one job is active

- [ ] **Test AgentEnvironment job monitoring** (Design Doc §5)
  - Test monitor does nothing in interactive mode
  - Test monitor triggers shutdown when all jobs complete
  - Test monitor prints warning for non-clean exit
  - Test monitor handles multiple jobs

- [ ] **Test TextUi interactive flag** (Design Doc §5)
  - Test prompt loop spawns in interactive mode
  - Test prompt loop does NOT spawn in non-interactive mode

- [ ] **Test StructuredIO interactive flag** (Design Doc §5)
  - Test input reader spawns in interactive mode
  - Test input reader does NOT spawn in non-interactive mode

### 9.2 Integration Tests for Task 1.1

- [ ] **Test full non-interactive flow with TextUi** (Design Doc §5)
  - Create EventBus, Session, Worker, TextUi with interactive=false
  - Add input message to conversation history
  - Launch AgentLoop job
  - Run AgentEnvironment with timeout
  - Verify clean exit

- [ ] **Test full non-interactive flow with StructuredIO** (Design Doc §5)
  - Same as above but with StructuredIO
  - Verify JSON events are output
  - Verify clean exit

- [ ] **Test error: no input in non-interactive mode** (Design Doc §5)
  - Call ChatArgs::execute() with no_interactive=true and input=None
  - Verify error is returned

### 9.3 Manual Tests for Task 1.1

- [ ] **Manual test: TextUi non-interactive** (Design Doc §5)
  - Run: `q chat --no-interactive "What is 2+2?"`
  - Verify: processes prompt, displays response, exits cleanly

- [ ] **Manual test: StructuredIO non-interactive** (Design Doc §5)
  - Run: `q chat --no-interactive --ui-mode=structured "What is 2+2?"`
  - Verify: outputs JSON events, exits cleanly

- [ ] **Manual test: no input error** (Design Doc §5)
  - Run: `q chat --no-interactive`
  - Verify: error message displayed

- [ ] **Manual test: interactive mode still works** (Design Doc §5)
  - Run: `q chat "What is 2+2?"`
  - Verify: processes prompt, shows prompt for next input

### 9.4 Unit Tests for Task 1.6

- [ ] **Test WorkerEvent::Created handler** (Design Doc §5)
  - Create StructuredIO with test writer
  - Send WorkerEvent::Created
  - Verify JSON output contains event="worker_created", worker_id, name

- [ ] **Test WorkerEvent::Deleted handler** (Design Doc §5)
  - Create StructuredIO with test writer
  - Send WorkerEvent::Deleted
  - Verify JSON output contains event="worker_deleted", worker_id

- [ ] **Test JobEvent::Started handler** (Design Doc §5)
  - Create StructuredIO with test writer
  - Send JobEvent::Started
  - Verify JSON output contains event="job_started", worker_id, job_id, task_type

- [ ] **Test JobEvent::Completed handler** (Design Doc §5)
  - Create StructuredIO with test writer
  - Send JobEvent::Completed with each result type
  - Verify JSON output contains event="job_completed", result field

- [ ] **Test quit command parsing** (Design Doc §5)
  - Simulate stdin with `{"command":"quit"}`
  - Verify PromptResult::Command(Quit) is sent
  - Verify shutdown signal is triggered

### 9.5 Integration Tests for Task 1.6

- [ ] **Test event timing fix** (Design Doc §5)
  - Create EventBus, Session
  - Create StructuredIO with test writer
  - Create Worker (publishes Created event)
  - Verify StructuredIO received and output the Created event

- [ ] **Test quit command responsiveness** (Design Doc §5)
  - Create StructuredIO, start it
  - Send quit command via stdin
  - Measure time to shutdown signal
  - Verify shutdown happens within 100ms

- [ ] **Test stdin EOF handling** (Design Doc §5)
  - Create pipe with EOF
  - Create StructuredIO with pipe as stdin
  - Start StructuredIO
  - Verify input reader exits cleanly on EOF

### 9.6 Manual Tests for Task 1.6

- [ ] **Manual test: Worker Created event** (Design Doc §5)
  - Run: `q chat --ui-mode=structured "hello" | jq 'select(.event == "worker_created")'`
  - Verify: worker_created event is output

- [ ] **Manual test: Quit command responsiveness** (Design Doc §5)
  - Run: `echo '{"command":"quit"}' | q chat --ui-mode=structured`
  - Verify: exits immediately without hanging

- [ ] **Manual test: Event ordering** (Design Doc §5)
  - Run: `q chat --ui-mode=structured "hello" | jq -c '.event' | head -20`
  - Verify: events appear in correct chronological order

- [ ] **Manual test: Stdin EOF handling** (Design Doc §5)
  - Run: `echo "hello" | q chat --ui-mode=structured`
  - Verify: processes prompt, reaches EOF, continues until job completes, exits cleanly

---

## Phase 10: Documentation and Cleanup

### 10.1 Update Documentation

- [ ] **Update ChatArgs documentation** (Design Doc §4)
  - Document --no-interactive flag behavior
  - Add examples of usage

- [ ] **Update StructuredIO documentation** (Design Doc §4)
  - Document new event types (worker_created, worker_deleted, job_started, job_completed)
  - Add JSON format examples
  - Document quit command behavior

- [ ] **Update README** (Design Doc §4)
  - Add examples of non-interactive mode usage
  - Add examples of StructuredIO event output

### 10.2 Code Cleanup

- [ ] **Run cargo fmt** 
  - Format all modified files

- [ ] **Run cargo clippy**
  - Fix any warnings in modified code

- [ ] **Run cargo test**
  - Ensure all tests pass

---

## Implementation Notes

### Dependencies Between Tasks

- Task 1.1 and Task 1.6 are independent
- Both modify StructuredIO and ChatArgs::execute()
- Implement Task 1.1 first to minimize merge conflicts

### Critical Considerations

**Task 1.1**:
- Job completion monitor MUST be spawned BEFORE jobs are launched (avoid race condition)
- Use `interactive` flag internally (inverted from CLI flag) for clearer logic
- Error handling for edge cases (no input, no jobs) is critical

**Task 1.6**:
- Remove worker_id filtering to output ALL worker events
- Reader task pattern is essential for quit command responsiveness
- Event timing fix requires creating UI before worker

### Estimated Effort

- **Phase 1-5 (Task 1.1)**: 1-2 hours
- **Phase 6-8 (Task 1.6)**: 2-3 hours
- **Phase 9 (Testing)**: 1-2 hours
- **Phase 10 (Documentation)**: 0.5 hours

**Total**: 4.5-7.5 hours (conservative estimate including testing)

---

## Acceptance Criteria

### Task 1.1 Complete When:
- ✅ `q chat --no-interactive "hello"` runs once and exits cleanly
- ✅ `q chat --no-interactive --ui-mode=structured "hello"` runs once and exits cleanly
- ✅ No input reading occurs in non-interactive mode
- ✅ Warning displayed for non-clean exits (tool approval pending)
- ✅ Error displayed when no input provided
- ✅ All tests pass

### Task 1.6 Complete When:
- ✅ WorkerEvent::Created appears in JSON output
- ✅ WorkerEvent::Deleted appears in JSON output
- ✅ JobEvent::Started appears in JSON output
- ✅ JobEvent::Completed appears in JSON output
- ✅ Quit command responds immediately (<100ms)
- ✅ Events appear in correct chronological order
- ✅ All tests pass

---

## References

- **Design Document**: `planning/mvp-small-wins/mvp-small-wins-2-design.md`
- **Scope Document**: `planning/mvp-small-wins/mvp-small-wins-0-scope.md`
- **Research Document**: `planning/mvp-small-wins/mvp-small-wins-1-research.md`
- **Architecture Overview**: `codebase/agent-environment/README.md`
