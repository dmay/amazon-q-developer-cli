# Agent Environment Architecture

**Status**: 🚧 In Transition - Preparing for EventBus Architecture

## Overview

The Agent Environment architecture enables **multiple AI agents to run in parallel**, each working independently on different tasks while sharing common infrastructure. This design supports having multiple specialized agents that can execute different tasks simultaneously without blocking each other.

**Current State**: Core architecture (Worker, Session, WorkerJob, Tasks) is intact. Demo code and WorkerToHostInterface have been removed in preparation for EventBus-centered redesign. See `planning/event-bus/` for new architecture design.

## Key Design Goals

1. **Parallel Execution**: Multiple agents can run concurrently with independent state management
2. **Flexible Configuration**: Each agent can be customized with different parameters, tools, and behaviors
3. **Resource Sharing**: Agents share common resources (LLM providers, thread pools) efficiently
4. **Task Abstraction**: Different task types (agent loops, commands, orchestration) use the same execution framework
5. **Clean Lifecycle**: Proper cancellation, error handling, and resource cleanup

## Architecture Components

The architecture consists of several key components:

- **[Worker](./worker.md)**: Complete AI agent configuration (model provider, state, context, error tracking)
- **[ContextContainer](./context-container.md)**: Manages conversation history and contextual information
- **[WorkerTask](./tasks.md)**: Interface for executable work units (agent loops, commands, etc.)
- **[WorkerJob](./job.md)**: Running instance combining Worker + Task + execution infrastructure
- **[Session](./session.md)**: Central orchestrator managing all Workers and Jobs
- **[WorkerToHostInterface](./interface.md)**: Communication contract between Workers and UI layer
- **[ModelProvider](./model-provider.md)**: Abstraction for LLM communication
- **[TUI](./tui.md)**: Terminal User Interface for interactive agent sessions

## Code Location

All implementation is in: `crates/chat-cli/src/agent_env/`

```
agent_env/
├── mod.rs                          # Module exports
├── worker.rs                       # Worker implementation (MODIFIED: set_state simplified)
├── worker_task.rs                  # WorkerTask trait
├── worker_job.rs                   # WorkerJob implementation
├── worker_job_continuations.rs    # Job completion callbacks
├── session.rs                      # Session orchestrator (MODIFIED: run_agent_loop stubbed)
├── context_container/              # Context management
│   ├── mod.rs                     # Module exports
│   ├── context_container.rs       # ContextContainer struct
│   ├── conversation_history.rs    # ConversationHistory management
│   └── conversation_entry.rs      # ConversationEntry type
├── model_providers/                # LLM provider abstractions
│   ├── model_provider.rs          # ModelProvider trait
│   └── bedrock_converse_stream.rs # AWS Bedrock implementation
└── worker_tasks/                   # Task implementations
    ├── agent_loop.rs              # Main agent loop (MODIFIED: interface removed)
    └── mod.rs

REMOVED (in preparation for EventBus architecture):
├── worker_interface.rs            # DELETED - replaced by EventBus
└── demo/                           # DELETED - replaced by new entry point
```

UI implementation is in: `crates/chat-cli/src/cli/chat/agent_env_ui/`

```
agent_env_ui/
├── mod.rs                              # Module exports (simplified)
├── input_handler.rs                    # User input with rustyline
└── ctrl_c_handler.rs                   # Ctrl+C signal handling

REMOVED:
├── text_ui_worker_to_host_interface.rs # DELETED
└── prompt_queue.rs                      # DELETED
```

## Execution Flow

1. **Session Creation**: Initialize with model providers
2. **Worker Creation**: Build workers with specific configurations
3. **Task Launch**: Create task (e.g., AgentLoop) and launch via Session
4. **Execution**: Task runs asynchronously, communicating via WorkerToHostInterface
5. **State Management**: Worker transitions through states (Working → Requesting → Receiving → Inactive)
6. **Completion**: Job completes normally, is cancelled, or fails with error

## State Machine

Workers transition through these states:

```
Inactive → Working → Requesting → Receiving → [Waiting/UsingTool]* → Inactive
                                                                    ↓
                                                              InactiveFailed
```

- **Inactive**: Worker idle, ready for new task
- **Working**: Preparing request
- **Requesting**: Sending request to LLM
- **Receiving**: Streaming response from LLM
- **Waiting**: Waiting for user input
- **UsingTool**: Executing tool
- **InactiveFailed**: Task failed with error

## Example Usage

**Note**: Example code is outdated. Demo implementation has been removed. See `planning/event-bus/` for new architecture design.

```rust
// OUTDATED - For reference only
// New implementation will use EventBus architecture

// Create session with model provider
let session = Session::new(vec![model_provider]);

// Build worker
let worker = session.build_worker("main".to_string());

// Add message to worker's context
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_input_message("Hello, world!".to_string());

// Launch agent loop - CURRENTLY STUBBED
// Will be reimplemented with EventBus
let input = AgentLoopInput {};
let job = session.run_agent_loop(worker, input)?; // Returns unimplemented!()
```

## Related Documentation

**Current Architecture (Partially Intact)**:
- [Worker Details](./worker.md) - Core worker implementation (set_state simplified)
- [Context Container](./context-container.md) - Context management (unchanged)
- [Task System](./tasks.md) - WorkerTask trait (unchanged)
- [Job Management](./job.md) - WorkerJob implementation (unchanged)
- [Session Orchestration](./session.md) - Session with stubbed methods
- [Model Providers](./model-provider.md) - LLM abstraction (unchanged)

**Removed/Outdated**:
- ~~UI Interface~~ - WorkerToHostInterface removed, see EventBus design
- ~~Demo Implementation~~ - Removed, see EventBus design

**New Architecture Design**:
- [EventBus Design](../../planning/event-bus/event-bus-1-design.md) - New architecture
- [Implementation Plan](../../planning/event-bus/event-bus-2-implementation-plan.md) - Step-by-step tasks
- [Files to Keep](../../planning/event-bus/event-bus-files-to-keep.md) - Migration guide
- [Preparation Complete](../../planning/event-bus/preparation-complete.md) - Current status
