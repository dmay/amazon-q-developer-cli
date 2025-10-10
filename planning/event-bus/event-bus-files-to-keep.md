# Files to Keep for EventBus Architecture

This document lists files from the current implementation that will be kept and modified for the new EventBus-centered architecture.

## Files to Keep and Modify

### Core Agent Environment (`crates/chat-cli/src/agent_env/`)

#### Keep and Modify:
- **`mod.rs`** - Module exports, will add new modules (events, event_bus, agent_environment, commands)
- **`worker.rs`** - Worker state container, will add lifecycle_state and task_metadata fields
- **`session.rs`** - Session orchestrator, will add EventBus and event publishing
- **`worker_job.rs`** - Job execution, will add EventBus integration
- **`worker_job_continuations.rs`** - Job completion callbacks, keep as-is
- **`worker_task.rs`** - WorkerTask trait, keep as-is

#### Context Container (keep all):
- **`context_container/mod.rs`** - Module exports
- **`context_container/context_container.rs`** - Context container struct
- **`context_container/conversation_history.rs`** - Conversation history management
- **`context_container/conversation_entry.rs`** - Conversation entry types

#### Model Providers (keep all):
- **`model_providers/mod.rs`** - Module exports
- **`model_providers/model_provider.rs`** - ModelProvider trait
- **`model_providers/bedrock_converse_stream.rs`** - Bedrock implementation

#### Worker Tasks:
- **`worker_tasks/mod.rs`** - Module exports
- **`worker_tasks/agent_loop.rs`** - Main agent loop, will modify to use EventBus instead of WorkerToHostInterface

### UI Implementation (`crates/chat-cli/src/cli/chat/agent_env_ui/`)

#### Keep and Modify:
- **`input_handler.rs`** - User input handling with rustyline, keep as-is
- **`ctrl_c_handler.rs`** - Ctrl+C signal handling, keep as-is

#### Keep mod.rs but heavily modify:
- **`mod.rs`** - Will be restructured to define UI traits and export new UI implementations

## Files to Delete

### Agent Environment:
- **`agent_env/worker_interface.rs`** - Deprecated, replaced by EventBus
- **`agent_env/demo/`** (entire directory) - Demo implementation, will be replaced by new entry point
  - `demo/mod.rs`
  - `demo/init.rs`
  - `demo/proto_loop.rs`
  - `demo/cli_interface.rs`

### UI Implementation:
- **`agent_env_ui/text_ui_worker_to_host_interface.rs`** - Old interface implementation, replaced by new TextUi
- **`agent_env_ui/prompt_queue.rs`** - Will be reimplemented differently in new TextUi

## New Files to Create

### Core Agent Environment (`crates/chat-cli/src/agent_env/`):
- **`events.rs`** - Event type definitions (Phase 1)
- **`event_bus.rs`** - EventBus implementation (Phase 1)
- **`agent_environment.rs`** - AgentEnvironment coordinator (Phase 6)
- **`commands.rs`** - Command system (Phase 5)

### Worker Tasks:
- **`worker_tasks/conversation_compact.rs`** - Conversation compaction task (Phase 10)

### UI Implementation (`crates/chat-cli/src/cli/chat/agent_env_ui/`):
- **`text_ui.rs`** - New TextUi implementation (Phase 7)
- **`ui_utils.rs`** - Shared UI utilities (Phase 5)
- **`structured_io.rs`** - Structured JSON I/O (Phase 9)
- **`web_api.rs`** - Web API stub (Phase 9, optional)

## Summary

**Keep**: 18 files (will modify 8 of them)
**Delete**: 6 files
**Create**: 8 new files

## Rationale

### Why Keep These Files:

1. **Core Architecture**: Worker, Session, WorkerJob, WorkerTask form the foundation and are sound
2. **Context Management**: Context container system works well and doesn't need changes
3. **Model Providers**: Abstraction layer is good, just needs EventBus integration
4. **AgentLoop Task**: Core logic is solid, just needs to publish events instead of calling interface
5. **Input/Signal Handlers**: Reusable utilities that work independently

### Why Delete These Files:

1. **worker_interface.rs**: Replaced by EventBus - no longer need synchronous interface
2. **demo/**: Prototype implementation being replaced by production architecture
3. **text_ui_worker_to_host_interface.rs**: Old UI implementation tied to deprecated interface
4. **prompt_queue.rs**: Will be reimplemented with different pattern in new TextUi

### Why Create New Files:

1. **events.rs, event_bus.rs**: Core of new architecture
2. **agent_environment.rs**: New coordinator layer for UI management
3. **commands.rs**: Formalize command system
4. **text_ui.rs, structured_io.rs**: New UI implementations using EventBus
5. **ui_utils.rs**: Share common utilities across UIs
6. **conversation_compact.rs**: New task type for conversation management

## Migration Notes

- The entry point (`crates/chat-cli/src/cli/chat/mod.rs`) will be heavily modified but not deleted
- All kept files will remain in their current locations
- New files follow the existing directory structure
- No breaking changes to external APIs (CLI arguments, etc.)
