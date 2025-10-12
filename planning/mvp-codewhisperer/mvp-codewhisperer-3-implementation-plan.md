# MVP CodeWhisperer - Implementation Plan

## Overview

This implementation plan provides a step-by-step checklist for integrating CodeWhisperer as an alternative model provider in the agent_env architecture. The plan is organized into phases with atomic, executable tasks.

**Design Reference:** `mvp-codewhisperer-2-design.md`

**Estimated Total Effort:** 8-12 hours

---

## Implementation Phases

### Phase 1: Core Infrastructure

**Goal:** Prepare the ModelProvider trait and supporting infrastructure for conversation ID tracking.

**Estimated Effort:** 1.5-2 hours

---

- [ ] **Task 1.1: Add conversation_id field to ModelRequest** (Design Doc §4.1, §5.1)
  - Open `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`
  - Add `pub conversation_id: Option<String>` field to `ModelRequest` struct
  - This enables conversation tracking across requests for CodeWhisperer
  - **Validation:** Code compiles (may have errors in dependent code, that's expected)

- [ ] **Task 1.2: Update BedrockConverseStreamModelProvider to handle new field** (Design Doc §4.1)
  - Open `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`
  - Update `request()` method signature to accept new `ModelRequest` structure
  - Bedrock doesn't use conversation_id, so simply ignore the field (no functional changes needed)
  - **Validation:** Bedrock provider compiles without errors

- [ ] **Task 1.3: Add streaming_client() accessor to ApiClient** (Design Doc §4.5, §5.4)
  - Open `crates/chat-cli/src/api_client/mod.rs`
  - Add public method: `pub fn streaming_client(&self) -> Option<CodewhispererStreamingClient>`
  - Method should return `self.streaming_client.clone()`
  - Add doc comment: "Get the CodeWhisperer streaming client (bearer token auth)"
  - **Validation:** Method compiles and returns correct type

- [ ] **Task 1.4: Update AgentLoop to extract conversation_id from ContextContainer** (Design Doc §4.1)
  - Open `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`
  - Locate where `ModelRequest` is constructed in `query_llm()` method
  - Add code to extract conversation_id: `let conversation_id = self.worker.context_container.get_conversation_id().map(|s| s.to_string());`
  - Update ModelRequest construction to include: `conversation_id`
  - **Note:** If `get_conversation_id()` doesn't exist, use `None` for now and document as future work
  - **Validation:** AgentLoop compiles, ModelRequest includes conversation_id field

---

### Phase 2: CodeWhisperer Provider Implementation

**Goal:** Implement CodeWhispererModelProvider with streaming and tool use support.

**Estimated Effort:** 4-5 hours

**Prerequisites:** Phase 1 complete

---

- [ ] **Task 2.1: Create codewhisperer.rs file** (Design Doc §5.1)
  - Create new file: `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`
  - Add module header with imports:
    - `use amzn_codewhisperer_streaming_client::Client as CodeWhispererStreamingClient;`
    - `use amzn_codewhisperer_streaming_client::types::*;`
    - `use std::sync::Arc;`
    - `use async_trait::async_trait;`
    - `use eyre::Result;`
    - `use tokio_util::sync::CancellationToken;`
  - **Validation:** File compiles with imports

- [ ] **Task 2.2: Define CodeWhispererModelProvider struct** (Design Doc §5.1)
  - Add struct definition: `pub struct CodeWhispererModelProvider { client: Arc<CodeWhispererStreamingClient> }`
  - Add constructor: `pub fn new(client: CodeWhispererStreamingClient) -> Self`
  - Constructor should wrap client in Arc
  - **Validation:** Struct compiles, constructor works

- [ ] **Task 2.3: Implement build_send_message_input() helper** (Design Doc §4.1, §5.3)
  - Add private method: `fn build_send_message_input(&self, request: ModelRequest) -> Result<SendMessageInput>`
  - Create `UserInputMessage` from `request.prompt`
  - Extract or generate conversation_id: `request.conversation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())`
  - Create `ConversationState` with:
    - `current_message`: UserInputMessage
    - `chat_trigger_type`: ChatTriggerType::Manual
    - `conversation_id`: from above
    - Leave `history` empty (MVP limitation)
  - Create and return `SendMessageInput` with conversation_state
  - **Validation:** Method compiles, returns correct type

- [ ] **Task 2.4: Implement process_stream_event() helper** (Design Doc §4.2, §5.3)
  - Add private method: `fn process_stream_event(&self, event: ChatResponseStream, accumulated_content: &mut String, tool_requests: &mut Vec<ToolRequest>, when_received: &dyn Fn(ModelResponseChunk)) -> Result<()>`
  - Handle `ChatResponseStream::AssistantResponseEvent`:
    - Append `evt.content` to `accumulated_content`
    - Call `when_received(ModelResponseChunk::AssistantMessage(evt.content))`
  - Handle `ChatResponseStream::ToolUseEvent`:
    - Extract tool_use from event
    - Serialize `tool_use.input` to JSON string using `serde_json::to_string()`
    - Add to `tool_requests` vector
    - Call `when_received(ModelResponseChunk::ToolUseRequest { tool_name, parameters })`
  - Ignore all other event types (no-op)
  - **Validation:** Method compiles, handles both event types correctly

- [ ] **Task 2.5: Implement ModelProvider trait - request() method skeleton** (Design Doc §5.2)
  - Add `#[async_trait]` attribute above impl block
  - Implement `async fn request(...)` signature matching trait
  - Add TODO comments for main steps:
    1. Build SendMessageInput
    2. Send request with cancellation
    3. Signal receiving started
    4. Process stream
    5. Return ModelResponse
  - Return placeholder `Ok(ModelResponse { content: String::new(), tool_requests: vec![] })`
  - **Validation:** Trait implementation compiles

- [ ] **Task 2.6: Implement request() - Build and send request** (Design Doc §5.2, §4.7)
  - Replace TODO #1: Call `self.build_send_message_input(request)?`
  - Replace TODO #2: Use `tokio::select!` to send request with cancellation:
    - Success branch: `self.client.send_message().set_input(Some(input)).send()`
    - Cancellation branch: `cancellation_token.cancelled()`
  - Add error handling with context-specific messages (auth, rate limit, network)
  - **Validation:** Request sending works, cancellation is supported

- [ ] **Task 2.7: Implement request() - Stream processing loop** (Design Doc §5.2)
  - Replace TODO #3: Call `when_receiving_begin()`
  - Replace TODO #4: Create stream processing loop:
    - Initialize `accumulated_content` and `tool_requests`
    - Get `chat_response_stream` from response
    - Loop with `tokio::select!` for stream events and cancellation
    - Call `process_stream_event()` for each event
    - Break on stream end (None)
    - Handle stream errors
  - Replace TODO #5: Return `ModelResponse` with accumulated data
  - **Validation:** Full streaming works, events are processed correctly

- [ ] **Task 2.8: Export CodeWhispererModelProvider from module** (Design Doc §5.4)
  - Open `crates/chat-cli/src/agent_env/model_providers/mod.rs`
  - Add: `pub mod codewhisperer;`
  - Add: `pub use codewhisperer::CodeWhispererModelProvider;`
  - **Validation:** Module exports correctly, can be imported elsewhere

- [ ] **Task 2.9: Add uuid dependency to Cargo.toml** (Design Doc §4.1)
  - Open `crates/chat-cli/Cargo.toml`
  - Add to dependencies: `uuid = { version = "1.0", features = ["v4"] }`
  - This is needed for fallback conversation_id generation
  - **Validation:** Cargo build succeeds with new dependency

---

### Phase 3: Platform Selection & Integration

**Goal:** Add platform selection mechanism and integrate CodeWhisperer into ChatArgs.

**Estimated Effort:** 2-3 hours

**Prerequisites:** Phase 2 complete

---

- [ ] **Task 3.1: Define Platform enum** (Design Doc §4.6, §5.4)
  - Open `crates/chat-cli/src/cli/chat/mod.rs`
  - Add import: `use clap::ValueEnum;`
  - Define enum before ChatArgs:
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
    pub enum Platform {
        #[value(name = "bedrock")]
        Bedrock,
        #[value(name = "codewhisperer")]
        CodeWhisperer,
    }
    ```
  - Implement `Default` trait to return `Platform::CodeWhisperer`
  - **Validation:** Enum compiles, default is CodeWhisperer

- [ ] **Task 3.2: Add platform field to ChatArgs** (Design Doc §4.6, §5.4)
  - In ChatArgs struct, add field: `#[arg(long = "platform", value_enum)] pub platform: Option<Platform>`
  - Add doc comment: "Platform to use for LLM"
  - **Validation:** Field compiles, clap accepts --platform flag

- [ ] **Task 3.3: Extract create_model_provider() method** (Design Doc §4.6, §5.4)
  - Create new private async method in ChatArgs impl:
    ```rust
    async fn create_model_provider(
        platform: Platform,
        os: &mut Os,
    ) -> Result<Arc<dyn ModelProvider>>
    ```
  - Move Bedrock client creation logic from execute() into this method's Bedrock branch
  - Add CodeWhisperer branch:
    - Create ApiClient using `ApiClient::new(&os.env, &os.fs, &mut os.database, None).await?`
    - Extract streaming client: `api_client.streaming_client().ok_or_else(...)?`
    - Create CodeWhispererModelProvider: `Arc::new(CodeWhispererModelProvider::new(streaming_client))`
  - Add helpful error message for missing streaming client
  - **Validation:** Method compiles, returns correct type for both platforms

- [ ] **Task 3.4: Update ChatArgs::execute() to use platform selection** (Design Doc §4.6, §5.4)
  - At start of execute(), add: `let platform = self.platform.unwrap_or_default();`
  - Replace existing Bedrock client creation with: `let model_provider = Self::create_model_provider(platform, os).await?;`
  - Remove old Bedrock-specific code that was moved to create_model_provider()
  - **Validation:** execute() is cleaner, platform selection works

- [ ] **Task 3.5: Add necessary imports to mod.rs** (Design Doc §5.4)
  - Add: `use crate::agent_env::model_providers::CodeWhispererModelProvider;`
  - Add: `use crate::api_client::ApiClient;`
  - Add: `use eyre::Context;` (for error context)
  - **Validation:** All imports resolve correctly

- [ ] **Task 3.6: Verify compilation of full integration** (Design Doc §5.4)
  - Run `cargo check --package chat_cli`
  - Fix any compilation errors
  - Ensure both Bedrock and CodeWhisperer paths compile
  - **Validation:** No compilation errors, both platforms are integrated

---

### Phase 4: Testing & Validation

**Goal:** Verify CodeWhisperer integration works correctly through manual and automated testing.

**Estimated Effort:** 2-3 hours

**Prerequisites:** Phase 3 complete

---

#### Unit Tests

- [ ] **Task 4.1: Test build_send_message_input() with conversation_id** (Design Doc §8)
  - Add test in `codewhisperer.rs`: `#[test] fn test_build_send_message_input_with_conversation_id()`
  - Create ModelRequest with conversation_id: `Some("test-conv-123".to_string())`
  - Call `build_send_message_input()`
  - Assert conversation_state.conversation_id matches input
  - Assert chat_trigger_type is Manual
  - **Validation:** Test passes

- [ ] **Task 4.2: Test build_send_message_input() without conversation_id** (Design Doc §8)
  - Add test: `#[test] fn test_build_send_message_input_generates_fallback_id()`
  - Create ModelRequest with conversation_id: `None`
  - Call `build_send_message_input()`
  - Assert conversation_state.conversation_id is not empty (UUID was generated)
  - Assert it's a valid UUID format
  - **Validation:** Test passes, fallback generation works

- [ ] **Task 4.3: Test process_stream_event() for AssistantResponseEvent** (Design Doc §8)
  - Add test: `#[test] fn test_process_assistant_response_event()`
  - Create mock AssistantResponseEvent with content: "Hello world"
  - Call `process_stream_event()`
  - Assert accumulated_content contains "Hello world"
  - Assert when_received was called with AssistantMessage chunk
  - **Validation:** Test passes, text events are processed

- [ ] **Task 4.4: Test process_stream_event() for ToolUseEvent** (Design Doc §8)
  - Add test: `#[test] fn test_process_tool_use_event()`
  - Create mock ToolUseEvent with tool_name and input Document
  - Call `process_stream_event()`
  - Assert tool_requests contains the tool
  - Assert parameters are valid JSON
  - Assert when_received was called with ToolUseRequest chunk
  - **Validation:** Test passes, tool events are processed

#### Integration Tests

- [ ] **Task 4.5: Test basic conversation with CodeWhisperer** (Design Doc §8)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer --no-interactive "What is 2+2?"`
  - Verify streaming response appears
  - Verify answer is displayed
  - Verify no errors occur
  - **Validation:** Basic conversation works end-to-end

- [ ] **Task 4.6: Test basic conversation with Bedrock** (Design Doc §8)
  - Run: `cargo run --bin chat_cli -- chat --platform=bedrock --no-interactive "What is 2+2?"`
  - Verify Bedrock still works after changes
  - Verify response format is correct
  - **Validation:** Bedrock compatibility maintained

- [ ] **Task 4.7: Test default platform is CodeWhisperer** (Design Doc §4.6)
  - Run: `cargo run --bin chat_cli -- chat --no-interactive "Hello"`
  - Verify CodeWhisperer is used (not Bedrock)
  - Check logs or output for platform indication
  - **Validation:** Default platform is CodeWhisperer

- [ ] **Task 4.8: Test streaming output display** (Design Doc §8)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "Write a short story about a robot"`
  - Observe output appearing incrementally (not all at once)
  - Verify smooth streaming experience
  - **Validation:** Streaming works correctly

- [ ] **Task 4.9: Test cancellation with Ctrl+C** (Design Doc §4.7, §8)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "Write a very long story"`
  - Press Ctrl+C during response
  - Verify graceful cancellation (no panic)
  - Verify appropriate error message
  - **Validation:** Cancellation works correctly

- [ ] **Task 4.10: Test authentication error handling** (Design Doc §4.7, §8)
  - Temporarily invalidate auth (rename token file or logout)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "Hello"`
  - Verify clear error message about authentication
  - Verify suggestion to run 'q login'
  - Restore auth
  - **Validation:** Auth errors are user-friendly

- [ ] **Task 4.11: Test tool use request display** (Design Doc §8)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "What files are in the current directory?"`
  - If CodeWhisperer supports tool use, verify tool request is displayed
  - If not supported, verify graceful handling
  - **Note:** Tool execution not implemented yet, just display
  - **Validation:** Tool use requests are handled appropriately

- [ ] **Task 4.12: Test platform switching** (Design Doc §8)
  - Run multiple commands alternating platforms:
    - `cargo run --bin chat_cli -- chat --platform=bedrock --no-interactive "Hello from Bedrock"`
    - `cargo run --bin chat_cli -- chat --platform=codewhisperer --no-interactive "Hello from CodeWhisperer"`
  - Verify both work correctly
  - Verify no interference between platforms
  - **Validation:** Platform switching works seamlessly

#### Error Scenarios

- [ ] **Task 4.13: Test with missing streaming client** (Design Doc §4.5)
  - Set environment variable: `export AMAZON_Q_SIGV4=true` (forces QDeveloper client)
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "Hello"`
  - Verify clear error message about streaming client unavailability
  - Unset variable: `unset AMAZON_Q_SIGV4`
  - **Validation:** Missing client error is clear and actionable

- [ ] **Task 4.14: Test with network errors** (Design Doc §4.7)
  - Disconnect network or use invalid endpoint
  - Run: `cargo run --bin chat_cli -- chat --platform=codewhisperer "Hello"`
  - Verify network error is caught and reported
  - Verify no panic or crash
  - **Validation:** Network errors are handled gracefully

---

## Summary

### Task Count by Phase

- **Phase 1: Core Infrastructure** - 4 tasks (1.5-2 hours)
- **Phase 2: CodeWhisperer Provider Implementation** - 9 tasks (4-5 hours)
- **Phase 3: Platform Selection & Integration** - 6 tasks (2-3 hours)
- **Phase 4: Testing & Validation** - 14 tasks (2-3 hours)

**Total: 33 tasks, 10-13 hours estimated**

### Files Modified

**New Files:**
- `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs` - CodeWhisperer provider implementation

**Modified Files:**
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - Add conversation_id to ModelRequest
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Handle new ModelRequest field
- `crates/chat-cli/src/agent_env/model_providers/mod.rs` - Export CodeWhispererModelProvider
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - Extract conversation_id from ContextContainer
- `crates/chat-cli/src/api_client/mod.rs` - Add streaming_client() accessor
- `crates/chat-cli/src/cli/chat/mod.rs` - Platform enum, ChatArgs updates, create_model_provider()
- `crates/chat-cli/Cargo.toml` - Add uuid dependency

### Key Dependencies

**External Dependencies:**
- `amzn-codewhisperer-streaming-client` (existing)
- `uuid` crate with v4 feature (new)

**Internal Dependencies:**
- Phase 2 depends on Phase 1 (ModelRequest changes)
- Phase 3 depends on Phase 2 (CodeWhispererModelProvider)
- Phase 4 depends on Phase 3 (full integration)

### Critical Path

1. ModelRequest.conversation_id field (Task 1.1)
2. CodeWhispererModelProvider implementation (Tasks 2.1-2.8)
3. Platform selection integration (Tasks 3.1-3.6)
4. Basic conversation test (Task 4.5)

---

## Implementation Notes

### Design Decisions Recap

1. **Conversation ID Flow:** Added to ModelRequest, extracted from ContextContainer by AgentLoop, used or generated by CodeWhispererModelProvider
2. **Empty History:** MVP uses empty conversation history, full history support is future work
3. **Event Filtering:** Only AssistantResponseEvent and ToolUseEvent are processed, others ignored
4. **Default Platform:** CodeWhisperer is the default (not Bedrock)
5. **ApiClient Reuse:** Leverages existing ApiClient infrastructure for auth and configuration
6. **Error Handling:** Provides context-specific error messages for common failures

### MVP Limitations

- **No conversation history:** Each request is independent, no multi-turn context
- **No tool execution:** Tool use requests are displayed but not executed
- **No model selection:** Uses default CodeWhisperer model
- **Limited event handling:** Only text and tool use events, ignoring citations, metadata, etc.

### Future Enhancements

- Add conversation history support (extend ModelRequest or pass Worker reference)
- Implement tool execution and result responses
- Add model selection per platform
- Handle additional CodeWhisperer events (citations, reasoning, etc.)
- Support SIGV4 authentication mode (QDeveloperStreamingClient)
- Add conversation persistence across sessions
- Implement platform-specific optimizations

---

## Validation Checklist

After completing all tasks, verify:

- [ ] CodeWhisperer basic conversation works
- [ ] Bedrock still works (no regression)
- [ ] Streaming displays correctly for both platforms
- [ ] Default platform is CodeWhisperer
- [ ] Platform switching works seamlessly
- [ ] Cancellation works (Ctrl+C)
- [ ] Authentication errors are clear
- [ ] Tool use requests are displayed (if supported)
- [ ] No compilation errors or warnings
- [ ] Unit tests pass
- [ ] Integration tests pass

---

## Troubleshooting Guide

### Common Issues

**Issue: "CodeWhisperer streaming client not available"**
- **Cause:** ApiClient couldn't create streaming client
- **Solutions:**
  - Run `q login` to authenticate
  - Check if `AMAZON_Q_SIGV4` environment variable is set (unset it for MVP)
  - Verify network connectivity

**Issue: Compilation error in BedrockConverseStreamModelProvider**
- **Cause:** ModelRequest structure changed
- **Solution:** Update Bedrock provider to handle new conversation_id field (Task 1.2)

**Issue: UUID not found**
- **Cause:** uuid crate not added to dependencies
- **Solution:** Add uuid to Cargo.toml (Task 2.9)

**Issue: Streaming not working**
- **Cause:** Event processing loop issue
- **Solution:** Verify process_stream_event() handles AssistantResponseEvent correctly (Task 2.4)

**Issue: Tool use not displaying**
- **Cause:** ToolUseEvent not processed or CodeWhisperer doesn't support it
- **Solution:** Check process_stream_event() ToolUseEvent branch (Task 2.4), or document as unsupported

---

## References

- **Design Document:** `mvp-codewhisperer-2-design.md`
- **Research Document:** `mvp-codewhisperer-1-research.md`
- **Scope Document:** `mvp-codewhisperer-0-scope.md`
- **Architecture Overview:** `codebase/agent-environment/README.md`
- **ModelProvider Documentation:** `codebase/agent-environment/model-provider.md`

---

## Completion Criteria

This implementation is complete when:

1. All 33 tasks are checked off
2. All validation checklist items pass
3. CodeWhisperer works as a drop-in replacement for Bedrock
4. No regressions in existing Bedrock functionality
5. Tests pass (unit and integration)
6. Documentation is updated (if needed)

**Next Steps After Completion:**
- Update primary plan sheet to mark mvp-codewhisperer as "Implemented"
- Create implementation log document
- Consider future enhancements (conversation history, tool execution, etc.)
