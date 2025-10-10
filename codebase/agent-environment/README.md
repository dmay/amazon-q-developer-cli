# Agent Environment Architecture

**Status**: ✅ EventBus Architecture Implemented (October 2025)

## Overview

The Agent Environment architecture enables **multiple AI agents to run in parallel**, each working independently on different tasks while sharing common infrastructure. The system uses an **event-driven architecture** with a centralized EventBus for communication between components.

**Current State**: EventBus-centered architecture is fully implemented and operational. All components communicate via events, enabling flexible UI implementations and clean separation of concerns.

## Key Design Goals

1. **Parallel Execution**: Multiple agents can run concurrently with independent state management
2. **Flexible Configuration**: Each agent can be customized with different parameters, tools, and behaviors
3. **Resource Sharing**: Agents share common resources (LLM providers, thread pools) efficiently
4. **Task Abstraction**: Different task types (agent loops, commands, orchestration) use the same execution framework
5. **Clean Lifecycle**: Proper cancellation, error handling, and resource cleanup

## Architecture Components

The architecture consists of several key components:

### Core Components
- **[EventBus](./event-bus.md)**: Central event distribution system using tokio broadcast channels
- **[AgentEnvironment](./agent-environment.md)**: Top-level coordinator managing event multicasting and UI coordination
- **[Session](./session.md)**: Orchestrator managing Workers, Jobs, and publishing lifecycle events
- **[Worker](./worker.md)**: Agent state container with lifecycle state, task metadata, and conversation context
- **[WorkerJob](./job.md)**: Running task instance with lifecycle management and cancellation support
- **[WorkerTask](./tasks.md)**: Interface for executable work units (agent loops, commands, etc.)

### Supporting Components
- **[ContextContainer](./context-container.md)**: Manages conversation history and contextual information
- **[ModelProvider](./model-provider.md)**: Abstraction for LLM communication
- **[Commands](./commands.md)**: Command system for UI interactions (Prompt, Compact, Quit)
- **[UI Implementations](../chat-cli/ui-implementations.md)**: TextUi, StructuredIO, and future UIs

## Code Location

All implementation is in: `crates/chat-cli/src/agent_env/`

```
agent_env/
├── mod.rs                          # Module exports
├── events.rs                       # Event type definitions (NEW)
├── event_bus.rs                    # EventBus implementation (NEW)
├── agent_environment.rs            # AgentEnvironment coordinator (NEW)
├── commands.rs                     # Command system (NEW)
├── session.rs                      # Session orchestrator (UPDATED: event publishing)
├── worker.rs                       # Worker implementation (UPDATED: lifecycle state, metadata)
├── worker_job.rs                   # WorkerJob implementation
├── worker_job_continuations.rs    # Job completion callbacks
├── worker_task.rs                  # WorkerTask trait
├── context_container/              # Context management
│   ├── mod.rs                     
│   ├── context_container.rs       
│   ├── conversation_history.rs    
│   └── conversation_entry.rs      
├── model_providers/                # LLM provider abstractions
│   ├── model_provider.rs          
│   └── bedrock_converse_stream.rs 
└── worker_tasks/                   # Task implementations
    ├── agent_loop.rs              # Main agent loop (UPDATED: event publishing)
    └── mod.rs
```

UI implementation is in: `crates/chat-cli/src/cli/chat/agent_env_ui/`

```
agent_env_ui/
├── mod.rs                              # Module exports
├── text_ui.rs                          # Text-based interactive UI (NEW)
├── structured_io.rs                    # JSON I/O for scripting (NEW)
├── ui_utils.rs                         # Shared UI utilities (NEW)
├── input_handler.rs                    # User input with rustyline
└── ctrl_c_handler.rs                   # Ctrl+C signal handling
```

## Execution Flow

1. **Initialization**: ChatArgs::execute() creates EventBus, Session, Worker, UI, and AgentEnvironment
2. **Event Multicasting**: AgentEnvironment spawns task to forward events to all UIs
3. **UI Startup**: UI starts (spawns prompt loop for TextUi, input reader for StructuredIO)
4. **Command Processing**: AgentEnvironment receives commands from UI via channel
5. **Task Execution**: Session launches tasks (AgentLoop), publishes events throughout lifecycle
6. **Event Delivery**: EventBus broadcasts events to all subscribers (UIs, tests, etc.)
7. **Completion**: Task completes, Session updates worker state, publishes completion event
8. **Shutdown**: AgentEnvironment coordinates cleanup, cancels jobs, exits gracefully

## Event-Driven Communication

All components communicate via events published to the EventBus:

### Event Types
- **WorkerEvent**: Created, Deleted, LifecycleStateChanged
- **JobEvent**: Started, Completed, OutputChunk
- **AgentLoopEvent**: ResponseReceived, ToolUseRequestReceived
- **SystemEvent**: ShutdownInitiated

### Worker Lifecycle States
- **Idle**: Worker ready for new task
- **Busy**: Worker executing task
- **IdleFailed**: Worker idle after task failure

### Event Flow
```
Component → EventBus.publish() → broadcast::Sender → All Subscribers
                                                    ├─ AgentEnvironment (multicast)
                                                    │  ├─ Main UI
                                                    │  └─ Headless UIs
                                                    └─ Tests
```

## Example Usage

```rust
// Create EventBus and Session
let event_bus = EventBus::default();
let session = Arc::new(Session::new(event_bus.clone(), vec![model_provider]));

// Create Worker
let worker = session.build_worker("main".to_string());

// Add message to conversation
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_input_message("Hello, world!".to_string());

// Create UI
let (text_ui, cmd_receiver) = TextUi::new(session.clone(), worker.id, None)?;

// Create AgentEnvironment
let agent_env = AgentEnvironment::new(
    session.clone(),
    event_bus.clone(),
    Some(Arc::new(text_ui)),
    vec![], // No headless UIs
);

// Launch agent loop
session.run_task__agent_loop(worker, AgentLoopInput {})?;

// Run main loop (blocks until shutdown)
agent_env.run().await?;
```

## Related Documentation

**Core Architecture**:
- [EventBus](./event-bus.md) - Event distribution system
- [AgentEnvironment](./agent-environment.md) - Top-level coordinator
- [Session](./session.md) - Worker and job orchestration with event publishing
- [Worker](./worker.md) - Agent state with lifecycle and metadata
- [Context Container](./context-container.md) - Context management
- [Task System](./tasks.md) - WorkerTask trait and implementations
- [Job Management](./job.md) - WorkerJob implementation
- [Model Providers](./model-provider.md) - LLM abstraction
- [Commands](./commands.md) - Command system for UI interactions

**UI Implementations**:
- [UI Implementations](../chat-cli/ui-implementations.md) - TextUi, StructuredIO, and future UIs

**Design Documents**:
- [EventBus Design](../../planning/event-bus/event-bus-1-design.md) - Complete architecture design
- [Implementation Plan](../../planning/event-bus/event-bus-2-implementation-plan.md) - Step-by-step implementation
- [Implementation Log](../../planning/event-bus/event-bus-3-implementation-log.md) - Progress tracking
