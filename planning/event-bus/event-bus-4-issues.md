# Event Bus implementation issues

## [X] 1. No streaming output - FIXED
**Root cause**: AgentLoop was ignoring streaming chunks from the model provider. The `when_received` callback in `query_llm()` was stubbed out with a comment "Chunk handling stubbed out - will be replaced by EventBus".

**Fix**: 
- Updated the `when_received` callback to publish `JobEvent::OutputChunk` events for each `ModelResponseChunk::AssistantMessage` as they arrive
- Removed duplicate event publishing after `query_llm()` returns (was publishing complete response)
- Now chunks are published in real-time as they stream from Bedrock

**Files modified**:
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`


## [X] 2. When launched with CLI input, prompt must remain off - FIXED
**Root cause**: TextUi's `start()` method was unconditionally signaling `prompt_ready` immediately, regardless of whether a job was already running. This caused the prompt to appear even when the worker was Busy processing the initial input.

**Fix**:
- Modified `TextUi::start()` to check the worker's current lifecycle state before signaling `prompt_ready`
- Only signals `prompt_ready` if the worker is in `Idle` state
- If a job is already running (worker is `Busy`), the prompt will be enabled later when the `WorkerLifecycleState::Idle` event is received

**Files modified**:
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

## [X] 3. Ctrl+C when a job is active stops the app - FIXED
**Root cause**: The `CtrlCHandler` existed but was never instantiated or started. Without it, the default Ctrl+C behavior (exit application) was occurring.

**Fix**:
- Integrated `CtrlCHandler` into `AgentEnvironment::run()` method
- Handler now listens for Ctrl+C signals and implements the desired behavior:
  - First Ctrl+C: Cancel all active jobs (worker returns to Idle, prompt re-appears)
  - Second Ctrl+C (within 1 second): Force exit application
- Made `agent_env_ui` module public to allow access to `CtrlCHandler`

**Files modified**:
- `crates/chat-cli/src/agent_env/agent_environment.rs`
- `crates/chat-cli/src/cli/chat/mod.rs`

## [X] 4. /quit doesn't work - FIXED
**Root cause**: Race condition in the command processing loop. After handling the Quit command (which calls `notify_waiters()` on the shutdown signal), the loop would continue to the next iteration. Since no task was waiting on the signal at the moment `notify_waiters()` was called, the notification was lost.

**Fix**:
- Check if the command is a Quit command before handling it
- Break the loop immediately after handling Quit command
- This ensures the application exits promptly when /quit is entered

**Files modified**:
- `crates/chat-cli/src/agent_env/agent_environment.rs`
