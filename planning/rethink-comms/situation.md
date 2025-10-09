# Summary of the environment and product statement

## What we have

We maintain a pool of AI assistants that can do things in parallel.

Our main elemets of the architecture are:

- `Worker` - a basic unit, a combination of all data and state that's needed to do the assistants' tasks
    - Code: `crates/chat-cli/src/agent_env/worker.rs`
    - static data: conversation history, resource/context files (only used to form the request to LLM)
        - Context container: `crates/chat-cli/src/agent_env/context_container/context_container.rs`
        - Conversation history: `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`
        - Conversation entry: `crates/chat-cli/src/agent_env/context_container/conversation_entry.rs`
    - state: tools trust state and auto-approval rules (used by code to execute the tools)
- `Task` - an implementation of some task that can be executed on `Worker` data. Basic assistant/agent loop, or 'compact conversation history', or anything else.
    - Trait: `crates/chat-cli/src/agent_env/worker_task.rs`
    - AgentLoop implementation: `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`
    - Demo ProtoLoop: `crates/chat-cli/src/agent_env/demo/proto_loop.rs`
    - Tasks are supposed to be the processes that involve LLM calls, and relatively long running operations
- `Job` - a runing instance of a `Task`, associated with a `Worker`
    - Code: `crates/chat-cli/src/agent_env/worker_job.rs`
    - Continuations: `crates/chat-cli/src/agent_env/worker_job_continuations.rs`
- `Session` - a master object that maintains the list of Workers and Jobs. Provides API to create/drop workers, launch or cancel jobs.
    - Code: `crates/chat-cli/src/agent_env/session.rs`

## What we want

We are making a CLI app, in the end. So we need to give `Session` some kind of UI.

I'm looking for the way to organize main application loop with enough flexibility to be able to modify and extend various UIs in the future.

At P0 I expect basic TUI that would print the output from the tasks of ONE worker, and handle prompting. I want an ability to easily swap it in the future with UI that would print the output in a different format; or plug-in a web API with REST endpoints and websockets for the output; or swap user-facing UI with full blown fancy all-screen TUI with widgets and panes.

**Current demo implementation:**
- Entry point: `crates/chat-cli/src/cli/chat/mod.rs` (lines 1-309, `ChatArgs::execute()`)
- UI main loop: `crates/chat-cli/src/cli/chat/agent_env_ui/mod.rs` (`AgentEnvTextUi`)
- Worker interface: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui_worker_to_host_interface.rs`
- Prompt queue: `crates/chat-cli/src/cli/chat/agent_env_ui/prompt_queue.rs`
- Input handler: `crates/chat-cli/src/cli/chat/agent_env_ui/input_handler.rs`
- Ctrl+C handler: `crates/chat-cli/src/cli/chat/agent_env_ui/ctrl_c_handler.rs`

## What we have - with "events"

Note: I use "events" in quotes because Rust doesn't have explicit single concept for events, similar to C# or some other languages, so I want to just show off the nature of that thing here.

Our elements provide following "events", that we want to react in UI:

- `Session` (`crates/chat-cli/src/agent_env/session.rs`)
    - `Worker_Created(worker_id)`
    - `Worker_Deleted(worker_id)`
    - `Job_Stated(worker_id, job_id, job_type)`
    - `Job_Completed(worker_id, job_id, job_type, completion_type, type_specific_completion_type)`
- `Worker` (`crates/chat-cli/src/agent_env/worker.rs`)
    - `Worker_State_Changed(InActiveJob|Idle(Nominal|LastJobFailed|LastJobCancelled))`
    - States defined in `WorkerStates` enum: Inactive, Working, Requesting, Receiving, Waiting, UsingTool, InactiveFailed
- `Job` (`crates/chat-cli/src/agent_env/worker_job.rs`)
    - Job state tracking via `JobState` enum: Active, Completed, Cancelled, Failed
    - Continuations system: `crates/chat-cli/src/agent_env/worker_job_continuations.rs`
- `Task` (trait: `crates/chat-cli/src/agent_env/worker_task.rs`)
    - Tasks are expected to have their own set of "events", specific to each task ..well, task.
        - AgentLoop (`crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`) will have following events:
            - `AgentLoop_Status_Changed(Working|Requesting|Receiving|Waiting_ToolApproval|UsingTool)`
                - note: `Waiting_ToolApproval` is optional, we can have it implemented differently
    - All tasks are expected to have streaming-style output. Think of Bedrock Converse Stream API model.
        - Model provider abstraction: `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`
        - Bedrock implementation: `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Current communication mechanism:**
- `WorkerToHostInterface` trait (`crates/chat-cli/src/agent_env/worker_interface.rs`) defines:
    - `worker_state_change(worker_id, new_state)` - Worker state transitions
    - `response_chunk_received(worker_id, chunk)` - Streaming LLM response chunks
    - `get_tool_confirmation(worker_id, request, cancellation_token)` - Tool approval requests. NOTE: This is nice-to-have, but not required.
- Implementation: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui_worker_to_host_interface.rs`

## What we want - the ask

The goal is to come up with a Rust-native architecture that would allow us maintain clean 'core app loop', with an ability to easily swap user-facing UI implementations or their components, as well as combine them ("simple" TUI + web API).
