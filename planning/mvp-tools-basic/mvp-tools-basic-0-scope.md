# MVP Tools Basic - Scope

## Overview

This workflow covers the integration of the tool system into the agent_env architecture. The goal is to implement two basic tools (fs_read and fs_write) and create a strategy for migrating other tools. This is the most complex task in the MVP and requires significant research and design work.

## Corrections provided by the team's senior SDE - pay attention, update this scope doc

- In this workflow we add several moving pieces 
   - ToolProvider class that will be used by Worker to access tools
      - a property in Worker, built in WorkerBuilder based on the agent config - refer to the existing implementation
      - encapsulates state for: 
         - the list of available tools, 
         - parameters of auto-approval for each tool
      - provides `execute_tool(ToolRequest)` that can return `ToolResult` - hides calling actual tools from the consumer (AgentLoopTask)
   - `ToolResult` is a enum of Success/Failure/ApprovalRequired
   - AgentLoopTask would add Success and Failure results to Conversation History in the Context Container, and continue the loop - query LLM again with new request
      - Conversation History must be able to keep multiple Tool Requests per Assistant Response
      - Conversation History must be able to keep Tool Results for each request 
      - Conversation History structure must stay close to model APIs - need to research how Codewhisperer API and Bedrock API handle tool request/response transmission
   - ContextBuilder taking in the list of tools and passing it through updated ModelRequest to ModelProviders
   - ContextBuilder passing the list of Tool Requests and Results in the Conversation History through updated ModelRequest to ModelProviders
   - BOTH existing ModelProviders must be able to handle updated ModelRequest properly - Bedrock and Codewhisperer
   - AgentLooptask will put ApprovalRequired results into Worker's 'OpenToolsApprovalRequests' and exit the loop (end the task)
      - note that we can teorethically have more than one tool use requested per model response
      - OpenToolsApprovalRequest will contain the source ToolUse reqest
   - AgentLooptask must send new special EventBus event for each open tool aproval request
   - UI can update each OpenToolsApprovalRequest with approval result: Approved or Rejected(reason)
   - UI can, by request from the user, update `worker.tool_provider.approval_rules[tool_id].full_trust = true`
      - That flag would make ToolProvider auto-approve all consequtive calls to that tool
   - TextUi must support tool approvals
      - Should initiate when task complete, as an alternative mode for the prompt
      - Simplified model like in the current implemenation (research the code base for text "Allow this action? Use 't' to trust (always allow) this tool for the session. [y/n/t]:" (`t`,`y`, `n`, and `t` are displayed in green color, separately))
         - y sets request to Approved
         - t sets 'full_trust' flag and sets request to Approved
         - n sets request to Rejected
         - any other text sets request to Rejected(text) - user can provide a rejection reason to the model
      - Once main worker has no open requests left - launch AgentLoopTask again
   - StructuredIO must support tool approvals
      - `{worker_id:...,approval_id:...,result:...}` where result either approve|reject|nay text (will be taken as Reject(text))
         - also causes a check and if the worker has no open requests left - launch AgentLoopTask for it
      - `{worker_id:...,tool_id:...,full_trust:true|false}`
   - AgentLoopTask on start must check if worker has any OpenToolsApprovalRequest
      - execute those with results and put result into the conversation history, remove the approval request
      - if there's any with no results - stop
      - otherwise - start the regular loop
   
Refer to available CodeWhisperer API documentation:
- codebase/aws-codewhisperer-clients.md
- codebase/aws-codewhisperer-calls.md
- codebase/aws-codewhisperer-calls-example-request.json
- codebase/aws-codewhisperer-calls-example-response.json


----

## Tasks Included

### 1.3: Tool System Integration

---

## Task 1.3: Tool System Integration

### Current State

**Existing Tool System:**
- Tool implementations exist in `cli/chat/tools/`
- Tools include: fs_read, fs_write, execute_bash, use_aws, knowledge, thinking, etc.
- ToolManager handles tool discovery and execution
- Tools use old architecture (ConversationState, ToolContext)
- Permission system exists with trust/approval mechanisms

**Agent Environment:**
- AgentLoop has no tool execution capability
- ModelProvider returns only text responses
- No tool use detection or handling
- No integration with existing tool system

**Bedrock API:**
- Supports tool use via tool_use blocks in responses
- Requires tool definitions in requests
- Returns tool_use requests in responses
- Expects tool results in follow-up requests

### Requirements

**Core Functionality:**
- Integrate tool system into agent_env architecture
- Support tool use requests from LLM
- Execute fs_read and fs_write tools
- Return tool results to LLM for continuation
- Maintain permission/approval system
- Support tool use events for UI display

**Tool Lifecycle:**
```
1. LLM Request: Include available tools in ModelRequest
2. LLM Response: Detect tool_use blocks in ModelResponse
3. Tool Execution: Execute requested tool with parameters
4. Tool Result: Format result and send back to LLM
5. LLM Continuation: LLM processes result and continues
```

**Permission System:**
- Respect --trust-all-tools flag
- Respect --trust-tools list
- Prompt user for approval when needed
- Support per-tool and per-invocation approval
- Handle approval denial gracefully

### Key Challenges

1. **Tool Abstraction:**
   - Need agent_env-compatible Tool trait
   - Must support async execution
   - Must support cancellation
   - Must integrate with existing tool implementations

2. **Tool Discovery:**
   - How are tools registered and made available?
   - Worker-specific tools vs Session-shared tools?
   - Dynamic tool loading (MCP servers)?

3. **Tool Execution:**
   - Inline execution vs separate task?
   - Cancellation support
   - Error handling and retries
   - Timeout handling

4. **Tool Results:**
   - Formatting for LLM consumption
   - Success vs error results
   - Large output handling (truncation, streaming)

5. **Model Provider:**
   - Extend ModelProvider trait for tool use
   - ModelRequest must include tool definitions
   - ModelResponse must include tool_use blocks
   - Support tool result messages

6. **Permission System:**
   - Integrate with existing approval mechanisms
   - UI interaction for approval prompts
   - State management for approved tools
   - Worker-specific vs session-wide approvals

### Research Questions

**Critical Questions:**
1. How does Bedrock API handle tool use?
   - Request format with tool definitions
   - Response format with tool_use blocks
   - Tool result format in follow-up requests

2. How does CodeWhisperer API handle tool use?
   - Does it support tools at all?
   - If yes, what format?
   - Compatibility with Bedrock format?

3. Tool Storage Strategy:
   - Should tools be Worker-specific or Session-shared?
   - How to handle worker-specific tool configurations?
   - How to handle MCP tools (future)?

4. Permission Model:
   - Per-tool approval (trust fs_read for session)?
   - Per-invocation approval (approve each use)?
   - Worker-specific approvals?
   - How to persist approvals?

5. Tool Execution Context:
   - What context do tools need? (working directory, environment, etc.)
   - How to pass Worker state to tools?
   - How to handle tool side effects?

### Design Phases

This task requires a phased approach:

#### Phase 1: Research (6-8 hours)
**Deliverable:** `mvp-tools-basic-1-research.md`

**Research Tasks:**
- Study existing tool implementations (fs_read, fs_write)
- Analyze ToolManager and Tool trait
- Research Bedrock tool use format
  - Request format with tool definitions
  - Response format with tool_use blocks
  - Tool result format
- Research CodeWhisperer tool use support
- Document current tool execution flow
- Identify permission/approval mechanisms
- Analyze tool context requirements
- Create comparison: old vs new architecture needs

**Key Questions to Answer:**
- What is the Bedrock tool use API format?
- What context do tools need to execute?
- How are permissions currently managed?
- What are the tool execution patterns?
- What are the error handling strategies?

#### Phase 2: Design (8-10 hours)
**Deliverable:** `mvp-tools-basic-2-design.md`

**Design Tasks:**
- Design agent_env-compatible Tool trait
- Design tool registration mechanism
- Design tool execution flow with cancellation
- Design tool result formatting
- Design permission/approval integration
- Extend ModelProvider trait for tool use
- Design ModelRequest/ModelResponse for tools
- Create sequence diagrams for tool execution
- Plan error handling and retry logic
- Design tool migration strategy for other tools

**Design Decisions:**
- Tool storage: Worker-specific or Session-shared?
- Tool execution: Inline or separate task?
- Permission model: Per-tool, per-invocation, or session-wide?
- Tool result format: Match Bedrock or abstract?
- Tool context: What state is passed to tools?

**Architecture Diagrams:**
- Tool registration and discovery
- Tool execution lifecycle
- Permission approval flow
- Tool result handling

#### Phase 3: Implementation (12-16 hours)
**Deliverable:** Working tool system with fs_read and fs_write

**Implementation Tasks:**
1. Create agent_env Tool trait
2. Create ToolRegistry for tool management
3. Extend ModelProvider trait with tool use support
4. Update ModelRequest to include available tools
5. Update ModelResponse to include tool use requests
6. Add tool execution to AgentLoop
7. Implement tool result handling
8. Add tool use events to EventBus
9. Update BedrockConverseStreamModelProvider for tools
10. Port fs_read to agent_env Tool trait
11. Port fs_write to agent_env Tool trait
12. Implement permission checking
13. Add tool use events
14. Test tool execution flow
15. Test permission system
16. Test error handling

### Proposed Architecture (Preliminary)

**Tool Trait:**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult>;
    
    fn requires_approval(&self) -> bool;
}

pub struct ToolContext {
    pub worker_id: String,
    pub working_directory: PathBuf,
    pub environment: HashMap<String, String>,
    // Other context as needed
}

pub struct ToolResult {
    pub status: ToolResultStatus,
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

**ToolRegistry:**
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    approved_tools: Arc<Mutex<HashSet<String>>>, // Session-wide approvals
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Arc<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn list(&self) -> Vec<&str>;
    pub fn is_approved(&self, name: &str) -> bool;
    pub fn approve(&self, name: &str);
}
```

**ModelProvider Extension:**
```rust
pub struct ModelRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>, // NEW
    pub model_id: String,
}

pub struct ModelResponse {
    pub content: String,
    pub tool_uses: Vec<ToolUse>, // NEW
    pub stop_reason: StopReason,
}

pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub parameters: serde_json::Value,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}
```

**AgentLoop Integration:**
```rust
// In AgentLoop::run()
loop {
    // Send request with tools
    let response = model_provider.query(request).await?;
    
    // Check for tool uses
    if !response.tool_uses.is_empty() {
        // Execute tools
        let tool_results = execute_tools(
            &response.tool_uses,
            &tool_registry,
            &tool_context,
        ).await?;
        
        // Send tool results back to LLM
        request = create_tool_result_request(tool_results);
        continue;
    }
    
    // No tool uses, conversation continues
    break;
}
```

### Tool Migration Strategy

After implementing fs_read and fs_write, other tools can be migrated using this pattern:

1. **Identify Tool:**
   - Review existing tool implementation
   - Identify dependencies and context requirements

2. **Implement Tool Trait:**
   - Create new struct implementing agent_env Tool trait
   - Port execute logic from old implementation
   - Update parameter schema

3. **Register Tool:**
   - Add to ToolRegistry in Session initialization
   - Configure permission requirements

4. **Test Tool:**
   - Test execution in agent_env
   - Test permission system
   - Test error handling

**Priority Order for Migration:**
- fs_read, fs_write (MVP)
- execute_bash (high value)
- thinking (internal tool)
- knowledge (context management)
- use_aws (AWS operations)
- Other tools as needed

### Acceptance Criteria

**Phase 1 (Research):**
- Complete understanding of Bedrock tool use API
- Documentation of existing tool system
- Clear requirements for agent_env integration

**Phase 2 (Design):**
- Complete tool system architecture
- Sequence diagrams for tool execution
- Clear migration strategy for other tools
- Design review and approval

**Phase 3 (Implementation):**
- Tool trait defined and documented
- ToolRegistry implemented
- ModelProvider supports tool use
- AgentLoop can detect and handle tool requests
- fs_read works in agent_env
- fs_write works in agent_env
- Permission system functional
- Tool use events published to EventBus
- Error handling robust
- Tests passing

### Testing Strategy

```bash
# Test fs_read
q chat "Read the file /tmp/test.txt"

# Test fs_write
q chat "Write 'Hello World' to /tmp/output.txt"

# Test permission system
q chat "Read /etc/passwd"
# (should prompt for approval)

# Test --trust-all-tools
q chat --trust-all-tools "Read /tmp/test.txt"
# (should not prompt)

# Test --trust-tools
q chat --trust-tools=fs_read "Read /tmp/test.txt"
# (should not prompt for fs_read)

q chat --trust-tools=fs_read "Write to /tmp/output.txt"
# (should prompt for fs_write)

# Test tool errors
q chat "Read /nonexistent/file.txt"
# (should handle error gracefully)

# Test multi-tool use
q chat "Read /tmp/input.txt and write the contents to /tmp/output.txt"
# (should use both tools)
```

### Dependencies

- History accumulation (Task 1.2) - helpful but not required
- ModelProvider abstraction - already exists
- EventBus - already exists

### Estimated Effort

**Total: 26-34 hours**
- Phase 1 (Research): 6-8 hours
- Phase 2 (Design): 8-10 hours
- Phase 3 (Implementation): 12-16 hours

This is the critical path for MVP.

---

## Success Metrics

- fs_read and fs_write work in agent_env
- Tool use requests are detected and executed
- Tool results are returned to LLM
- Permission system works correctly
- Tool use events are published
- Error handling is robust
- Clear migration path for other tools

---

## Risks and Mitigation

**Risk 1: Bedrock API Complexity**
- Mitigation: Thorough research phase, prototype testing

**Risk 2: Permission System Integration**
- Mitigation: Reuse existing approval mechanisms, incremental testing

**Risk 3: Tool Execution Errors**
- Mitigation: Comprehensive error handling, graceful degradation

**Risk 4: Performance Impact**
- Mitigation: Async execution, proper cancellation support

**Risk 5: CodeWhisperer Compatibility**
- Mitigation: Abstract tool use format, provider-specific adapters

---

## Related Files

**Existing Tool System:**
- `crates/chat-cli/src/cli/chat/tools/` - Tool implementations
- `crates/chat-cli/src/cli/chat/tool_manager.rs` - Tool management
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config with tool permissions

**Agent Environment:**
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - AgentLoop implementation
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelProvider trait
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Bedrock implementation
- `crates/chat-cli/src/agent_env/events.rs` - Event definitions
- `crates/chat-cli/src/agent_env/session.rs` - Session orchestration
- `crates/chat-cli/src/agent_env/worker.rs` - Worker state

**New Files (to be created):**
- `crates/chat-cli/src/agent_env/tools/mod.rs` - Tool system module
- `crates/chat-cli/src/agent_env/tools/tool_trait.rs` - Tool trait definition
- `crates/chat-cli/src/agent_env/tools/tool_registry.rs` - Tool registry
- `crates/chat-cli/src/agent_env/tools/fs_read.rs` - fs_read implementation
- `crates/chat-cli/src/agent_env/tools/fs_write.rs` - fs_write implementation
- `crates/chat-cli/src/agent_env/tools/tool_context.rs` - Tool execution context

---

## Documentation Updates

After implementation:
- Document tool system architecture
- Create tool development guide
- Document tool migration process
- Add tool use examples
- Update ModelProvider documentation
- Document permission system
