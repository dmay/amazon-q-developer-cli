# MVP Tools Basic - Research Document

**Status**: Complete  
**Date**: 2025-10-12  
**Workflow**: mvp-tools-basic  

## Document Purpose

This research document provides a comprehensive analysis of the existing codebase, API requirements, and architectural patterns needed to implement tool execution in the agent_env architecture. The findings inform the design phase and identify critical integration challenges.

## Research Scope

This research covers:
1. Current agent_env architecture and event-driven patterns
2. Bedrock Converse API tool use format and requirements
3. CodeWhisperer API tool use format and requirements
4. Existing tool implementations (fs_read, fs_write, ToolManager)
5. ConversationHistory structure and message handling
6. ModelProvider trait and implementation patterns
7. AgentLoop task execution flow
8. Current approval system and UI patterns
9. Critical integration challenges and pitfalls

## Key Findings Summary

- **Architecture**: EventBus-centered with Session managing Workers and Jobs
- **Tool Support**: Both Bedrock and CodeWhisperer APIs support tools with similar but distinct formats
- **Existing Tools**: Complex ToolManager system with permission evaluation and Agent config integration
- **Message Structures**: UserMessage/AssistantMessage already support tool use tracking
- **AgentLoop**: Currently detects tool requests but doesn't execute them
- **Approval System**: Per-invocation approval with session-wide trust flags
- **Critical Challenges**: 12 major integration challenges identified (see Section 8)

---

## Section 1: Architecture Overview

### 1.1 Agent Environment Architecture

The agent_env architecture is **EventBus-centered** with clean separation of concerns:

**Core Components**:
- **EventBus**: Central event distribution using tokio broadcast channels
- **AgentEnvironment**: Top-level coordinator managing event multicasting to UIs
- **Session**: Orchestrator managing Workers, Jobs, and publishing lifecycle events
- **Worker**: Agent state container with lifecycle state, task metadata, and conversation context
- **WorkerJob**: Running task instance with lifecycle management and cancellation
- **WorkerTask**: Interface for executable work units (AgentLoop, commands, etc.)

**Code Location**: `crates/chat-cli/src/agent_env/`

### 1.2 Event-Driven Communication

All components communicate via events published to EventBus:

**Event Types**:
- `WorkerEvent`: Created, Deleted, LifecycleStateChanged
- `JobEvent`: Started, Completed, OutputChunk
- `AgentLoopEvent`: ResponseReceived, ToolUseRequestReceived
- `SystemEvent`: ShutdownInitiated

**Event Flow**:
```
Component → EventBus.publish() → broadcast::Sender → All Subscribers
                                                    ├─ AgentEnvironment (multicast)
                                                    │  ├─ Main UI
                                                    │  └─ Headless UIs
                                                    └─ Tests
```

### 1.3 Worker Lifecycle States

Workers transition through states:
- **Inactive**: Ready for new task
- **Working**: Executing task
- **Requesting**: Querying LLM
- **Receiving**: Streaming LLM response
- **InactiveFailed**: Idle after task failure

### 1.4 UI Implementations

Two UI implementations exist:
- **TextUi**: Interactive text-based UI with rustyline input
- **StructuredIO**: JSON-based I/O for scripting and automation

Both subscribe to EventBus and receive all events. UIs send commands to AgentEnvironment via channels.

### 1.5 Execution Flow

1. ChatArgs::execute() creates EventBus, Session, Worker, UI, AgentEnvironment
2. AgentEnvironment spawns task to forward events to all UIs
3. UI starts (prompt loop for TextUi, input reader for StructuredIO)
4. AgentEnvironment receives commands from UI via channel
5. Session launches tasks (AgentLoop), publishes events throughout lifecycle
6. EventBus broadcasts events to all subscribers
7. Task completes, Session updates worker state, publishes completion event
8. AgentEnvironment coordinates cleanup, cancels jobs, exits gracefully

### 1.6 Implications for Tool System

The tool system must integrate with this architecture:
- **ToolProvider** should be a property of Worker (worker-specific tools)
- **Tool execution** should publish events to EventBus
- **Tool approval requests** should be stored in Worker state
- **UI integration** should use event subscription and command channels
- **AgentLoop** should handle tool execution and approval flow


## Section 2: API Format Analysis

### 2.1 Bedrock Converse API Tool Use Format

**Documentation**: AWS Bedrock Converse API tool use examples

#### Request Format

Tools are defined in `toolConfig`:

```python
tool_config = {
    "tools": [
        {
            "toolSpec": {
                "name": "top_song",
                "description": "Get the most popular song played on a radio station.",
                "inputSchema": {
                    "json": {
                        "type": "object",
                        "properties": {
                            "sign": {
                                "type": "string",
                                "description": "The call sign for the radio station..."
                            }
                        },
                        "required": ["sign"]
                    }
                }
            }
        }
    ]
}
```

Request includes:
- `messages`: Array of conversation messages
- `toolConfig`: Tool definitions

#### Response Format

When tool is requested:
- `stopReason`: `"tool_use"`
- Tool requests in `response['output']['message']['content']` as `toolUse` blocks

Tool use block structure:
```python
{
    "toolUse": {
        "toolUseId": "tool_01XYZ789ABC123DEF456GHI",
        "name": "fs_write",
        "input": {
            "path": "fibonacci.py",
            "content": "def fibonacci(n): ..."
        }
    }
}
```

#### Tool Result Format

Tool results sent back as user message:
```python
tool_result_message = {
    "role": "user",
    "content": [
        {
            "toolResult": {
                "toolUseId": "tool_01XYZ789ABC123DEF456GHI",
                "content": [{"json": {"song": "Elemental Hotel", "artist": "8 Storey Hike"}}],
                "status": "error"  # Optional, for errors
            }
        }
    ]
}
```

#### Streaming Behavior

For `converse_stream`:
- `contentBlockStart`: Signals start of tool use block
- `contentBlockDelta`: Streams tool input as JSON string chunks
- `contentBlockStop`: Signals end of tool use block
- Tool input must be accumulated and parsed as complete JSON

### 2.2 CodeWhisperer API Tool Use Format

**Documentation**: `codebase/aws-codewhisperer-calls-example-request.json` and `codebase/aws-codewhisperer-calls-example-response.json`

#### Request Format

Tools defined in `userInputMessageContext.tools`:

```json
{
    "userInputMessage": {
        "content": "Can you help me write a Python function?",
        "userInputMessageContext": {
            "tools": [
                {
                    "toolSpec": {
                        "name": "fs_write",
                        "description": "Create or modify files",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string"},
                                "content": {"type": "string"}
                            }
                        }
                    }
                }
            ],
            "toolResults": []
        }
    }
}
```

History maintains alternating messages:
- `userInputMessage`: User input with context
- `assistantResponseMessage`: Assistant response with tool uses

#### Response Format

Tool use in streaming response:
```json
{
    "toolUseEvent": {
        "toolUse": {
            "toolUseId": "tool_01XYZ789ABC123DEF456GHI",
            "name": "fs_write",
            "input": {
                "path": "fibonacci.py",
                "content": "def fibonacci(n): ..."
            }
        }
    }
}
```

Stop reason: `"tool_use"` when tools requested

#### Tool Result Format

Tool results in next request's `userInputMessageContext.toolResults`:

```json
{
    "toolResults": [
        {
            "toolUseId": "tool_01XYZ789ABC123DEF456GHI",
            "content": "File written successfully",
            "status": "success"
        }
    ]
}
```

### 2.3 API Comparison

| Aspect | Bedrock | CodeWhisperer |
|--------|---------|---------------|
| Tool Definition Location | `toolConfig` at request level | `userInputMessageContext.tools` |
| Tool Result Location | User message `content` | `userInputMessageContext.toolResults` |
| Tool Use ID Field | `toolUseId` | `toolUseId` |
| Input Format | JSON object | JSON object |
| Streaming | Chunks as JSON string | Complete object in event |
| Stop Reason | `"tool_use"` | `"tool_use"` |
| History Format | Messages array | Alternating user/assistant |

### 2.4 Unified Internal Format Requirements

To support both APIs, internal structures need:
- **ToolRequest**: `tool_use_id: String`, `tool_name: String`, `parameters: serde_json::Value`
- **ToolResult**: `tool_use_id: String`, `status: Success/Error`, `content: String`
- **ToolDefinition**: `name: String`, `description: String`, `input_schema: serde_json::Value`

Conversion functions needed:
- `to_bedrock_tool_config()` - Convert ToolDefinition to Bedrock format
- `from_bedrock_tool_use()` - Parse Bedrock tool use to ToolRequest
- `to_bedrock_tool_result()` - Convert ToolResult to Bedrock message format
- `to_codewhisperer_tools()` - Convert ToolDefinition to CodeWhisperer format
- `from_codewhisperer_tool_use()` - Parse CodeWhisperer tool use to ToolRequest
- `to_codewhisperer_tool_results()` - Convert ToolResult to CodeWhisperer format


## Section 3: Existing Tool System Analysis

### 3.1 Tool Implementation Pattern

**Location**: `crates/chat-cli/src/cli/chat/tools/`

Current tools implement a common pattern:

#### Tool Structure (fs_read example)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct FsRead {
    pub operations: Vec<FsReadOperation>,
    pub summary: Option<String>,
}

impl FsRead {
    pub async fn validate(&mut self, os: &Os) -> Result<()> { ... }
    
    pub async fn queue_description(&self, os: &Os, updates: &mut impl Write) -> Result<()> { ... }
    
    pub fn eval_perm(&self, os: &Os, agent: &Agent) -> PermissionEvalResult { ... }
    
    pub async fn invoke(&self, os: &Os, updates: &mut impl Write) -> Result<InvokeOutput> { ... }
}
```

#### Key Methods

1. **validate()**: Validates tool parameters before execution
2. **queue_description()**: Formats tool description for display
3. **eval_perm()**: Evaluates permissions based on Agent config
4. **invoke()**: Executes the tool logic

### 3.2 Permission Evaluation System

**Location**: `crates/chat-cli/src/cli/agent/mod.rs`

```rust
pub enum PermissionEvalResult {
    Allow,      // Tool allowed without confirmation
    Ask,        // Tool requires user confirmation
    Deny(Vec<String>),  // Tool denied with reasons
}
```

Permission evaluation considers:
- **allowed_tools**: HashSet of pre-approved tools
- **tools_settings**: HashMap with per-tool configuration
- **trust_all_tools**: Global flag to auto-approve all tools

#### Tool Settings Example (fs_read)

```rust
struct Settings {
    allowed_paths: Vec<String>,
    denied_paths: Vec<String>,
    allow_read_only: bool,
}
```

Settings are stored in Agent config and evaluated per-invocation:
- Check if tool in `allowed_tools` → Allow
- Check if path matches `denied_paths` → Deny
- Check if path matches `allowed_paths` → Allow
- Check if `allow_read_only` is true → Allow (for read operations)
- Otherwise → Ask

### 3.3 Agent Configuration

**Location**: `crates/chat-cli/src/cli/agent/mod.rs`

```rust
pub struct Agent {
    pub name: String,
    pub tools: Vec<String>,                    // Tools visible to agent
    pub tool_aliases: HashMap<String, String>, // Tool name remapping
    pub allowed_tools: HashSet<String>,        // Pre-approved tools
    pub tools_settings: HashMap<String, serde_json::Value>, // Per-tool config
    pub mcp_servers: McpServerConfig,          // MCP server configuration
    // ... other fields
}
```

Agent config provides:
- Tool visibility (which tools agent can see)
- Tool permissions (which tools are pre-approved)
- Tool-specific settings (paths, behaviors, etc.)
- MCP server integration

### 3.4 ToolManager Complexity

**Location**: `crates/chat-cli/src/cli/chat/tool_manager.rs`

ToolManager is a complex system that:
- Manages built-in tools (fs_read, fs_write, execute_bash, etc.)
- Manages MCP server connections and tools
- Handles tool namespacing (server_name::tool_name)
- Loads tool configurations from Agent
- Provides tool discovery and registration
- Manages tool lifecycle (initialization, updates, cleanup)

**Key Challenge**: ToolManager is tightly coupled to current chat flow and uses global state. Scope document says to "reuse existing code" but ToolManager may be difficult to integrate directly into agent_env architecture.

### 3.5 Tool Invocation Flow (Current)

1. LLM requests tool use
2. Tool request parsed from response
3. Permission evaluated via `eval_perm()`
4. If Ask → User prompted for approval
5. If approved → Tool `invoke()` called
6. Tool result formatted and sent back to LLM

### 3.6 Reusability Assessment

**Highly Reusable**:
- Tool trait pattern (validate, eval_perm, invoke)
- Permission evaluation logic
- Tool-specific settings structure
- Agent config integration

**Challenging to Reuse**:
- ToolManager (too coupled to current flow)
- MCP integration (separate workflow: mvp-tools-mcp)
- Tool namespacing (MCP-specific)
- Global state management

**Recommendation**: Create new ToolProvider for agent_env that:
- Reuses tool trait pattern and permission logic
- Integrates with Agent config for settings
- Stores tools in Worker (not global)
- Defers MCP integration to separate workflow


## Section 4: ConversationHistory and Message Structures

### 4.1 Agent_Env ConversationHistory

**Location**: `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`

Simple structure for agent_env:

```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,
}

pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

Methods:
- `push_input_message(content: String)` - Add user message
- `push_assistant_message(assistant: AssistantMessage)` - Add assistant message
- `get_entries()` - Get all entries
- `len()`, `is_empty()` - Query size

### 4.2 Main Chat Message Structures

**Location**: `crates/chat-cli/src/cli/chat/message.rs`

More complex structures used in main chat:

#### UserMessage

```rust
pub struct UserMessage {
    pub additional_context: String,
    pub env_context: UserEnvContext,
    pub content: UserMessageContent,
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub images: Option<Vec<ImageBlock>>,
}

pub enum UserMessageContent {
    Prompt { prompt: String },
    CancelledToolUses { 
        prompt: Option<String>,
        tool_use_results: Vec<ToolUseResult>,
    },
    ToolUseResults { 
        tool_use_results: Vec<ToolUseResult>,
    },
}
```

#### AssistantMessage

```rust
pub enum AssistantMessage {
    Response {
        message_id: Option<String>,
        content: String,
    },
    ToolUse {
        message_id: Option<String>,
        content: String,
        tool_uses: Vec<AssistantToolUse>,
    },
}
```

**Key Observation**: AssistantMessage already supports tool use tracking with `ToolUse` variant!

### 4.3 Tool-Related Structures

```rust
pub struct AssistantToolUse {
    pub id: String,           // tool_use_id
    pub name: String,         // tool name
    pub input: serde_json::Value,  // tool parameters
    pub accepted: bool,       // approval status
}

pub struct ToolUseResult {
    pub id: String,           // tool_use_id
    pub output: String,       // tool output
    pub status: ToolResultStatus,
}

pub enum ToolResultStatus {
    Success,
    Error,
}
```

### 4.4 Conversion to API Formats

Main chat has conversion implementations:

```rust
impl From<AssistantMessage> for AssistantResponseMessage {
    // Converts to CodeWhisperer format
}
```

These conversions handle:
- Message ID preservation
- Tool use array conversion
- Content extraction

### 4.5 Integration Strategy

**Option 1: Use Main Chat Structures**
- Pros: Already supports tool use, has API conversions
- Cons: Complex, many fields not needed in agent_env

**Option 2: Extend Agent_Env Structures**
- Pros: Simpler, tailored to agent_env needs
- Cons: Need to implement API conversions

**Option 3: Bridge Pattern**
- Pros: Keep agent_env simple, reuse conversions
- Cons: Additional conversion layer

**Recommendation**: Use main chat structures (UserMessage, AssistantMessage) in agent_env ConversationHistory. They already support tool use and have proven API conversions. Simplify by using only needed fields.

### 4.6 Required Changes

To support tools in ConversationHistory:

1. **Import main chat message types**:
   ```rust
   use crate::cli::chat::message::{UserMessage, AssistantMessage};
   ```

2. **Add tool result methods**:
   ```rust
   impl ConversationHistory {
       pub fn push_tool_results(&mut self, results: Vec<ToolUseResult>) {
           // Create UserMessage with ToolUseResults content
       }
   }
   ```

3. **Update ContextBuilder** to convert to API formats:
   - Extract tool uses from AssistantMessage::ToolUse
   - Extract tool results from UserMessage::ToolUseResults
   - Format for Bedrock or CodeWhisperer


## Section 5: ModelProvider Analysis

### 5.1 Current ModelProvider Trait

**Location**: `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

```rust
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: Box<dyn Fn() + Send>,
        when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse, eyre::Error>;
}
```

### 5.2 Current Request/Response Structures

```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,
    pub context: Option<String>,
    pub conversation_id: Option<String>,
}

pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,  // ⚠️ Simple string, no tool support
}

pub struct ModelResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,  // ✓ Already present!
}

pub struct ToolRequest {
    pub tool_name: String,
    pub parameters: String,  // ⚠️ String, not structured JSON
}

pub enum ModelResponseChunk {
    AssistantMessage(String),
    ToolUseRequest { tool_name: String, parameters: String },
}
```

### 5.3 Required Extensions

To support tools properly:

1. **Add tool_use_id to ToolRequest**:
   ```rust
   pub struct ToolRequest {
       pub tool_use_id: String,  // NEW
       pub tool_name: String,
       pub parameters: serde_json::Value,  // Changed from String
   }
   ```

2. **Add tool definitions to ModelRequest**:
   ```rust
   pub struct ModelRequest {
       // ... existing fields
       pub tools: Vec<ToolDefinition>,  // NEW
   }
   
   pub struct ToolDefinition {
       pub name: String,
       pub description: String,
       pub input_schema: serde_json::Value,
   }
   ```

3. **Support tool results in messages**:
   - ConversationMessage needs to support tool result content
   - Or use richer message structure from main chat

### 5.4 Bedrock Implementation

**Location**: `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

Current implementation:
- Uses `converse_stream()` API
- Converts messages to Bedrock format
- Parses streaming response
- Returns ModelResponse

**Required Changes**:
- Add `toolConfig` to request
- Parse `ContentBlockDelta::ToolUse` events
- Accumulate tool input JSON chunks
- Extract `toolUseId` from response

### 5.5 CodeWhisperer Implementation

**Status**: Not yet implemented in agent_env

**Required**:
- Create `CodeWhispererModelProvider` implementing ModelProvider trait
- Use `generate_assistant_response()` or `send_message()` API
- Convert ModelRequest to ConversationState
- Add tools to `userInputMessageContext`
- Parse `toolUseEvent` from streaming response
- Convert response to ModelResponse

---

## Section 6: AgentLoop Execution Flow

### 6.1 Current Implementation

**Location**: `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

Current flow:
1. Check cancellation
2. Set worker state to Working
3. Build ModelRequest using ContextBuilder
4. Query LLM via ModelProvider
5. Publish OutputChunk events for streaming
6. Publish ToolUseRequestReceived events for tool requests
7. Publish ResponseReceived event
8. Add assistant message to conversation history
9. Set completion state metadata
10. Set worker state to Inactive

### 6.2 Worker State Transitions

```
Inactive → Working → Requesting → Receiving → Inactive
                                            ↓
                                    InactiveFailed (on error)
```

### 6.3 Tool Request Handling (Current)

```rust
// Publish events for tool use requests
for tool_request in &response.tool_requests {
    let tool_input: serde_json::Value = serde_json::from_str(&tool_request.parameters)
        .unwrap_or_else(|_| serde_json::Value::String(tool_request.parameters.clone()));

    // Publish OutputChunk event
    self.event_bus.publish(AgentEnvironmentEvent::Job(
        JobEvent::OutputChunk {
            chunk: OutputChunk::ToolUse {
                tool_name: tool_request.tool_name.clone(),
                tool_input: tool_input.clone(),
            },
            ...
        }
    ));

    // Publish AgentLoopEvent
    self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
        AgentLoopEvent::ToolUseRequestReceived { ... }
    ));
}
```

**Key Observation**: Tool requests are detected and events published, but **no execution happens**.

### 6.4 Completion State Metadata

```rust
if !response.tool_requests.is_empty() {
    self.worker.set_task_metadata(
        task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
        serde_json::Value::String("completed_with_tool_request".to_string()),
    );
} else {
    self.worker.set_task_metadata(
        task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
        serde_json::Value::String("completed_ready_for_prompt".to_string()),
    );
}
```

This metadata signals to UI whether tool approval is needed.

### 6.5 Required Changes for Tool Execution

1. **Add ToolProvider to Worker**
2. **Check for pending approvals on task start**
3. **Execute approved tools**
4. **Handle tool execution results**:
   - Success → Add to history, continue loop
   - Failure → Add error to history, continue loop
   - ApprovalRequired → Store request, exit task
5. **Implement tool execution loop**:
   - Query LLM
   - If tool requests → Execute or request approval
   - If all tools executed → Query LLM again with results
   - If approval needed → Exit and wait
6. **Update completion state** to distinguish:
   - Ready for prompt
   - Waiting for tool approval
   - Tool execution in progress

---

## Section 7: Approval System Analysis

### 7.1 Current Approval Flow

**Location**: `crates/chat-cli/src/cli/chat/mod.rs`

Current implementation uses state machine:

```rust
pub struct Chat {
    pending_tool_index: Option<usize>,  // Which tool needs approval
    tool_uses: Vec<AssistantToolUse>,   // All tool requests
    tool_use_status: ToolUseStatus,     // Idle/Pending/Executing
    // ... other fields
}
```

### 7.2 Approval Prompt UI

```rust
execute!(
    self.stderr,
    style::Print("\nAllow this action? Use '"),
    style::SetForegroundColor(Color::Green),
    style::Print("t"),
    style::SetForegroundColor(Color::DarkGrey),
    style::Print("' to trust (always allow) this tool for the session. ["),
    style::SetForegroundColor(Color::Green),
    style::Print("y/n/t"),
    style::Print("]:\n\n"),
)?;
```

User responses:
- **'y'** or **'Y'**: Approve this invocation
- **'n'** or **'N'**: Reject this invocation
- **'t'** or **'T'**: Trust tool for session (add to allowed_tools)

### 7.3 Trust Management

```rust
if is_trust {
    let formatted_tool_name = /* ... get tool name ... */;
    self.conversation.agents.trust_tools(vec![formatted_tool_name]);
    
    // Print updated permissions
    agent.print_overridden_permissions(&mut self.stderr)?;
}
tool_use.accepted = true;
```

Trust is managed via `Agents::trust_tools()`:
```rust
pub fn trust_tools(&mut self, tool_names: Vec<String>) {
    if let Some(agent) = self.get_active_mut() {
        agent.allowed_tools.extend(tool_names);
    }
}
```

### 7.4 Rejection Handling

```rust
if ["n", "N"].contains(&user_input.trim()) {
    let user_input = "I deny this tool request. Ask a follow up question clarifying the expected action".to_string();
    self.conversation.abandon_tool_use(&self.tool_uses, user_input);
}
```

Rejection sends message to LLM explaining denial.

### 7.5 Approval Characteristics

- **Per-invocation**: Each tool call requires approval
- **Session-wide trust**: 't' command adds to allowed_tools for session
- **Synchronous**: Blocks until user responds
- **Single tool at a time**: Uses pending_tool_index for one tool
- **State-based**: Uses ChatState enum to manage flow

### 7.6 Agent_Env Approval Requirements

For agent_env architecture:

1. **Worker-based state**: Store pending approvals in Worker
2. **Event-driven**: Publish approval request events
3. **Asynchronous**: Don't block AgentLoop
4. **Multiple tools**: Support multiple pending approvals
5. **UI-agnostic**: Work with TextUi and StructuredIO

**Proposed Structure**:
```rust
pub struct Worker {
    // ... existing fields
    pub open_tools_approval_requests: Arc<Mutex<Vec<ToolApprovalRequest>>>,
}

pub struct ToolApprovalRequest {
    pub approval_id: Uuid,
    pub tool_request: ToolRequest,
    pub approval_status: Option<ToolApprovalStatus>,
    pub created_at: Instant,
}

pub enum ToolApprovalStatus {
    Approved,
    Rejected { reason: Option<String> },
}
```

**Flow**:
1. AgentLoop detects tool request
2. ToolProvider returns ApprovalRequired
3. AgentLoop stores in open_tools_approval_requests
4. AgentLoop publishes ToolApprovalRequestEvent
5. AgentLoop exits task
6. UI displays approval prompt
7. User responds
8. UI updates approval_status
9. UI publishes ToolApprovalUpdatedEvent
10. UI relaunches AgentLoop
11. AgentLoop checks pending approvals on start
12. AgentLoop executes approved tools


## Section 8: Critical Integration Challenges

### 8.1 API Format Mismatch

**Challenge**: Bedrock and CodeWhisperer have similar but distinct tool formats.

**Impact**: High - Core functionality depends on correct API usage

**Details**:
- Both use `toolUseId` but in different locations
- Bedrock: Tool results in message content
- CodeWhisperer: Tool results in userInputMessageContext
- Need unified internal format with conversion functions

**Mitigation**:
- Create unified ToolRequest/ToolResult structures
- Implement conversion functions for each API
- Test with both providers

### 8.2 Tool Parameters as Structured JSON

**Challenge**: Current ModelProvider uses String for parameters, APIs need serde_json::Value.

**Impact**: High - Cannot properly parse or validate tool parameters

**Details**:
- Current: `parameters: String`
- Required: `parameters: serde_json::Value`
- Bedrock streams tool input as JSON string chunks
- Need to accumulate and parse

**Mitigation**:
- Change ToolRequest.parameters to serde_json::Value
- Implement JSON accumulation for streaming
- Add parameter validation

### 8.3 Message Structure Complexity

**Challenge**: UserMessage/AssistantMessage in main chat are complex with many variants.

**Impact**: Medium - Affects history management and API conversion

**Details**:
- Main chat structures have many fields (additional_context, env_context, timestamp, images)
- Agent_env currently uses simpler structures
- Need to decide: use main chat structures or extend agent_env structures

**Mitigation**:
- Use main chat structures (already support tools)
- Import only needed functionality
- Document which fields are used

### 8.4 ToolManager Complexity and Coupling

**Challenge**: Existing ToolManager is tightly coupled to current chat flow.

**Impact**: High - Scope says "reuse existing code" but direct reuse is difficult

**Details**:
- ToolManager manages MCP servers (out of scope for this workflow)
- Uses global state and session-wide tools
- Handles tool namespacing for MCP
- Tightly integrated with current Chat struct

**Mitigation**:
- Create new ToolProvider for agent_env
- Reuse tool trait pattern (validate, eval_perm, invoke)
- Reuse permission evaluation logic
- Defer MCP integration to mvp-tools-mcp workflow

### 8.5 Agent Config Integration

**Challenge**: Agent struct has tools_settings and allowed_tools, need to pass to Worker/ToolProvider.

**Impact**: Medium - Required for permission evaluation

**Details**:
- Agent config currently loaded in main chat
- Worker doesn't have reference to Agent
- Need to pass Agent config to Worker during construction
- ToolProvider needs Agent config for permission evaluation

**Mitigation**:
- Add Agent field to Worker
- Pass Agent config in WorkerBuilder
- ToolProvider accesses Agent via Worker

### 8.6 Approval State Management

**Challenge**: Current uses pending_tool_index in Chat struct, agent_env needs Worker-based state.

**Impact**: High - Core approval flow depends on this

**Details**:
- Current: Single pending_tool_index in Chat
- Required: Multiple pending approvals in Worker
- Need to support multiple tool requests per LLM response
- Need to track approval status per tool

**Mitigation**:
- Add open_tools_approval_requests to Worker
- Use Vec<ToolApprovalRequest> for multiple requests
- Track approval_status per request
- AgentLoop checks on task start

### 8.7 Tool Execution Context

**Challenge**: Tools need Os, working_directory, other context - Worker doesn't store Os.

**Impact**: Medium - Tools cannot execute without context

**Details**:
- Tools need Os for file operations
- Tools need working_directory for path resolution
- Worker currently doesn't store Os
- ContextBuilder needs Os for building requests

**Mitigation**:
- Add Os field to Worker
- Set Os during Worker construction
- Pass Os to ToolProvider for execution
- Create ToolContext struct with needed fields

### 8.8 History Conversion

**Challenge**: Need to convert between agent_env ConversationHistory and API-specific formats.

**Impact**: High - Required for both Bedrock and CodeWhisperer

**Details**:
- Bedrock: messages array with role and content blocks
- CodeWhisperer: alternating userInputMessage and assistantResponseMessage
- Tool results formatted differently in each API
- Need to preserve tool_use_id for correlation

**Mitigation**:
- ContextBuilder converts ConversationHistory into shared ModelRequest structure
- Each ModelProvider implements its own specific logic how to convert to its underlying LLM client
- Use main chat message structures (already have conversions)
- Test round-trip conversion
- Document format requirements

### 8.9 Streaming Tool Input

**Challenge**: Bedrock streams tool input as JSON string chunks - need to accumulate before parsing.

**Impact**: Medium - Affects Bedrock implementation

**Details**:
- ContentBlockDelta events contain partial JSON strings
- Need to accumulate until ContentBlockStop
- Then parse complete JSON
- Handle malformed JSON gracefully

**Mitigation**:
- Accumulate tool input in BedrockModelProvider
- Parse on ContentBlockStop
- Add error handling for malformed JSON
- Log parsing errors

### 8.10 Multiple Tool Requests

**Challenge**: Single LLM response can have multiple tool requests - need to handle approval for each.

**Impact**: High - Core functionality requirement

**Details**:
- LLM can request multiple tools in one response
- Each tool needs separate approval (unless pre-approved)
- Need to track approval status for each
- AgentLoop should only restart when all approved

**Mitigation**:
- Use Vec<ToolApprovalRequest> in Worker
- Track approval_status per request
- UI iterates through pending requests
- AgentLoop checks all have status before executing

### 8.11 Tool Result Format

**Challenge**: Bedrock wants toolResult with status, CodeWhisperer wants ToolResult in array.

**Impact**: Medium - Affects API conversion

**Details**:
- Bedrock: toolResult in message content with optional status field
- CodeWhisperer: ToolResult array in userInputMessageContext
- Need unified internal format
- Need conversion functions for each API

**Mitigation**:
- Create unified ToolResult struct
- Each ModelProvider implements its own specific logic how to convert to its underlying LLM client
- Test with both APIs

### 8.12 Session vs Worker Scope

**Challenge**: Tools could be session-wide (shared) or worker-specific.

**Impact**: Medium - Affects architecture design

**Details**:
- Scope suggests worker-specific (ToolProvider in Worker)
- Existing code uses session-wide tools (ToolManager)
- MCP servers are session-wide (expensive to initialize)
- Built-in tools could be worker-specific

**Mitigation**:
- Use worker-specific ToolProvider for built-in tools
- Store tool registry in Worker
- Defer MCP integration (session-wide) to mvp-tools-mcp
- Document scoping decision

### 8.13 Summary of Challenges

| Challenge | Impact | Complexity | Mitigation Effort |
|-----------|--------|------------|-------------------|
| API Format Mismatch | High | Medium | Medium |
| Tool Parameters JSON | High | Low | Low |
| Message Structure | Medium | Medium | Low |
| ToolManager Coupling | High | High | High |
| Agent Config Integration | Medium | Medium | Medium |
| Approval State | High | Medium | Medium |
| Tool Execution Context | Medium | Low | Low |
| History Conversion | High | High | High |
| Streaming Tool Input | Medium | Medium | Medium |
| Multiple Tool Requests | High | Medium | Medium |
| Tool Result Format | Medium | Low | Low |
| Session vs Worker Scope | Medium | Low | Low |

**Total Estimated Effort**: High (76-98 hours as per scope document)


## Section 9: Recommendations and Design Considerations

### 9.1 Architecture Recommendations

#### ToolProvider Design

**Recommendation**: Create new ToolProvider for agent_env, don't reuse ToolManager directly.

**Rationale**:
- ToolManager is tightly coupled to current chat flow
- MCP integration is separate workflow (mvp-tools-mcp)
- Worker-specific tools align with agent_env architecture
- Simpler implementation for MVP

**Structure**:
```rust
pub struct ToolProvider {
    tools: HashMap<String, Arc<dyn Tool>>,
    agent: Arc<Agent>,  // For permission evaluation
}

impl ToolProvider {
    pub async fn execute_tool(&self, request: ToolRequest, context: &ToolContext) -> ToolResult;
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition>;
    pub fn requires_approval(&self, tool_name: &str) -> bool;
}
```

#### Worker Integration

**Recommendation**: Add tool-related fields to Worker.

```rust
pub struct Worker {
    // ... existing fields
    pub tool_provider: Option<Arc<Mutex<ToolProvider>>>,
    pub open_tools_approval_requests: Arc<Mutex<Vec<ToolApprovalRequest>>>,
    pub agent: Option<Arc<Agent>>,
    pub os: Option<Arc<Os>>,
}
```

### 9.2 Message Structure Recommendations

**Recommendation**: Use main chat message structures (UserMessage, AssistantMessage) in agent_env.

**Rationale**:
- Already support tool use tracking
- Have proven API conversions
- Reduce duplication
- Easier to merge implementations later

**Implementation**:
- Import from `crate::cli::chat::message`
- Use in ConversationHistory
- Leverage existing conversion functions

### 9.3 ModelProvider Extensions

**Recommendation**: Extend ModelProvider structures incrementally.

**Phase 1**: Add tool support to existing structures
```rust
pub struct ToolRequest {
    pub tool_use_id: String,  // NEW
    pub tool_name: String,
    pub parameters: serde_json::Value,  // Changed from String
}

pub struct ModelRequest {
    // ... existing fields
    pub tools: Vec<ToolDefinition>,  // NEW
}
```

**Phase 2**: Implement in Bedrock provider
- Add toolConfig to request
- Parse tool use from response
- Handle streaming tool input

**Phase 3**: Implement CodeWhisperer provider
- Create new provider
- Add tools to userInputMessageContext
- Parse toolUseEvent from response

### 9.4 AgentLoop Execution Strategy

**Recommendation**: Implement approval-first execution pattern.

**Flow**:
1. **Task Start**: Check for pending approvals
   - If pending and unapproved → Exit immediately
   - If pending and approved → Execute tools, add results to history
2. **Query LLM**: Build request with tools and history
3. **Process Response**:
   - If tool requests → Check approval for each
   - If all approved → Execute and continue loop
   - If any need approval → Store requests, exit task
4. **Continue Loop**: If tools executed, query LLM again with results

**Benefits**:
- Clean separation of approval and execution
- Supports multiple tool requests
- Works with event-driven architecture
- UI can relaunch task after approval

### 9.5 Approval System Design

**Recommendation**: Use Worker-based approval state with event-driven UI.

**Components**:
```rust
pub struct ToolApprovalRequest {
    pub approval_id: Uuid,
    pub tool_request: ToolRequest,
    pub approval_status: Option<ToolApprovalStatus>,
    pub created_at: Instant,
}

pub enum ToolApprovalStatus {
    Approved,
    Rejected { reason: Option<String> },
}
```

**Events**:
- `ToolApprovalRequestEvent`: Published when approval needed
- `ToolApprovalUpdatedEvent`: Published when user responds
- `ToolExecutionStartedEvent`: Published when tool starts
- `ToolExecutionCompletedEvent`: Published when tool finishes

**UI Integration**:
- TextUi: Display approval prompt, read user input, update status
- StructuredIO: Output JSON approval request, read JSON response, update status
- Both: Relaunch AgentLoop after all approvals set

### 9.6 Tool Implementation Strategy

**Recommendation**: Port fs_read and fs_write first, establish migration pattern.

**Steps**:
1. Create Tool trait for agent_env
2. Implement FsReadTool and FsWriteTool
3. Reuse existing logic from main chat tools
4. Integrate with ToolProvider
5. Document migration pattern for other tools

**Tool Trait**:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, parameters: serde_json::Value, context: &ToolContext) -> Result<String>;
}
```

### 9.7 Testing Strategy

**Recommendation**: Test at multiple levels.

**Unit Tests**:
- ToolProvider tool registration and execution
- Permission evaluation logic
- Message structure conversions
- API format conversions

**Integration Tests**:
- AgentLoop with tool execution
- Approval flow (mock UI)
- Multi-tool execution
- Error handling

**End-to-End Tests**:
- Full conversation with tools (Bedrock)
- Full conversation with tools (CodeWhisperer)
- Approval flow (TextUi and StructuredIO)
- Tool failure scenarios

### 9.8 Implementation Priorities

**Critical Path** (must be done in order):
1. Extend ModelProvider structures (ToolRequest, ModelRequest)
2. Implement Bedrock tool support
3. Create ToolProvider and Tool trait
4. Add tool fields to Worker
5. Implement AgentLoop tool execution flow
6. Implement approval system
7. Integrate with TextUi
8. Implement fs_read and fs_write

**Parallel Work** (can be done concurrently):
- CodeWhisperer provider implementation
- StructuredIO integration
- Event definitions and publishing
- Documentation

### 9.9 Risk Mitigation

**High-Risk Areas**:
1. **History Conversion**: Complex, affects both APIs
   - Mitigation: Reuse main chat conversions, test thoroughly
2. **ToolManager Coupling**: Scope says reuse, but difficult
   - Mitigation: Create new ToolProvider, reuse patterns not code
3. **Multiple Tool Requests**: Complex approval flow
   - Mitigation: Design approval state carefully, test edge cases

**Medium-Risk Areas**:
1. **Streaming Tool Input**: Bedrock-specific complexity
   - Mitigation: Implement accumulation logic, test with real API
2. **Agent Config Integration**: Need to pass through Worker
   - Mitigation: Add Agent field to Worker, document usage

### 9.10 Success Criteria

**Functional**:
- ✓ LLM can request fs_read and fs_write tools
- ✓ Tools execute successfully
- ✓ Tool results return to LLM
- ✓ User can approve/reject via TextUi
- ✓ User can approve/reject via StructuredIO
- ✓ Both Bedrock and CodeWhisperer support tools

**Quality**:
- ✓ Test coverage > 80%
- ✓ No clippy warnings
- ✓ All code documented
- ✓ Architecture documentation updated

**Performance**:
- ✓ Tool execution latency < 100ms
- ✓ Approval prompt response < 50ms
- ✓ No memory leaks

---

## Conclusion

This research provides a comprehensive foundation for designing and implementing tool support in the agent_env architecture. The key findings are:

1. **Architecture is Ready**: EventBus and Worker patterns support tool integration
2. **APIs are Similar**: Bedrock and CodeWhisperer have compatible tool formats
3. **Existing Code is Reusable**: Tool patterns and permission logic can be reused
4. **Challenges are Manageable**: 12 identified challenges have clear mitigations
5. **Path Forward is Clear**: Recommendations provide actionable design guidance

The next phase (Design) should focus on:
- Detailed component designs
- API conversion specifications
- Approval flow sequence diagrams
- Tool trait and ToolProvider interfaces
- Integration points with existing code

**Estimated Effort**: 76-98 hours (as per scope document)  
**Risk Level**: Medium-High (due to API complexity and integration challenges)  
**Confidence**: High (research is thorough, path is clear)

