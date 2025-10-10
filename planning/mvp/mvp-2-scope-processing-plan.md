# MVP Scope Processing Plan

## Overview

This document provides an actionable task list for implementing the MVP. Tasks are ordered by priority and dependencies. Large tasks requiring design work are broken into research → design → implement phases.

---

## Phase 1: Quick Wins (Day 1)

### Task 1.1: Implement --no-interactive Support
**Type:** Direct Implementation  
**Effort:** 1-2 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Modify TextUi to check --no-interactive flag
- [ ] Skip prompt loop spawn in TextUi when flag is set
- [ ] Modify StructuredIO to check --no-interactive flag
- [ ] Skip stdin reader spawn in StructuredIO when flag is set
- [ ] Pass --no-interactive flag to AgentEnvironment
- [ ] Add job tracking in AgentEnvironment (initial jobs created vs active jobs)
- [ ] Add shutdown trigger when no active jobs remain (after initial jobs complete)
- [ ] Test with initial input and verify exit
- [ ] Verify no input reading occurs in non-interactive mode

**Acceptance Criteria:**
- `q chat --no-interactive "hello"` runs once and exits
- `q chat --no-interactive --ui-mode=structured "hello"` runs once and exits
- Both TextUi and StructuredIO modes work correctly
- No input reading occurs in non-interactive mode
- No hanging processes or prompt loops

---

### Task 1.2: Enhance StructuredIO Event Display
**Type:** Direct Implementation  
**Effort:** 2-3 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Investigate current StructuredIO input reading mechanism
- [ ] Test if `lines.next_line()` blocks quit command handling
- [ ] If needed, replace with interruptible input (tokio::select! or channel-based)
- [ ] Add WorkerEvent::Created handler to StructuredIO
- [ ] Add WorkerEvent::Deleted handler to StructuredIO
- [ ] Verify EventBus subscription happens before Session operations
- [ ] Test quit command: `echo '{"command":"quit"}' | q chat --ui-mode=structured`
- [ ] Verify event ordering and completeness in output

**Acceptance Criteria:**
- Worker creation event appears in JSON output
- Worker deletion event appears in JSON output
- Quit command properly interrupts and exits immediately (not blocked by input reading)
- All events appear in correct order

---

## Phase 2: Foundation (Day 1-2)

### Task 2.1: Implement History Accumulation in AgentLoop
**Type:** Direct Implementation  
**Effort:** 3-4 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Modify AgentLoop::query_llm() to read entire conversation_history
- [ ] Format all history entries into ModelRequest (not just last message)
- [ ] Update ModelRequest structure to support message arrays
- [ ] Modify AgentLoop::run() to capture ModelResponse
- [ ] Create AssistantMessage from response
- [ ] Append to Worker's conversation_history after successful response
- [ ] Ensure proper mutex locking
- [ ] Test multi-turn conversation flow
- [ ] Verify history persists across multiple agent loop invocations
- [ ] Verify entire history is sent to LLM on each request

**Acceptance Criteria:**
- Assistant responses appear in conversation history
- Entire conversation history is sent to LLM on each request
- Multi-turn conversations maintain context
- No race conditions or deadlocks
- History can be inspected via /context command

---

## Phase 3: Tool System Integration (Day 2-5) ⚠️ CRITICAL

### Task 3.1: Research Tool System Architecture
**Type:** Research & Analysis  
**Effort:** 4-6 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Study existing tool implementations (fs_read, fs_write)
- [ ] Analyze ToolManager and Tool trait
- [ ] Research Bedrock tool use format (tool_use blocks)
- [ ] Research CodeWhisperer tool use support (if any)
- [ ] Document current tool execution flow
- [ ] Identify permission/approval mechanisms
- [ ] Create comparison: old vs new architecture needs

**Deliverable:** `planning/mvp/tool-system-research.md`

**Key Questions to Answer:**
- How are tools currently registered and discovered?
- What is the tool execution lifecycle?
- How are tool results formatted and returned?
- What permission models exist?
- How does Bedrock handle tool use in API?

---

### Task 3.2: Design agent_env Tool System
**Type:** Design  
**Effort:** 6-8 hours  
**Dependencies:** Task 3.1

**Subtasks:**
- [ ] Design agent_env-compatible Tool trait
- [ ] Design tool registration mechanism (Worker vs Session)
- [ ] Design tool execution flow with cancellation
- [ ] Design tool result formatting
- [ ] Design permission/approval integration
- [ ] Extend ModelProvider trait for tool use
- [ ] Design ModelRequest/ModelResponse for tools
- [ ] Create sequence diagrams for tool execution
- [ ] Plan error handling and retry logic

**Deliverable:** `planning/mvp/tool-system-design.md`

**Design Decisions:**
- Tool storage: Worker-specific or Session-shared?
- Tool execution: Inline or separate task?
- Permission model: Per-tool, per-invocation, or session-wide?
- Tool result format: Match Bedrock or abstract?

---

### Task 3.3: Implement Core Tool Infrastructure
**Type:** Implementation  
**Effort:** 8-10 hours  
**Dependencies:** Task 3.2

**Subtasks:**
- [ ] Create agent_env Tool trait
- [ ] Create ToolRegistry for tool management
- [ ] Extend ModelProvider trait with tool use support
- [ ] Update ModelRequest to include available tools
- [ ] Update ModelResponse to include tool use requests
- [ ] Add tool execution to AgentLoop
- [ ] Implement tool result handling
- [ ] Add tool use events to EventBus
- [ ] Update BedrockConverseStreamModelProvider for tools

**Acceptance Criteria:**
- Tool trait defined and documented
- ModelProvider supports tool use
- AgentLoop can detect and handle tool requests
- Tool results flow back to LLM

---

### Task 3.4: Implement fs_read and fs_write Tools
**Type:** Implementation  
**Effort:** 4-6 hours  
**Dependencies:** Task 3.3

**Subtasks:**
- [ ] Port fs_read to agent_env Tool trait
- [ ] Port fs_write to agent_env Tool trait
- [ ] Implement permission checking
- [ ] Add tool use events
- [ ] Test fs_read with various file types
- [ ] Test fs_write with create/replace/append
- [ ] Test permission denial flow
- [ ] Test error handling (file not found, permission denied)

**Acceptance Criteria:**
- fs_read works in agent_env
- fs_write works in agent_env
- Permission system functional
- Error handling robust

---

## Phase 4: Context and Model Provider (Day 5-7)

### Task 4.1: Research Agent Context Loading
**Type:** Research  
**Effort:** 2-3 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Study Agent config resource loading
- [ ] Analyze how context files are currently read
- [ ] Research context injection points (system prompt vs messages)
- [ ] Study glob pattern handling for file://paths
- [ ] Document context size limits and truncation

**Deliverable:** `planning/mvp/context-loading-research.md`

---

### Task 4.2: Implement --agent Context Loading
**Type:** Implementation  
**Effort:** 4-6 hours  
**Dependencies:** Task 4.1

**Subtasks:**
- [ ] Load Agent config in ChatArgs::execute()
- [ ] Implement resource file reading with glob support
- [ ] Format context for injection
- [ ] Add context to Worker's ContextContainer
- [ ] Ensure ModelProvider includes context in requests
- [ ] Test with various resource patterns
- [ ] Test with large context files

**Acceptance Criteria:**
- --agent flag loads correct config
- Resource files are read and injected
- Context appears in LLM requests
- Glob patterns work correctly

---

### Task 4.3: Research CodeWhisperer API
**Type:** Research  
**Effort:** 4-6 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Study CodeWhisperer client in codebase
- [ ] Research CodeWhisperer chat/conversation API
- [ ] Determine streaming support
- [ ] Research tool use support (if any)
- [ ] Document authentication requirements
- [ ] Compare with Bedrock API differences

**Deliverable:** `planning/mvp/codewhisperer-api-research.md`

---

### Task 4.4: Implement CodeWhisperer Model Provider
**Type:** Implementation  
**Effort:** 8-12 hours  
**Dependencies:** Task 4.3

**Subtasks:**
- [ ] Create CodeWhispererModelProvider struct
- [ ] Implement ModelProvider trait
- [ ] Implement streaming response handling
- [ ] Implement tool use (if supported)
- [ ] Add authentication/session management
- [ ] Add --platform flag to ChatArgs
- [ ] Update ChatArgs::execute() to select provider
- [ ] Test basic conversation flow
- [ ] Test streaming output
- [ ] Test error handling

**Acceptance Criteria:**
- CodeWhisperer provider works for basic chat
- Streaming responses display correctly
- --platform flag switches between providers
- Error handling is robust

---

## Phase 5: Polish and Demo Prep (Day 7-8)

### Task 5.1: Integration Testing
**Type:** Testing  
**Effort:** 4-6 hours  
**Dependencies:** All previous tasks

**Subtasks:**
- [ ] Test complete flow: --agent with context + tools
- [ ] Test --no-interactive with tools
- [ ] Test StructuredIO with full event stream
- [ ] Test both Bedrock and CodeWhisperer providers
- [ ] Test error scenarios and edge cases
- [ ] Test cancellation and cleanup
- [ ] Performance testing with multiple workers

**Acceptance Criteria:**
- All features work together
- No crashes or hangs
- Clean shutdown in all scenarios

---

### Task 5.2: Documentation and Examples
**Type:** Documentation  
**Effort:** 3-4 hours  
**Dependencies:** Task 5.1

**Subtasks:**
- [ ] Update architecture docs with tool system
- [ ] Create example agent configs
- [ ] Write usage examples for --no-interactive
- [ ] Document --platform flag
- [ ] Create demo script for presentation
- [ ] Update README with new features

**Deliverable:** Updated documentation and demo materials

---

## Phase 6: Web UI (Day 9-12) - Optional

### Task 6.1: Research Web UI Architecture
**Type:** Research & Design  
**Effort:** 4-6 hours  
**Dependencies:** None

**Subtasks:**
- [ ] Study /Volumes/workplace/web-q/ reference implementation
- [ ] Design HTTP server integration (axum)
- [ ] Design WebSocket/SSE event streaming
- [ ] Design command handling from web UI
- [ ] Plan static file serving
- [ ] Design security model (auth, CORS)

**Deliverable:** `planning/mvp/web-ui-design.md`

---

### Task 6.2: Implement Web UI Backend
**Type:** Implementation  
**Effort:** 8-12 hours  
**Dependencies:** Task 6.1

**Subtasks:**
- [ ] Add axum HTTP server
- [ ] Implement WebSocket endpoint
- [ ] Create headless UI for web event forwarding
- [ ] Implement command handling from WebSocket
- [ ] Add static file serving
- [ ] Test event streaming
- [ ] Test command execution

---

### Task 6.3: Implement Web UI Frontend
**Type:** Implementation  
**Effort:** 8-12 hours  
**Dependencies:** Task 6.2

**Subtasks:**
- [ ] Port web-q design to new structure
- [ ] Implement WebSocket client
- [ ] Display worker list and states
- [ ] Display streaming output
- [ ] Implement prompt input
- [ ] Implement cancel button
- [ ] Style and polish UI

---

## Phase 7: MCP Integration (Future) - Out of MVP Scope

### Task 7.1: Research MCP Integration
**Type:** Research  
**Effort:** 6-8 hours  
**Dependencies:** Tool system (Phase 3)

**Note:** This is documented for future work but not part of MVP.

**Subtasks:**
- [ ] Study existing MCP client code
- [ ] Analyze MCP server initialization in ToolManager
- [ ] Research MCP tool discovery
- [ ] Design MCP server manager for Session
- [ ] Plan MCP tool mapping to agent_env tools

---

## Summary

### MVP Core (Must Have)
- Phase 1: Quick wins (Tasks 1.1, 1.2)
- Phase 2: Foundation (Task 2.1)
- Phase 3: Tool system (Tasks 3.1-3.4) ⚠️ Critical
- Phase 4: Context and providers (Tasks 4.1-4.4)
- Phase 5: Integration and docs (Tasks 5.1-5.2)

### MVP Extended (Nice to Have)
- Phase 6: Web UI (Tasks 6.1-6.3)

### Future Work (Post-MVP)
- Phase 7: MCP integration

### Estimated Timeline
- Core MVP: 7-8 days
- With Web UI: 11-12 days
- With MCP: +2-3 days

### Critical Path
1. Tool system research and design (3.1, 3.2)
2. Tool system implementation (3.3, 3.4)
3. Integration testing (5.1)

All other tasks can proceed in parallel or are independent.
