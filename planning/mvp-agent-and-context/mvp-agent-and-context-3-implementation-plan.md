# MVP Agent and Context - Implementation Plan

## Overview

This implementation plan provides a step-by-step checklist for implementing agent configuration and context management in the Agent Environment architecture. The plan is organized into phases that can be executed sequentially, with each task referencing the relevant design document sections.

**Design Document:** `mvp-agent-and-context-2-design.md`

## Implementation Phases

### Phase 1: Core Infrastructure

This phase updates foundational data structures to support agent context and multi-turn conversations.

- [x] **Task 1.1: Update ContextContainer structure** (Design Doc §1)
  - [x] Add `agent_prompt: Arc<Mutex<Option<String>>>` field to ContextContainer struct
  - [x] Add `agent_resources: Arc<Mutex<Option<String>>>` field to ContextContainer struct
  - [x] Add `#[serde(skip, default = "default_agent_prompt")]` attribute to agent_prompt
  - [x] Add `#[serde(skip, default = "default_agent_resources")]` attribute to agent_resources
  - [x] Implement `default_agent_prompt()` function returning `Arc::new(Mutex::new(None))`
  - [x] Implement `default_agent_resources()` function returning `Arc::new(Mutex::new(None))`

- [x] **Task 1.2: Add ContextContainer methods** (Design Doc §1)
  - [x] Implement `set_agent_prompt(&self, prompt: String)` method
  - [x] Implement `get_agent_prompt(&self) -> Option<String>` method
  - [x] Implement `set_agent_resources(&self, resources: String)` method
  - [x] Implement `get_agent_resources(&self) -> Option<String>` method
  - [x] Update `new()` constructor to initialize new fields

- [x] **Task 1.3: Define enhanced ModelRequest structure** (Design Doc §2)
  - [x] Create `ConversationMessage` struct with `role: MessageRole` and `content: String` fields
  - [x] Create `MessageRole` enum with `User` and `Assistant` variants
  - [x] Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` to MessageRole
  - [x] Update `ModelRequest` struct to replace `prompt: String` with `messages: Vec<ConversationMessage>`
  - [x] Add `system_prompt: Option<String>` field to ModelRequest
  - [x] Add `context: Option<String>` field to ModelRequest
  - [x] Keep existing `conversation_id: Option<String>` field

- [x] **Task 1.4: Verify ConversationHistory methods** (Design Doc §1)
  - [x] Confirm `push_input_message(String)` method exists
  - [x] Confirm `push_assistant_message(AssistantMessage)` method exists
  - [x] Confirm `get_entries() -> &[ConversationEntry]` method exists
  - [x] Add any missing methods if needed

### Phase 2: ContextBuilder

This phase creates the ContextBuilder component that converts ContextContainer to ModelRequest.

- [x] **Task 2.1: Create ContextBuilder file and structure** (Design Doc §3)
  - [x] Create new file `crates/chat-cli/src/agent_env/context_builder.rs`
  - [x] Add necessary imports: `ContextContainer`, `ModelRequest`, `ConversationMessage`, `MessageRole`, `AssistantMessage`
  - [x] Define `pub struct ContextBuilder;` (stateless, no fields)

- [x] **Task 2.2: Implement build_request method** (Design Doc §3)
  - [x] Create `pub fn build_request(context_container: &ContextContainer) -> Result<ModelRequest>`
  - [x] Call `Self::build_messages(context_container)?` to get messages array
  - [x] Call `context_container.get_agent_prompt()` to get system_prompt
  - [x] Call `context_container.get_agent_resources()` to get context
  - [x] Construct and return `ModelRequest` with all fields (conversation_id set to None)

- [x] **Task 2.3: Implement build_messages helper** (Design Doc §3)
  - [x] Create `fn build_messages(context_container: &ContextContainer) -> Result<Vec<ConversationMessage>>`
  - [x] Lock conversation_history and get entries
  - [x] Return error if entries is empty: `Err(eyre::eyre!("No messages in conversation history"))`
  - [x] Iterate through entries and extract user messages with `MessageRole::User`
  - [x] Iterate through entries and extract assistant messages with `MessageRole::Assistant`
  - [x] Handle both `AssistantMessage::Response` and `AssistantMessage::ToolUse` variants
  - [x] Return messages vector maintaining conversation order

- [x] **Task 2.4: Add module exports** (Design Doc §3)
  - [x] Add `mod context_builder;` to `crates/chat-cli/src/agent_env/mod.rs`
  - [x] Add `pub use context_builder::ContextBuilder;` to `crates/chat-cli/src/agent_env/mod.rs`

- [ ] **Task 2.5: Write ContextBuilder unit tests** (Design Doc §Testing Strategy)
  - [ ] Test `build_request` with full context (history + prompt + resources)
  - [ ] Test `build_request` with empty history (should return error)
  - [ ] Test `build_request` with no agent context (should work with just history)
  - [ ] Test `build_messages` with mixed entries (user only, assistant only, both)
  - [ ] Test handling of both AssistantMessage variants (Response and ToolUse)

### Phase 3: WorkerBuilder

This phase creates the WorkerBuilder component that encapsulates worker creation with agent configuration.

- [x] **Task 3.1: Create WorkerBuilder file and structure** (Design Doc §4)
  - [x] Create new file `crates/chat-cli/src/agent_env/worker_builder.rs`
  - [x] Add necessary imports: `Session`, `Worker`, `Agent`, `Platform`, `Os`, `Arc`, `Result`
  - [x] Define `pub struct WorkerBuilder` with fields: `agent_name: Option<String>`, `platform: Platform`, `model: Option<String>`, `initial_input: Option<String>`

- [x] **Task 3.2: Implement WorkerBuilder constructor and setters** (Design Doc §4)
  - [x] Implement `pub fn new() -> Self` returning WorkerBuilder with default values
  - [x] Implement `pub fn agent(mut self, agent_name: Option<String>) -> Self` setter
  - [x] Implement `pub fn platform(mut self, platform: Platform) -> Self` setter
  - [x] Implement `pub fn model(mut self, model: Option<String>) -> Self` setter
  - [x] Implement `pub fn initial_input(mut self, input: Option<String>) -> Self` setter

- [x] **Task 3.3: Implement agent loading in build method** (Design Doc §4)
  - [x] Create `pub async fn build(self, session: Arc<Session>, os: &Os) -> Result<Arc<Worker>>`
  - [x] Load agent config: if `agent_name` is Some, call `Agent::get_agent_by_name(os, agent_name).await?`
  - [x] If `agent_name` is None, use `Agent::default()`
  - [x] Store loaded agent for use in subsequent steps

- [x] **Task 3.4: Implement resource loading helper** (Design Doc §4)
  - [x] Create `async fn load_resources(resources: &[ResourcePath], os: &Os) -> Result<String>`
  - [x] Import `glob::glob` for pattern matching
  - [x] Iterate through resources, skip non-file:// URLs
  - [x] Strip "file://" prefix from each resource path
  - [x] For paths with '*', use `glob(path)?` to expand pattern
  - [x] For each matched file, read content with `os.fs.read_to_string(&file_path).await`
  - [x] Format as `\n--- {path} ---\n{content}\n` and append to result string
  - [x] For single files (no glob), read directly with same formatting
  - [x] Gracefully handle errors (missing files, permission errors) by continuing to next resource
  - [x] Return concatenated resources string

- [x] **Task 3.5: Complete build method implementation** (Design Doc §4)
  - [x] Call `Self::load_resources(&agent.resources, os).await?` to get resources_content
  - [x] Create worker through Session: `session.build_worker("main".to_string())`
  - [x] If agent.prompt is Some, call `worker.context_container.set_agent_prompt(prompt.clone())`
  - [x] If resources_content is not empty, call `worker.context_container.set_agent_resources(resources_content)`
  - [x] If initial_input is Some, add to conversation: `worker.context_container.conversation_history.lock().unwrap().push_input_message(input.clone())`
  - [x] Return worker wrapped in Ok

- [x] **Task 3.6: Add module exports** (Design Doc §4)
  - [x] Add `mod worker_builder;` to `crates/chat-cli/src/agent_env/mod.rs`
  - [x] Add `pub use worker_builder::WorkerBuilder;` to `crates/chat-cli/src/agent_env/mod.rs`

- [ ] **Task 3.7: Write WorkerBuilder unit tests** (Design Doc §Testing Strategy)
  - [ ] Test `build` with default agent (no agent_name provided)
  - [ ] Test `build` with specific agent config
  - [ ] Test `load_resources` with single file:// URL
  - [ ] Test `load_resources` with glob pattern (file://**/*.md)
  - [ ] Test graceful handling of missing files
  - [ ] Test graceful handling of invalid glob patterns
  - [ ] Test initial_input is added to conversation history

### Phase 4: ModelProvider Updates

This phase updates both ModelProvider implementations to handle the new ModelRequest structure.

- [x] **Task 4.1: Update BedrockConverseStreamModelProvider - system blocks** (Design Doc §5)
  - [x] In `request()` method, create `Vec<SystemContentBlock>` for system content
  - [x] If `request.system_prompt` is Some, push `SystemContentBlock::Text(prompt)` to system_blocks
  - [x] If `request.context` is Some, push `SystemContentBlock::Text(context)` to system_blocks

- [x] **Task 4.2: Update BedrockConverseStreamModelProvider - messages conversion** (Design Doc §5)
  - [x] Convert `request.messages` to `Vec<Message>` using iterator and map
  - [x] For each message, convert `MessageRole::User` to `ConversationRole::User`
  - [x] For each message, convert `MessageRole::Assistant` to `ConversationRole::Assistant`
  - [x] Build each Message with role and `ContentBlock::Text(msg.content.clone())`

- [x] **Task 4.3: Update BedrockConverseStreamModelProvider - request building** (Design Doc §5)
  - [x] Create request builder: `self.client.converse_stream().model_id(&self.model_id)`
  - [x] Call `.set_messages(Some(messages))` with converted messages
  - [x] If system_blocks is not empty, call `.set_system(Some(system_blocks))`
  - [x] Verify streaming and cancellation logic remains unchanged

- [x] **Task 4.4: Update CodeWhispererModelProvider - content concatenation** (Design Doc §6)
  - [x] In `request()` method, create mutable `String` for content
  - [x] If `request.system_prompt` is Some, append prompt + "\n\n"
  - [x] If `request.context` is Some, append context + "\n\n"
  - [x] Iterate through `request.messages` and format each as "User: {content}\n\n" or "Assistant: {content}\n\n" based on role
  - [x] Concatenate all formatted messages into content string

- [x] **Task 4.5: Update CodeWhispererModelProvider - request building** (Design Doc §6)
  - [x] Build `UserInputMessage` with concatenated content
  - [x] Build `ConversationState` with UserInputMessage and conversation_id
  - [x] Verify streaming and event handling logic remains unchanged

- [ ] **Task 4.6: Update ModelProvider tests** (Design Doc §Testing Strategy)
  - [ ] Update existing Bedrock tests to use new ModelRequest structure
  - [ ] Update existing CodeWhisperer tests to use new ModelRequest structure
  - [ ] Add test for Bedrock with system_prompt and context
  - [ ] Add test for CodeWhisperer with system_prompt and context
  - [ ] Add test for multi-message conversation (3+ messages)

### Phase 5: AgentLoop Integration

This phase integrates ContextBuilder into the AgentLoop task.

- [x] **Task 5.1: Update AgentLoop::query_llm method** (Design Doc §7)
  - [x] In `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`, locate `query_llm()` method
  - [x] Remove existing code that extracts last message from conversation history
  - [x] Replace with single line: `let request = crate::agent_env::ContextBuilder::build_request(&self.worker.context_container)?;`
  - [x] Verify that `self.worker.set_state(WorkerStates::Requesting)` and subsequent provider call remain unchanged
  - [x] Verify error handling and cancellation logic remain unchanged

- [x] **Task 5.2: Verify response accumulation** (Design Doc §7)
  - [x] Confirm that `run()` method already calls `push_assistant_message()` after receiving response
  - [x] Verify that both Response and ToolUse assistant messages are properly added to history
  - [x] No changes needed if accumulation already works correctly

### Phase 6: Entry Point Integration

This phase integrates WorkerBuilder into the ChatArgs entry point.

- [x] **Task 6.1: Update ChatArgs::execute method** (Design Doc §8)
  - [x] In `crates/chat-cli/src/cli/chat/mod.rs`, locate worker creation code
  - [x] Remove existing `session.build_worker("main".to_string())` call
  - [x] Remove manual initial_input addition to conversation history
  - [x] Replace with WorkerBuilder usage:
    ```rust
    let main_worker = WorkerBuilder::new()
        .agent(self.agent.clone())
        .platform(platform)
        .model(self.model.clone())
        .initial_input(self.input.clone())
        .build(session.clone(), os)
        .await?;
    ```
  - [x] Verify that UI creation and AgentEnvironment setup remain unchanged

- [x] **Task 6.2: Verify error handling** (Design Doc §8)
  - [x] Confirm that agent loading errors propagate correctly with `?` operator
  - [x] Verify that error messages are clear for missing agent configs
  - [x] Test that invalid agent names produce helpful error messages

### Phase 7: Testing and Validation

This phase ensures all components work correctly through comprehensive testing.

- [ ] **Task 7.1: Integration test - Multi-turn conversation** (Design Doc §Testing Strategy)
  - [ ] Create test that sends initial message: "What is 2+2?"
  - [ ] Verify response contains "4"
  - [ ] Send follow-up message: "What about the previous number plus 1?"
  - [ ] Verify response references "4" and provides "5"
  - [ ] Confirm full conversation history is maintained

- [ ] **Task 7.2: Integration test - Agent with prompt** (Design Doc §Testing Strategy)
  - [ ] Create test agent config with custom prompt: "You are a helpful assistant who always responds in haiku format"
  - [ ] Save to temporary agent config file
  - [ ] Run chat with `--agent test-agent --no-interactive "Tell me about the weather"`
  - [ ] Verify response follows haiku format (3 lines, 5-7-5 syllable pattern)
  - [ ] Clean up test agent config

- [ ] **Task 7.3: Integration test - Agent with single file resource** (Design Doc §Testing Strategy)
  - [ ] Create temporary resource file: `/tmp/test-guidelines.txt` with content "Project uses Rust 2021 edition"
  - [ ] Create test agent config with resource: `["file:///tmp/test-guidelines.txt"]`
  - [ ] Run chat with `--agent rust-agent --no-interactive "What Rust edition does this project use?"`
  - [ ] Verify response references "Rust 2021 edition"
  - [ ] Clean up test files

- [ ] **Task 7.4: Integration test - Agent with glob pattern resources** (Design Doc §Testing Strategy)
  - [ ] Create multiple temporary files: `/tmp/rule-1.txt` ("Rule 1"), `/tmp/rule-2.txt` ("Rule 2")
  - [ ] Create test agent config with glob resource: `["file:///tmp/rule-*.txt"]`
  - [ ] Run chat with `--agent glob-agent --no-interactive "List all rules"`
  - [ ] Verify response mentions both "Rule 1" and "Rule 2"
  - [ ] Clean up test files

- [ ] **Task 7.5: Integration test - Platform selection** (Design Doc §Testing Strategy)
  - [ ] Run chat with `--platform bedrock --no-interactive "Hello"`
  - [ ] Verify BedrockConverseStreamModelProvider is used (check logs or events)
  - [ ] Run chat with `--platform codewhisperer --no-interactive "Hello"`
  - [ ] Verify CodeWhispererModelProvider is used
  - [ ] Confirm both platforms handle multi-turn conversations correctly

- [ ] **Task 7.6: Manual test - Interactive multi-turn** (Design Doc §Testing Strategy)
  - [ ] Start interactive chat: `q chat`
  - [ ] Send message: "My name is Alice"
  - [ ] Send follow-up: "What is my name?"
  - [ ] Verify agent responds with "Alice"
  - [ ] Test with multiple exchanges (5+ turns)

- [ ] **Task 7.7: Manual test - Agent switching** (Design Doc §Testing Strategy)
  - [ ] Create multiple test agent configs with different prompts
  - [ ] Run chat with `--agent agent1` and verify behavior
  - [ ] Run chat with `--agent agent2` and verify different behavior
  - [ ] Verify agent-specific resources are loaded correctly

- [ ] **Task 7.8: Error handling test - Missing agent** (Design Doc §Testing Strategy)
  - [ ] Run chat with `--agent nonexistent-agent`
  - [ ] Verify clear error message is displayed
  - [ ] Confirm application exits gracefully without panic

- [ ] **Task 7.9: Error handling test - Missing resource files** (Design Doc §Testing Strategy)
  - [ ] Create agent config with resource pointing to non-existent file
  - [ ] Run chat with this agent
  - [ ] Verify worker creation succeeds (graceful handling)
  - [ ] Confirm missing file doesn't cause failure

- [ ] **Task 7.10: Error handling test - Invalid glob pattern** (Design Doc §Testing Strategy)
  - [ ] Create agent config with malformed glob pattern
  - [ ] Run chat with this agent
  - [ ] Verify worker creation succeeds (graceful handling)
  - [ ] Confirm invalid pattern doesn't cause failure

- [ ] **Task 7.11: Regression test - Existing functionality** (Design Doc §Testing Strategy)
  - [ ] Run existing test suite: `cargo test`
  - [ ] Verify all existing tests pass
  - [ ] Confirm no regressions in EventBus, Session, Worker, or UI components
  - [ ] Test TextUi and StructuredIO still work correctly


## Implementation Summary

### Task Count by Phase
- **Phase 1: Core Infrastructure** - 4 tasks
- **Phase 2: ContextBuilder** - 5 tasks
- **Phase 3: WorkerBuilder** - 7 tasks
- **Phase 4: ModelProvider Updates** - 6 tasks
- **Phase 5: AgentLoop Integration** - 2 tasks
- **Phase 6: Entry Point Integration** - 2 tasks
- **Phase 7: Testing and Validation** - 11 tasks

**Total: 37 tasks**

### Critical Path
1. Phase 1 must complete before Phase 2 and Phase 3 (ModelRequest structure needed)
2. Phase 2 and Phase 3 can be done in parallel
3. Phase 4 depends on Phase 1 (ModelRequest structure)
4. Phase 5 depends on Phase 2 (ContextBuilder)
5. Phase 6 depends on Phase 3 (WorkerBuilder)
6. Phase 7 depends on all previous phases

### Estimated Effort
- **Phase 1**: 2 hours
- **Phase 2**: 2 hours
- **Phase 3**: 4 hours
- **Phase 4**: 4 hours
- **Phase 5**: 2 hours
- **Phase 6**: 1 hour
- **Phase 7**: 7 hours (3h unit tests + 2h integration + 2h manual)

**Total: ~22 hours (~3 days)**

## Dependencies

### External Crates
- `glob` - For glob pattern expansion (likely already in Cargo.toml)
- `eyre` - For error handling (already in use)
- `tokio` - For async runtime (already in use)

### Internal Dependencies
- Existing Agent config system (`crates/chat-cli/src/cli/agent/mod.rs`)
- Existing ConversationHistory implementation
- Existing ModelProvider trait and implementations
- Session and Worker infrastructure
- EventBus architecture

### Blocked By
None - can be implemented immediately

### Blocks
- **mvp-tools-basic** - Tools will need access to agent config for tool permissions
- **Conversation persistence** - Future --resume flag will build on this foundation
- **Context management commands** - /context command enhancements

## Important Notes

### Breaking Changes
- **ModelRequest structure** changes from simple `prompt: String` to structured messages array
- All code using ModelRequest must be updated simultaneously:
  - `model_provider.rs` - ModelRequest definition
  - `bedrock_converse_stream.rs` - request() method
  - `codewhisperer.rs` - request() method
  - `agent_loop.rs` - query_llm() method
  - Any tests using ModelRequest

### Error Handling Strategy
- **Agent loading errors** → Fail worker creation with clear error message
- **Missing resource files** → Silently skip, continue with other resources
- **Invalid glob patterns** → Silently skip, continue with other resources
- **Permission errors** → Silently skip, continue with other resources
- **Empty conversation history** → Return error from ContextBuilder

### Resource Format
Resources are concatenated with file path headers:
```
--- /path/to/file1.md ---
<file1 content>

--- /path/to/file2.rs ---
<file2 content>
```

### Module Structure
New files:
- `crates/chat-cli/src/agent_env/context_builder.rs`
- `crates/chat-cli/src/agent_env/worker_builder.rs`

Modified files:
- `crates/chat-cli/src/agent_env/context_container/context_container.rs`
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`
- `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`
- `crates/chat-cli/src/cli/chat/mod.rs`
- `crates/chat-cli/src/agent_env/mod.rs`

## Success Criteria

All tasks must be completed and the following criteria met:

1. ✅ Multi-turn conversations maintain full context
2. ✅ Agent configs load successfully with --agent flag
3. ✅ Agent prompts are sent to LLM
4. ✅ Agent resources are loaded and sent to LLM
5. ✅ Glob patterns expand correctly
6. ✅ Both Bedrock and CodeWhisperer providers work with new ModelRequest
7. ✅ WorkerBuilder encapsulates all worker creation logic
8. ✅ ContextBuilder cleanly separates request construction
9. ✅ Error handling is robust (missing files, invalid agents)
10. ✅ No regressions in existing functionality
11. ✅ All unit tests pass
12. ✅ All integration tests pass
13. ✅ Manual testing confirms expected behavior

## Next Steps After Completion

1. Update primary plan sheet to mark workflow as "Implemented"
2. Create implementation log document
3. Document any deviations from plan
4. Identify lessons learned
5. Prepare for next workflow (mvp-tools-basic)
