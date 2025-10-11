# MVP Tools Basic - Research and Analysis

## Document Plan

This research document covers the following sections:

1. Overview
2. Current Tool System Analysis
3. How Tools Are Sent to LLM (CodeWhisperer/QDeveloper)
4. Existing Tool Implementations (fs_read, fs_write)
5. CodeWhisperer/QDeveloper API Format
6. Bedrock API Research Needs
7. Critical Issues and Challenges
8. Integration Requirements for agent_env
9. Key Files Reference

---

## 1. Overview

This document contains research findings for integrating the tool system into the agent_env architecture. The goal is to understand:

- How tools currently work in the old chat architecture
- How tool specifications are formatted and sent to LLMs
- How tool use requests and results are handled
- What needs to be adapted for agent_env
- Differences between CodeWhisperer/QDeveloper and Bedrock APIs

This research focuses on fs_read and fs_write as the initial tools to implement, with a strategy for migrating other tools.

---
---

## 2. Current Tool System Analysis


### Tool Enum Structure

**Location:** `crates/chat-cli/src/cli/chat/tools/mod.rs`

The current tool system uses an enum to represent all available tools:

```rust
pub enum Tool {
    FsRead(FsRead),
    FsWrite(FsWrite),
    ExecuteCommand(ExecuteCommand),
    UseAws(UseAws),
    Custom(CustomTool),
    GhIssue(GhIssue),
    Introspect(Introspect),
    Knowledge(Knowledge),
    Thinking(Thinking),
    Todo(TodoList),
    Delegate(Delegate),
}
```

**Key Methods:**
- `display_name() -> String` - Returns tool name (e.g., "fs_read", "execute_bash")
- `requires_acceptance(&self, os: &Os, agent: &Agent) -> PermissionEvalResult` - Permission check
- `async invoke(&self, os: &Os, stdout: &mut impl Write, ...) -> Result<InvokeOutput>` - Executes tool
- `async queue_description(&self, os: &Os, output: &mut impl Write)` - Displays intent to user
- `async validate(&mut self, os: &Os)` - Validates parameters before execution

**Observations:**
- ✅ Well-structured with clear separation of concerns
- ✅ Permission system integrated at tool level
- ✅ Async execution support
- ❌ Tightly coupled to old architecture (ConversationState, ToolContext)
- ❌ No cancellation support
- ❌ Enum-based design doesn't support dynamic tool loading (MCP)

### ToolSpec Structure

**Location:** `crates/chat-cli/src/cli/chat/tools/mod.rs`

Tool specifications are sent to the LLM to describe available tools:

```rust
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
    pub tool_origin: ToolOrigin,  // Native or McpServer
}

pub struct InputSchema(pub serde_json::Value);
```

**Example from tool_index.json:**
```json
{
  "fs_read": {
    "name": "fs_read",
    "description": "Tool for reading files, directories and images...",
    "input_schema": {
      "type": "object",
      "properties": {
        "operations": {
          "type": "array",
          "items": { ... }
        }
      },
      "required": ["operations"]
    }
  }
}
```

**Observations:**
- ✅ JSON Schema format for parameters
- ✅ Supports complex nested structures
- ✅ Tool origin tracking (native vs MCP)
- ⚠️ InputSchema is just a wrapper around serde_json::Value
- ⚠️ No validation of schema format

### Permission System

**Location:** `crates/chat-cli/src/cli/agent/mod.rs` and tool implementations

```rust
pub enum PermissionEvalResult {
    Allow,
    Ask,
    Deny(Vec<String>),  // With reasons
}
```

**Permission Evaluation:**
Each tool implements `eval_perm()` which checks:
1. `--trust-all-tools` flag → Allow
2. Tool in `agent.allowed_tools` → Allow
3. Tool-specific settings (e.g., `allowed_paths` for fs_read)
4. Denied paths/patterns → Deny with reasons
5. Otherwise → Ask for approval

**Example from FsRead:**
```rust
pub fn eval_perm(&self, os: &Os, agent: &Agent) -> PermissionEvalResult {
    let settings = agent.tools_settings.get("fs_read");
    // Check allowed_paths, denied_paths, allow_read_only
    // Build glob sets for path matching
    // Return Allow, Ask, or Deny
}
```

**Observations:**
- ✅ Flexible permission model
- ✅ Tool-specific configuration via agent.tools_settings
- ✅ Glob pattern support for path-based tools
- ✅ Clear deny reasons for user feedback
- ⚠️ Permission state not persisted across invocations
- ⚠️ No per-session approval tracking

### Tool Execution Flow (Old Architecture)

**High-level flow:**
1. LLM returns response with tool use requests
2. Parser extracts ToolUse events from stream
3. Tools are queued for approval
4. User approves/denies each tool
5. Approved tools are executed via `tool.invoke()`
6. Results are formatted and sent back to LLM
7. LLM continues with tool results

**Key Components:**
- **ToolManager**: Manages tool discovery, MCP servers, tool specs
- **QueuedTool**: Represents a tool waiting for approval
- **InvokeOutput**: Result from tool execution

```rust
pub struct QueuedTool {
    pub id: String,
    pub name: String,
    pub accepted: bool,
    pub tool: Tool,
    pub tool_input: serde_json::Value,
}

pub struct InvokeOutput {
    pub output: OutputKind,
}

pub enum OutputKind {
    Text(String),
    Json(serde_json::Value),
    Images(RichImageBlocks),
    Mixed { text: String, images: RichImageBlocks },
}
```

**Observations:**
- ✅ Clear separation between queueing and execution
- ✅ Support for multiple output types
- ❌ No cancellation mechanism
- ❌ Synchronous approval flow blocks execution
- ❌ No timeout handling


---

## 3. How Tools Are Sent to LLM (CodeWhisperer/QDeveloper)

### UserInputMessageContext Structure

**Location:** `crates/chat-cli/src/api_client/model.rs`

Tools are sent to the LLM via the `UserInputMessageContext`:

```rust
pub struct UserInputMessageContext {
    pub env_state: Option<EnvState>,
    pub git_state: Option<GitState>,
    pub tool_results: Option<Vec<ToolResult>>,  // Results from previous tool executions
    pub tools: Option<Vec<Tool>>,                // Available tool specifications
}
```

**Tool Specification Format:**

```rust
pub enum Tool {
    ToolSpecification(ToolSpecification),
}

pub struct ToolSpecification {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
}

pub struct ToolInputSchema {
    pub json: Option<FigDocument>,  // JSON Schema as AWS Document
}
```

**Observations:**
- ✅ Tools are sent in every request (available tools list)
- ✅ Tool results are sent separately (from previous executions)
- ✅ Supports both CodeWhisperer and QDeveloper APIs (via From traits)
- ⚠️ FigDocument is a wrapper around AWS Document type
- ⚠️ Tool list is static per request (no dynamic updates mid-conversation)

### Tool Use in LLM Responses

**Location:** `crates/chat-cli/src/api_client/model.rs`

LLM responses can contain tool use requests via streaming events:

```rust
pub enum ChatResponseStream {
    AssistantResponseEvent { content: String },
    ToolUseEvent {
        tool_use_id: String,
        name: String,
        input: Option<String>,  // JSON string
        stop: Option<bool>,
    },
    // ... other events
}
```

**ToolUse Structure (for results):**

```rust
pub struct ToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input: FigDocument,  // Parsed JSON parameters
}
```

**Observations:**
- ✅ Tool use requests are streamed incrementally
- ✅ Each tool use has unique ID for tracking
- ✅ Parameters are provided as JSON
- ⚠️ `stop` field indicates if tool use is complete (streaming)
- ⚠️ Input can be partial during streaming (need to accumulate)

### Tool Result Format

**Location:** `crates/chat-cli/src/api_client/model.rs`

After executing tools, results are sent back to LLM:

```rust
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    pub status: ToolResultStatus,
}

pub enum ToolResultContentBlock {
    Json(AwsDocument),
    Text(String),
}

pub enum ToolResultStatus {
    Error,
    Success,
}
```

**Observations:**
- ✅ Results linked to tool use via tool_use_id
- ✅ Supports both text and JSON content
- ✅ Status indicates success/failure
- ✅ Multiple content blocks per result
- ⚠️ No support for images in tool results (only text/JSON)

### Request/Response Flow

**Complete flow:**

1. **Initial Request:**
   ```rust
   UserInputMessage {
       content: "Read file.txt",
       context: UserInputMessageContext {
           tools: Some(vec![
               Tool::ToolSpecification(fs_read_spec),
               Tool::ToolSpecification(fs_write_spec),
           ]),
           tool_results: None,
       }
   }
   ```

2. **LLM Response (streaming):**
   ```rust
   ChatResponseStream::ToolUseEvent {
       tool_use_id: "toolu_123",
       name: "fs_read",
       input: Some(r#"{"operations": [{"mode": "Line", "path": "file.txt"}]}"#),
       stop: Some(true),
   }
   ```

3. **Tool Execution:**
   - Parse input JSON
   - Create FsRead tool instance
   - Check permissions
   - Execute tool
   - Format result

4. **Follow-up Request:**
   ```rust
   UserInputMessage {
       content: "",  // Empty for tool result continuation
       context: UserInputMessageContext {
           tools: Some(vec![...]),  // Same tools
           tool_results: Some(vec![
               ToolResult {
                   tool_use_id: "toolu_123",
                   content: vec![ToolResultContentBlock::Text("file contents...")],
                   status: ToolResultStatus::Success,
               }
           ]),
       }
   }
   ```

5. **LLM Continuation:**
   - LLM processes tool results
   - Generates final response or requests more tools

**Observations:**
- ✅ Clear request/response cycle
- ✅ Tool results are sent in follow-up request
- ✅ Tools list is included in every request
- ⚠️ No batching of tool executions (sequential)
- ⚠️ Empty content for tool result requests (might confuse conversation history)


---

## 4. Existing Tool Implementations (fs_read, fs_write)

### FsRead Implementation

**Location:** `crates/chat-cli/src/cli/chat/tools/fs_read.rs`

**Structure:**
```rust
pub struct FsRead {
    pub operations: Vec<FsReadOperation>,
    pub summary: Option<String>,
}

pub enum FsReadOperation {
    Line(FsLine),
    Directory(FsDirectory),
    Search(FsSearch),
    Image(FsImage),
}
```

**Key Features:**
- Batch operations support (multiple operations in one tool call)
- Four operation modes: Line, Directory, Search, Image
- Each operation has its own parameters and validation

**Permission Evaluation:**
```rust
pub fn eval_perm(&self, os: &Os, agent: &Agent) -> PermissionEvalResult {
    // Load settings from agent.tools_settings["fs_read"]
    let Settings {
        allowed_paths,
        denied_paths,
        allow_read_only,
    } = ...;
    
    // Build glob sets for allowed/denied paths
    // Check each operation's path against globs
    // Return Allow, Ask, or Deny with reasons
}
```

**Settings Structure:**
```rust
struct Settings {
    allowed_paths: Vec<String>,      // Glob patterns
    denied_paths: Vec<String>,       // Glob patterns
    allow_read_only: bool,           // Auto-approve read operations
}
```

**Execution:**
- Validates all operations before execution
- Executes operations sequentially
- Returns combined output (text or images)
- Handles errors gracefully per operation

**Observations:**
- ✅ Well-designed batch operation pattern
- ✅ Comprehensive permission system with glob support
- ✅ Supports multiple output types (text, images)
- ✅ CWD is automatically added to allowed_paths
- ⚠️ No cancellation support during execution
- ⚠️ Large file handling could be improved (no streaming)

### FsWrite Implementation

**Location:** `crates/chat-cli/src/cli/chat/tools/fs_write.rs`

**Structure:**
```rust
pub struct FsWrite {
    pub command: FsWriteCommand,
    pub summary: Option<String>,
}

pub enum FsWriteCommand {
    Create { path: String, file_text: String },
    StrReplace { path: String, old_str: String, new_str: String },
    Insert { path: String, insert_line: i32, new_str: String },
    Append { path: String, new_str: String },
}
```

**Key Features:**
- Four write operations: Create, StrReplace, Insert, Append
- Line tracking for multi-step edits
- Validation before execution
- Summary field for explaining intent

**Permission Evaluation:**
Similar to FsRead but with write-specific checks:
- Checks `allowed_paths` and `denied_paths`
- No `allow_read_only` equivalent (writes always need approval unless trusted)
- More restrictive by default

**Execution:**
- Validates path and operation
- Performs file operation
- Updates line tracker for subsequent operations
- Returns success/error message

**Observations:**
- ✅ Comprehensive write operations
- ✅ Line tracking for complex edits
- ✅ Clear operation semantics
- ⚠️ StrReplace requires exact match (can be fragile)
- ⚠️ No atomic operations (partial writes possible on error)
- ⚠️ No backup/undo mechanism

### Common Patterns

Both tools follow these patterns:

1. **Deserialization from JSON:**
   - Serde derives for automatic parsing
   - Tagged enums for operation types
   - Optional fields with defaults

2. **Validation:**
   - `async validate(&mut self, os: &Os)` method
   - Checks paths exist, parameters valid
   - Returns errors before execution

3. **Permission Checking:**
   - `eval_perm(&self, os: &Os, agent: &Agent)` method
   - Reads settings from agent config
   - Uses glob patterns for path matching
   - Returns Allow/Ask/Deny

4. **Execution:**
   - `async invoke(&self, os: &Os, stdout: &mut impl Write, ...)` method
   - Performs actual work
   - Writes progress to stdout
   - Returns InvokeOutput with result

5. **User Feedback:**
   - `async queue_description(&self, os: &Os, output: &mut impl Write)` method
   - Displays human-readable intent
   - Shows paths and operations
   - Helps user understand what will happen

**Key Dependencies:**
- `Os` struct for filesystem operations (abstraction for testing)
- `Agent` struct for configuration and permissions
- `FileLineTracker` for tracking file modifications
- Various utility functions for path handling

**Observations:**
- ✅ Consistent patterns across tools
- ✅ Good separation of concerns
- ✅ Testable via Os abstraction
- ❌ Tightly coupled to old architecture
- ❌ No cancellation token support
- ❌ Synchronous approval flow


---

## 5. CodeWhisperer/QDeveloper API Format

### API Client Structure

**Location:** `crates/chat-cli/src/api_client/model.rs`

The codebase supports both CodeWhisperer and QDeveloper APIs with identical formats:

```rust
// Conversion traits for both APIs
impl From<Tool> for amzn_codewhisperer_streaming_client::types::Tool { ... }
impl From<Tool> for amzn_qdeveloper_streaming_client::types::Tool { ... }
```

**Observations:**
- ✅ Both APIs use identical tool format
- ✅ Conversion is automatic via From traits
- ✅ Single codebase supports both backends

### Tool Specification Format

**Sent to LLM:**
```json
{
  "toolSpecification": {
    "name": "fs_read",
    "description": "Tool for reading files...",
    "inputSchema": {
      "json": {
        "type": "object",
        "properties": { ... },
        "required": [ ... ]
      }
    }
  }
}
```

**Key Points:**
- Tool name must match exactly for invocation
- Description guides LLM on when to use tool
- Input schema is JSON Schema format
- Schema is nested in `json` field (AWS Document format)

### Tool Use Request Format

**Received from LLM (streaming):**
```json
{
  "toolUseEvent": {
    "toolUseId": "toolu_abc123",
    "name": "fs_read",
    "input": "{\"operations\": [...]}",
    "stop": true
  }
}
```

**Parsed Structure:**
```rust
pub struct ToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input: FigDocument,  // Parsed JSON
}
```

**Key Points:**
- `toolUseId` is unique identifier for tracking
- `name` matches tool specification name
- `input` is JSON string (needs parsing)
- `stop` indicates if streaming is complete

### Tool Result Format

**Sent back to LLM:**
```json
{
  "toolResult": {
    "toolUseId": "toolu_abc123",
    "content": [
      { "text": "File contents here..." }
    ],
    "status": "success"
  }
}
```

**Structure:**
```rust
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    pub status: ToolResultStatus,
}

pub enum ToolResultContentBlock {
    Json(AwsDocument),
    Text(String),
}

pub enum ToolResultStatus {
    Error,
    Success,
}
```

**Key Points:**
- Must reference original `toolUseId`
- Content can be multiple blocks
- Supports both text and JSON content
- Status indicates success/failure

### Streaming Behavior

**Tool Use Streaming:**
1. LLM starts tool use: `ToolUseEvent { stop: false, input: Some("partial...") }`
2. LLM continues: `ToolUseEvent { stop: false, input: Some("more...") }`
3. LLM completes: `ToolUseEvent { stop: true, input: Some("final") }`

**Observations:**
- ✅ Tool use can be streamed incrementally
- ✅ `stop` field indicates completion
- ⚠️ Need to accumulate input across events
- ⚠️ Parser must handle partial JSON

### API Differences: CodeWhisperer vs QDeveloper

**From code analysis:**
```rust
// Both use identical structures
impl From<ToolSpecification> for amzn_codewhisperer_streaming_client::types::ToolSpecification { ... }
impl From<ToolSpecification> for amzn_qdeveloper_streaming_client::types::ToolSpecification { ... }
```

**Observations:**
- ✅ No format differences between APIs
- ✅ Same tool specification format
- ✅ Same tool use/result format
- ✅ Single implementation works for both

### Integration Points

**Where tools are added to requests:**
- Old architecture: In conversation state management
- Location: `crates/chat-cli/src/cli/chat/mod.rs` (old chat flow)
- Tools are loaded from ToolManager
- Tool specs converted to API format
- Added to UserInputMessageContext

**Where tool use is detected:**
- Streaming parser extracts ToolUseEvent
- Events are accumulated until `stop: true`
- Tool use is queued for approval
- After approval, tool is executed
- Result is formatted and sent in next request

**Observations:**
- ✅ Clear integration points
- ✅ Streaming support built-in
- ❌ Tightly coupled to old architecture
- ❌ No abstraction for different LLM backends


---

## 6. Bedrock API Research Needs

### Current Bedrock Implementation

**Location:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Current ModelRequest:**
```rust
pub struct ModelRequest {
    pub prompt: String,  // Single string only
}
```

**Current Implementation:**
```rust
let message = Message::builder()
    .role(ConversationRole::User)
    .content(ContentBlock::Text(request.prompt))
    .build()?;

let response = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .messages(message)
    .send()
```

**Observations:**
- ❌ No tool support in current implementation
- ❌ Only sends single text message
- ❌ No tool specifications in request
- ❌ No tool use detection in response
- ⚠️ Uses `converse_stream()` API (not `chat()` like CodeWhisperer)

### Critical Research Questions

#### Q1: Does Bedrock Converse API Support Tools?

**Need to investigate:**
- Does `converse_stream()` accept tool specifications?
- What is the parameter name? (`.tools()`, `.tool_config()`, etc.)
- What format does it expect?

**Hypothesis:**
- Bedrock likely supports tools (Claude models have tool use capability)
- Format might differ from CodeWhisperer
- May need to check AWS SDK documentation

**Investigation approach:**
```rust
// Check if BedrockClient has tool-related methods
self.client
    .converse_stream()
    .model_id(&self.model_id)
    .messages(messages)
    .tools(???)  // Does this exist?
    .send()
```

#### Q2: What is Bedrock's Tool Specification Format?

**Need to investigate:**
- Is it JSON Schema like CodeWhisperer?
- Does it use AWS Document format?
- Are there any format differences?

**Possible formats:**
```rust
// Option A: Similar to CodeWhisperer
pub struct BedrockToolSpec {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

// Option B: AWS-specific format
pub struct BedrockToolConfig {
    tools: Vec<ToolDefinition>,
    tool_choice: ToolChoice,  // auto, any, specific tool
}
```

#### Q3: How Does Bedrock Return Tool Use Requests?

**Need to investigate:**
- What streaming events indicate tool use?
- Is there a `ToolUseEvent` equivalent?
- How is tool input provided?

**Current response handling:**
```rust
match output {
    ConverseStreamOutput::ContentBlockDelta(delta) => {
        // Only handles text deltas
        if let ContentBlockDelta::Text(text) = delta_content {
            accumulated_content.push_str(&text);
        }
    }
    ConverseStreamOutput::MessageStop(_) => break,
    _ => {}
}
```

**Possible tool use events:**
```rust
// Need to check if these exist:
ConverseStreamOutput::ToolUse(???)
ConverseStreamOutput::ToolUseStart(???)
ConverseStreamOutput::ToolUseEnd(???)
```

#### Q4: How to Send Tool Results Back to Bedrock?

**Need to investigate:**
- How are tool results formatted?
- Are they sent as messages or separate field?
- What is the structure?

**Possible approaches:**
```rust
// Option A: Tool results as user messages
Message::builder()
    .role(ConversationRole::User)
    .content(ContentBlock::ToolResult(result))
    .build()

// Option B: Separate tool_results field
self.client
    .converse_stream()
    .messages(messages)
    .tools(tool_specs)
    .tool_results(results)  // Does this exist?
    .send()
```

#### Q5: Does Bedrock Support System Prompts?

**Related to Task 1.4 (Agent Context):**
- Does `converse_stream()` accept system messages?
- Is there a separate `system` parameter?
- How are system prompts formatted?

**Investigation:**
```rust
self.client
    .converse_stream()
    .model_id(&self.model_id)
    .system(???)  // Does this exist?
    .messages(messages)
    .send()
```

### Research Strategy

**Step 1: Check AWS SDK Documentation**
- Look for `aws-sdk-bedrockruntime` crate documentation
- Check `ConverseStreamFluentBuilder` methods
- Look for tool-related types

**Step 2: Examine AWS SDK Types**
- Check `aws_sdk_bedrockruntime::types` module
- Look for Tool, ToolConfig, ToolUse, ToolResult types
- Compare with CodeWhisperer types

**Step 3: Prototype Testing**
- Create minimal test with Bedrock API
- Try sending tool specifications
- Observe response format
- Document findings

**Step 4: Compare with Claude API**
- Bedrock uses Claude models
- Check Anthropic's Claude API documentation
- Tool use format should be similar

### Expected Findings

**Likely scenario:**
- Bedrock supports tools (Claude has tool use)
- Format similar to CodeWhisperer (both AWS services)
- May have minor differences in structure
- Will need adapter layer for agent_env

**Worst case scenario:**
- Bedrock doesn't support tools via converse_stream
- Need to use different API endpoint
- Significant refactoring required

**Best case scenario:**
- Bedrock format identical to CodeWhisperer
- Can reuse existing structures
- Minimal changes needed

### Documentation to Review

1. **AWS SDK for Rust - Bedrock Runtime:**
   - https://docs.rs/aws-sdk-bedrockruntime/
   - Check `ConverseStreamFluentBuilder` methods
   - Look for tool-related types

2. **AWS Bedrock Documentation:**
   - Converse API reference
   - Tool use examples
   - Model-specific capabilities

3. **Anthropic Claude Documentation:**
   - Tool use format
   - Best practices
   - Limitations

4. **Existing Codebase:**
   - Check if there are any Bedrock tool examples
   - Look for commented-out code
   - Search for "tool" in Bedrock-related files


---

## 7. Critical Issues and Challenges

### Issue 1: ModelProvider Interface Too Simple

**Problem:** Current ModelProvider only supports text prompts, no tool specifications.

**Current Interface:**
```rust
pub struct ModelRequest {
    pub prompt: String,  // ❌ No tool support
}

pub struct ModelResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,  // ✅ Has tool requests
}
```

**Impact:**
- Cannot send tool specifications to LLM
- Need to extend ModelRequest structure
- Affects all ModelProvider implementations

**Solution Approach:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // From Task 1.2
    pub tools: Vec<ToolDefinition>,          // NEW
    pub system_prompt: Option<String>,       // From Task 1.4
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

### Issue 2: Tool Storage and Registration

**Problem:** Where should tools be stored and how are they registered?

**Options:**

**Option A: Worker-Specific Tools**
- Each Worker has its own tool registry
- Pros: Worker isolation, different tools per worker
- Cons: Duplication, harder to share tools

**Option B: Session-Shared Tools**
- Session owns tool registry, workers reference it
- Pros: Shared resources, easier management
- Cons: Less isolation, global state

**Option C: Hybrid Approach**
- Session has default tools
- Workers can add worker-specific tools
- Pros: Flexibility, best of both worlds
- Cons: More complex

**Recommendation:** Option B (Session-shared) for MVP, Option C for future.

**Rationale:**
- Most tools are stateless (fs_read, fs_write, execute_bash)
- Sharing reduces duplication
- Simpler for initial implementation
- Can add worker-specific tools later

### Issue 3: Tool Execution Context

**Problem:** Tools need context to execute (working directory, environment, etc.)

**Current Tool Signature:**
```rust
async fn invoke(
    &self,
    os: &Os,
    stdout: &mut impl Write,
    line_tracker: &mut HashMap<String, FileLineTracker>,
    agents: &Agents,
) -> Result<InvokeOutput>
```

**Issues:**
- Too many parameters
- Tightly coupled to old architecture
- No cancellation support
- No worker context

**Proposed Solution:**
```rust
pub struct ToolContext {
    pub worker_id: WorkerId,
    pub working_directory: PathBuf,
    pub environment: HashMap<String, String>,
    pub os: Arc<Os>,
    pub cancellation_token: CancellationToken,
    // Future: line_tracker, agents, etc.
}

#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult>;
}
```

**Benefits:**
- Single context parameter
- Easy to extend
- Cancellation support
- Worker-aware

### Issue 4: Permission and Approval Flow

**Problem:** How to integrate permission checking and user approval in agent_env?

**Current Flow (Old Architecture):**
1. Tool use detected
2. Tool queued for approval
3. UI prompts user
4. User approves/denies
5. Tool executed
6. Result sent to LLM

**Challenges in agent_env:**
- AgentLoop runs in background task
- UI is separate from execution
- Need async approval mechanism
- Must support cancellation

**Proposed Solution:**

**Option A: Pause AgentLoop for Approval**
- AgentLoop detects tool use
- Publishes ToolApprovalNeeded event
- Waits on channel for approval
- Continues after approval

**Option B: Separate Approval Task**
- AgentLoop queues tools
- Separate task handles approvals
- Tools executed after approval
- Results sent back to AgentLoop

**Option C: Auto-Approve with Events**
- Check permissions immediately
- Auto-approve if allowed
- Publish events for UI display
- Only pause for Ask permissions

**Recommendation:** Option C for MVP (simpler), Option A for full implementation.

### Issue 5: Tool Result Handling

**Problem:** How to send tool results back to LLM and continue conversation?

**Challenges:**
- Tool results need to be added to conversation history
- Need to trigger new LLM request with results
- Must maintain conversation flow
- Handle multiple tool uses

**Proposed Flow:**
```
1. AgentLoop receives response with tool uses
2. Execute tools (with approval)
3. Format tool results
4. Add results to conversation history
5. Create new ModelRequest with tool results
6. Send to LLM
7. Continue loop
```

**Implementation:**
```rust
// In AgentLoop::run()
loop {
    let response = self.query_llm().await?;
    
    if !response.tool_requests.is_empty() {
        // Execute tools
        let results = self.execute_tools(&response.tool_requests).await?;
        
        // Add to history
        self.add_tool_results_to_history(results)?;
        
        // Continue loop (will send results in next request)
        continue;
    }
    
    // No tools, conversation complete
    break;
}
```

### Issue 6: Bedrock API Compatibility

**Problem:** Unknown if Bedrock supports tools, format may differ from CodeWhisperer.

**Risks:**
- Bedrock may not support tools at all
- Format may be incompatible
- May need significant refactoring

**Mitigation:**
- Research Bedrock API thoroughly (Section 6)
- Create abstraction layer for tool format
- Support multiple backends via adapters
- Fallback to CodeWhisperer if needed

**Proposed Abstraction:**
```rust
pub trait ModelProvider {
    async fn request(
        &self,
        request: ModelRequest,
        ...
    ) -> Result<ModelResponse>;
    
    fn supports_tools(&self) -> bool;
    fn tool_format(&self) -> ToolFormat;
}

pub enum ToolFormat {
    CodeWhisperer,
    Bedrock,
    OpenAI,  // Future
}
```

### Issue 7: Tool Migration Strategy

**Problem:** How to migrate existing tools to agent_env without breaking old architecture?

**Challenges:**
- Old chat still needs tools
- Can't break existing functionality
- Need gradual migration
- Shared code vs duplication

**Proposed Strategy:**

**Phase 1: Create New Tool Trait**
- Define agent_env Tool trait
- Keep old Tool enum
- No breaking changes

**Phase 2: Implement Adapters**
- Create adapters: old Tool → new Tool
- Reuse execution logic
- Minimal duplication

**Phase 3: Port Tools Gradually**
- Start with fs_read, fs_write
- Test thoroughly
- Port other tools incrementally

**Phase 4: Deprecate Old System**
- After all tools ported
- Remove old Tool enum
- Clean up code

**Example Adapter:**
```rust
pub struct FsReadAdapter {
    inner: crate::cli::chat::tools::fs_read::FsRead,
}

#[async_trait]
impl agent_env::Tool for FsReadAdapter {
    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        // Convert params to old format
        // Call old invoke method
        // Convert result to new format
    }
}
```

### Issue 8: Error Handling and Retries

**Problem:** How to handle tool execution errors and retries?

**Scenarios:**
- Tool execution fails (file not found, permission denied)
- Tool times out
- Tool is cancelled
- LLM requests invalid tool

**Proposed Handling:**
```rust
pub enum ToolExecutionError {
    NotFound(String),
    PermissionDenied(String),
    ExecutionFailed(String),
    Timeout,
    Cancelled,
    InvalidParameters(String),
}

impl ToolExecutionError {
    pub fn to_tool_result(&self) -> ToolResult {
        ToolResult {
            status: ToolResultStatus::Error,
            content: vec![ToolResultContentBlock::Text(self.to_string())],
        }
    }
}
```

**Retry Strategy:**
- No automatic retries (LLM decides)
- Return error as tool result
- LLM can retry with different parameters
- User can cancel or approve retry

### Issue 9: Performance and Concurrency

**Problem:** Tool execution can be slow, blocking AgentLoop.

**Considerations:**
- File operations can be slow
- Network requests (use_aws) can timeout
- Multiple tools may be requested

**Proposed Solutions:**
- Execute tools concurrently when possible
- Use timeouts for all operations
- Stream large outputs
- Cancel on user request

**Implementation:**
```rust
// Execute tools concurrently
let results = futures::future::join_all(
    tool_requests.iter().map(|req| {
        self.execute_tool_with_timeout(req, Duration::from_secs(30))
    })
).await;
```

### Issue 10: Testing Strategy

**Problem:** How to test tool integration without real LLM calls?

**Approaches:**
- Mock ModelProvider for unit tests
- Mock tools for integration tests
- Test tool execution separately
- Test permission system independently

**Example Mock:**
```rust
struct MockModelProvider {
    responses: Vec<ModelResponse>,
}

impl ModelProvider for MockModelProvider {
    async fn request(&self, ...) -> Result<ModelResponse> {
        Ok(self.responses.remove(0))
    }
}
```


---

## 8. Integration Requirements for agent_env

### 8.1 New Components Needed

#### ToolRegistry

**Purpose:** Manage available tools and their specifications.

**Location:** `crates/chat-cli/src/agent_env/tools/tool_registry.rs`

**Interface:**
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    tool_specs: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Arc<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn get_spec(&self, name: &str) -> Option<&ToolDefinition>;
    pub fn list_specs(&self) -> Vec<ToolDefinition>;
}
```

#### Tool Trait

**Purpose:** Define interface for all tools in agent_env.

**Location:** `crates/chat-cli/src/agent_env/tools/tool_trait.rs`

**Interface:**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult>;
    
    fn requires_approval(&self, context: &ToolContext) -> PermissionEvalResult;
}
```

#### ToolContext

**Purpose:** Provide execution context to tools.

**Location:** `crates/chat-cli/src/agent_env/tools/tool_context.rs`

**Structure:**
```rust
pub struct ToolContext {
    pub worker_id: WorkerId,
    pub working_directory: PathBuf,
    pub environment: HashMap<String, String>,
    pub os: Arc<Os>,
    pub agent: Arc<Agent>,
    pub cancellation_token: CancellationToken,
}
```

#### ToolResult

**Purpose:** Standardized tool execution result.

**Location:** `crates/chat-cli/src/agent_env/tools/tool_result.rs`

**Structure:**
```rust
pub struct ToolResult {
    pub status: ToolResultStatus,
    pub content: Vec<ToolResultContent>,
}

pub enum ToolResultStatus {
    Success,
    Error,
}

pub enum ToolResultContent {
    Text(String),
    Json(serde_json::Value),
}
```

#### ToolDefinition

**Purpose:** Tool specification for LLM.

**Location:** `crates/chat-cli/src/agent_env/tools/tool_definition.rs`

**Structure:**
```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

### 8.2 ModelProvider Extensions

#### Extended ModelRequest

**Location:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

**Changes:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // From Task 1.2
    pub tools: Vec<ToolDefinition>,          // NEW
    pub system_prompt: Option<String>,       // From Task 1.4
}
```

#### Extended ModelResponse

**Already has tool support:**
```rust
pub struct ModelResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,  // ✅ Already exists
}

pub struct ToolRequest {
    pub tool_name: String,
    pub parameters: String,  // JSON string
}
```

### 8.3 Session Extensions

#### Tool Registry Storage

**Location:** `crates/chat-cli/src/agent_env/session.rs`

**Changes:**
```rust
pub struct Session {
    // ... existing fields ...
    tool_registry: Arc<ToolRegistry>,  // NEW
}

impl Session {
    pub fn new(
        event_bus: EventBus,
        model_providers: Vec<Arc<dyn ModelProvider>>,
        tool_registry: Arc<ToolRegistry>,  // NEW
    ) -> Self;
    
    pub fn tool_registry(&self) -> &Arc<ToolRegistry>;
}
```

### 8.4 AgentLoop Extensions

#### Tool Execution Loop

**Location:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Changes:**
```rust
impl AgentLoop {
    async fn run(&self) -> Result<()> {
        loop {
            // Query LLM with tools
            let response = self.query_llm().await?;
            
            // Check for tool uses
            if !response.tool_requests.is_empty() {
                // Execute tools
                let results = self.execute_tools(&response.tool_requests).await?;
                
                // Add results to history
                self.add_tool_results_to_history(results)?;
                
                // Continue loop
                continue;
            }
            
            // No tools, done
            break;
        }
        
        Ok(())
    }
    
    async fn execute_tools(
        &self,
        requests: &[ToolRequest],
    ) -> Result<Vec<ToolResult>> {
        // Get tool registry from session
        // For each request:
        //   - Get tool from registry
        //   - Check permissions
        //   - Execute tool
        //   - Collect result
        // Return all results
    }
}
```

### 8.5 Event System Extensions

#### New Events

**Location:** `crates/chat-cli/src/agent_env/events.rs`

**New event types:**
```rust
pub enum AgentLoopEvent {
    // ... existing events ...
    ToolExecutionStarted {
        worker_id: WorkerId,
        job_id: JobId,
        tool_name: String,
        tool_input: serde_json::Value,
    },
    ToolExecutionCompleted {
        worker_id: WorkerId,
        job_id: JobId,
        tool_name: String,
        result: ToolResult,
    },
    ToolApprovalNeeded {
        worker_id: WorkerId,
        job_id: JobId,
        tool_name: String,
        tool_input: serde_json::Value,
    },
}
```

### 8.6 UI Extensions

#### Tool Approval Handling

**Location:** `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Changes:**
- Handle ToolApprovalNeeded events
- Prompt user for approval
- Send approval response via command channel
- Display tool execution progress

### 8.7 Bedrock Provider Extensions

#### Tool Support

**Location:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Changes:**
```rust
impl ModelProvider for BedrockConverseStreamModelProvider {
    async fn request(&self, request: ModelRequest, ...) -> Result<ModelResponse> {
        // Convert messages
        let messages = convert_messages(&request.messages);
        
        // Convert tools (if Bedrock supports them)
        let tools = convert_tools(&request.tools);
        
        // Build request
        let mut builder = self.client
            .converse_stream()
            .model_id(&self.model_id)
            .set_messages(Some(messages));
        
        // Add tools if supported
        if !tools.is_empty() {
            builder = builder.set_tools(Some(tools));  // Research needed
        }
        
        // Add system prompt if supported
        if let Some(system) = request.system_prompt {
            builder = builder.system(system);  // Research needed
        }
        
        let response = builder.send().await?;
        
        // Parse response including tool uses
        self.parse_response(response).await
    }
}
```

### 8.8 Tool Implementations

#### FsRead for agent_env

**Location:** `crates/chat-cli/src/agent_env/tools/fs_read.rs`

**Approach:**
- Reuse existing FsRead struct
- Implement new Tool trait
- Adapt to ToolContext
- Keep permission logic

**Implementation:**
```rust
pub struct FsReadTool {
    // Reuse existing implementation
}

#[async_trait]
impl Tool for FsReadTool {
    fn name(&self) -> &str { "fs_read" }
    
    fn description(&self) -> &str {
        "Tool for reading files, directories and images..."
    }
    
    fn input_schema(&self) -> serde_json::Value {
        // Load from tool_index.json or generate
    }
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        // Deserialize parameters to FsRead
        let fs_read: FsRead = serde_json::from_value(parameters)?;
        
        // Validate
        fs_read.validate(&context.os).await?;
        
        // Execute (adapt old invoke method)
        let output = execute_fs_read(fs_read, context).await?;
        
        // Convert to ToolResult
        Ok(ToolResult {
            status: ToolResultStatus::Success,
            content: vec![ToolResultContent::Text(output)],
        })
    }
    
    fn requires_approval(&self, context: &ToolContext) -> PermissionEvalResult {
        // Reuse existing permission logic
    }
}
```

#### FsWrite for agent_env

**Similar approach to FsRead.**

### 8.9 Integration with ChatArgs

**Location:** `crates/chat-cli/src/cli/chat/mod.rs`

**Changes:**
```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // ... existing EventBus, Session creation ...
        
        // NEW: Create and populate ToolRegistry
        let tool_registry = Arc::new(create_tool_registry());
        
        // NEW: Pass tool registry to Session
        let session = Arc::new(Session::new(
            event_bus.clone(),
            model_providers,
            tool_registry,
        ));
        
        // ... rest of initialization ...
    }
}

fn create_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    
    // Register native tools
    registry.register(Arc::new(FsReadTool::new()));
    registry.register(Arc::new(FsWriteTool::new()));
    // ... other tools ...
    
    registry
}
```

### 8.10 Testing Requirements

#### Unit Tests
- Tool trait implementations
- ToolRegistry operations
- Permission evaluation
- Tool result formatting

#### Integration Tests
- AgentLoop with mock tools
- Tool execution flow
- Error handling
- Cancellation

#### End-to-End Tests
- Real tool execution
- Multi-turn with tools
- Permission prompts
- Tool result continuation


---

## 9. Key Files Reference

### Existing Tool System

**Tool Definitions:**
- `crates/chat-cli/src/cli/chat/tools/mod.rs` - Tool enum, ToolSpec, common utilities
- `crates/chat-cli/src/cli/chat/tools/fs_read.rs` - FsRead implementation (1,400+ lines)
- `crates/chat-cli/src/cli/chat/tools/fs_write.rs` - FsWrite implementation (1,500+ lines)
- `crates/chat-cli/src/cli/chat/tools/tool_index.json` - Tool specifications in JSON

**Tool Management:**
- `crates/chat-cli/src/cli/chat/tool_manager.rs` - ToolManager for old architecture
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config with tool permissions

**API Models:**
- `crates/chat-cli/src/api_client/model.rs` - Tool, ToolSpecification, ToolUse, ToolResult
- `crates/chat-cli/src/api_client/mod.rs` - API client implementation

### Agent Environment (Current)

**Core:**
- `crates/chat-cli/src/agent_env/mod.rs` - Module exports
- `crates/chat-cli/src/agent_env/session.rs` - Session orchestrator
- `crates/chat-cli/src/agent_env/worker.rs` - Worker state
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - AgentLoop implementation

**Model Providers:**
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelProvider trait
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Bedrock implementation

**Events:**
- `crates/chat-cli/src/agent_env/events.rs` - Event definitions
- `crates/chat-cli/src/agent_env/event_bus.rs` - EventBus implementation

**UI:**
- `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs` - Text UI
- `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs` - Structured IO

**Entry Point:**
- `crates/chat-cli/src/cli/chat/mod.rs` - ChatArgs::execute()

### New Files to Create

**Tool System:**
- `crates/chat-cli/src/agent_env/tools/mod.rs` - Module exports
- `crates/chat-cli/src/agent_env/tools/tool_trait.rs` - Tool trait definition
- `crates/chat-cli/src/agent_env/tools/tool_registry.rs` - ToolRegistry implementation
- `crates/chat-cli/src/agent_env/tools/tool_context.rs` - ToolContext structure
- `crates/chat-cli/src/agent_env/tools/tool_result.rs` - ToolResult types
- `crates/chat-cli/src/agent_env/tools/tool_definition.rs` - ToolDefinition structure

**Tool Implementations:**
- `crates/chat-cli/src/agent_env/tools/fs_read.rs` - FsRead for agent_env
- `crates/chat-cli/src/agent_env/tools/fs_write.rs` - FsWrite for agent_env

**Tests:**
- `crates/chat-cli/src/agent_env/tools/tests/` - Tool system tests

---

## Summary

### Key Findings

1. **Current Tool System is Well-Designed:**
   - Clear separation of concerns
   - Comprehensive permission system
   - Good error handling
   - But tightly coupled to old architecture

2. **CodeWhisperer/QDeveloper API is Well-Documented:**
   - Tool specifications use JSON Schema
   - Tool use requests are streamed
   - Tool results are sent in follow-up requests
   - Format is consistent between both APIs

3. **Bedrock API Needs Research:**
   - Unknown if tools are supported
   - Format may differ from CodeWhisperer
   - Need to investigate AWS SDK documentation
   - May require adapter layer

4. **Integration is Feasible:**
   - Can reuse existing tool implementations
   - Need new trait for agent_env
   - ToolRegistry for management
   - Event system for UI integration

5. **Main Challenges:**
   - Bedrock API compatibility (research needed)
   - Permission/approval flow in async context
   - Tool result handling in conversation loop
   - Migration strategy for existing tools

### Next Steps

1. **Research Phase (Complete):**
   - ✅ Understand current tool system
   - ✅ Document API formats
   - ✅ Identify integration points
   - ⚠️ Research Bedrock API (needs investigation)

2. **Design Phase (Next):**
   - Design Tool trait for agent_env
   - Design ToolRegistry architecture
   - Design tool execution flow
   - Design permission/approval mechanism
   - Design Bedrock adapter (after research)
   - Create sequence diagrams
   - Document migration strategy

3. **Implementation Phase (After Design):**
   - Implement core tool infrastructure
   - Implement fs_read and fs_write
   - Extend ModelProvider for tools
   - Update AgentLoop for tool execution
   - Add tool events to EventBus
   - Update UIs for tool display
   - Write tests

### Estimated Complexity

**Overall:** High (26-34 hours as estimated in scope)

**Breakdown:**
- Tool trait and registry: Medium (4-6 hours)
- ModelProvider extensions: Medium (4-6 hours)
- AgentLoop tool execution: High (6-8 hours)
- Permission/approval flow: Medium (4-6 hours)
- Tool implementations (fs_read, fs_write): Medium (4-6 hours)
- Bedrock adapter: Unknown (depends on research)
- Testing: Medium (4-6 hours)

**Risk Factors:**
- Bedrock API compatibility (high risk)
- Permission flow complexity (medium risk)
- Tool result handling (medium risk)
- Migration strategy (low risk)

### Critical Path

1. Research Bedrock API (blocking)
2. Design tool system (blocking)
3. Implement core infrastructure (blocking)
4. Implement fs_read/fs_write (parallel with #3)
5. Integration testing (after #3 and #4)
6. UI updates (parallel with #5)

**MVP Completion:** After fs_read and fs_write working with basic approval flow.

**Post-MVP:** Other tools, advanced approval, MCP integration.
