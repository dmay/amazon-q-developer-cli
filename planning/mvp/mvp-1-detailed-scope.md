# MVP Detailed Scope Breakdown

## Overview

This document breaks down the MVP requirements into structured tasks with analysis of complexity, dependencies, and implementation approaches.

---

## 1. Minimal Functionality

### 1.1 Support --no-interactive in Both UIs

**Current State:**
- `--no-interactive` flag exists in ChatArgs
- TextUi has prompt loop that needs to be disabled
- StructuredIO has always-reading pattern that needs to be disabled

**Requirements:**
- TextUi: Skip prompt loop, only process initial input, exit after completion
- StructuredIO: Disable stdin reading completely, only process initial input, exit after completion
- Both: No input reading whatsoever in non-interactive mode
- Both: Ensure proper shutdown after task completion

**Complexity:** Low
**Dependencies:** None
**Estimated Effort:** Small (1-2 hours)

**Implementation Options:**
1. Add flag check in TextUi::new() to skip prompt task spawn
2. Add flag check in StructuredIO::new() to skip stdin reader task spawn
3. Pass --no-interactive flag to AgentEnvironment
4. AgentEnvironment tracks initial job creation, then monitors for "no active jobs"
5. AgentEnvironment initiates shutdown when no active jobs remain (after initial jobs complete)
6. Both UIs: Completely disable input reading when flag is set

---

### 1.2 AgentLoop - History Accumulation

**Current State:**
- AgentLoop queries conversation_history for last entry
- No clear mechanism for appending assistant responses back to history
- ConversationHistory has push methods but not integrated with AgentLoop

**Requirements:**
- After LLM response, append AssistantMessage to conversation_history
- Send entire conversation history in ModelRequest to LLM
- Maintain proper conversation flow for multi-turn interactions
- Preserve history across multiple agent loop invocations
- Format history correctly for model provider (user/assistant message sequence)

**Complexity:** Medium
**Dependencies:** None
**Estimated Effort:** Medium (3-4 hours)

**Implementation Approach:**
- Modify AgentLoop::query_llm() to read entire conversation_history
- Format all entries into ModelRequest (not just last message)
- Modify AgentLoop::run() to append ModelResponse to conversation_history
- Ensure proper locking and thread safety
- Add conversation entry with assistant message after successful response
- Update ModelProvider to handle multi-message history

---

### 1.3 AgentLoop - Minimal Tools (fs_read, fs_write)

**Current State:**
- Tool system exists in `cli/chat/tools/` with fs_read.rs and fs_write.rs
- Tools use old architecture (ToolManager, ConversationState, etc.)
- AgentLoop has no tool execution capability
- ModelProvider returns only text responses, no tool use detection

**Requirements:**
- Integrate tool system into agent_env architecture
- Support tool use requests from LLM
- Execute fs_read and fs_write tools
- Return tool results to LLM for continuation

**Complexity:** High
**Dependencies:** Requires significant research and design
**Estimated Effort:** Large (2-3 days)

**Key Challenges:**
1. **Tool Abstraction:** Need agent_env-compatible tool trait
2. **Tool Discovery:** How tools are registered and made available to Worker
3. **Tool Execution:** Async execution with cancellation support
4. **Tool Results:** Formatting and returning results to LLM
5. **Model Provider:** Extend to support tool use in requests/responses
6. **Permission System:** Integrate trust/approval mechanism

**Research Questions:**
- How does CodeWhisperer API handle tool use? (Bedrock uses tool_use blocks)
- Should tools be Worker-specific or Session-shared?
- How to handle tool execution errors and retries?
- Permission model: per-tool, per-invocation, or session-wide?

**Design Phases:**
1. Research: Study existing tool system and Bedrock tool use format
2. Design: Create agent_env tool architecture
3. Implement: Build minimal tool support for fs_read/fs_write

---

### 1.4 Default Worker with --agent Context

**Current State:**
- Agent config system exists in `cli/agent/mod.rs`
- Agent has resources field for context files
- No integration with agent_env Worker
- Context loading happens in old chat flow

**Requirements:**
- Load Agent config when --agent flag provided
- Read resources (context files) from Agent config
- Inject context into Worker's ContextContainer
- Support file:// paths and glob patterns

**Complexity:** Medium
**Dependencies:** Agent config loading, context file reading
**Estimated Effort:** Medium (4-6 hours)

**Research Questions:**
- How are context files currently loaded and formatted?
- Where in conversation history should context be injected?
- How to handle large context files (truncation, summarization)?
- Should context be in system prompt or user message?

**Implementation Approach:**
1. Load Agent from config file in ChatArgs::execute()
2. Read and parse resource files
3. Format context for injection
4. Add to Worker's conversation_history or separate context field
5. Ensure ModelProvider includes context in requests

---

### 1.5 CodeWhispererAPI Model Provider

**Current State:**
- Only BedrockConverseStreamModelProvider exists
- CodeWhisperer client exists in codebase (amzn_codewhisperer_client)
- Need to understand CodeWhisperer streaming API

**Requirements:**
- Implement CodeWhispererModelProvider
- Support streaming responses
- Handle tool use (if supported by CodeWhisperer)
- Add --platform flag to select provider

**Complexity:** High
**Dependencies:** Understanding CodeWhisperer API
**Estimated Effort:** Large (1-2 days)

**Research Questions:**
- What is CodeWhisperer's chat/conversation API?
- Does it support streaming?
- How does it handle tool use?
- Authentication and session management?

**Implementation Phases:**
1. Research: Study CodeWhisperer API documentation and existing usage
2. Design: Plan ModelProvider implementation
3. Implement: Build CodeWhispererModelProvider
4. Test: Verify streaming and basic functionality

---

### 1.6 StructuredIO Enhancements

**Current State:**
- StructuredIO exists with JSON input/output
- Subscribes to EventBus for events
- May not display all worker lifecycle events

**Requirements:**
- Display worker creation/deletion events
- Start listening to EventBus before Session sends events
- Ensure proper quit command handling with `{"command":"quit"}`
- Verify event ordering and completeness
- May need to replace `lines.next_line()` with interruptible input method for immediate quit response

**Complexity:** Low-Medium
**Dependencies:** EventBus event types
**Estimated Effort:** Small-Medium (2-3 hours)

**Research Note:**
- Current implementation may use blocking `lines.next_line()` which can't be interrupted
- Need to investigate if quit command requires tokio::select! or channel-based input reading
- Test if current implementation already handles quit properly or needs refactoring

**Implementation Approach:**
1. Review StructuredIO event handling
2. Add handlers for WorkerEvent::Created and WorkerEvent::Deleted
3. Verify EventBus subscription timing
4. Test quit command interrupt behavior

---

## 2. Fancy Features

### 2.1 Web UI with Minimal Worker Presentation

**Current State:**
- Reference implementation exists at /Volumes/workplace/web-q/
- No web UI in current agent_env architecture
- EventBus supports multiple UI subscribers

**Requirements:**
- One-page web app showing workers
- Display worker state (Idle, Busy, IdleFailed)
- Show streaming output from jobs
- Basic controls (send prompt, cancel job)

**Complexity:** High
**Dependencies:** Web server, WebSocket/SSE for events, frontend framework
**Estimated Effort:** Large (3-5 days)

**Implementation Approach:**
1. Add HTTP server (axum or similar)
2. WebSocket endpoint for EventBus subscription
3. Serve static files from web-q design
4. Implement headless UI that forwards events to WebSocket
5. Frontend: Display workers, handle events, send commands

**Design Considerations:**
- Should web UI run in same process or separate?
- How to handle authentication/security?
- Port configuration and conflicts?

---

## 3. MCP Integration (Future Work)

### 3.1 MCP Server Initialization

**Current State:**
- MCP client code exists in codebase
- Agent config has mcp_servers field
- ToolManager handles MCP tool discovery
- No integration with agent_env

**Requirements:**
- Initialize MCP servers at application startup
- Share MCP instances across all workers
- Discover tools from MCP servers
- Make MCP tools available to agent_env tool system

**Complexity:** High
**Dependencies:** Tool system integration (1.3)
**Estimated Effort:** Large (2-3 days)

**Research Questions:**
- How are MCP servers currently initialized?
- Lifecycle management (startup, shutdown, reconnection)?
- How to map MCP tools to agent_env tool system?
- Error handling for MCP server failures?

**Design Phases:**
1. Research: Study existing MCP integration in ToolManager
2. Design: Plan MCP initialization in agent_env
3. Implement: Build MCP server manager for Session
4. Integrate: Connect MCP tools to tool execution system

---

### 3.2 Worker-Specific MCP Instances (Future)

**Requirements:**
- Allow workers to have their own MCP server instances
- Useful for isolated environments or different tool sets
- More complex lifecycle management

**Complexity:** Very High
**Dependencies:** MCP integration (3.1), tool system (1.3)
**Estimated Effort:** Very Large (5+ days)

**Note:** This is explicitly out of scope for MVP. First goal is shared MCP instances.

---

## Summary of Task Categories

### Small Tasks (< 4 hours)
- 1.1: --no-interactive support
- 1.6: StructuredIO enhancements

### Medium Tasks (4-8 hours)
- 1.2: History accumulation
- 1.4: --agent context loading

### Large Tasks (1-3 days, need design)
- 1.3: Tool system integration ⚠️ Critical path
- 1.5: CodeWhisperer model provider
- 3.1: MCP integration

### Very Large Tasks (3+ days)
- 2.1: Web UI
- 3.2: Worker-specific MCP (out of scope)

---

## Critical Path Analysis

**Blocking Dependencies:**
1. Tool system (1.3) blocks MCP integration (3.1)
2. Model provider abstraction affects tool use design
3. Context loading (1.4) independent but needed for demo

**Recommended Implementation Order:**
1. Quick wins: 1.1 (--no-interactive), 1.6 (StructuredIO)
2. Foundation: 1.2 (history accumulation)
3. Critical: 1.3 (tool system) - requires design phase
4. Parallel: 1.4 (context loading), 1.5 (CodeWhisperer provider)
5. Polish: 2.1 (Web UI)
6. Future: 3.1 (MCP integration)
