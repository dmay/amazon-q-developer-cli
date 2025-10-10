# Some info about the initialization process

**Status**: 🚧 Entry point minimized for EventBus architecture preparation

## Startup Call Chain
- [main()](../../crates/chat-cli/src/main.rs) - parses arguments, creates tokio runtime, passes to Cli.execute in...
- [Cli.execute()](../../crates/chat-cli/src/cli/mod.rs#L217) - sets up logger, creates `Os`, executes subcommand (below), closes telemetry
- [RootSubcommand.execute()](../../crates/chat-cli/src/cli/mod.rs#L139) - telemetry, passes to the actual subcommand execution
  - subcommands are defined as a [enum RootSubcommand](../../crates/chat-cli/src/cli/mod.rs#L93)
  - We are interested in `Chat(ChatArgs)`
  - `ChatArgs` are defined in "chat" folder:  [ChatArgs](../../crates/chat-cli/src/cli/chat/mod.rs#L210)
- **Chat entry point is** [`ChatArgs.execute()`](../../crates/chat-cli/src/cli/chat/mod.rs#L229)
    - **CURRENTLY MINIMIZED** - Just prints "Hello" and exits
    - Will be reimplemented with EventBus architecture
    - See `planning/event-bus/` for new design

## Agent Environment Architecture

**Status**: Core architecture intact, demo/interface removed for EventBus redesign.

New parallel agent execution architecture. See [Agent Environment Documentation](../agent-environment/README.md) for details.

### Core Components (Intact)
- [Worker](../agent-environment/worker.md) - Agent configuration and state management
  - Implementation: [worker.rs](../../crates/chat-cli/src/agent_env/worker.rs)
  - **MODIFIED**: `set_state()` simplified (no interface parameter)
  - States: Inactive, Working, Requesting, Receiving, Waiting, UsingTool, InactiveFailed
- [Session](../agent-environment/session.md) - Central orchestrator for workers and jobs
  - Implementation: [session.rs](../../crates/chat-cli/src/agent_env/session.rs)
  - **MODIFIED**: `run_agent_loop()` stubbed with `unimplemented!()`
  - Manages worker creation, job launching, resource sharing
- [WorkerJob](../agent-environment/job.md) - Running task instance with lifecycle management
  - Implementation: [worker_job.rs](../../crates/chat-cli/src/agent_env/worker_job.rs)
  - Continuations: [worker_job_continuations.rs](../../crates/chat-cli/src/agent_env/worker_job_continuations.rs)

### Task System (Partially Intact)
- [WorkerTask Trait](../agent-environment/tasks.md) - Interface for executable work units
  - Trait definition: [worker_task.rs](../../crates/chat-cli/src/agent_env/worker_task.rs)
  - AgentLoop implementation: [agent_loop.rs](../../crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs)
    - **MODIFIED**: Removed `host_interface` field, chunk handling stubbed

### Communication (Removed)
- ~~WorkerToHostInterface~~ - **DELETED** - Will be replaced by EventBus
  - ~~Trait definition: worker_interface.rs~~ - File deleted
  - ~~CLI implementation: cli_interface.rs~~ - File deleted

### Model Providers (Intact)
- [ModelProvider System](../agent-environment/model-provider.md) - LLM abstraction layer
  - Trait definition: [model_provider.rs](../../crates/chat-cli/src/agent_env/model_providers/model_provider.rs)
  - Bedrock implementation: [bedrock_converse_stream.rs](../../crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs)

### Demo (Removed)
- ~~Demo Implementation~~ - **DELETED** - Will be replaced by EventBus architecture
  - ~~Entry point: init.rs~~ - File deleted
  - ~~ProtoLoop: proto_loop.rs~~ - File deleted
  - ~~CLI interface: cli_interface.rs~~ - File deleted

### UI Components (Partially Intact)
- Reusable utilities preserved:
  - [input_handler.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/input_handler.rs) - User input with rustyline
  - [ctrl_c_handler.rs](../../crates/chat-cli/src/cli/chat/agent_env_ui/ctrl_c_handler.rs) - Ctrl+C signal handling
- Removed:
  - ~~text_ui_worker_to_host_interface.rs~~ - **DELETED**
  - ~~prompt_queue.rs~~ - **DELETED**
  - ~~AgentEnvTextUi~~ - **DELETED** from mod.rs

## Next Steps

See EventBus architecture design and implementation plan:
- [Design Document](../../planning/event-bus/event-bus-1-design.md)
- [Implementation Plan](../../planning/event-bus/event-bus-2-implementation-plan.md)
- [Files to Keep](../../planning/event-bus/event-bus-files-to-keep.md)
- [Preparation Status](../../planning/event-bus/preparation-complete.md)
