# Chat CLI - File Structure and Initialization

**Status**: ✅ EventBus Architecture Implemented (October 2025)

## Startup Call Chain

- [main()](../../crates/chat-cli/src/main.rs) - Parses arguments, creates tokio runtime, passes to Cli.execute
- [Cli.execute()](../../crates/chat-cli/src/cli/mod.rs#L217) - Sets up logger, creates `Os`, executes subcommand, closes telemetry
- [RootSubcommand.execute()](../../crates/chat-cli/src/cli/mod.rs#L139) - Telemetry, passes to actual subcommand execution
  - Subcommands defined as [enum RootSubcommand](../../crates/chat-cli/src/cli/mod.rs#L93)
  - We are interested in `Chat(ChatArgs)`
  - `ChatArgs` defined in "chat" folder: [ChatArgs](../../crates/chat-cli/src/cli/chat/mod.rs)
- **Chat entry point**: [`ChatArgs.execute()`](../../crates/chat-cli/src/cli/chat/mod.rs)
  - Creates EventBus, Session, Worker, UI, and AgentEnvironment
  - Runs AgentEnvironment main loop (blocks until shutdown)
  - See implementation details below

## EventBus Architecture

Complete event-driven architecture for parallel agent execution. See [Agent Environment Documentation](../agent-environment/README.md) for overview.

### Core Components

#### EventBus System
- [EventBus](../agent-environment/event-bus.md) - Central event distribution
  - Implementation: [event_bus.rs](../../crates/chat-cli/src/agent_env/event_bus.rs)
  - Events: [events.rs](../../crates/chat-cli/src/agent_env/events.rs)
  - Uses tokio broadcast channels for efficient multicasting
  - Publishes WorkerEvent, JobEvent, AgentLoopEvent, SystemEvent

#### Coordination Layer
- [AgentEnvironment](../agent-environment/agent-environment.md) - Top-level coordinator
  - Implementation: [agent_environment.rs](../../crates/chat-cli/src/agent_env/agent_environment.rs)
  - Manages event multicasting to all UIs
  - Processes commands from main UI
  - Coordinates shutdown and cleanup
  - Supports multiple concurrent UIs (main + headless)

#### Orchestration Layer
- [Session](../agent-environment/session.md) - Worker and job orchestrator
  - Implementation: [session.rs](../../crates/chat-cli/src/agent_env/session.rs)
  - Creates and manages workers
  - Launches and monitors jobs
  - Publishes lifecycle events (worker creation, state changes, job completion)
  - Manages resource sharing (model providers)

#### State Management
- [Worker](../agent-environment/worker.md) - Agent state container
  - Implementation: [worker.rs](../../crates/chat-cli/src/agent_env/worker.rs)
  - Lifecycle state: Idle, Busy, IdleFailed (managed by Session)
  - Task metadata: Extensible HashMap for task-specific state
  - Context container: Conversation history and context
  - Fully serializable for persistence

#### Execution Layer
- [WorkerJob](../agent-environment/job.md) - Running task instance
  - Implementation: [worker_job.rs](../../crates/chat-cli/src/agent_env/worker_job.rs)
  - Continuations: [worker_job_continuations.rs](../../crates/chat-cli/src/agent_env/worker_job_continuations.rs)
  - Manages task lifecycle and cancellation
  - Runs completion callbacks

#### Task System
- [WorkerTask Trait](../agent-environment/tasks.md) - Interface for executable work units
  - Trait definition: [worker_task.rs](../../crates/chat-cli/src/agent_env/worker_task.rs)
  - AgentLoop implementation: [agent_loop.rs](../../crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs)
    - Publishes OutputChunk events for streaming responses
    - Publishes AgentLoopEvent for responses and tool use
    - Sets task metadata for completion state

#### Command System
- [Commands](../agent-environment/commands.md) - Command types and parsing
  - Implementation: [commands.rs](../../crates/chat-cli/src/agent_env/commands.rs)
  - AgentEnvironmentCommand: Prompt, Compact, Quit
  - UiCommand: Usage, Context, Status, Workers
  - CommandParser: Parses explicit and implicit commands

### UI Implementations

See [UI Implementations](./ui-implementations.md) for detailed documentation.

#### TextUi - Text-Based Interactive UI
- Implementation: [text_ui.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs)
- Features:
  - Readline-style input with command history
  - Streaming output display
  - Prompt queue pattern (only reads when worker is Idle)
  - UI commands handled internally (/usage, /context, /status, /workers)
  - Agent commands forwarded to AgentEnvironment

#### StructuredIO - JSON I/O for Scripting
- Implementation: [structured_io.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs)
- Features:
  - Always-reading pattern (continuously reads stdin)
  - JSON output for responses and lifecycle events
  - Suitable for piping commands from scripts
  - No interactive prompts

#### Shared UI Utilities
- [ui_utils.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/ui_utils.rs)
  - Token usage calculation
  - Context information formatting
  - Shared helper functions
- [input_handler.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/input_handler.rs)
  - User input with rustyline
  - Command history support
- [ctrl_c_handler.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/ctrl_c_handler.rs)
  - Ctrl+C signal handling

### Model Providers

- [ModelProvider System](../agent-environment/model-provider.md) - LLM abstraction layer
  - Trait definition: [model_provider.rs](../../crates/chat-cli/src/agent_env/model_providers/model_provider.rs)
  - Bedrock implementation: [bedrock_converse_stream.rs](../../crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs)

### Context Management

- [ContextContainer](../agent-environment/context-container.md) - Context management
  - Implementation: [context_container.rs](../../crates/chat-cli/src/agent_env/context_container/context_container.rs)
  - Conversation history: [conversation_history.rs](../../crates/chat-cli/src/agent_env/context_container/conversation_history.rs)
  - Conversation entry: [conversation_entry.rs](../../crates/chat-cli/src/agent_env/context_container/conversation_entry.rs)

## Initialization Flow

```
ChatArgs::execute()
├─ Create EventBus
├─ Load AWS config and create Bedrock client
├─ Create Session with EventBus and model providers
├─ Create main Worker
├─ Add initial input to conversation (if provided)
├─ Create UI based on --ui-mode flag
│  ├─ Text (default): TextUi with history path
│  ├─ Structured: StructuredIO for JSON I/O
│  └─ None: Headless mode (no main UI)
├─ Create AgentEnvironment with Session, EventBus, UI
├─ Launch agent loop if initial input provided
└─ Run AgentEnvironment.run() (blocks until shutdown)
```

## Event Flow

```
Component → EventBus.publish()
           ↓
    broadcast::Sender
           ↓
    All Subscribers
    ├─ AgentEnvironment (event multicast task)
    │  ├─ Main UI (via handle_event)
    │  └─ Headless UIs (via handle_event)
    └─ Tests (optional subscribers)
```

## Command Flow

```
User Input → UI.prompt_loop
           ↓
    CommandParser.parse()
           ↓
    Command (Agent or Ui)
           ↓
    ├─ UiCommand: Handled by UI internally
    └─ AgentEnvironmentCommand: Sent via channel
                              ↓
                   AgentEnvironment.handle_command()
                              ↓
                   Session.run_task__*()
                              ↓
                   Task execution with event publishing
```

## File Structure

```
crates/chat-cli/src/
├─ agent_env/                           # Core agent environment
│  ├─ mod.rs                           # Module exports
│  ├─ events.rs                        # Event type definitions
│  ├─ event_bus.rs                     # EventBus implementation
│  ├─ agent_environment.rs             # AgentEnvironment coordinator
│  ├─ commands.rs                      # Command system
│  ├─ session.rs                       # Session orchestrator
│  ├─ worker.rs                        # Worker state container
│  ├─ worker_job.rs                    # Job execution
│  ├─ worker_job_continuations.rs     # Job completion callbacks
│  ├─ worker_task.rs                   # WorkerTask trait
│  ├─ context_container/               # Context management
│  │  ├─ mod.rs
│  │  ├─ context_container.rs
│  │  ├─ conversation_history.rs
│  │  └─ conversation_entry.rs
│  ├─ model_providers/                 # LLM abstractions
│  │  ├─ mod.rs
│  │  ├─ model_provider.rs
│  │  └─ bedrock_converse_stream.rs
│  └─ worker_tasks/                    # Task implementations
│     ├─ mod.rs
│     └─ agent_loop.rs                # Main agent loop
│
└─ cli/chat/
   ├─ mod.rs                           # Entry point (ChatArgs::execute)
   └─ agent_env_ui/                    # UI implementations
      ├─ mod.rs                        # Module exports
      ├─ text_ui.rs                    # Text-based interactive UI
      ├─ structured_io.rs              # JSON I/O for scripting
      ├─ ui_utils.rs                   # Shared UI utilities
      ├─ input_handler.rs              # Input handling
      └─ ctrl_c_handler.rs             # Signal handling
```

## Related Documentation

**Architecture**:
- [Agent Environment Overview](../agent-environment/README.md)
- [EventBus](../agent-environment/event-bus.md)
- [AgentEnvironment](../agent-environment/agent-environment.md)
- [Session](../agent-environment/session.md)
- [Worker](../agent-environment/worker.md)
- [UI Implementations](./ui-implementations.md)
