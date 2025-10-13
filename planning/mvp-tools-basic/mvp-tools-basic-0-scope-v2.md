# MVP Tools Basic - Scope v2

**Status**: Draft - Re-scoped based on EventBus architecture and senior SDE feedback

**Date**: 2025-10-12

## Comments provided by the team's senior SDE - pay attention, update this scope doc

While designing a solution for this scope, we have to try to reuse the existing code, and
do our best to keep as close to established practices as possible. We don't want to rewrite
90% of tools code later, we want to be able to merge it into our new architecture.

For example, in `ToolProvider` you use `dyn Tool` and `ToolApprovalRule`. Existing code has
numerous tool configs that can be defined in 'agent config' and loaded into the ToolProvider.

Existing implementations of the tools have comprehensive logic that allows users to predefine
auto-approval rules specific for each tool, using tools business problem.

---

## 1. Overview

### 1.1 Problem Statement

The agent_env architecture currently lacks tool execution capabilities. While the system can conduct multi-turn conversations with LLMs, it cannot execute tools (like fs_read, fs_write, execute_bash) that are essential for practical agent functionality. This workflow implements a complete tool system that integrates with the EventBus architecture, supports both Bedrock and CodeWhisperer model providers, and provides user approval flows for tool execution.

### 1.2 Goals

**Primary Goals:**
1. Enable LLMs to request and execute tools through the agent_env architecture
2. Implement tool approval system with user control over tool execution
3. Support tool execution in both Bedrock and CodeWhisperer model providers
4. Integrate tool approvals with TextUi and StructuredIO interfaces
5. Implement fs_read and fs_write as the first two tools in the new architecture

**Secondary Goals:**
1. Establish patterns for migrating other tools to agent_env
2. Maintain conversation continuity across tool executions
3. Provide clear EventBus events for tool lifecycle
4. Support auto-approval for trusted tools

### 1.3 High-Level Approach

The tool system integration follows a layered architecture:

**Layer 1: Tool Abstraction**
- ToolProvider class encapsulates tool registry and approval rules
- Tool trait defines interface for tool implementations
- ToolResult enum provides Success/Failure/ApprovalRequired outcomes

**Layer 2: Worker Integration**
- ToolProvider becomes a property of Worker
- OpenToolsApprovalRequests stores pending approvals in Worker state
- WorkerBuilder constructs ToolProvider from agent config

**Layer 3: Conversation History**
- ConversationHistory extended to store tool requests and results
- Structure aligns with Bedrock and CodeWhisperer API formats
- Supports multiple tool requests per assistant response

**Layer 4: Model Provider**
- ModelRequest extended to include tool definitions
- ModelResponse extended to include tool use requests
- Both Bedrock and CodeWhisperer implementations updated

**Layer 5: AgentLoop Execution**
- AgentLoop detects tool requests from LLM responses
- Calls ToolProvider.execute_tool() for each request
- Handles Success/Failure by continuing loop
- Handles ApprovalRequired by storing request and exiting task

**Layer 6: UI Integration**
- TextUi provides interactive approval prompts
- StructuredIO provides JSON-based approval interface
- Both UIs can set full_trust flag for auto-approval
- UIs relaunch AgentLoop after approvals

### 1.4 Key Architectural Changes from Original Scope

The original scope focused on porting existing tools to agent_env. The updated architecture introduces several new components and patterns:

**New Components:**
1. **ToolProvider** - Centralized tool management in Worker
2. **ContextBuilder** - Builds ModelRequest with tools and conversation history
3. **OpenToolsApprovalRequests** - Worker state for pending approvals
4. **Tool approval events** - EventBus events for approval lifecycle

**Updated Components:**
1. **ConversationHistory** - Extended for tool requests/results
2. **ModelRequest/ModelResponse** - Extended for tool definitions and requests
3. **AgentLoop** - Tool execution and approval handling
4. **TextUi/StructuredIO** - Approval flows

**Key Patterns:**
1. **Approval-first execution** - Tools require approval unless auto-approved
2. **Task restart pattern** - AgentLoop exits on approval requests, restarts after approval
3. **Multi-tool support** - Single LLM response can request multiple tools in one response
4. **Full trust mode** - Users can auto-approve all future calls to a tool

### 1.5 Scope Boundaries

**In Scope:**
- ToolProvider implementation
- Tool trait and ToolResult enum
- Conversation History updates for tools
- ContextBuilder for ModelRequest construction
- ModelProvider updates (Bedrock and CodeWhisperer)
- AgentLoop tool execution flow
- EventBus events for tool approvals
- TextUi approval flow
- StructuredIO approval flow
- fs_read and fs_write implementations
- Tool migration strategy documentation

**Out of Scope:**
- MCP (Model Context Protocol) tool integration (separate workflow: mvp-tools-mcp)
- Migration of other tools beyond fs_read and fs_write (post-MVP)
- Tool result streaming (future enhancement)
- Tool execution parallelization (future enhancement)
- Tool execution sandboxing (future enhancement)
- Persistent approval rules across sessions (future enhancement)

### 1.6 Success Criteria

**Functional Criteria:**
1. LLM can request fs_read and fs_write tools
2. Tools execute successfully and return results to LLM
3. LLM receives tool results and continues conversation
4. User can approve/reject tool requests via TextUi
5. User can approve/reject tool requests via StructuredIO
6. User can set full_trust flag for auto-approval in both TextUi and StructuredIO
7. Both Bedrock and CodeWhisperer providers support tools

**Quality Criteria:**
1. Tool execution errors handled gracefully
2. Approval flow is intuitive and responsive
3. EventBus events provide clear tool lifecycle visibility
4. Code is well-documented and testable
5. Migration path for other tools is clear

**Performance Criteria:**
1. Tool execution adds minimal latency to conversation
2. Approval prompts appear immediately after tool requests
3. No memory leaks in tool execution loop

---

## 2. Architecture Components

### 2.1 ToolProvider

**Purpose**: ToolProvider encapsulates all tool-related functionality for a Worker, including tool registry, approval rules, and tool execution.

**Location**: Property of Worker, constructed by WorkerBuilder based on agent config.

**Structure**:
```rust
pub struct ToolProvider {
    // Registry of available tools
    tools: HashMap<String, Arc<dyn Tool>>,
    
    // Approval rules per tool
    approval_rules: HashMap<String, ToolApprovalRule>,
    
    // Global trust settings (from --trust-all-tools flag)
    trust_all_tools: bool,
}

pub struct ToolApprovalRule {
    // Auto-approve all calls to this tool (set by user via 't' command)
    pub full_trust: bool,
    
    // Tool is in --trust-tools list
    pub pre_approved: bool,
}
```

**Key Methods**:

```rust
impl ToolProvider {
    // Execute a tool request, returns Success/Failure/ApprovalRequired
    pub async fn execute_tool(
        &self,
        request: ToolRequest,
        context: &ToolContext,
    ) -> ToolResult;
    
    // Check if tool requires approval
    fn requires_approval(&self, tool_name: &str) -> bool;
    
    // Get list of available tools for ModelRequest
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition>;
    
    // Update approval rule for a tool
    pub fn set_full_trust(&mut self, tool_name: &str, trust: bool);
}
```

**Execution Logic**:
1. Check if tool exists in registry
2. Check approval rules:
   - If `trust_all_tools` is true → execute
   - If `approval_rules[tool_name].full_trust` is true → execute
   - If `approval_rules[tool_name].pre_approved` is true → execute
   - Otherwise → return ApprovalRequired
3. If approved, execute tool and return Success or Failure

**Integration with Worker**:
```rust
pub struct Worker {
    // ... existing fields ...
    pub tool_provider: Option<Arc<Mutex<ToolProvider>>>,
}
```

**Construction**:
```rust
// In WorkerBuilder or Session::build_worker()
let tool_provider = ToolProvider::new(
    agent_config.tools,
    agent_config.allowed_tools,
    trust_all_tools_flag,
    trust_tools_list,
);
worker.tool_provider = Some(Arc::new(Mutex::new(tool_provider)));
```

### 2.2 ToolResult Enum

**Purpose**: Represents the outcome of tool execution, enabling different handling paths in AgentLoop.

**Structure**:
```rust
pub enum ToolResult {
    // Tool executed successfully
    Success {
        tool_name: String,
        tool_use_id: String,
        output: String,
    },
    
    // Tool execution failed
    Failure {
        tool_name: String,
        tool_use_id: String,
        error: String,
    },
    
    // Tool requires user approval
    ApprovalRequired {
        tool_request: ToolRequest,
    },
}
```

**Usage in AgentLoop**:
```rust
match tool_result {
    ToolResult::Success { tool_name, tool_use_id, output } => {
        // Add tool result to conversation history
        conversation_history.push_tool_result(tool_use_id, output);
        // Continue loop - query LLM again with tool result
        continue;
    }
    ToolResult::Failure { tool_name, tool_use_id, error } => {
        // Add error to conversation history
        conversation_history.push_tool_error(tool_use_id, error);
        // Continue loop - LLM can handle error
        continue;
    }
    ToolResult::ApprovalRequired { tool_request } => {
        // Store in Worker's OpenToolsApprovalRequests
        worker.open_tools_approval_requests.push(tool_request);
        // Publish event
        event_bus.publish(ToolApprovalRequestEvent { ... });
        // Exit task
        break;
    }
}
```

### 2.3 OpenToolsApprovalRequests

**Purpose**: Stores pending tool approval requests in Worker state, enabling UI to display and user to approve/reject.

**Structure**:
```rust
pub struct ToolApprovalRequest {
    // Unique ID for this approval request
    pub approval_id: Uuid,
    
    // The tool request from LLM
    pub tool_request: ToolRequest,
    
    // Approval status (None = pending, Some = approved/rejected)
    pub approval_status: Option<ToolApprovalStatus>,
    
    // Timestamp when request was created
    pub created_at: Instant,
}

pub enum ToolApprovalStatus {
    Approved,
    Rejected { reason: Option<String> },
}

// In Worker
pub struct Worker {
    // ... existing fields ...
    pub open_tools_approval_requests: Arc<Mutex<Vec<ToolApprovalRequest>>>,
}
```

**Lifecycle**:
1. **Creation**: AgentLoop receives tool request, ToolProvider returns ApprovalRequired
2. **Storage**: AgentLoop adds to `worker.open_tools_approval_requests`
3. **Event**: AgentLoop publishes `ToolApprovalRequestEvent` to EventBus
4. **UI Display**: UI receives event, displays approval prompt to user
5. **User Action**: User approves/rejects via UI command
6. **Update**: UI updates `approval_status` in the request
7. **Restart**: UI relaunches AgentLoop
8. **Execution**: AgentLoop checks requests on start, executes approved ones
9. **Cleanup**: AgentLoop removes executed requests from list

**Multiple Requests**: A single LLM response can request multiple tools, so the list can contain multiple pending requests. AgentLoop only restarts when all requests have approval_status set.

### 2.4 ToolRequest and ToolDefinition

**ToolRequest** (from LLM response):
```rust
pub struct ToolRequest {
    // Unique ID for this tool use (from LLM response)
    pub tool_use_id: String,
    
    // Name of tool to execute
    pub tool_name: String,
    
    // JSON parameters for tool
    pub parameters: serde_json::Value,
}
```

**ToolDefinition** (sent to LLM in request):
```rust
pub struct ToolDefinition {
    // Tool name
    pub name: String,
    
    // Tool description for LLM
    pub description: String,
    
    // JSON schema for parameters
    pub parameters_schema: serde_json::Value,
}
```

### 2.5 Tool Trait

**Purpose**: Interface for tool implementations in agent_env architecture.

**Structure**:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    // Tool name (e.g., "fs_read")
    fn name(&self) -> &str;
    
    // Description for LLM
    fn description(&self) -> &str;
    
    // JSON schema for parameters
    fn parameters_schema(&self) -> serde_json::Value;
    
    // Execute tool with parameters
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<String>;
}

pub struct ToolContext {
    pub worker_id: Uuid,
    pub working_directory: PathBuf,
    pub os: Arc<Os>,
    // Other context as needed
}
```

**Implementation Example** (fs_read):
```rust
pub struct FsReadTool;

#[async_trait]
impl Tool for FsReadTool {
    fn name(&self) -> &str {
        "fs_read"
    }
    
    fn description(&self) -> &str {
        "Read files, directories, and images"
    }
    
    fn parameters_schema(&self) -> serde_json::Value {
        // JSON schema for fs_read parameters
        json!({
            "type": "object",
            "properties": {
                "operations": { ... }
            }
        })
    }
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<String> {
        // Parse parameters
        let fs_read: FsRead = serde_json::from_value(parameters)?;
        
        // Execute tool logic
        let output = fs_read.invoke(&context.os).await?;
        
        Ok(output)
    }
}
```

---

## 3. Conversation History Updates

### 3.1 Current Structure

ConversationHistory currently stores alternating user and assistant messages:

```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,
}

pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

### 3.2 Required Changes

**Problem**: Current structure cannot represent:
1. Multiple tool requests in a single assistant response
2. Tool results associated with specific tool requests
3. Tool request/result flow that matches LLM API formats

**Solution**: Extend AssistantMessage and UserMessage to support tool use:

```rust
pub enum AssistantMessage {
    // Text response from LLM
    Text { content: String },
    
    // Tool use request(s) from LLM
    ToolUse { tool_requests: Vec<ToolRequest> },
    
    // Mixed: text + tool requests
    TextAndToolUse {
        content: String,
        tool_requests: Vec<ToolRequest>,
    },
}

pub enum UserMessage {
    // User input text
    Text { content: String },
    
    // Tool results from execution
    ToolResults { results: Vec<ToolResultMessage> },
    
    // Mixed: text + tool results (for follow-up after tools)
    TextAndToolResults {
        content: String,
        results: Vec<ToolResultMessage>,
    },
}

pub struct ToolResultMessage {
    pub tool_use_id: String,
    pub tool_name: String,
    pub status: ToolResultStatus,
    pub content: String,
}

pub enum ToolResultStatus {
    Success,
    Error,
}
```

### 3.3 Conversation Flow Example

**Turn 1**: User asks to read a file
```
User: Text { content: "Read /tmp/test.txt" }
Assistant: ToolUse { tool_requests: [{ tool_use_id: "1", tool_name: "fs_read", ... }] }
```

**Turn 2**: Tool result provided
```
User: ToolResults { results: [{ tool_use_id: "1", status: Success, content: "file contents" }] }
Assistant: Text { content: "The file contains..." }
```

**Turn 3**: Multiple tools requested
```
User: Text { content: "Read input.txt and write to output.txt" }
Assistant: ToolUse { tool_requests: [
    { tool_use_id: "2", tool_name: "fs_read", ... },
    { tool_use_id: "3", tool_name: "fs_write", ... }
]}
```

### 3.4 API Alignment

**Bedrock Converse API** format:
```json
{
  "messages": [
    { "role": "user", "content": [{ "text": "Read file" }] },
    { "role": "assistant", "content": [{ "toolUse": { "toolUseId": "1", "name": "fs_read", "input": {...} } }] },
    { "role": "user", "content": [{ "toolResult": { "toolUseId": "1", "content": [...] } }] },
    { "role": "assistant", "content": [{ "text": "The file contains..." }] }
  ]
}
```

**CodeWhisperer API** format:
```json
{
  "history": [
    { "userInputMessage": { "content": "Read file" } },
    { "assistantResponseMessage": { "toolUses": [{ "id": "1", "name": "fs_read", "input": {...} }] } },
    { "userInputMessage": { "userInputMessageContext": { "toolResults": [{ "id": "1", "content": "..." }] } } },
    { "assistantResponseMessage": { "content": "The file contains..." } }
  ]
}
```

Our structure must map cleanly to both formats.

---

## 4. ContextBuilder

### 4.1 Purpose

ContextBuilder constructs ModelRequest from Worker state, including:
1. Tool definitions from ToolProvider
2. Conversation history with tool requests and results
3. System prompts and other context

### 4.2 Structure

```rust
pub struct ContextBuilder {
    // Reference to worker's tool provider
    tool_provider: Option<Arc<Mutex<ToolProvider>>>,
    
    // Reference to conversation history
    conversation_history: Arc<Mutex<ConversationHistory>>,
}

impl ContextBuilder {
    pub fn build_request(&self) -> ModelRequest {
        let mut request = ModelRequest::new();
        
        // Add tool definitions
        if let Some(tool_provider) = &self.tool_provider {
            let tools = tool_provider.lock().unwrap().get_tool_definitions();
            request.tools = tools;
        }
        
        // Add conversation history
        let history = self.conversation_history.lock().unwrap();
        request.messages = self.convert_history_to_messages(&history);
        
        request
    }
    
    fn convert_history_to_messages(&self, history: &ConversationHistory) -> Vec<Message> {
        // Convert ConversationHistory entries to ModelRequest messages
        // Handle tool requests and results properly
    }
}
```

### 4.3 Integration

ContextBuilder is used by AgentLoop before each LLM request:

```rust
// In AgentLoop::run()
let context_builder = ContextBuilder::new(
    worker.tool_provider.clone(),
    worker.context_container.conversation_history.clone(),
);

let request = context_builder.build_request();
let response = model_provider.request(request, ...).await?;
```

---

## 5. ModelProvider Updates

### 5.1 ModelRequest Changes

Current ModelRequest only has a prompt string. Extended version:

```rust
pub struct ModelRequest {
    // Conversation messages (replaces simple prompt)
    pub messages: Vec<Message>,
    
    // Available tools
    pub tools: Vec<ToolDefinition>,
    
    // Model ID
    pub model_id: String,
}

pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}
```

### 5.2 ModelResponse Changes

Current ModelResponse only has content string. Extended version:

```rust
pub struct ModelResponse {
    // Text content (if any)
    pub content: Option<String>,
    
    // Tool use requests (if any)
    pub tool_requests: Vec<ToolRequest>,
    
    // Stop reason
    pub stop_reason: StopReason,
}

pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}
```

### 5.3 ModelResponseChunk Changes

For streaming, chunks can be text or tool requests:

```rust
pub enum ModelResponseChunk {
    // Text content chunk
    Text(String),
    
    // Tool use request (streamed as complete unit)
    ToolUse(ToolRequest),
}
```

### 5.4 Bedrock Implementation

**Request Mapping**:
```rust
// In BedrockConverseStreamModelProvider::request()
let mut bedrock_messages = vec![];

for message in request.messages {
    match message {
        Message::User(user_msg) => {
            bedrock_messages.push(
                aws_sdk_bedrockruntime::types::Message::builder()
                    .role(ConversationRole::User)
                    .content(convert_user_message(user_msg))
                    .build()?
            );
        }
        Message::Assistant(assistant_msg) => {
            bedrock_messages.push(
                aws_sdk_bedrockruntime::types::Message::builder()
                    .role(ConversationRole::Assistant)
                    .content(convert_assistant_message(assistant_msg))
                    .build()?
            );
        }
    }
}

// Add tools
let mut tool_config = None;
if !request.tools.is_empty() {
    let bedrock_tools: Vec<_> = request.tools.iter()
        .map(|t| convert_tool_definition(t))
        .collect();
    tool_config = Some(ToolConfiguration::builder()
        .set_tools(Some(bedrock_tools))
        .build()?);
}

let bedrock_request = client
    .converse_stream()
    .model_id(request.model_id)
    .set_messages(Some(bedrock_messages))
    .set_tool_config(tool_config);
```

**Response Parsing**:
```rust
// Parse streaming events
match event {
    ConverseStreamOutput::ContentBlockDelta(delta) => {
        match delta.delta {
            Some(ContentBlockDelta::Text(text)) => {
                when_received(ModelResponseChunk::Text(text));
            }
            Some(ContentBlockDelta::ToolUse(tool_use)) => {
                let tool_request = ToolRequest {
                    tool_use_id: tool_use.tool_use_id,
                    tool_name: tool_use.name,
                    parameters: serde_json::from_str(&tool_use.input)?,
                };
                when_received(ModelResponseChunk::ToolUse(tool_request));
            }
            _ => {}
        }
    }
    _ => {}
}
```

### 5.5 CodeWhisperer Implementation

**Request Mapping**:
```rust
// In CodeWhispererModelProvider::request()
let mut cw_messages = vec![];

for message in request.messages {
    match message {
        Message::User(user_msg) => {
            let mut user_input = UserInputMessage::builder()
                .content(extract_text(&user_msg));
            
            // Add tool results if present
            if let Some(tool_results) = extract_tool_results(&user_msg) {
                user_input = user_input.set_user_input_message_context(
                    UserInputMessageContext::builder()
                        .set_tool_results(Some(tool_results))
                        .build()
                );
            }
            
            cw_messages.push(ChatMessage::UserInputMessage(user_input.build()?));
        }
        Message::Assistant(assistant_msg) => {
            let assistant_response = AssistantResponseMessage::builder()
                .set_content(extract_text(&assistant_msg))
                .set_tool_uses(extract_tool_uses(&assistant_msg))
                .build()?;
            
            cw_messages.push(ChatMessage::AssistantResponseMessage(assistant_response));
        }
    }
}

// Add tools to context
let tools: Vec<_> = request.tools.iter()
    .map(|t| convert_tool_definition_cw(t))
    .collect();

let conversation_state = ConversationState::builder()
    .set_history(Some(cw_messages))
    .user_input_message(
        UserInputMessage::builder()
            .content(last_user_message)
            .user_input_message_context(
                UserInputMessageContext::builder()
                    .set_tools(Some(tools))
                    .build()
            )
            .build()?
    )
    .build()?;
```

**Response Parsing**:
```rust
// Parse streaming events
match event {
    ChatResponseStream::AssistantResponseEvent(response) => {
        if let Some(content) = response.content {
            when_received(ModelResponseChunk::Text(content));
        }
        
        if let Some(tool_uses) = response.tool_uses {
            for tool_use in tool_uses {
                let tool_request = ToolRequest {
                    tool_use_id: tool_use.id,
                    tool_name: tool_use.name,
                    parameters: serde_json::from_str(&tool_use.input)?,
                };
                when_received(ModelResponseChunk::ToolUse(tool_request));
            }
        }
    }
    _ => {}
}
```

### 5.6 Provider Abstraction

Both providers must implement the same ModelProvider trait:

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: impl Fn() + Send,
        when_received: impl Fn(ModelResponseChunk) + Send,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse>;
}
```

This ensures AgentLoop works with both providers without provider-specific code.

---

## 6. AgentLoop Integration

### 6.1 Task Start Behavior

When AgentLoop starts, it must check for pending approval requests:

```rust
async fn run(&self, worker: Arc<Worker>) -> Result<()> {
    // Check for pending approval requests
    let pending_requests = worker.open_tools_approval_requests.lock().unwrap();
    
    if !pending_requests.is_empty() {
        // Check if all have approval status
        let all_approved = pending_requests.iter()
            .all(|req| req.approval_status.is_some());
        
        if !all_approved {
            // Some requests still pending - exit immediately
            return Ok(());
        }
        
        // Execute approved requests
        for request in pending_requests.iter() {
            match &request.approval_status {
                Some(ToolApprovalStatus::Approved) => {
                    // Execute tool
                    let result = self.execute_tool_directly(&request.tool_request, &worker).await?;
                    
                    // Add result to conversation history
                    self.add_tool_result_to_history(&worker, result)?;
                }
                Some(ToolApprovalStatus::Rejected { reason }) => {
                    // Add rejection to conversation history
                    self.add_tool_rejection_to_history(&worker, &request.tool_request, reason)?;
                }
                None => unreachable!(), // Already checked above
            }
        }
        
        // Clear processed requests
        drop(pending_requests);
        worker.open_tools_approval_requests.lock().unwrap().clear();
    }
    
    // Continue with normal agent loop
    self.run_loop(worker).await
}
```

### 6.2 Tool Execution Flow

Main loop handles tool requests from LLM:

```rust
async fn run_loop(&self, worker: Arc<Worker>) -> Result<()> {
    loop {
        // Build request with tools and history
        let context_builder = ContextBuilder::new(
            worker.tool_provider.clone(),
            worker.context_container.conversation_history.clone(),
        );
        let request = context_builder.build_request();
        
        // Query LLM
        let response = worker.model_provider
            .as_ref()
            .unwrap()
            .request(request, || {}, |chunk| {
                // Publish output chunks
                self.event_bus.publish(JobEvent::OutputChunk { ... });
            }, self.cancellation_token.clone())
            .await?;
        
        // Check for tool requests
        if !response.tool_requests.is_empty() {
            // Process each tool request
            let mut has_approval_required = false;
            
            for tool_request in response.tool_requests {
                let tool_result = self.execute_tool(&tool_request, &worker).await?;
                
                match tool_result {
                    ToolResult::Success { .. } | ToolResult::Failure { .. } => {
                        // Add to conversation history
                        self.add_tool_result_to_history(&worker, tool_result)?;
                    }
                    ToolResult::ApprovalRequired { tool_request } => {
                        // Store for approval
                        self.store_approval_request(&worker, tool_request)?;
                        has_approval_required = true;
                    }
                }
            }
            
            if has_approval_required {
                // Exit loop - wait for approval
                break;
            }
            
            // All tools succeeded/failed - continue loop
            continue;
        }
        
        // No tool requests - add assistant response and exit
        self.add_assistant_response_to_history(&worker, response)?;
        break;
    }
    
    Ok(())
}
```

### 6.3 Tool Execution Method

```rust
async fn execute_tool(
    &self,
    tool_request: &ToolRequest,
    worker: &Arc<Worker>,
) -> Result<ToolResult> {
    let tool_provider = worker.tool_provider
        .as_ref()
        .ok_or_else(|| eyre!("No tool provider"))?;
    
    let context = ToolContext {
        worker_id: worker.id,
        working_directory: std::env::current_dir()?,
        os: Arc::new(Os::new()),
    };
    
    tool_provider.lock().unwrap()
        .execute_tool(tool_request.clone(), &context)
        .await
}
```

### 6.4 Approval Request Storage

```rust
fn store_approval_request(
    &self,
    worker: &Arc<Worker>,
    tool_request: ToolRequest,
) -> Result<()> {
    let approval_request = ToolApprovalRequest {
        approval_id: Uuid::new_v4(),
        tool_request: tool_request.clone(),
        approval_status: None,
        created_at: Instant::now(),
    };
    
    // Store in worker
    worker.open_tools_approval_requests
        .lock()
        .unwrap()
        .push(approval_request.clone());
    
    // Publish event
    self.event_bus.publish(Event::ToolApprovalRequest(
        ToolApprovalRequestEvent {
            worker_id: worker.id,
            approval_id: approval_request.approval_id,
            tool_name: tool_request.tool_name,
            tool_use_id: tool_request.tool_use_id,
            parameters: tool_request.parameters,
            timestamp: Instant::now(),
        }
    ));
    
    Ok(())
}
```

### 6.5 History Management

```rust
fn add_tool_result_to_history(
    &self,
    worker: &Arc<Worker>,
    tool_result: ToolResult,
) -> Result<()> {
    let mut history = worker.context_container
        .conversation_history
        .lock()
        .unwrap();
    
    match tool_result {
        ToolResult::Success { tool_use_id, output, .. } => {
            history.push_tool_result(ToolResultMessage {
                tool_use_id,
                status: ToolResultStatus::Success,
                content: output,
            });
        }
        ToolResult::Failure { tool_use_id, error, .. } => {
            history.push_tool_result(ToolResultMessage {
                tool_use_id,
                status: ToolResultStatus::Error,
                content: error,
            });
        }
        _ => {}
    }
    
    Ok(())
}
```

---

## 7. EventBus Integration

### 7.1 New Event Types

```rust
// In events.rs
pub enum Event {
    // ... existing events ...
    
    // Tool approval request created
    ToolApprovalRequest(ToolApprovalRequestEvent),
    
    // Tool approval status updated
    ToolApprovalUpdated(ToolApprovalUpdatedEvent),
    
    // Tool execution started
    ToolExecutionStarted(ToolExecutionStartedEvent),
    
    // Tool execution completed
    ToolExecutionCompleted(ToolExecutionCompletedEvent),
}

pub struct ToolApprovalRequestEvent {
    pub worker_id: Uuid,
    pub approval_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub parameters: serde_json::Value,
    pub timestamp: Instant,
}

pub struct ToolApprovalUpdatedEvent {
    pub worker_id: Uuid,
    pub approval_id: Uuid,
    pub status: ToolApprovalStatus,
    pub timestamp: Instant,
}

pub struct ToolExecutionStartedEvent {
    pub worker_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub timestamp: Instant,
}

pub struct ToolExecutionCompletedEvent {
    pub worker_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub result: ToolExecutionResult,
    pub timestamp: Instant,
}

pub enum ToolExecutionResult {
    Success { output_length: usize },
    Failure { error: String },
}
```

### 7.2 Event Publishing Points

**AgentLoop**:
- Publishes `ToolApprovalRequest` when tool requires approval
- Publishes `ToolExecutionStarted` when executing approved tool
- Publishes `ToolExecutionCompleted` after tool execution

**UI**:
- Publishes `ToolApprovalUpdated` when user approves/rejects

**ToolProvider**:
- Could publish execution events (optional, may be redundant with AgentLoop events)

### 7.3 Event Subscribers

**TextUi**:
- Subscribes to `ToolApprovalRequest` to display approval prompts
- Subscribes to `ToolExecutionStarted` to show tool execution status
- Subscribes to `ToolExecutionCompleted` to show results

**StructuredIO**:
- Subscribes to `ToolApprovalRequest` to output JSON approval requests
- Subscribes to `ToolExecutionCompleted` to output JSON results

**AgentEnvironment**:
- Forwards all events to registered UIs

---

## 8. UI Integration

### 8.1 TextUi Approval Flow

**Trigger**: When AgentLoop task completes and worker has open approval requests.

**Implementation**:

```rust
// In TextUi::handle_event()
match event {
    Event::Job(JobEvent::Completed { worker_id, .. }) => {
        // Check if worker has pending approvals
        let worker = self.session.get_worker(worker_id)?;
        let pending = worker.open_tools_approval_requests.lock().unwrap();
        
        if !pending.is_empty() && pending.iter().any(|r| r.approval_status.is_none()) {
            // Enter approval mode
            self.show_approval_prompts(&worker)?;
        } else {
            // Normal prompt mode
            self.show_user_prompt()?;
        }
    }
    _ => {}
}
```

**Approval Prompt UI**:

```rust
fn show_approval_prompts(&self, worker: &Arc<Worker>) -> Result<()> {
    let mut pending = worker.open_tools_approval_requests.lock().unwrap();
    
    for request in pending.iter_mut() {
        if request.approval_status.is_some() {
            continue; // Already approved/rejected
        }
        
        // Display tool request
        println!("\n{} Tool approval required:", "⚠".yellow());
        println!("  Tool: {}", request.tool_request.tool_name.green());
        println!("  Parameters: {}", 
            serde_json::to_string_pretty(&request.tool_request.parameters)?);
        
        // Prompt for approval
        print!("\nAllow this action? Use {} to trust (always allow) this tool for the session. [{}/{}{}]: ",
            "'t'".green(),
            "y".green(),
            "n".green(),
            "/t".green()
        );
        std::io::stdout().flush()?;
        
        // Read user input
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        match input {
            "y" => {
                request.approval_status = Some(ToolApprovalStatus::Approved);
                println!("{} Tool approved", "✓".green());
            }
            "t" => {
                // Set full trust
                if let Some(tool_provider) = &worker.tool_provider {
                    tool_provider.lock().unwrap()
                        .set_full_trust(&request.tool_request.tool_name, true);
                }
                request.approval_status = Some(ToolApprovalStatus::Approved);
                println!("{} Tool approved and trusted for session", "✓".green());
            }
            "n" => {
                request.approval_status = Some(ToolApprovalStatus::Rejected { reason: None });
                println!("{} Tool rejected", "✗".red());
            }
            other => {
                // Any other text is rejection with reason
                request.approval_status = Some(ToolApprovalStatus::Rejected { 
                    reason: Some(other.to_string()) 
                });
                println!("{} Tool rejected: {}", "✗".red(), other);
            }
        }
        
        // Publish update event
        self.event_bus.publish(Event::ToolApprovalUpdated(
            ToolApprovalUpdatedEvent {
                worker_id: worker.id,
                approval_id: request.approval_id,
                status: request.approval_status.clone().unwrap(),
                timestamp: Instant::now(),
            }
        ));
    }
    
    // Check if all approved
    let all_have_status = pending.iter().all(|r| r.approval_status.is_some());
    drop(pending);
    
    if all_have_status {
        // Relaunch AgentLoop
        self.session.run_task__agent_loop(worker.clone(), AgentLoopInput {})?;
    }
    
    Ok(())
}
```

### 8.2 StructuredIO Approval Flow

**JSON Input Format**:

```json
{
  "type": "tool_approval",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "result": "approve"
}
```

Or with rejection reason:

```json
{
  "type": "tool_approval",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "result": "User said no"
}
```

**Full Trust Command**:

```json
{
  "type": "tool_trust",
  "worker_id": "uuid",
  "tool_name": "fs_read",
  "full_trust": true
}
```

**Implementation**:

```rust
// In StructuredIO::handle_input()
match input_type {
    "tool_approval" => {
        let worker_id: Uuid = input["worker_id"].as_str()?.parse()?;
        let approval_id: Uuid = input["approval_id"].as_str()?.parse()?;
        let result = input["result"].as_str()?;
        
        let worker = self.session.get_worker(worker_id)?;
        let mut pending = worker.open_tools_approval_requests.lock().unwrap();
        
        // Find request
        if let Some(request) = pending.iter_mut()
            .find(|r| r.approval_id == approval_id) 
        {
            request.approval_status = Some(match result {
                "approve" => ToolApprovalStatus::Approved,
                "reject" => ToolApprovalStatus::Rejected { reason: None },
                other => ToolApprovalStatus::Rejected { 
                    reason: Some(other.to_string()) 
                },
            });
            
            // Publish event
            self.event_bus.publish(Event::ToolApprovalUpdated(...));
        }
        
        // Check if all have status
        let all_have_status = pending.iter().all(|r| r.approval_status.is_some());
        drop(pending);
        
        if all_have_status {
            // Relaunch AgentLoop
            self.session.run_task__agent_loop(worker.clone(), AgentLoopInput {})?;
        }
    }
    "tool_trust" => {
        let worker_id: Uuid = input["worker_id"].as_str()?.parse()?;
        let tool_name = input["tool_name"].as_str()?;
        let full_trust = input["full_trust"].as_bool()?;
        
        let worker = self.session.get_worker(worker_id)?;
        if let Some(tool_provider) = &worker.tool_provider {
            tool_provider.lock().unwrap()
                .set_full_trust(tool_name, full_trust);
        }
    }
    _ => {}
}
```

**JSON Output Format** (on approval request):

```json
{
  "type": "tool_approval_request",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "tool_name": "fs_read",
  "tool_use_id": "1",
  "parameters": { ... },
  "timestamp": "2025-10-12T21:42:36Z"
}
```

---

## 9. Implementation Phases

### Phase 1: Research (6-8 hours)

**Deliverable**: `mvp-tools-basic-1-research.md`

**Tasks**:
1. Study Bedrock Converse API tool use format
   - Request format with tool definitions
   - Response format with tool use blocks
   - Tool result format in follow-up requests
   - Streaming behavior for tool requests
2. Study CodeWhisperer API tool use format
   - UserInputMessageContext with tools
   - AssistantResponseMessage with tool_uses
   - Tool results in UserInputMessageContext
   - Compatibility with Bedrock format
3. Analyze existing tool implementations
   - fs_read structure and execution
   - fs_write structure and execution
   - Permission checking patterns
   - Error handling patterns
4. Document conversation history requirements
   - Message structure for both APIs
   - Tool request/result representation
   - Multi-tool request handling
5. Research approval patterns in existing codebase
   - Current approval UI (search for "Allow this action")
   - Trust flag mechanisms
   - Session-wide vs per-invocation approval

**Key Questions to Answer**:
- What is the exact Bedrock tool use API format?
- What is the exact CodeWhisperer tool use API format?
- How do we map our ConversationHistory to both formats?
- What context do tools need to execute?
- How are permissions currently managed?

### Phase 2: Design (8-10 hours)

**Deliverable**: `mvp-tools-basic-2-design.md`

**Tasks**:
1. Design ToolProvider architecture
   - Tool registry structure
   - Approval rules structure
   - execute_tool() implementation
   - Integration with Worker
2. Design ToolResult enum and handling
   - Success/Failure/ApprovalRequired variants
   - Data carried by each variant
   - Usage patterns in AgentLoop
3. Design ConversationHistory extensions
   - AssistantMessage variants
   - UserMessage variants
   - ToolResultMessage structure
   - API mapping functions
4. Design ContextBuilder
   - ModelRequest construction
   - History conversion logic
   - Tool definition extraction
5. Design ModelProvider extensions
   - ModelRequest structure
   - ModelResponse structure
   - ModelResponseChunk variants
   - Bedrock implementation details
   - CodeWhisperer implementation details
6. Design AgentLoop tool execution flow
   - Task start logic (check pending approvals)
   - Tool request detection
   - Tool execution and result handling
   - Approval request storage
   - Task exit conditions
7. Design EventBus events
   - Event types and structures
   - Publishing points
   - Subscriber patterns
8. Design UI approval flows
   - TextUi approval prompt UI
   - StructuredIO JSON formats
   - Full trust flag setting
   - AgentLoop relaunch logic
9. Create sequence diagrams
   - Tool execution with approval
   - Tool execution with auto-approval
   - Multi-tool request handling
   - Tool failure handling

**Design Decisions to Make**:
- Tool storage: Worker-specific or Session-shared?
- Approval rules: Per-tool or per-invocation?
- History structure: Enum variants or trait objects?
- ContextBuilder: Separate struct or Worker method?

### Phase 3: Core Infrastructure (10-12 hours)

**Tasks**:
1. Implement Tool trait
   - Define trait in `agent_env/tools/tool_trait.rs`
   - Define ToolContext struct
   - Add documentation and examples
2. Implement ToolResult enum
   - Define in `agent_env/tools/tool_result.rs`
   - Add helper methods
3. Implement ToolProvider
   - Create `agent_env/tools/tool_provider.rs`
   - Tool registry HashMap
   - Approval rules HashMap
   - execute_tool() method
   - get_tool_definitions() method
   - set_full_trust() method
4. Extend Worker
   - Add tool_provider field
   - Add open_tools_approval_requests field
   - Update serialization (skip tool_provider)
5. Implement ToolApprovalRequest
   - Define in `agent_env/tools/approval.rs`
   - approval_id, tool_request, approval_status fields
6. Update WorkerBuilder
   - Construct ToolProvider from agent config
   - Handle --trust-all-tools flag
   - Handle --trust-tools list

**Tests**:
- ToolProvider tool registration
- ToolProvider approval checking
- ToolProvider execute_tool() with different approval states
- Worker tool_provider integration

### Phase 4: Conversation History Updates (6-8 hours)

**Tasks**:
1. Extend AssistantMessage enum
   - Add Text, ToolUse, TextAndToolUse variants
   - Update serialization
2. Extend UserMessage enum
   - Add Text, ToolResults, TextAndToolResults variants
   - Update serialization
3. Add ToolResultMessage struct
   - tool_use_id, tool_name, status, content fields
4. Update ConversationHistory methods
   - push_tool_result() method
   - push_tool_request() method
   - get_last_tool_requests() method
5. Implement ContextBuilder
   - Create `agent_env/context_builder.rs`
   - build_request() method
   - convert_history_to_messages() method
   - Map to Bedrock format
   - Map to CodeWhisperer format

**Tests**:
- ConversationHistory with tool requests
- ConversationHistory with tool results
- ContextBuilder message conversion
- Round-trip serialization

### Phase 5: ModelProvider Updates (10-12 hours)

**Tasks**:
1. Update ModelRequest struct
   - Add messages field (Vec<Message>)
   - Add tools field (Vec<ToolDefinition>)
   - Deprecate prompt field
2. Update ModelResponse struct
   - Add tool_requests field
   - Update stop_reason enum
3. Update ModelResponseChunk enum
   - Add ToolUse variant
4. Update BedrockConverseStreamModelProvider
   - Implement tool definitions in request
   - Parse tool use from response
   - Handle tool results in conversation
   - Update streaming logic
5. Create CodeWhispererModelProvider
   - Implement ModelProvider trait
   - Map ModelRequest to ConversationState
   - Map tool definitions to UserInputMessageContext
   - Parse tool uses from AssistantResponseMessage
   - Handle streaming responses
6. Update ModelProvider trait if needed
   - Ensure both implementations work

**Tests**:
- Bedrock tool request formatting
- Bedrock tool response parsing
- CodeWhisperer tool request formatting
- CodeWhisperer tool response parsing
- Both providers with same ModelRequest

### Phase 6: AgentLoop Integration (8-10 hours)

**Tasks**:
1. Update AgentLoop task start logic
   - Check open_tools_approval_requests
   - Execute approved tools
   - Exit if unapproved requests exist
2. Implement tool execution flow
   - Use ContextBuilder for ModelRequest
   - Detect tool requests in ModelResponse
   - Call ToolProvider.execute_tool()
   - Handle ToolResult variants
3. Implement Success/Failure handling
   - Add tool results to ConversationHistory
   - Continue loop with new LLM request
4. Implement ApprovalRequired handling
   - Create ToolApprovalRequest
   - Store in worker.open_tools_approval_requests
   - Publish ToolApprovalRequestEvent
   - Exit task
5. Implement history management
   - add_tool_result_to_history() method
   - add_tool_rejection_to_history() method
   - add_assistant_response_to_history() method
6. Update task metadata
   - Set completion state for tool approvals
   - Track last tool executed

**Tests**:
- AgentLoop with tool execution
- AgentLoop with approval required
- AgentLoop restart after approval
- AgentLoop with multiple tools
- AgentLoop with tool failure

### Phase 7: EventBus and Events (4-6 hours)

**Tasks**:
1. Define new event types
   - ToolApprovalRequestEvent
   - ToolApprovalUpdatedEvent
   - ToolExecutionStartedEvent
   - ToolExecutionCompletedEvent
2. Add events to Event enum
3. Implement event publishing in AgentLoop
   - Publish on approval request
   - Publish on tool execution start
   - Publish on tool execution complete
4. Update AgentEnvironment event forwarding
   - Ensure new events reach UIs

**Tests**:
- Event publishing
- Event subscription
- Event forwarding to multiple UIs

### Phase 8: TextUi Integration (6-8 hours)

**Tasks**:
1. Implement approval prompt UI
   - Detect pending approvals on job completion
   - Display tool request details
   - Show approval prompt with y/n/t options
   - Handle user input
2. Implement approval status update
   - Update ToolApprovalRequest in worker
   - Publish ToolApprovalUpdatedEvent
3. Implement full trust flag setting
   - Update ToolProvider on 't' command
4. Implement AgentLoop relaunch
   - Check all requests have status
   - Call session.run_task__agent_loop()
5. Update event handling
   - Subscribe to ToolApprovalRequestEvent
   - Subscribe to ToolExecutionCompletedEvent
   - Display tool execution status

**Tests**:
- Approval prompt display
- User approval flow
- User rejection flow
- Full trust flag setting
- AgentLoop relaunch

### Phase 9: StructuredIO Integration (4-6 hours)

**Tasks**:
1. Implement JSON approval input
   - Parse tool_approval messages
   - Update ToolApprovalRequest
   - Publish ToolApprovalUpdatedEvent
2. Implement JSON trust input
   - Parse tool_trust messages
   - Update ToolProvider
3. Implement JSON approval output
   - Output ToolApprovalRequestEvent as JSON
   - Output ToolExecutionCompletedEvent as JSON
4. Implement AgentLoop relaunch
   - Same logic as TextUi

**Tests**:
- JSON approval input parsing
- JSON trust input parsing
- JSON output formatting
- AgentLoop relaunch

### Phase 10: Tool Implementations (8-10 hours)

**Tasks**:
1. Implement FsReadTool
   - Create `agent_env/tools/fs_read.rs`
   - Implement Tool trait
   - Port logic from existing fs_read
   - Handle parameters parsing
   - Handle errors
2. Implement FsWriteTool
   - Create `agent_env/tools/fs_write.rs`
   - Implement Tool trait
   - Port logic from existing fs_write
   - Handle parameters parsing
   - Handle errors
3. Register tools in ToolProvider
   - Add to default tool registry
   - Configure approval requirements
4. Document tool migration pattern
   - Create migration guide
   - Document Tool trait usage
   - Provide examples

**Tests**:
- FsReadTool execution
- FsWriteTool execution
- Tool parameter parsing
- Tool error handling
- End-to-end tool use with LLM

### Phase 11: Integration Testing (6-8 hours)

**Tasks**:
1. End-to-end tool execution tests
   - LLM requests tool
   - Tool executes
   - Result returns to LLM
   - LLM continues conversation
2. Approval flow tests
   - Tool requires approval
   - User approves via TextUi
   - Tool executes
   - Conversation continues
3. Multi-tool tests
   - LLM requests multiple tools
   - All tools execute
   - All results return
4. Error handling tests
   - Tool execution fails
   - Error returns to LLM
   - LLM handles error
5. Both provider tests
   - Same tests with Bedrock
   - Same tests with CodeWhisperer

**Tests**:
- Full conversation with tools (Bedrock)
- Full conversation with tools (CodeWhisperer)
- Approval flow (TextUi)
- Approval flow (StructuredIO)
- Multi-tool execution
- Tool failure handling

---

## 10. Testing Strategy

### 10.1 Unit Tests

**ToolProvider**:
- Tool registration and retrieval
- Approval rule checking
- execute_tool() with different approval states
- Full trust flag setting

**ConversationHistory**:
- Tool request storage
- Tool result storage
- Multiple tool requests per assistant message
- Serialization/deserialization

**ContextBuilder**:
- ModelRequest construction
- History conversion to messages
- Tool definition extraction

**ModelProvider**:
- Tool request formatting (Bedrock)
- Tool response parsing (Bedrock)
- Tool request formatting (CodeWhisperer)
- Tool response parsing (CodeWhisperer)

**Tool Implementations**:
- FsReadTool parameter parsing
- FsReadTool execution
- FsWriteTool parameter parsing
- FsWriteTool execution

### 10.2 Integration Tests

**AgentLoop**:
- Tool execution flow
- Approval request handling
- Task restart after approval
- Multi-tool execution
- Tool failure handling

**UI Integration**:
- TextUi approval flow
- StructuredIO approval flow
- Event handling
- AgentLoop relaunch

### 10.3 End-to-End Tests

**Scenarios**:
1. **Simple tool use**: User asks to read file, LLM requests fs_read, tool executes, result returns
2. **Tool approval**: Tool requires approval, user approves via TextUi, tool executes
3. **Tool rejection**: Tool requires approval, user rejects, rejection returns to LLM
4. **Full trust**: User sets full trust, subsequent calls auto-approve
5. **Multi-tool**: LLM requests multiple tools, all execute, all results return
6. **Tool failure**: Tool execution fails, error returns to LLM, LLM handles error
7. **Both providers**: Same scenarios with Bedrock and CodeWhisperer

**Test Commands**:
```bash
# Test fs_read
q chat "Read the file /tmp/test.txt"

# Test fs_write
q chat "Write 'Hello World' to /tmp/output.txt"

# Test approval
q chat "Read /etc/passwd"
# (should prompt for approval)

# Test --trust-all-tools
q chat --trust-all-tools "Read /tmp/test.txt"
# (should not prompt)

# Test --trust-tools
q chat --trust-tools=fs_read "Read /tmp/test.txt"
# (should not prompt for fs_read)

# Test multi-tool
q chat "Read /tmp/input.txt and write the contents to /tmp/output.txt"
# (should use both tools)

# Test tool error
q chat "Read /nonexistent/file.txt"
# (should handle error gracefully)

# Test StructuredIO
echo '{"prompt": "Read /tmp/test.txt"}' | q chat --ui-mode=structured
```

---

## 11. Acceptance Criteria

### 11.1 Functional Criteria

**Core Functionality**:
- [ ] LLM can request fs_read tool
- [ ] LLM can request fs_write tool
- [ ] Tools execute successfully
- [ ] Tool results return to LLM
- [ ] LLM continues conversation with tool results
- [ ] Both Bedrock and CodeWhisperer providers support tools

**Approval System**:
- [ ] Tools require approval by default
- [ ] User can approve tools via TextUi
- [ ] User can reject tools via TextUi
- [ ] User can set full trust via TextUi
- [ ] User can approve tools via StructuredIO
- [ ] User can reject tools via StructuredIO
- [ ] User can set full trust via StructuredIO
- [ ] --trust-all-tools flag auto-approves all tools
- [ ] --trust-tools list auto-approves specified tools
- [ ] Full trust persists for session

**Multi-Tool Support**:
- [ ] LLM can request multiple tools in one response
- [ ] All tools execute
- [ ] All results return to LLM
- [ ] Approval required for each tool

**Error Handling**:
- [ ] Tool execution errors handled gracefully
- [ ] Errors return to LLM
- [ ] LLM can handle and respond to errors
- [ ] Invalid tool requests handled
- [ ] Missing tool parameters handled

**EventBus Integration**:
- [ ] ToolApprovalRequestEvent published
- [ ] ToolApprovalUpdatedEvent published
- [ ] ToolExecutionStartedEvent published
- [ ] ToolExecutionCompletedEvent published
- [ ] Events reach all UIs

### 11.2 Quality Criteria

**Code Quality**:
- [ ] All code documented with rustdoc comments
- [ ] All public APIs have examples
- [ ] Code follows Rust best practices
- [ ] No clippy warnings
- [ ] Code formatted with rustfmt

**Testing**:
- [ ] Unit tests for all components
- [ ] Integration tests for AgentLoop
- [ ] End-to-end tests for both providers
- [ ] Test coverage > 80%

**Documentation**:
- [ ] Architecture documentation updated
- [ ] Tool migration guide created
- [ ] API documentation complete
- [ ] Examples provided

**Performance**:
- [ ] Tool execution adds < 100ms latency
- [ ] Approval prompts appear immediately
- [ ] No memory leaks in tool execution loop
- [ ] Conversation history size manageable

### 11.3 Migration Criteria

**Tool Migration Path**:
- [ ] Clear migration guide for other tools
- [ ] Tool trait well-documented
- [ ] Examples for common patterns
- [ ] Migration checklist provided

---

## 12. Dependencies and Risks

### 12.1 Dependencies

**Existing Components**:
- EventBus (implemented)
- Session (implemented)
- Worker (implemented)
- ConversationHistory (implemented, needs extension)
- ModelProvider trait (implemented, needs extension)
- BedrockConverseStreamModelProvider (implemented, needs extension)
- TextUi (implemented, needs extension)
- StructuredIO (implemented, needs extension)

**External Dependencies**:
- AWS SDK Bedrock Runtime (for Bedrock API)
- CodeWhisperer client crates (for CodeWhisperer API)
- Existing tool implementations (for porting logic)

### 12.2 Risks and Mitigation

**Risk 1: Bedrock API Complexity**
- **Impact**: High - Core functionality depends on correct API usage
- **Probability**: Medium - API is well-documented but complex
- **Mitigation**: Thorough research phase, prototype testing, reference existing implementations

**Risk 2: CodeWhisperer API Compatibility**
- **Impact**: High - Must support both providers
- **Probability**: Medium - API format may differ from Bedrock
- **Mitigation**: Research both APIs early, design abstraction layer, test both providers

**Risk 3: Conversation History Complexity**
- **Impact**: Medium - Affects all tool interactions
- **Probability**: Medium - Multiple tool requests add complexity
- **Mitigation**: Design phase with sequence diagrams, incremental implementation, thorough testing

**Risk 4: Approval Flow UX**
- **Impact**: Medium - Poor UX affects usability
- **Probability**: Low - Pattern exists in current implementation
- **Mitigation**: Reference existing approval UI, user testing, iterate on feedback

**Risk 5: Performance Impact**
- **Impact**: Low - Tool execution should be fast
- **Probability**: Low - Tools are simple file operations
- **Mitigation**: Performance testing, async execution, proper cancellation

**Risk 6: Tool Migration Complexity**
- **Impact**: Medium - Affects future tool additions
- **Probability**: Low - Pattern should be clear after fs_read/fs_write
- **Mitigation**: Document migration process, provide examples, create checklist

---

## 13. Related Files and Documentation

### 13.1 Implementation Files

**New Files** (to be created):
```
crates/chat-cli/src/agent_env/tools/
├── mod.rs                      # Module exports
├── tool_trait.rs               # Tool trait definition
├── tool_provider.rs            # ToolProvider implementation
├── tool_result.rs              # ToolResult enum
├── approval.rs                 # ToolApprovalRequest and related types
├── fs_read.rs                  # FsReadTool implementation
└── fs_write.rs                 # FsWriteTool implementation

crates/chat-cli/src/agent_env/
├── context_builder.rs          # ContextBuilder implementation
└── events.rs                   # Updated with tool events
```

**Modified Files**:
```
crates/chat-cli/src/agent_env/
├── worker.rs                   # Add tool_provider and open_tools_approval_requests
├── model_providers/
│   ├── model_provider.rs       # Update ModelRequest/ModelResponse
│   ├── bedrock_converse_stream.rs  # Add tool support
│   └── codewhisperer.rs        # New: CodeWhisperer provider with tools
├── worker_tasks/
│   └── agent_loop.rs           # Add tool execution flow
└── context_container/
    ├── conversation_history.rs # Extend for tool requests/results
    └── conversation_entry.rs   # Update message types

crates/chat-cli/src/cli/chat/agent_env_ui/
├── text_ui.rs                  # Add approval flow
└── structured_io.rs            # Add approval flow
```

### 13.2 Architecture Documentation

**Existing Documentation**:
- `codebase/agent-environment/README.md` - Architecture overview
- `codebase/agent-environment/worker.md` - Worker documentation
- `codebase/agent-environment/session.md` - Session documentation
- `codebase/agent-environment/model-provider.md` - ModelProvider documentation
- `codebase/agent-environment/context-container.md` - ConversationHistory documentation
- `codebase/agent-environment/event-bus.md` - EventBus documentation

**New Documentation** (to be created):
- `codebase/agent-environment/tools.md` - Tool system documentation
- `codebase/agent-environment/tool-migration-guide.md` - Tool migration guide
- `codebase/agent-environment/context-builder.md` - ContextBuilder documentation

### 13.3 API Documentation

**Bedrock**:
- AWS Bedrock Converse API documentation
- Tool use format and examples

**CodeWhisperer**:
- `codebase/aws-codewhisperer-clients.md` - Client overview
- `codebase/aws-codewhisperer-calls.md` - API usage
- `codebase/aws-codewhisperer-calls-example-request.json` - Example request
- `codebase/aws-codewhisperer-calls-example-response.json` - Example response

### 13.4 Existing Tool Implementations

**Reference Implementations**:
- `crates/chat-cli/src/cli/chat/tools/fs_read.rs` - Current fs_read
- `crates/chat-cli/src/cli/chat/tools/fs_write.rs` - Current fs_write
- `crates/chat-cli/src/cli/chat/tool_manager.rs` - Current tool management
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config with tool permissions

---

## 14. Estimated Effort

### 14.1 Phase Breakdown

| Phase | Description | Estimated Hours |
|-------|-------------|-----------------|
| 1 | Research | 6-8 |
| 2 | Design | 8-10 |
| 3 | Core Infrastructure | 10-12 |
| 4 | Conversation History Updates | 6-8 |
| 5 | ModelProvider Updates | 10-12 |
| 6 | AgentLoop Integration | 8-10 |
| 7 | EventBus and Events | 4-6 |
| 8 | TextUi Integration | 6-8 |
| 9 | StructuredIO Integration | 4-6 |
| 10 | Tool Implementations | 8-10 |
| 11 | Integration Testing | 6-8 |
| **Total** | | **76-98 hours** |

### 14.2 Critical Path

**Critical Path Items** (must be completed in order):
1. Research (Phase 1)
2. Design (Phase 2)
3. Core Infrastructure (Phase 3)
4. Conversation History Updates (Phase 4)
5. ModelProvider Updates (Phase 5)
6. AgentLoop Integration (Phase 6)

**Parallel Work Opportunities**:
- EventBus events (Phase 7) can start after Phase 3
- UI integration (Phases 8-9) can start after Phase 6
- Tool implementations (Phase 10) can start after Phase 3

### 14.3 Risk Buffer

Add 20-30% buffer for:
- API complexity discoveries
- Integration issues
- Testing and debugging
- Documentation

**Total with buffer**: 90-130 hours

---

## 15. Success Metrics

### 15.1 Functional Metrics

- [ ] 100% of acceptance criteria met
- [ ] Both Bedrock and CodeWhisperer providers working
- [ ] fs_read and fs_write tools functional
- [ ] Approval flow working in both UIs
- [ ] All end-to-end tests passing

### 15.2 Quality Metrics

- [ ] Test coverage > 80%
- [ ] Zero clippy warnings
- [ ] All code documented
- [ ] Architecture documentation updated
- [ ] Migration guide complete

### 15.3 Performance Metrics

- [ ] Tool execution latency < 100ms
- [ ] Approval prompt response time < 50ms
- [ ] No memory leaks detected
- [ ] Conversation history size manageable (< 10MB for 100 turns)

---

**End of Scope Document**
