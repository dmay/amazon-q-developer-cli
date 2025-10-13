# MVP Tools Basic - Technical Design

**Status**: Draft  
**Date**: 2025-10-13  
**Workflow**: mvp-tools-basic  

## Document Purpose

This design document provides detailed technical specifications for implementing tool execution in the agent_env architecture. It covers all components, interfaces, data structures, and integration points needed to enable LLMs to request and execute tools (fs_read, fs_write) with user approval flows.

## Design Principles

1. **Reuse Existing Patterns**: Leverage existing tool implementations and permission evaluation logic
2. **Event-Driven**: All tool lifecycle events published to EventBus
3. **Worker-Scoped**: Tools are properties of Workers, not global state
4. **API-Agnostic**: Internal structures support both Bedrock and CodeWhisperer
5. **Approval-First**: Tools require explicit approval unless pre-approved
6. **Minimal Changes**: Extend existing structures rather than replacing them

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Tool System Components](#2-tool-system-components)
3. [Conversation History Design](#3-conversation-history-design)
4. [ModelProvider Extensions](#4-modelprovider-extensions)
5. [AgentLoop Integration](#5-agentloop-integration)
6. [EventBus Integration](#6-eventbus-integration)
7. [UI Integration](#7-ui-integration)
8. [Tool Implementations](#8-tool-implementations)
9. [Sequence Diagrams](#9-sequence-diagrams)
10. [API Mappings](#10-api-mappings)
11. [Error Handling](#11-error-handling)
12. [Testing Strategy](#12-testing-strategy)

---

## 1. Architecture Overview

### 1.1 Component Relationships

```
┌─────────────────────────────────────────────────────────────┐
│                      AgentEnvironment                        │
│                  (Event Multicasting)                        │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ├─────────► TextUi (Approval Prompts)
                         ├─────────► StructuredIO (JSON I/O)
                         │
┌────────────────────────▼────────────────────────────────────┐
│                        EventBus                              │
│              (Tool Lifecycle Events)                         │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                        Session                               │
│              (Worker & Job Management)                       │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                        Worker                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ToolProvider                                         │   │
│  │  - Tool Registry                                     │   │
│  │  - Approval Rules                                    │   │
│  │  - execute_tool()                                    │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ OpenToolsApprovalRequests                            │   │
│  │  - Pending Approvals                                 │   │
│  │  - Approval Status                                   │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ConversationHistory                                  │   │
│  │  - Tool Requests                                     │   │
│  │  - Tool Results                                      │   │
│  └──────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                     AgentLoop Task                           │
│  1. Check pending approvals                                  │
│  2. Build ModelRequest (ContextBuilder)                      │
│  3. Query LLM                                                │
│  4. Execute tools or request approval                        │
│  5. Continue loop or exit                                    │
└──────────────────────────────────────────────────────────────┘
```

### 1.2 Data Flow

**Tool Execution Flow**:
```
User Input → AgentLoop → ContextBuilder → ModelRequest
                                              ↓
                                         ModelProvider
                                              ↓
                                        ModelResponse
                                              ↓
                                    Tool Requests Detected
                                              ↓
                                        ToolProvider
                                              ↓
                                    ┌─────────┴─────────┐
                                    │                   │
                              Auto-Approved      Needs Approval
                                    │                   │
                              Execute Tool      Store Request
                                    │                   │
                              Tool Result        Exit Task
                                    │                   │
                              Add to History    UI Prompts
                                    │                   │
                              Continue Loop     User Responds
                                    │                   │
                                    │            Update Status
                                    │                   │
                                    │            Relaunch Task
                                    │                   │
                                    └─────────┬─────────┘
                                              │
                                        Query LLM Again
```

### 1.3 Key Design Decisions

**Decision 1: Worker-Scoped Tools**
- **Rationale**: Each Worker can have different tool configurations based on Agent config
- **Impact**: ToolProvider is a property of Worker, not Session
- **Trade-off**: More memory per Worker, but better isolation

**Decision 2: Reuse Existing Message Structures**
- **Rationale**: UserMessage/AssistantMessage already support tool use in main chat
- **Impact**: Import from cli::chat::message instead of creating new structures
- **Trade-off**: Some unused fields, but proven API conversions

**Decision 3: Approval-First Execution**
- **Rationale**: Security and user control are paramount
- **Impact**: AgentLoop exits on approval requests, restarts after approval
- **Trade-off**: Adds latency, but ensures user oversight

**Decision 4: ToolResult Enum**
- **Rationale**: Three distinct outcomes (Success/Failure/ApprovalRequired) need different handling
- **Impact**: AgentLoop uses match statement for clear flow control
- **Trade-off**: More complex than boolean, but more expressive

---

## 2. Tool System Components

### 2.1 Tool Trait

**Purpose**: Define interface for tool implementations in agent_env.

**Location**: `crates/chat-cli/src/agent_env/tools/tool_trait.rs`

**Design**:

```rust
use async_trait::async_trait;
use eyre::Result;
use serde_json::Value;

/// Context provided to tools during execution
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Worker ID executing the tool
    pub worker_id: uuid::Uuid,
    
    /// Current working directory
    pub working_directory: std::path::PathBuf,
    
    /// OS interface for file operations
    pub os: std::sync::Arc<crate::os::Os>,
    
    /// Agent configuration (for permission checking)
    pub agent: std::sync::Arc<crate::cli::agent::Agent>,
}

/// Tool trait for agent_env architecture
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (e.g., "fs_read")
    fn name(&self) -> &str;
    
    /// Tool description for LLM
    fn description(&self) -> &str;
    
    /// JSON schema for tool parameters
    fn parameters_schema(&self) -> Value;
    
    /// Execute tool with given parameters
    /// 
    /// # Arguments
    /// * `parameters` - JSON parameters from LLM
    /// * `context` - Execution context (worker, os, agent)
    /// 
    /// # Returns
    /// * `Ok(String)` - Tool output to send back to LLM
    /// * `Err(eyre::Error)` - Tool execution error
    async fn execute(
        &self,
        parameters: Value,
        context: &ToolContext,
    ) -> Result<String>;
    
    /// Check if tool requires approval for given parameters
    /// 
    /// Default implementation returns Ask (requires approval).
    /// Tools can override to implement custom permission logic.
    fn requires_approval(
        &self,
        parameters: &Value,
        context: &ToolContext,
    ) -> crate::cli::agent::PermissionEvalResult {
        use crate::cli::agent::PermissionEvalResult;
        
        // Check if tool is in allowed_tools
        if context.agent.allowed_tools.contains(self.name()) {
            return PermissionEvalResult::Allow;
        }
        
        // Default: ask for approval
        PermissionEvalResult::Ask
    }
}
```

**Key Features**:
- Async execution for I/O operations
- ToolContext provides all needed dependencies
- requires_approval() allows per-tool permission logic
- JSON schema for LLM parameter validation

**Design Rationale**:
- Reuses PermissionEvalResult from existing code
- ToolContext bundles dependencies cleanly
- Default requires_approval() is secure (Ask)
- Tools can override for custom logic (e.g., path-based permissions)

### 2.2 ToolResult Enum

**Purpose**: Represent the three possible outcomes of tool execution.

**Location**: `crates/chat-cli/src/agent_env/tools/tool_result.rs`

**Design**:

```rust
/// Result of tool execution
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// Tool executed successfully
    Success {
        tool_name: String,
        tool_use_id: String,
        output: String,
    },
    
    /// Tool execution failed
    Failure {
        tool_name: String,
        tool_use_id: String,
        error: String,
    },
    
    /// Tool requires user approval before execution
    ApprovalRequired {
        tool_request: ToolRequest,
    },
}

/// Tool request from LLM (unified format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Unique ID for this tool use (from LLM)
    pub tool_use_id: String,
    
    /// Tool name
    pub tool_name: String,
    
    /// JSON parameters
    pub parameters: serde_json::Value,
}

impl ToolResult {
    /// Check if result is success
    pub fn is_success(&self) -> bool {
        matches!(self, ToolResult::Success { .. })
    }
    
    /// Check if result is failure
    pub fn is_failure(&self) -> bool {
        matches!(self, ToolResult::Failure { .. })
    }
    
    /// Check if approval is required
    pub fn is_approval_required(&self) -> bool {
        matches!(self, ToolResult::ApprovalRequired { .. })
    }
    
    /// Get tool_use_id if available
    pub fn tool_use_id(&self) -> Option<&str> {
        match self {
            ToolResult::Success { tool_use_id, .. } => Some(tool_use_id),
            ToolResult::Failure { tool_use_id, .. } => Some(tool_use_id),
            ToolResult::ApprovalRequired { tool_request } => Some(&tool_request.tool_use_id),
        }
    }
}
```

**Usage Pattern**:

```rust
// In AgentLoop
match tool_result {
    ToolResult::Success { tool_use_id, output, .. } => {
        // Add to conversation history
        conversation_history.push_tool_result(tool_use_id, output, ToolResultStatus::Success);
        // Continue loop
        continue;
    }
    ToolResult::Failure { tool_use_id, error, .. } => {
        // Add error to conversation history
        conversation_history.push_tool_result(tool_use_id, error, ToolResultStatus::Error);
        // Continue loop (LLM can handle error)
        continue;
    }
    ToolResult::ApprovalRequired { tool_request } => {
        // Store for approval
        worker.open_tools_approval_requests.lock().unwrap().push(
            ToolApprovalRequest::new(tool_request)
        );
        // Publish event
        event_bus.publish(ToolApprovalRequestEvent { ... });
        // Exit task
        break;
    }
}
```

**Design Rationale**:
- Three variants match three execution paths
- Success/Failure carry tool_use_id for correlation
- ApprovalRequired carries full ToolRequest for later execution
- Helper methods for pattern matching

### 2.3 ToolProvider

**Purpose**: Manage tool registry, approval rules, and tool execution for a Worker.

**Location**: `crates/chat-cli/src/agent_env/tools/tool_provider.rs`

**Design**:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use eyre::{Result, eyre};

use super::tool_trait::{Tool, ToolContext};
use super::tool_result::{ToolResult, ToolRequest};
use crate::cli::agent::{Agent, PermissionEvalResult};

/// Manages tools for a Worker
pub struct ToolProvider {
    /// Registry of available tools
    tools: HashMap<String, Arc<dyn Tool>>,
    
    /// Agent configuration (for permission checking)
    agent: Arc<Agent>,
    
    /// Global trust flag (from --trust-all-tools)
    trust_all_tools: bool,
    
    /// Session-wide trusted tools (set by user via 't' command)
    session_trusted_tools: Arc<std::sync::Mutex<HashSet<String>>>,
}

impl ToolProvider {
    /// Create new ToolProvider from Agent config
    pub fn new(
        agent: Arc<Agent>,
        trust_all_tools: bool,
    ) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        
        // Register built-in tools based on agent.tools list
        for tool_name in &agent.tools {
            if let Some(tool) = Self::create_builtin_tool(tool_name) {
                tools.insert(tool_name.clone(), tool);
            }
        }
        
        Self {
            tools,
            agent,
            trust_all_tools,
            session_trusted_tools: Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }
    
    /// Execute a tool request
    pub async fn execute_tool(
        &self,
        request: ToolRequest,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        // Get tool from registry
        let tool = self.tools.get(&request.tool_name)
            .ok_or_else(|| eyre!("Tool not found: {}", request.tool_name))?;
        
        // Check approval
        if self.requires_approval(&request.tool_name, &request.parameters, context) {
            return Ok(ToolResult::ApprovalRequired { tool_request: request });
        }
        
        // Execute tool
        match tool.execute(request.parameters.clone(), context).await {
            Ok(output) => Ok(ToolResult::Success {
                tool_name: request.tool_name,
                tool_use_id: request.tool_use_id,
                output,
            }),
            Err(error) => Ok(ToolResult::Failure {
                tool_name: request.tool_name,
                tool_use_id: request.tool_use_id,
                error: error.to_string(),
            }),
        }
    }
    
    /// Check if tool requires approval
    fn requires_approval(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
        context: &ToolContext,
    ) -> bool {
        // Check global trust flag
        if self.trust_all_tools {
            return false;
        }
        
        // Check session-wide trust
        if self.session_trusted_tools.lock().unwrap().contains(tool_name) {
            return false;
        }
        
        // Check tool-specific permission logic
        if let Some(tool) = self.tools.get(tool_name) {
            match tool.requires_approval(parameters, context) {
                PermissionEvalResult::Allow => false,
                PermissionEvalResult::Ask => true,
                PermissionEvalResult::Deny(_) => true, // Treat deny as ask for now
            }
        } else {
            true // Unknown tool requires approval
        }
    }
    
    /// Set session-wide trust for a tool
    pub fn set_session_trust(&self, tool_name: String, trust: bool) {
        let mut trusted = self.session_trusted_tools.lock().unwrap();
        if trust {
            trusted.insert(tool_name);
        } else {
            trusted.remove(&tool_name);
        }
    }
    
    /// Get tool definitions for ModelRequest
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters_schema: tool.parameters_schema(),
            })
            .collect()
    }
    
    /// Create built-in tool by name
    fn create_builtin_tool(name: &str) -> Option<Arc<dyn Tool>> {
        match name {
            "fs_read" => Some(Arc::new(super::fs_read::FsReadTool)),
            "fs_write" => Some(Arc::new(super::fs_write::FsWriteTool)),
            _ => None,
        }
    }
}

/// Tool definition for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}
```

**Key Features**:
- Tool registry built from Agent.tools list
- Three-level approval checking: global trust → session trust → tool-specific
- execute_tool() returns ToolResult enum
- get_tool_definitions() for ModelRequest construction
- set_session_trust() for 't' command

**Design Rationale**:
- Agent config drives tool availability
- Approval hierarchy matches existing patterns
- ToolResult enum enables clean flow control
- Session trust persists for user session

### 2.4 Tool Approval Requests

**Purpose**: Store pending tool approval requests in Worker state.

**Location**: `crates/chat-cli/src/agent_env/tools/approval.rs`

**Design**:

```rust
use std::time::Instant;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use super::tool_result::ToolRequest;

/// Pending tool approval request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApprovalRequest {
    /// Unique ID for this approval request
    pub approval_id: Uuid,
    
    /// The tool request from LLM
    pub tool_request: ToolRequest,
    
    /// Approval status (None = pending)
    pub approval_status: Option<ToolApprovalStatus>,
    
    /// When request was created
    #[serde(skip)]
    pub created_at: Instant,
}

/// Approval status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolApprovalStatus {
    /// User approved execution
    Approved,
    
    /// User rejected execution
    Rejected {
        /// Optional rejection reason
        reason: Option<String>,
    },
}

impl ToolApprovalRequest {
    /// Create new approval request
    pub fn new(tool_request: ToolRequest) -> Self {
        Self {
            approval_id: Uuid::new_v4(),
            tool_request,
            approval_status: None,
            created_at: Instant::now(),
        }
    }
    
    /// Check if request is pending
    pub fn is_pending(&self) -> bool {
        self.approval_status.is_none()
    }
    
    /// Check if request is approved
    pub fn is_approved(&self) -> bool {
        matches!(self.approval_status, Some(ToolApprovalStatus::Approved))
    }
    
    /// Check if request is rejected
    pub fn is_rejected(&self) -> bool {
        matches!(self.approval_status, Some(ToolApprovalStatus::Rejected { .. }))
    }
    
    /// Approve the request
    pub fn approve(&mut self) {
        self.approval_status = Some(ToolApprovalStatus::Approved);
    }
    
    /// Reject the request
    pub fn reject(&mut self, reason: Option<String>) {
        self.approval_status = Some(ToolApprovalStatus::Rejected { reason });
    }
}
```

**Worker Integration**:

```rust
// In Worker struct
pub struct Worker {
    // ... existing fields ...
    
    /// Pending tool approval requests
    #[serde(skip, default)]
    pub open_tools_approval_requests: Arc<Mutex<Vec<ToolApprovalRequest>>>,
    
    /// Tool provider for this worker
    #[serde(skip, default)]
    pub tool_provider: Option<Arc<Mutex<ToolProvider>>>,
}
```

**Lifecycle**:
1. AgentLoop receives tool request from LLM
2. ToolProvider returns ApprovalRequired
3. AgentLoop creates ToolApprovalRequest
4. AgentLoop adds to worker.open_tools_approval_requests
5. AgentLoop publishes ToolApprovalRequestEvent
6. AgentLoop exits task
7. UI displays approval prompt
8. User responds (approve/reject)
9. UI updates approval_status
10. UI relaunches AgentLoop
11. AgentLoop checks requests on start
12. AgentLoop executes approved tools
13. AgentLoop removes processed requests

**Design Rationale**:
- Uuid for unique identification
- approval_status None = pending (explicit state)
- Helper methods for status checking
- Serializable for potential persistence
- created_at for timeout logic (future)

---

## 3. Conversation History Design

### 3.1 Current Structure Analysis

ConversationHistory currently uses UserMessage and AssistantMessage from `cli::chat::message`. These structures already support tool use:

```rust
// From cli/chat/message.rs
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

pub struct AssistantToolUse {
    pub id: String,           // tool_use_id
    pub name: String,         // tool name
    pub input: serde_json::Value,  // tool parameters
    pub accepted: bool,       // approval status
}

pub enum UserMessageContent {
    Prompt { prompt: String },
    ToolUseResults { 
        tool_use_results: Vec<ToolUseResult>,
    },
    CancelledToolUses { 
        prompt: Option<String>,
        tool_use_results: Vec<ToolUseResult>,
    },
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

**Key Observation**: These structures already support tool use! We can reuse them directly.

### 3.2 ConversationHistory Extensions

**No structural changes needed** - existing structures support tools. We only need helper methods:

**Location**: `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`

**New Methods**:

```rust
impl ConversationHistory {
    /// Add tool use request from LLM
    pub fn push_tool_use(
        &mut self,
        content: String,
        tool_uses: Vec<AssistantToolUse>,
    ) {
        self.entries.push(ConversationEntry::new_assistant(
            AssistantMessage::ToolUse {
                message_id: None,
                content,
                tool_uses,
            }
        ));
    }
    
    /// Add tool results from execution
    pub fn push_tool_results(
        &mut self,
        tool_use_results: Vec<ToolUseResult>,
    ) {
        self.entries.push(ConversationEntry::new_user(
            UserMessage::new_tool_results(tool_use_results)
        ));
    }
    
    /// Add single tool result
    pub fn push_tool_result(
        &mut self,
        tool_use_id: String,
        output: String,
        status: ToolResultStatus,
    ) {
        self.push_tool_results(vec![ToolUseResult {
            id: tool_use_id,
            output,
            status,
        }]);
    }
    
    /// Get last assistant message if it's a tool use
    pub fn get_last_tool_use(&self) -> Option<&AssistantMessage> {
        self.entries.last()
            .and_then(|entry| entry.assistant.as_ref())
            .filter(|msg| matches!(msg, AssistantMessage::ToolUse { .. }))
    }
}
```

**Design Rationale**:
- Reuse existing structures (no duplication)
- Helper methods for common operations
- Maintains compatibility with existing code
- Proven API conversion logic

### 3.3 Conversion to ToolRequest

**Helper Function**:

```rust
// In agent_env/tools/mod.rs
use crate::cli::chat::message::AssistantToolUse;
use super::tool_result::ToolRequest;

/// Convert AssistantToolUse to ToolRequest
pub fn assistant_tool_use_to_request(tool_use: &AssistantToolUse) -> ToolRequest {
    ToolRequest {
        tool_use_id: tool_use.id.clone(),
        tool_name: tool_use.name.clone(),
        parameters: tool_use.input.clone(),
    }
}

/// Convert multiple AssistantToolUse to ToolRequest vec
pub fn assistant_tool_uses_to_requests(tool_uses: &[AssistantToolUse]) -> Vec<ToolRequest> {
    tool_uses.iter()
        .map(assistant_tool_use_to_request)
        .collect()
}
```

---

## 4. ModelProvider Extensions

### 4.1 Current ModelProvider Analysis

Current structures:

```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,
    pub context: Option<String>,
    pub conversation_id: Option<String>,
}

pub struct ModelResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,  // Already exists!
}

pub struct ToolRequest {
    pub tool_name: String,
    pub parameters: String,  // ⚠️ String, not JSON
}
```

**Issue**: ToolRequest.parameters is String, but we need serde_json::Value.

### 4.2 ModelRequest Extensions

**Changes Needed**:

```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,
    pub context: Option<String>,
    pub conversation_id: Option<String>,
    
    /// Tool definitions for LLM (NEW)
    pub tools: Vec<ToolDefinition>,
}

/// Tool definition for LLM (NEW)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}
```

**Design Rationale**:
- Add tools field for tool definitions
- Keep existing fields for backward compatibility
- Empty tools vec = no tools available

### 4.3 ToolRequest Update

**Change**:

```rust
pub struct ToolRequest {
    pub tool_use_id: String,  // NEW: for correlation
    pub tool_name: String,
    pub parameters: serde_json::Value,  // CHANGED: from String
}
```

**Migration**:
- Update all usages of ToolRequest
- Parse parameters as JSON in providers
- Add tool_use_id extraction

### 4.4 ModelResponse Extensions

**No changes needed** - already has tool_requests field.

### 4.5 ConversationMessage Extensions

**Current**:

```rust
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,  // ⚠️ Simple string
}
```

**Issue**: Cannot represent tool results in messages.

**Solution**: Keep ConversationMessage simple, handle tool results in provider-specific conversion.

**Design Decision**: 
- ConversationMessage remains simple (role + content)
- ContextBuilder converts UserMessage/AssistantMessage to provider-specific formats
- Each ModelProvider implements its own conversion logic

---

## 5. AgentLoop Integration

### 5.1 Task Start Logic

**Purpose**: Check for pending approvals and execute approved tools before starting main loop.

**Location**: `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Design**:

```rust
impl AgentLoopTask {
    pub async fn run(&self, worker: Arc<Worker>) -> Result<()> {
        // Check for pending approval requests
        let has_pending = self.check_and_execute_pending_approvals(&worker).await?;
        
        if has_pending {
            // Some requests still pending - exit immediately
            return Ok(());
        }
        
        // Continue with main agent loop
        self.run_main_loop(worker).await
    }
    
    async fn check_and_execute_pending_approvals(
        &self,
        worker: &Arc<Worker>,
    ) -> Result<bool> {
        let mut pending = worker.open_tools_approval_requests.lock().unwrap();
        
        if pending.is_empty() {
            return Ok(false);
        }
        
        // Check if all have approval status
        let all_have_status = pending.iter().all(|req| req.approval_status.is_some());
        
        if !all_have_status {
            // Some still pending
            return Ok(true);
        }
        
        // Execute approved tools
        for request in pending.iter() {
            match &request.approval_status {
                Some(ToolApprovalStatus::Approved) => {
                    self.execute_approved_tool(&request.tool_request, worker).await?;
                }
                Some(ToolApprovalStatus::Rejected { reason }) => {
                    self.add_tool_rejection(worker, &request.tool_request, reason)?;
                }
                None => unreachable!(),
            }
        }
        
        // Clear processed requests
        pending.clear();
        drop(pending);
        
        Ok(false)
    }
    
    async fn execute_approved_tool(
        &self,
        tool_request: &ToolRequest,
        worker: &Arc<Worker>,
    ) -> Result<()> {
        let tool_provider = worker.tool_provider.as_ref()
            .ok_or_else(|| eyre!("No tool provider"))?;
        
        let context = ToolContext {
            worker_id: worker.id,
            working_directory: std::env::current_dir()?,
            os: worker.get_os().ok_or_else(|| eyre!("No OS"))?,
            agent: /* get from worker */,
        };
        
        // Execute directly (skip approval check)
        let tool = tool_provider.lock().unwrap()
            .tools.get(&tool_request.tool_name)
            .ok_or_else(|| eyre!("Tool not found"))?
            .clone();
        
        match tool.execute(tool_request.parameters.clone(), &context).await {
            Ok(output) => {
                worker.context_container.conversation_history
                    .lock().unwrap()
                    .push_tool_result(
                        tool_request.tool_use_id.clone(),
                        output,
                        ToolResultStatus::Success,
                    );
            }
            Err(error) => {
                worker.context_container.conversation_history
                    .lock().unwrap()
                    .push_tool_result(
                        tool_request.tool_use_id.clone(),
                        error.to_string(),
                        ToolResultStatus::Error,
                    );
            }
        }
        
        Ok(())
    }
}
```

### 5.2 Main Loop with Tool Execution

**Design**:

```rust
async fn run_main_loop(&self, worker: Arc<Worker>) -> Result<()> {
    loop {
        // Check cancellation
        if self.cancellation_token.is_cancelled() {
            break;
        }
        
        // Build request with tools
        let request = self.build_model_request(&worker)?;
        
        // Query LLM
        let response = self.query_llm(&worker, request).await?;
        
        // Check for tool requests
        if !response.tool_requests.is_empty() {
            let has_pending = self.process_tool_requests(&worker, response.tool_requests).await?;
            
            if has_pending {
                // Approval required - exit task
                break;
            }
            
            // All tools executed - continue loop
            continue;
        }
        
        // No tool requests - add response and exit
        worker.context_container.conversation_history
            .lock().unwrap()
            .push_assistant_message(AssistantMessage::Response {
                message_id: None,
                content: response.content,
            });
        
        break;
    }
    
    Ok(())
}

async fn process_tool_requests(
    &self,
    worker: &Arc<Worker>,
    tool_requests: Vec<ToolRequest>,
) -> Result<bool> {
    let tool_provider = worker.tool_provider.as_ref()
        .ok_or_else(|| eyre!("No tool provider"))?;
    
    let context = ToolContext {
        worker_id: worker.id,
        working_directory: std::env::current_dir()?,
        os: worker.get_os().ok_or_else(|| eyre!("No OS"))?,
        agent: /* get from worker */,
    };
    
    let mut has_pending = false;
    
    for tool_request in tool_requests {
        let result = tool_provider.lock().unwrap()
            .execute_tool(tool_request.clone(), &context)
            .await?;
        
        match result {
            ToolResult::Success { tool_use_id, output, .. } => {
                worker.context_container.conversation_history
                    .lock().unwrap()
                    .push_tool_result(tool_use_id, output, ToolResultStatus::Success);
            }
            ToolResult::Failure { tool_use_id, error, .. } => {
                worker.context_container.conversation_history
                    .lock().unwrap()
                    .push_tool_result(tool_use_id, error, ToolResultStatus::Error);
            }
            ToolResult::ApprovalRequired { tool_request } => {
                // Store for approval
                let approval_request = ToolApprovalRequest::new(tool_request);
                worker.open_tools_approval_requests
                    .lock().unwrap()
                    .push(approval_request.clone());
                
                // Publish event
                self.event_bus.publish(Event::ToolApprovalRequest(
                    ToolApprovalRequestEvent {
                        worker_id: worker.id,
                        approval_id: approval_request.approval_id,
                        tool_name: approval_request.tool_request.tool_name.clone(),
                        tool_use_id: approval_request.tool_request.tool_use_id.clone(),
                        parameters: approval_request.tool_request.parameters.clone(),
                        timestamp: Instant::now(),
                    }
                ));
                
                has_pending = true;
            }
        }
    }
    
    Ok(has_pending)
}
```

**Design Rationale**:
- Approval check at task start handles restart after approval
- Main loop continues until no more tool requests or approval needed
- ToolResult enum enables clean flow control
- Events published for UI integration

---

## 6. EventBus Integration

### 6.1 New Event Types

**Location**: `crates/chat-cli/src/agent_env/events.rs`

**Design**:

```rust
/// Tool approval request created
#[derive(Debug, Clone)]
pub struct ToolApprovalRequestEvent {
    pub worker_id: Uuid,
    pub approval_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub parameters: serde_json::Value,
    pub timestamp: Instant,
}

/// Tool approval status updated
#[derive(Debug, Clone)]
pub struct ToolApprovalUpdatedEvent {
    pub worker_id: Uuid,
    pub approval_id: Uuid,
    pub status: ToolApprovalStatus,
    pub timestamp: Instant,
}

/// Tool execution started
#[derive(Debug, Clone)]
pub struct ToolExecutionStartedEvent {
    pub worker_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub timestamp: Instant,
}

/// Tool execution completed
#[derive(Debug, Clone)]
pub struct ToolExecutionCompletedEvent {
    pub worker_id: Uuid,
    pub tool_name: String,
    pub tool_use_id: String,
    pub result: ToolExecutionResult,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub enum ToolExecutionResult {
    Success { output_length: usize },
    Failure { error: String },
}

/// Add to AgentEnvironmentEvent enum
pub enum AgentEnvironmentEvent {
    // ... existing variants ...
    ToolApprovalRequest(ToolApprovalRequestEvent),
    ToolApprovalUpdated(ToolApprovalUpdatedEvent),
    ToolExecutionStarted(ToolExecutionStartedEvent),
    ToolExecutionCompleted(ToolExecutionCompletedEvent),
}
```

### 6.2 Event Publishing Points

**AgentLoop**:
- `ToolApprovalRequestEvent`: When tool requires approval
- `ToolExecutionStartedEvent`: Before executing approved tool
- `ToolExecutionCompletedEvent`: After tool execution

**UI**:
- `ToolApprovalUpdatedEvent`: When user approves/rejects

---

## 7. UI Integration

### 7.1 TextUi Approval Flow

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Design**:

```rust
impl TextUi {
    /// Handle job completed event - check for pending approvals
    fn handle_job_completed(&mut self, worker_id: Uuid) -> Result<()> {
        let worker = self.session.get_worker(worker_id)?;
        let pending = worker.open_tools_approval_requests.lock().unwrap();
        
        if !pending.is_empty() && pending.iter().any(|r| r.is_pending()) {
            drop(pending);
            // Enter approval mode
            self.show_approval_prompts(worker)?;
        } else {
            // Normal prompt mode
            self.show_user_prompt()?;
        }
        
        Ok(())
    }
    
    /// Show approval prompts for pending tools
    fn show_approval_prompts(&mut self, worker: Arc<Worker>) -> Result<()> {
        let mut pending = worker.open_tools_approval_requests.lock().unwrap();
        
        for request in pending.iter_mut() {
            if !request.is_pending() {
                continue;
            }
            
            // Display tool request
            println!("\n{} Tool approval required:", "⚠".yellow());
            println!("  Tool: {}", request.tool_request.tool_name.green());
            println!("  Parameters:");
            println!("{}", 
                serde_json::to_string_pretty(&request.tool_request.parameters)?
                    .lines()
                    .map(|line| format!("    {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            
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
                "y" | "Y" => {
                    request.approve();
                    println!("{} Tool approved", "✓".green());
                }
                "t" | "T" => {
                    // Set session trust
                    if let Some(tool_provider) = &worker.tool_provider {
                        tool_provider.lock().unwrap()
                            .set_session_trust(request.tool_request.tool_name.clone(), true);
                    }
                    request.approve();
                    println!("{} Tool approved and trusted for session", "✓".green());
                }
                "n" | "N" => {
                    request.reject(None);
                    println!("{} Tool rejected", "✗".red());
                }
                other => {
                    request.reject(Some(other.to_string()));
                    println!("{} Tool rejected: {}", "✗".red(), other);
                }
            }
            
            // Publish update event
            self.event_bus.publish(AgentEnvironmentEvent::ToolApprovalUpdated(
                ToolApprovalUpdatedEvent {
                    worker_id: worker.id,
                    approval_id: request.approval_id,
                    status: request.approval_status.clone().unwrap(),
                    timestamp: Instant::now(),
                }
            ))?;
        }
        
        // Check if all have status
        let all_have_status = pending.iter().all(|r| !r.is_pending());
        drop(pending);
        
        if all_have_status {
            // Relaunch AgentLoop
            self.session.run_task__agent_loop(worker, AgentLoopInput {})?;
        }
        
        Ok(())
    }
}
```

### 7.2 StructuredIO Approval Flow

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**JSON Input Format**:

```json
{
  "type": "tool_approval",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "result": "approve"
}
```

Or with rejection:

```json
{
  "type": "tool_approval",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "result": "User denied access"
}
```

**Trust Command**:

```json
{
  "type": "tool_trust",
  "worker_id": "uuid",
  "tool_name": "fs_read",
  "trust": true
}
```

**JSON Output Format**:

```json
{
  "type": "tool_approval_request",
  "worker_id": "uuid",
  "approval_id": "uuid",
  "tool_name": "fs_read",
  "tool_use_id": "1",
  "parameters": { ... },
  "timestamp": "2025-10-13T08:16:57Z"
}
```

**Implementation**:

```rust
impl StructuredIO {
    fn handle_input(&mut self, input: serde_json::Value) -> Result<()> {
        let input_type = input["type"].as_str()
            .ok_or_else(|| eyre!("Missing type field"))?;
        
        match input_type {
            "tool_approval" => self.handle_tool_approval(input),
            "tool_trust" => self.handle_tool_trust(input),
            _ => Ok(()),
        }
    }
    
    fn handle_tool_approval(&mut self, input: serde_json::Value) -> Result<()> {
        let worker_id: Uuid = input["worker_id"].as_str()
            .ok_or_else(|| eyre!("Missing worker_id"))?
            .parse()?;
        let approval_id: Uuid = input["approval_id"].as_str()
            .ok_or_else(|| eyre!("Missing approval_id"))?
            .parse()?;
        let result = input["result"].as_str()
            .ok_or_else(|| eyre!("Missing result"))?;
        
        let worker = self.session.get_worker(worker_id)?;
        let mut pending = worker.open_tools_approval_requests.lock().unwrap();
        
        if let Some(request) = pending.iter_mut().find(|r| r.approval_id == approval_id) {
            match result {
                "approve" => request.approve(),
                "reject" => request.reject(None),
                other => request.reject(Some(other.to_string())),
            }
            
            // Publish event
            self.event_bus.publish(AgentEnvironmentEvent::ToolApprovalUpdated(
                ToolApprovalUpdatedEvent {
                    worker_id,
                    approval_id,
                    status: request.approval_status.clone().unwrap(),
                    timestamp: Instant::now(),
                }
            ))?;
        }
        
        // Check if all have status
        let all_have_status = pending.iter().all(|r| !r.is_pending());
        drop(pending);
        
        if all_have_status {
            self.session.run_task__agent_loop(worker, AgentLoopInput {})?;
        }
        
        Ok(())
    }
    
    fn handle_event(&mut self, event: AgentEnvironmentEvent) -> Result<()> {
        match event {
            AgentEnvironmentEvent::ToolApprovalRequest(e) => {
                let output = json!({
                    "type": "tool_approval_request",
                    "worker_id": e.worker_id.to_string(),
                    "approval_id": e.approval_id.to_string(),
                    "tool_name": e.tool_name,
                    "tool_use_id": e.tool_use_id,
                    "parameters": e.parameters,
                    "timestamp": e.timestamp,
                });
                println!("{}", serde_json::to_string(&output)?);
            }
            _ => {}
        }
        Ok(())
    }
}
```

---

## 8. Tool Implementations

### 8.1 FsReadTool

**Location**: `crates/chat-cli/src/agent_env/tools/fs_read.rs`

**Design**:

```rust
use async_trait::async_trait;
use eyre::Result;
use serde_json::{json, Value};

use super::tool_trait::{Tool, ToolContext};
use crate::cli::chat::tools::fs_read::FsRead;
use crate::cli::agent::PermissionEvalResult;

pub struct FsReadTool;

#[async_trait]
impl Tool for FsReadTool {
    fn name(&self) -> &str {
        "fs_read"
    }
    
    fn description(&self) -> &str {
        "Read files, directories, and images. Supports batch operations."
    }
    
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "mode": {
                                "type": "string",
                                "enum": ["Line", "Directory", "Search", "Image"]
                            },
                            "path": { "type": "string" },
                            "start_line": { "type": "integer" },
                            "end_line": { "type": "integer" },
                            "pattern": { "type": "string" },
                            "depth": { "type": "integer" }
                        },
                        "required": ["mode"]
                    }
                },
                "summary": { "type": "string" }
            },
            "required": ["operations"]
        })
    }
    
    async fn execute(&self, parameters: Value, context: &ToolContext) -> Result<String> {
        // Parse parameters
        let mut fs_read: FsRead = serde_json::from_value(parameters)?;
        
        // Validate
        fs_read.validate(&context.os).await?;
        
        // Execute (reuse existing logic)
        let mut output = Vec::new();
        fs_read.invoke(&context.os, &mut output).await?;
        
        Ok(String::from_utf8(output)?)
    }
    
    fn requires_approval(&self, parameters: &Value, context: &ToolContext) -> PermissionEvalResult {
        // Parse parameters
        let Ok(fs_read) = serde_json::from_value::<FsRead>(parameters.clone()) else {
            return PermissionEvalResult::Ask;
        };
        
        // Reuse existing permission logic
        fs_read.eval_perm(&context.os, &context.agent)
    }
}
```

**Design Rationale**:
- Reuses existing FsRead struct and logic
- Wraps in Tool trait for agent_env
- Permission logic delegates to existing eval_perm()
- Minimal code duplication

### 8.2 FsWriteTool

**Similar pattern to FsReadTool** - wraps existing FsWrite logic.

---

## 9. Sequence Diagrams

### 9.1 Tool Execution with Approval

```
User → TextUi: "Read /tmp/test.txt"
TextUi → AgentEnvironment: Prompt command
AgentEnvironment → Session: run_task__agent_loop()
Session → AgentLoop: Start task
AgentLoop → ContextBuilder: build_request()
ContextBuilder → ModelProvider: request(with tools)
ModelProvider → LLM: Query with tool definitions
LLM → ModelProvider: Response with tool_use
ModelProvider → AgentLoop: ModelResponse(tool_requests)
AgentLoop → ToolProvider: execute_tool(fs_read)
ToolProvider → AgentLoop: ApprovalRequired
AgentLoop → Worker: Store in open_tools_approval_requests
AgentLoop → EventBus: Publish ToolApprovalRequestEvent
AgentLoop: Exit task
EventBus → TextUi: Forward event
TextUi → User: Display approval prompt
User → TextUi: "y" (approve)
TextUi → Worker: Update approval_status = Approved
TextUi → EventBus: Publish ToolApprovalUpdatedEvent
TextUi → Session: run_task__agent_loop() (restart)
Session → AgentLoop: Start task
AgentLoop → Worker: Check open_tools_approval_requests
AgentLoop: Find approved request
AgentLoop → FsReadTool: execute()
FsReadTool → AgentLoop: Success(output)
AgentLoop → ConversationHistory: Add tool result
AgentLoop → ContextBuilder: build_request()
ContextBuilder → ModelProvider: request(with tool result)
ModelProvider → LLM: Query with tool result
LLM → ModelProvider: Response (text)
ModelProvider → AgentLoop: ModelResponse(content)
AgentLoop → ConversationHistory: Add assistant response
AgentLoop: Exit task (complete)
```

### 9.2 Tool Execution with Auto-Approval

```
User → TextUi: "Read /tmp/test.txt" (with --trust-all-tools)
TextUi → AgentEnvironment: Prompt command
AgentEnvironment → Session: run_task__agent_loop()
Session → AgentLoop: Start task
AgentLoop → ContextBuilder: build_request()
ContextBuilder → ModelProvider: request(with tools)
ModelProvider → LLM: Query with tool definitions
LLM → ModelProvider: Response with tool_use
ModelProvider → AgentLoop: ModelResponse(tool_requests)
AgentLoop → ToolProvider: execute_tool(fs_read)
ToolProvider: Check trust_all_tools = true
ToolProvider → FsReadTool: execute()
FsReadTool → ToolProvider: Success(output)
ToolProvider → AgentLoop: Success(output)
AgentLoop → ConversationHistory: Add tool result
AgentLoop → ContextBuilder: build_request()
ContextBuilder → ModelProvider: request(with tool result)
ModelProvider → LLM: Query with tool result
LLM → ModelProvider: Response (text)
ModelProvider → AgentLoop: ModelResponse(content)
AgentLoop → ConversationHistory: Add assistant response
AgentLoop: Exit task (complete)
```

---

## 10. API Mappings

### 10.1 Bedrock Converse API

**Tool Definitions**:

```rust
// Convert ToolDefinition to Bedrock format
fn to_bedrock_tool_spec(def: &ToolDefinition) -> aws_sdk_bedrockruntime::types::ToolSpecification {
    ToolSpecification::builder()
        .name(&def.name)
        .description(&def.description)
        .input_schema(ToolInputSchema::Json(def.parameters_schema.to_string()))
        .build()
        .unwrap()
}

// Add to request
let tool_config = ToolConfiguration::builder()
    .set_tools(Some(
        tool_definitions.iter()
            .map(to_bedrock_tool_spec)
            .collect()
    ))
    .build()?;

let request = client.converse_stream()
    .model_id(model_id)
    .set_messages(Some(messages))
    .set_tool_config(Some(tool_config));
```

**Tool Use Parsing**:

```rust
// Parse streaming response
match event {
    ConverseStreamOutput::ContentBlockStart(start) => {
        if let Some(ContentBlockStart::ToolUse(tool_use)) = start.start {
            current_tool_use = Some(ToolUseAccumulator {
                tool_use_id: tool_use.tool_use_id,
                name: tool_use.name,
                input: String::new(),
            });
        }
    }
    ConverseStreamOutput::ContentBlockDelta(delta) => {
        if let Some(ContentBlockDelta::ToolUse(tool_use_delta)) = delta.delta {
            if let Some(ref mut acc) = current_tool_use {
                acc.input.push_str(&tool_use_delta.input);
            }
        }
    }
    ConverseStreamOutput::ContentBlockStop(_) => {
        if let Some(acc) = current_tool_use.take() {
            tool_requests.push(ToolRequest {
                tool_use_id: acc.tool_use_id,
                tool_name: acc.name,
                parameters: serde_json::from_str(&acc.input)?,
            });
        }
    }
}
```

**Tool Results**:

```rust
// Convert ToolUseResult to Bedrock format
fn to_bedrock_tool_result(result: &ToolUseResult) -> ContentBlock {
    ContentBlock::ToolResult(
        ToolResultBlock::builder()
            .tool_use_id(&result.id)
            .content(ToolResultContentBlock::Text(result.output.clone()))
            .status(match result.status {
                ToolResultStatus::Success => ToolResultStatus::Success,
                ToolResultStatus::Error => ToolResultStatus::Error,
            })
            .build()
            .unwrap()
    )
}
```

### 10.2 CodeWhisperer API

**Tool Definitions**:

```rust
// Convert ToolDefinition to CodeWhisperer format
fn to_cw_tool_spec(def: &ToolDefinition) -> ToolSpec {
    ToolSpec::builder()
        .name(&def.name)
        .description(&def.description)
        .input_schema(def.parameters_schema.to_string())
        .build()
        .unwrap()
}

// Add to request
let user_input_context = UserInputMessageContext::builder()
    .set_tools(Some(
        tool_definitions.iter()
            .map(to_cw_tool_spec)
            .collect()
    ))
    .build()?;
```

**Tool Use Parsing**:

```rust
// Parse streaming response
match event {
    ChatResponseStream::ToolUseEvent(tool_use_event) => {
        tool_requests.push(ToolRequest {
            tool_use_id: tool_use_event.tool_use.id,
            tool_name: tool_use_event.tool_use.name,
            parameters: serde_json::from_str(&tool_use_event.tool_use.input)?,
        });
    }
}
```

**Tool Results**:

```rust
// Convert ToolUseResult to CodeWhisperer format
fn to_cw_tool_result(result: &ToolUseResult) -> ToolResult {
    ToolResult::builder()
        .tool_use_id(&result.id)
        .content(&result.output)
        .status(match result.status {
            ToolResultStatus::Success => "success",
            ToolResultStatus::Error => "error",
        })
        .build()
        .unwrap()
}

// Add to next request
let user_input_context = UserInputMessageContext::builder()
    .set_tool_results(Some(
        tool_results.iter()
            .map(to_cw_tool_result)
            .collect()
    ))
    .build()?;
```

---

## 11. Error Handling

### 11.1 Tool Execution Errors

**Strategy**: Convert to ToolResult::Failure, send to LLM

```rust
match tool.execute(parameters, context).await {
    Ok(output) => ToolResult::Success { ... },
    Err(error) => ToolResult::Failure {
        tool_name,
        tool_use_id,
        error: error.to_string(),
    },
}
```

**LLM receives error** and can:
- Retry with different parameters
- Ask user for clarification
- Abandon task

### 11.2 Invalid Tool Requests

**Strategy**: Return error to LLM

```rust
if !tool_provider.has_tool(&tool_name) {
    return Ok(ToolResult::Failure {
        tool_name,
        tool_use_id,
        error: format!("Tool '{}' not found", tool_name),
    });
}
```

### 11.3 Parameter Parsing Errors

**Strategy**: Return error to LLM

```rust
let fs_read: FsRead = match serde_json::from_value(parameters) {
    Ok(v) => v,
    Err(e) => return Ok(ToolResult::Failure {
        tool_name,
        tool_use_id,
        error: format!("Invalid parameters: {}", e),
    }),
};
```

---

## 12. Testing Strategy

### 12.1 Unit Tests

**ToolProvider**:
- Tool registration
- Approval checking (trust_all_tools, session_trust, tool-specific)
- execute_tool() with different approval states

**Tool Implementations**:
- FsReadTool parameter parsing
- FsReadTool execution
- FsWriteTool parameter parsing
- FsWriteTool execution

**ConversationHistory**:
- push_tool_use()
- push_tool_results()
- Serialization

### 12.2 Integration Tests

**AgentLoop**:
- Tool execution flow
- Approval request handling
- Task restart after approval
- Multi-tool execution

**UI Integration**:
- TextUi approval flow
- StructuredIO approval flow
- Event handling

### 12.3 End-to-End Tests

**Scenarios**:
1. Simple tool use (auto-approved)
2. Tool approval (user approves)
3. Tool rejection (user rejects)
4. Session trust ('t' command)
5. Multi-tool execution
6. Tool failure handling
7. Both providers (Bedrock + CodeWhisperer)

---

## 13. Implementation Notes

### 13.1 Worker Construction

**Add to WorkerBuilder or Session::build_worker()**:

```rust
// Create ToolProvider from Agent config
let tool_provider = if let Some(agent) = agent_config {
    Some(Arc::new(Mutex::new(
        ToolProvider::new(
            Arc::new(agent.clone()),
            trust_all_tools,
        )
    )))
} else {
    None
};

worker.tool_provider = tool_provider;
worker.open_tools_approval_requests = Arc::new(Mutex::new(Vec::new()));
```

### 13.2 Agent Config Access

**Add Agent field to Worker**:

```rust
pub struct Worker {
    // ... existing fields ...
    
    #[serde(skip, default)]
    pub agent: Option<Arc<Agent>>,
}
```

### 13.3 ContextBuilder

**Create ContextBuilder struct**:

```rust
pub struct ContextBuilder {
    conversation_history: Arc<Mutex<ConversationHistory>>,
    tool_provider: Option<Arc<Mutex<ToolProvider>>>,
}

impl ContextBuilder {
    pub fn build_request(&self) -> ModelRequest {
        let history = self.conversation_history.lock().unwrap();
        let messages = self.convert_history_to_messages(&history);
        
        let tools = if let Some(tp) = &self.tool_provider {
            tp.lock().unwrap().get_tool_definitions()
        } else {
            vec![]
        };
        
        ModelRequest {
            messages,
            tools,
            system_prompt: None,
            context: None,
            conversation_id: None,
        }
    }
}
```

---

## 14. Design Summary

### 14.1 Key Components

1. **Tool Trait**: Interface for tool implementations
2. **ToolProvider**: Manages tools and approval rules per Worker
3. **ToolResult**: Three-variant enum for execution outcomes
4. **ToolApprovalRequest**: Pending approval state in Worker
5. **ConversationHistory**: Reuses existing structures for tool use
6. **ModelProvider**: Extended with tools field
7. **AgentLoop**: Approval-first execution pattern
8. **EventBus**: Tool lifecycle events
9. **UI**: Approval prompts in TextUi and StructuredIO

### 14.2 Design Principles Applied

✓ **Reuse Existing Patterns**: Tool trait mirrors existing tools, permission logic reused  
✓ **Event-Driven**: All tool lifecycle events published to EventBus  
✓ **Worker-Scoped**: ToolProvider is property of Worker  
✓ **API-Agnostic**: Internal structures support both Bedrock and CodeWhisperer  
✓ **Approval-First**: Tools require explicit approval unless pre-approved  
✓ **Minimal Changes**: Extended existing structures rather than replacing  

### 14.3 Critical Paths

1. ModelProvider extensions (ToolRequest, ModelRequest)
2. ToolProvider implementation
3. AgentLoop tool execution flow
4. UI approval flows
5. Tool implementations (fs_read, fs_write)

### 14.4 Risk Mitigation

- **API Complexity**: Detailed mappings provided for both providers
- **Approval UX**: Reuses existing patterns from main chat
- **Tool Migration**: Clear pattern established with fs_read/fs_write
- **Testing**: Comprehensive strategy at unit, integration, and E2E levels

---

**End of Design Document**

