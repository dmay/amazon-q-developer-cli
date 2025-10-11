# MVP Small Wins - Scope

## Overview

This workflow covers quick implementation tasks that provide immediate value and can be completed independently. These are foundational improvements to the agent_env architecture that enhance usability and robustness.

## Tasks Included

### 1.1: --no-interactive Support

### 1.6: StructuredIO Enhancements

---

## Task 1.1: --no-interactive Support

I want to be able to run CLI tool in non-interactive mode, when it would process provided task (prompt provided through CLI argimgents) and complete.

There are two states a task can complete in:

- clean, when the loop was completed with agent's response (normally UI would wait for a simple prompt here)
- non-clean, when the loop was completed with waiting for tool use approval (normally UI would wait for tool approval signal here)

CLI tool must indicate when the task completed in non-clean state, and exit anyway.

### Current State

- `--no-interactive` flag exists in ChatArgs but is not fully implemented
- TextUi spawns a prompt loop that continues reading input
- StructuredIO spawns a stdin reader task that continues reading
- Both UIs don't properly exit after processing initial input in non-interactive mode
- AgentEnvironment doesn't track job completion for shutdown

### Requirements

**Core Behavior:**

- When `--no-interactive` flag is set, the CLI should:
  1. Process only the initial input provided via command line
  2. Execute the agent loop once
  3. Exit cleanly after task completion
  4. Not spawn any input reading tasks
  5. Not display prompts or wait for user input
  6. Indicate if the task completed clearly. I.e AgentLoop completed waiting for a prompt, and NOT for a tool approval.
     1. That could require an extra flag that tasks could report back on completion. Right now we only have success/fail state on Job level, but here we introduce semantical, but NOT specific to AgentLoop, exit flag. Remember, the forwaqrd vision is to have multiple possible tasks, which can be launched in various ways, and some could also expect approval-like or required-arguments-like input to continue execution.
     2. Another option is to let AgentEnvironment to scan task_metadata in active workers, to see if AgentLoop (or any other specific task added in the future) has its specific flag that indicates required user input.

**UI-Specific Requirements:**

**TextUi:**

- Skip prompt loop spawn entirely when flag is set
- Display output normally (streaming responses)
- Exit after initial job completes

**StructuredIO:**

- Skip stdin reader task spawn when flag is set
- Output JSON events normally
- Exit after initial job completes

**AgentEnvironment:**

- Track initial jobs created at startup
- Monitor active job count
- Initiate shutdown when no active jobs remain (after initial jobs complete)
- Confirm all workers remain in clean state (none have flags in task metadata that indicate task ended waiting for user input, like tools approval). Instruct active UI to display a warning about non-clean abort.
- Ensure clean cancellation and resource cleanup

### Implementation Approach

1. Pass `--no-interactive` flag to UI constructors
2. Modify TextUi::new() to conditionally spawn prompt task
3. Modify StructuredIO::new() to conditionally spawn stdin reader task
4. Add job tracking to AgentEnvironment:
   - Track initial job IDs created before run() is called
   - Subscribe to JobEvent::Completed events
   - When JobEvent::Completed received, and there's no active jobs in the session's list, trigger shutdown
5. Test both UI modes with and without the flag

### Acceptance Criteria

- `q chat --no-interactive "hello"` runs once and exits cleanly
- `q chat --no-interactive --ui-mode=structured "hello"` runs once and exits cleanly
- No input reading occurs in non-interactive mode
- No hanging processes or prompt loops
- Proper shutdown with all resources cleaned up
- Exit code reflects success/failure of the task

### Dependencies

None - can be implemented immediately

### Estimated Effort

1-2 hours

---

## Task 1.6: StructuredIO Enhancements

### Current State

- StructuredIO exists with JSON input/output
- Subscribes to EventBus for events
- May not display all worker lifecycle events
- Uses `lines.next_line()` for input reading which may block quit command handling
- EventBus subscription timing relative to Session operations unclear

### Requirements

**Event Display:**

- Display WorkerEvent::Created when workers are created
- Display WorkerEvent::Deleted when workers are deleted
- Display JobEvent::Started and ::Completed accordingly
- Ensure events are received before Session starts sending them
- Maintain proper event ordering

**Quit Command Handling:**

- `{"command":"quit"}` should immediately interrupt and exit
- Current implementation may block on `lines.next_line()` preventing immediate response
- Need to investigate if input reading blocks quit command processing

**Event Ordering:**

- EventBus subscription must happen before any Session operations
- Initial worker creation event must be captured
- All lifecycle events must appear in correct order

### Implementation Approach

1. **Investigate Input Blocking:**

   - Test if `lines.next_line()` blocks quit command handling
   - If blocking occurs, replace with interruptible input method:
     - Option A: Use tokio::select! with stdin reader and command channel
     - Option B: Use channel-based input reading with separate task
     - Identify more oprtions, if available
2. **Add Event Handlers:**

   - Add WorkerEvent::Created handler to StructuredIO::handle_event()
   - Add WorkerEvent::Deleted handler to StructuredIO::handle_event()
   - Format events as JSON output
3. **Verify Event Timing:**

   - Review StructuredIO::new() to ensure EventBus subscription happens first
   - Review ChatArgs::execute() to ensure UI is created before Session operations
     - This also applies to TextUi, it should also be able to handle all events from the start
   - Add test to verify initial worker creation event is captured
4. **Test Quit Command:**

   - Test: `echo '{"command":"quit"}' | q chat --ui-mode=structured`
   - Verify immediate exit without blocking
   - Verify proper cleanup

### Acceptance Criteria

- Worker creation events appear in JSON output
- Worker deletion events appear in JSON output
- Quit command properly interrupts and exits immediately
- No blocking on input reading when quit is issued
- All events appear in correct chronological order
- Initial worker creation event is captured

### Technical Considerations

**Input Reading Options:**

Current (potentially blocking):

```rust
while let Some(line) = lines.next_line().await? {
    // Process line
}
```

Option A - tokio::select!:

```rust
loop {
    tokio::select! {
        line = lines.next_line() => {
            // Process line
        }
        _ = shutdown_rx.recv() => {
            break;
        }
    }
}
```

Option B - Channel-based:

```rust
// Separate task reads stdin and sends to channel
// Main loop uses tokio::select! on channel and shutdown signal
```

**Event JSON Format:**

```json
{
  "event": "worker_created",
  "worker_id": "main",
  "timestamp": "2025-10-10T18:07:06Z"
}
```

### Dependencies

None - can be implemented immediately

### Estimated Effort

2-3 hours (includes investigation and potential refactoring of input reading)

---

## Success Metrics

### Task 1.1 Success

- Both UI modes support --no-interactive
- Clean exit after single execution
- No resource leaks or hanging processes
- Proper exit codes

### Task 1.6 Success

- All worker lifecycle events visible in StructuredIO
- Quit command responds immediately
- Event ordering is correct and complete
- No input blocking issues

---

## Implementation Order

1. **Task 1.1 first** - Simpler, provides immediate value, no research needed
2. **Task 1.6 second** - May require input reading refactoring based on investigation

Both tasks are independent and can be implemented in parallel if needed.

---

## Testing Strategy

### Task 1.1 Tests

```bash
# Test TextUi non-interactive
q chat --no-interactive "What is 2+2?"

# Test StructuredIO non-interactive
q chat --no-interactive --ui-mode=structured "What is 2+2?"

# Test without initial input (should error or exit immediately)
q chat --no-interactive

# Test with complex input
q chat --no-interactive "Write a hello world program in Python"
```

### Task 1.6 Tests

```bash
# Test quit command
echo '{"command":"quit"}' | q chat --ui-mode=structured

# Test worker events
q chat --ui-mode=structured "hello" | jq 'select(.event == "worker_created")'

# Test event ordering
q chat --ui-mode=structured "hello" | jq -c '.event' | head -20
```

---

## Documentation Updates

After implementation:

- Update ChatArgs documentation for --no-interactive flag
- Update StructuredIO documentation with event types
- Add examples to README
- Update UI implementations documentation

---

## Related Files

**Task 1.1:**

- `crates/chat-cli/src/cli/chat/mod.rs` - ChatArgs::execute()
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs` - TextUi implementation
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - StructuredIO implementation
- `crates/chat-cli/src/agent_env/agent_environment.rs` - AgentEnvironment main loop

**Task 1.6:**

- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - StructuredIO implementation
- `crates/chat-cli/src/agent_env/events.rs` - Event type definitions
- `crates/chat-cli/src/agent_env/event_bus.rs` - EventBus implementation
