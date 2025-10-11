# MVP Tools MCP - Research and Analysis

## Document Overview

This research document analyzes the Model Context Protocol (MCP) integration requirements for the agent_env architecture. MCP enables external tools and resources to be provided by separate server processes, allowing dynamic tool discovery and execution.

**Research Goals:**
- Understand the existing MCP client implementation
- Document MCP protocol communication patterns
- Analyze MCP server lifecycle management
- Identify integration points with agent_env
- Document critical challenges and solutions

**Scope:**
- Task 3.1: MCP Integration (Shared Instances) - IN SCOPE
- Task 3.2: Worker-Specific MCP (Future) - OUT OF SCOPE (documented for reference only)

---

## Table of Contents

1. [MCP Protocol Overview](#1-mcp-protocol-overview)
2. [Current MCP Implementation Analysis](#2-current-mcp-implementation-analysis)
3. [MCP Server Lifecycle Management](#3-mcp-server-lifecycle-management)
4. [Tool Discovery and Registration](#4-tool-discovery-and-registration)
5. [Tool Execution Flow](#5-tool-execution-flow)
6. [Integration with agent_env Architecture](#6-integration-with-agent_env-architecture)
7. [Critical Issues and Challenges](#7-critical-issues-and-challenges)
8. [Key Files Reference](#8-key-files-reference)
9. [Summary and Next Steps](#9-summary-and-next-steps)

---

## 1. MCP Protocol Overview

### What is MCP?

Model Context Protocol (MCP) is a protocol that enables external processes (MCP servers) to provide tools and resources to AI applications. MCP servers are separate processes that communicate with the main application via JSON-RPC over stdio or HTTP.

**Key Concepts:**
- **MCP Server**: External process that exposes tools, prompts, and resources
- **MCP Client**: Application component that communicates with MCP servers
- **Tools**: Functions that the AI can invoke (e.g., file operations, git commands)
- **Prompts**: Pre-defined prompt templates from servers
- **Resources**: Data sources that servers can provide

### Communication Patterns

**Transport Types:**
1. **Stdio Transport** (default):
   - MCP server runs as child process
   - Communication via stdin/stdout
   - JSON-RPC messages over stdio
   - Server stderr captured for logging

2. **HTTP Transport**:
   - MCP server runs as web service
   - Communication via HTTP requests
   - Supports OAuth authentication
   - Used for remote/cloud-based servers

### JSON-RPC Protocol

MCP uses JSON-RPC 2.0 for all communication:

**Client → Server Requests:**
- `initialize` - Handshake and capability negotiation
- `list_tools` - Discover available tools
- `call_tool` - Execute a tool
- `list_prompts` - Discover available prompts
- `get_prompt` - Retrieve a prompt template

**Server → Client Notifications:**
- `notifications/tools/list_changed` - Tool list updated
- `notifications/prompts/list_changed` - Prompt list updated
- `notifications/message/logged` - Server log message

**Pagination:**
- List operations support pagination via `cursor` parameter
- Client must fetch all pages to get complete list

### Tool Naming Convention

**Format:** `server_name::tool_name`

Example:
- Server: `filesystem`
- Tool: `read_file`
- Full name: `filesystem::read_file`

This prevents naming conflicts between different MCP servers.

### Configuration Format

MCP servers are configured in agent config:

```yaml
name: "developer"
mcpServers:
  filesystem:
    type: stdio
    command: "mcp-server-filesystem"
    args: ["/workspace"]
    env:
      HOME: "${env:HOME}"
  
  github:
    type: http
    url: "https://mcp.github.com"
    headers:
      Authorization: "Bearer ${env:GITHUB_TOKEN}"
```

**Configuration Fields:**
- `type`: Transport type (stdio or http)
- `command`: Executable path (stdio only)
- `args`: Command arguments (stdio only)
- `env`: Environment variables
- `url`: Server URL (http only)
- `headers`: HTTP headers (http only)
- `timeout`: Request timeout in milliseconds
- `disabled`: Flag to disable server

---

## 2. Current MCP Implementation Analysis

### Architecture Overview

**Key Components:**

1. **McpClientService** (`crates/chat-cli/src/mcp_client/client.rs`)
   - Implements rmcp `Service` trait
   - Handles server-initiated requests and notifications
   - Manages server lifecycle

2. **RunningService** (`crates/chat-cli/src/mcp_client/client.rs`)
   - Wrapper around rmcp service instances
   - Enables cloning via peer mechanism
   - Manages OAuth authentication (for HTTP transport)

3. **ToolManager** (`crates/chat-cli/src/cli/chat/tool_manager.rs`)
   - Orchestrates MCP server initialization
   - Manages tool discovery and registration
   - Handles server lifecycle events

4. **CustomTool** (`crates/chat-cli/src/cli/chat/tools/custom_tool.rs`)
   - Represents tools from MCP servers
   - Implements tool execution via MCP client
   - Integrates with old Tool enum

### Current Initialization Flow

**Location:** `ToolManager::build()` in `tool_manager.rs`

```
1. Load MCP server configs from Agent
2. Separate enabled vs disabled servers
3. Create McpClientService for each enabled server
4. Spawn orchestrator task (handles async events)
5. Spawn loading display task (shows progress)
6. Initialize each MCP client:
   - Start server process (stdio) or connect (http)
   - Perform handshake (initialize request)
   - Discover tools (list_tools with pagination)
   - Discover prompts (list_prompts with pagination)
7. Register tools in ToolManager
8. Return initialized ToolManager
```

### McpClientService Implementation

**Service Trait Methods:**

```rust
impl Service<RoleClient> for McpClientService {
    // Handle server-initiated requests
    async fn handle_request(&self, request, context) -> Result<Response> {
        match request {
            ServerRequest::PingRequest(_) => Ok(ClientResult::empty(())),
            // Other requests return method_not_found
        }
    }
    
    // Handle server-initiated notifications
    async fn handle_notification(&self, notification, context) -> Result<()> {
        match notification {
            ToolListChangedNotification => self.on_tool_list_changed(context).await,
            PromptListChangedNotification => self.on_prompt_list_changed(context).await,
            LoggingMessageNotification => self.on_logging_message(params, context).await,
            // Others ignored for now
        }
    }
    
    // Provide client info for handshake
    fn get_info(&self) -> InitializeRequestParam {
        InitializeRequestParam {
            protocol_version: Default::default(),
            capabilities: Default::default(),
            client_info: Implementation {
                name: "Q DEV CLI",
                version: "1.0.0",
            },
        }
    }
}
```

**Key Observations:**
- ✅ Implements full MCP protocol via rmcp crate
- ✅ Handles dynamic tool list updates
- ✅ Supports both stdio and HTTP transports
- ✅ OAuth support for HTTP servers
- ⚠️ Tightly coupled to old ToolManager architecture
- ⚠️ No cancellation support for server processes

### RunningService and Cloning

**Problem:** rmcp's `RunningService` is not directly cloneable

**Solution:** Wrapper enum with peer mechanism

```rust
pub enum InnerService {
    Original(rmcp::service::RunningService<...>),
    Peer(rmcp::service::Peer<RoleClient>),
}

impl Clone for InnerService {
    fn clone(&self) -> Self {
        match self {
            InnerService::Original(rs) => InnerService::Peer((*rs).clone()),
            InnerService::Peer(peer) => InnerService::Peer(peer.clone()),
        }
    }
}
```

**Benefits:**
- Allows sharing MCP clients across components
- First clone converts to peer, subsequent clones are cheap
- Maintains connection to same server process

### Authentication Support

**For HTTP Transport:**

```rust
pub struct RunningService {
    inner_service: InnerService,
    auth_client: Option<AuthClientWrapper>,  // OAuth client
}
```

**Token Refresh Pattern:**

```rust
// Decorator macro for automatic retry with token refresh
decorate_with_auth_retry!(CallToolRequestParam, call_tool, CallToolResult);

// Implementation:
// 1. Try operation
// 2. If fails and auth_client exists, refresh token
// 3. Retry operation
// 4. Return result or original error
```

**Observations:**
- ✅ Automatic token refresh on auth errors
- ✅ Transparent to callers
- ⚠️ Cannot re-authenticate entirely (would need transport swap)

### Messenger Pattern

**Purpose:** Bridge between MCP client and ToolManager

**Location:** `crates/chat-cli/src/cli/chat/server_messenger.rs`

```rust
pub struct ServerMessenger {
    sender: tokio::sync::mpsc::Sender<UpdateEventMessage>,
}

pub enum UpdateEventMessage {
    ToolsListResult { server_name, result },
    PromptsListResult { server_name, result },
    // Other events
}
```

**Flow:**
```
MCP Client → ServerMessenger → Orchestrator Task → ToolManager
```

**Benefits:**
- Decouples MCP client from ToolManager
- Async event handling
- Multiple servers can send events concurrently

**Observations:**
- ✅ Clean separation of concerns
- ✅ Supports concurrent server initialization
- ⚠️ Specific to old architecture
- ⚠️ Would need adaptation for agent_env

### Orchestrator Task

**Purpose:** Central event handler for all MCP servers

**Responsibilities:**
1. Receive tool/prompt list results from servers
2. Validate tool names (regex, length, characters)
3. Sanitize tool names if needed
4. Register tools in ToolManager
5. Handle server errors and warnings
6. Update loading display
7. Send telemetry

**Key Logic:**

```rust
// Spawned in ToolManager::build()
spawn_orchestrator_task(
    has_new_stuff,           // Flag for new tools
    loading_servers,         // Track initialization time
    msg_rx,                  // Receive from ServerMessenger
    prompt_list_receiver,    // Prompt queries
    prompt_list_sender,      // Prompt results
    pending,                 // Servers still initializing
    agent,                   // Agent config
    database,                // For telemetry
    regex,                   // Tool name validation
    notify_weak,             // Notify when done
    load_record,             // Loading status record
    telemetry,               // Telemetry client
    loading_status_sender,   // Update loading display
    new_tool_specs,          // Store discovered tools
    total,                   // Total server count
    conv_id,                 // Conversation ID
);
```

**Observations:**
- ✅ Centralized event handling
- ✅ Concurrent server initialization
- ✅ Comprehensive error handling
- ❌ Complex with many parameters
- ❌ Tightly coupled to old architecture

### Loading Display

**Purpose:** Show user which servers are initializing

**Pattern:**
```
⠋ Loading tools from filesystem...
⠙ Loading tools from git...
✓ filesystem (1.2s)
✗ git (failed: connection refused)
```

**Implementation:**
- Separate task for display updates
- Spinner animation while loading
- Success/error indicators
- Timing information
- Only in interactive mode

**Observations:**
- ✅ Good user experience
- ✅ Non-blocking
- ⚠️ Terminal-specific (crossterm)
- ⚠️ Would need adaptation for WebUI

---

## 3. MCP Server Lifecycle Management

### Initialization Sequence

**Stdio Transport:**

```
1. Expand command path (shellexpand with env vars)
2. Process environment variables (substitute ${env:VAR})
3. Create tokio Command with:
   - Command and args
   - Environment variables
   - Process group (Unix only)
   - Stderr piped for logging
4. Spawn child process via TokioChildProcess
5. Create rmcp service over stdio transport
6. Perform handshake (initialize request/response)
7. Negotiate capabilities
8. Return RunningService
```

**HTTP Transport:**

```
1. Parse and validate URL
2. Process headers (substitute env vars)
3. Check if OAuth is needed
4. If OAuth:
   - Create AuthClientWrapper
   - Perform OAuth flow
   - Get access token
5. Create HTTP service with auth
6. Perform handshake
7. Return RunningService with auth_client
```

### Handshake Protocol

**Client sends Initialize Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "Q DEV CLI",
      "version": "1.0.0"
    }
  }
}
```

**Server responds with Initialize Result:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": {
        "listChanged": true
      },
      "prompts": {
        "listChanged": true
      }
    },
    "serverInfo": {
      "name": "filesystem-server",
      "version": "1.0.0"
    }
  }
}
```

**Capability Negotiation:**
- Client declares what it supports
- Server declares what it provides
- Both sides respect negotiated capabilities
- `listChanged` indicates server will send notifications

### Process Management (Stdio)

**Child Process Handling:**

```rust
// Spawn with process group (Unix)
#[cfg(not(windows))]
cmd.process_group(0);

// Capture stderr for logging
let (tokio_child_process, child_stderr) = 
    TokioChildProcess::builder(command)
        .stderr(Stdio::piped())
        .spawn()?;

// Spawn task to read stderr
if let Some(mut stderr) = child_stderr {
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match stderr.read(&mut buf).await {
                Ok(0) => break,  // EOF
                Ok(size) => {
                    tracing::info!(target: "mcp", 
                        "{} logged: {}", 
                        server_name, 
                        String::from_utf8_lossy(&buf[0..size])
                    );
                }
                Err(e) => break,  // Error
            }
        }
    });
}
```

**Observations:**
- ✅ Process group for clean termination
- ✅ Stderr captured for debugging
- ✅ Non-blocking stderr reading
- ⚠️ No explicit process cleanup on shutdown
- ⚠️ No health monitoring
- ⚠️ No automatic restart on crash

### Server State Tracking

**InitializedMcpClient Enum:**

```rust
pub enum InitializedMcpClient {
    Pending(JoinHandle<Result<RunningService, McpClientError>>),
    Ready(RunningService),
}
```

**Purpose:**
- Avoid blocking main thread during initialization
- Lazy initialization pattern
- Convert from Pending to Ready when needed

**Usage:**

```rust
// In ToolManager
pub clients: HashMap<String, InitializedMcpClient>

// When tool is invoked:
let client = match &mut self.clients.get_mut(server_name) {
    Some(InitializedMcpClient::Pending(handle)) => {
        // Wait for initialization to complete
        let service = handle.await??;
        *client = InitializedMcpClient::Ready(service.clone());
        service
    }
    Some(InitializedMcpClient::Ready(service)) => service.clone(),
    None => return Err("Server not found"),
};
```

**Observations:**
- ✅ Non-blocking initialization
- ✅ Lazy evaluation
- ✅ Reduces startup latency
- ⚠️ First tool use may have delay
- ⚠️ No timeout on initialization

### Dynamic Updates

**Tool List Changed Notification:**

```rust
async fn on_tool_list_changed(&self, context: NotificationContext<RoleClient>) {
    let peer = context.peer;
    
    // Re-fetch all tools with pagination
    paginated_fetch! {
        final_result_type: ListToolsResult,
        content_type: rmcp::model::Tool,
        service_method: list_tools,
        result_field: tools,
        messenger_method: send_tools_list_result,
        service: peer,
        messenger: self.messenger,
        server_name: self.server_name
    };
}
```

**Flow:**
```
1. Server sends ToolListChangedNotification
2. Client receives notification
3. Client re-fetches complete tool list
4. Client sends updated list to ToolManager via messenger
5. ToolManager updates tool registry
6. New tools become available
```

**Observations:**
- ✅ Supports dynamic tool updates
- ✅ Automatic re-discovery
- ⚠️ Re-fetches entire list (not delta)
- ⚠️ No notification to user about changes

### Error Handling

**Initialization Errors:**

```rust
match mcp_client.init(os).await {
    Ok(running_service) => {
        // Handle name collisions by appending '1'
        while let Some(collided) = clients.insert(name.clone(), running_service) {
            name.push('1');
            running_service = collided;
        }
    }
    Err(e) => {
        error!("Error initializing mcp client for {}: {:?}", name, e);
        
        // Send telemetry
        os.telemetry.send_mcp_server_init(...).await.ok();
        
        // Send error to messenger
        let temp_messenger = messenger_builder.build_with_name(name);
        temp_messenger
            .send_tools_list_result(Err(ServiceError::UnexpectedResponse), None)
            .await;
    }
}
```

**Runtime Errors:**
- Server process crashes → No automatic restart
- Tool execution fails → Error returned to LLM
- Network errors (HTTP) → Retry with token refresh
- Timeout → Configurable per server

**Observations:**
- ✅ Graceful degradation (failed servers don't block others)
- ✅ Name collision handling
- ✅ Telemetry for debugging
- ❌ No automatic recovery
- ❌ No health checks
- ❌ No circuit breaker pattern

### Shutdown

**Current Implementation:**
- No explicit shutdown in ToolManager
- Child processes may be orphaned
- Relies on OS cleanup when parent exits

**Issues:**
- ⚠️ Processes may not terminate cleanly
- ⚠️ No graceful shutdown protocol
- ⚠️ Resources may leak

**Needed for agent_env:**
- Explicit shutdown method
- Send shutdown notification to servers
- Wait for processes to exit
- Kill if timeout exceeded
- Clean up resources

---

## 4. Tool Discovery and Registration

### Tool Discovery Process

**After Handshake:**

```rust
// Check if server supports tools
if init_result.capabilities.tools.is_some() {
    // Fetch all tools with pagination
    paginated_fetch! {
        final_result_type: ListToolsResult,
        content_type: rmcp::model::Tool,
        service_method: list_tools,
        result_field: tools,
        messenger_method: send_tools_list_result,
        service: service_clone,
        messenger: messenger_clone,
        server_name: server_name
    };
}
```

**Pagination Macro:**

```rust
let mut cursor = None::<String>;
let mut content = Vec::<rmcp::model::Tool>::new();

loop {
    let param = Some(PaginatedRequestParam { cursor: cursor.clone() });
    match service.list_tools(param).await {
        Ok(mut result) => {
            if let Some(s) = result.next_cursor {
                cursor.replace(s);
            }
            content.append(&mut result.tools);
        }
        Err(e) => {
            // Handle error
            break;
        }
    }
    if cursor.is_none() {
        break;  // No more pages
    }
}

// Send complete list to messenger
messenger.send_tools_list_result(Ok(ListToolsResult { tools: content }), Some(service)).await;
```

**Observations:**
- ✅ Handles pagination automatically
- ✅ Fetches complete tool list
- ✅ Async and non-blocking
- ⚠️ No timeout per page
- ⚠️ Entire list fails if one page fails

### Tool Schema Format

**MCP Tool Structure:**

```rust
// From rmcp crate
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,  // JSON Schema
}
```

**Example Tool Schema:**

```json
{
  "name": "read_file",
  "description": "Read contents of a file",
  "inputSchema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to file"
      }
    },
    "required": ["path"]
  }
}
```

**Observations:**
- ✅ Standard JSON Schema format
- ✅ Compatible with LLM tool specifications
- ✅ Flexible parameter definitions
- ⚠️ No validation of schema correctness

### Tool Name Validation and Sanitization

**Validation Rules:**

```rust
const VALID_TOOL_NAME: &str = "^[a-zA-Z][a-zA-Z0-9_]*$";

// Tool name must:
// - Start with letter
// - Contain only letters, numbers, underscores
// - Not be empty
// - Not exceed max length
```

**Sanitization Process:**

```rust
// In orchestrator task
for tool in tools {
    let original_name = tool.name.clone();
    
    // Validate against regex
    if !regex.is_match(&original_name) {
        // Sanitize: replace invalid chars with underscore
        let sanitized = original_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect::<String>();
        
        tool.name = sanitized;
        
        // Track mapping: sanitized -> original
        name_mapping.insert(sanitized, original_name);
    }
    
    // Check description
    if tool.description.is_none() || tool.description.as_ref().unwrap().is_empty() {
        warn!("Tool {} has empty description", tool.name);
    }
}
```

**Name Collision Handling:**

```rust
// If tool name already exists from another server
let full_name = format!("{}::{}", server_name, tool_name);

// Store with server prefix
tool_spec.name = full_name;
```

**Observations:**
- ✅ Prevents invalid tool names
- ✅ Maintains mapping to original names
- ✅ Handles collisions with server prefix
- ⚠️ Sanitization may create new collisions
- ⚠️ LLM sees sanitized names, not originals

### Tool Registration

**ToolSpec Structure:**

```rust
pub struct ToolSpec {
    pub name: String,              // Full name: "server::tool"
    pub description: String,
    pub input_schema: InputSchema,
    pub tool_origin: ToolOrigin,
}

pub enum ToolOrigin {
    Native,
    McpServer(String),  // Server name
}
```

**Registration Flow:**

```
1. Orchestrator receives tools from server
2. Validate and sanitize tool names
3. Create ToolSpec for each tool
4. Store in new_tool_specs map:
   - Key: server_name
   - Value: (name_mapping, vec<ToolSpec>)
5. Set has_new_stuff flag
6. Main loop detects flag
7. Retrieves new specs from map
8. Adds to ToolManager.schema
9. Adds to ToolManager.tn_map
10. Tools now available for LLM
```

**Data Structures:**

```rust
// In ToolManager
pub tn_map: HashMap<ModelToolName, ToolInfo>
pub schema: HashMap<ModelToolName, ToolSpec>

pub struct ToolInfo {
    pub server_name: String,
    pub host_tool_name: HostToolName,  // Original name
}
```

**Observations:**
- ✅ Tracks both sanitized and original names
- ✅ Associates tools with servers
- ✅ Supports dynamic registration
- ⚠️ Global namespace (all tools visible to all workers)
- ⚠️ No tool versioning

### Tool Filtering

**Agent Config:**

```yaml
tools:
  - "*"                    # All tools
  - "fs_read"             # Specific native tool
  - "@filesystem"         # All tools from server
  - "@filesystem/read"    # Specific tool from server
```

**Filtering Logic:**

```rust
// Check if tool is allowed
fn is_tool_allowed(tool_name: &str, agent: &Agent) -> bool {
    // Check wildcard
    if agent.tools.contains("*") {
        return true;
    }
    
    // Check exact match
    if agent.tools.contains(tool_name) {
        return true;
    }
    
    // Check server wildcard
    if let Some(server_name) = extract_server_name(tool_name) {
        let server_wildcard = format!("@{}", server_name);
        if agent.tools.contains(&server_wildcard) {
            return true;
        }
    }
    
    false
}
```

**Observations:**
- ✅ Flexible filtering options
- ✅ Per-agent tool visibility
- ✅ Server-level and tool-level control
- ⚠️ Filtering happens at request time (not registration)
- ⚠️ No dynamic filter updates

### Tool Aliases

**Agent Config:**

```yaml
toolAliases:
  "filesystem::read_file": "read"
  "git::commit": "commit_changes"
```

**Purpose:**
- Simplify tool names for LLM
- Avoid verbose server prefixes
- Create semantic aliases

**Implementation:**

```rust
// When building tool list for LLM
for (original_name, alias) in &agent.tool_aliases {
    if let Some(spec) = tool_specs.get(original_name) {
        let mut aliased_spec = spec.clone();
        aliased_spec.name = alias.clone();
        tool_list.push(aliased_spec);
    }
}
```

**Observations:**
- ✅ Improves LLM usability
- ✅ Per-agent customization
- ⚠️ Must track reverse mapping for execution
- ⚠️ Aliases can create collisions

---

## 5. Tool Execution Flow

### CustomTool Structure

**Location:** `crates/chat-cli/src/cli/chat/tools/custom_tool.rs`

```rust
pub struct CustomTool {
    pub name: String,              // Original tool name (not prefixed)
    pub server_name: String,       // MCP server name
    pub client: RunningService,    // MCP client instance
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}
```

**Namespaced Name:**

```rust
impl CustomTool {
    pub fn namespaced_tool_name(&self) -> String {
        format!("@{}{}{}", self.server_name, MCP_SERVER_TOOL_DELIMITER, self.name)
    }
}
```

**Observations:**
- ✅ Stores reference to MCP client
- ✅ Maintains server association
- ✅ Parameters pre-parsed from JSON
- ⚠️ Client is cloned (uses peer mechanism)

### Tool Invocation

**Implementation:**

```rust
impl CustomTool {
    pub async fn invoke(&self, _os: &Os, _updates: &mut impl Write) -> Result<InvokeOutput> {
        // Build call_tool request
        let params = CallToolRequestParam {
            name: Cow::from(self.name.clone()),
            arguments: self.params.clone(),
        };
        
        // Call MCP server
        let resp = self.client.call_tool(params.clone()).await?;
        
        // Check if error
        if resp.is_error.is_none_or(|v| !v) {
            Ok(InvokeOutput {
                output: OutputKind::Json(serde_json::json!(resp)),
            })
        } else {
            warn!("Tool call for {} failed", self.name);
            Ok(InvokeOutput {
                output: OutputKind::Json(serde_json::json!(resp)),
            })
        }
    }
}
```

**CallToolResult Structure:**

```rust
// From rmcp crate
pub struct CallToolResult {
    pub content: Vec<ToolResultContent>,
    pub is_error: Option<bool>,
}

pub enum ToolResultContent {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { resource: ResourceContents },
}
```

**Observations:**
- ✅ Simple forwarding to MCP server
- ✅ Supports multiple content types
- ✅ Error flag in response
- ⚠️ No timeout handling
- ⚠️ No cancellation support
- ⚠️ Returns error as success (doesn't propagate)

### Request/Response Format

**Request to MCP Server:**

```json
{
  "jsonrpc": "2.0",
  "id": 123,
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": {
      "path": "/workspace/file.txt"
    }
  }
}
```

**Response from MCP Server:**

```json
{
  "jsonrpc": "2.0",
  "id": 123,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "File contents here..."
      }
    ],
    "isError": false
  }
}
```

**Error Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 123,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Error: File not found"
      }
    ],
    "isError": true
  }
}
```

**Observations:**
- ✅ Standard JSON-RPC format
- ✅ Structured error handling
- ✅ Multiple content blocks supported
- ⚠️ Error is in result, not JSON-RPC error

### Permission Evaluation

**Implementation:**

```rust
impl CustomTool {
    pub fn eval_perm(&self, os: &Os, agent: &Agent) -> PermissionEvalResult {
        // Check if tool is in allowed_tools
        let namespaced_name = self.namespaced_tool_name();
        
        if agent.allowed_tools.contains(&namespaced_name) {
            return PermissionEvalResult::Allow;
        }
        
        // Check if server is allowed
        let server_wildcard = format!("@{}", self.server_name);
        if agent.allowed_tools.contains(&server_wildcard) {
            return PermissionEvalResult::Allow;
        }
        
        // Otherwise ask for approval
        PermissionEvalResult::Ask
    }
}
```

**Observations:**
- ✅ Respects agent allowed_tools
- ✅ Server-level and tool-level permissions
- ✅ Defaults to Ask (safe)
- ⚠️ No per-tool settings (unlike native tools)
- ⚠️ No path-based or parameter-based filtering

### Tool Creation Flow

**When LLM Requests Tool:**

```
1. LLM returns ToolUseEvent with tool name and parameters
2. Parser extracts tool use from stream
3. Look up tool in ToolManager:
   - Check tn_map for name mapping
   - Get original tool name and server name
4. Get MCP client for server:
   - Look up in clients HashMap
   - Convert from Pending to Ready if needed
5. Create CustomTool instance:
   - Set name (original, not prefixed)
   - Set server_name
   - Set client (cloned RunningService)
   - Set params (parsed JSON)
6. Wrap in Tool::Custom enum variant
7. Queue for approval
8. After approval, invoke tool
9. Format result and send to LLM
```

**Code Path:**

```rust
// In old chat flow
let tool_info = tool_manager.tn_map.get(tool_name)?;
let client = tool_manager.get_client(&tool_info.server_name).await?;

let custom_tool = CustomTool {
    name: tool_info.host_tool_name.clone(),
    server_name: tool_info.server_name.clone(),
    client: client.clone(),
    params: Some(parameters),
};

let tool = Tool::Custom(custom_tool);
```

**Observations:**
- ✅ Lazy client initialization
- ✅ Name mapping handled transparently
- ✅ Client sharing via cloning
- ⚠️ Synchronous approval blocks execution
- ⚠️ No batching of tool calls

### Timeout Handling

**Configuration:**

```yaml
mcpServers:
  filesystem:
    command: "mcp-server-filesystem"
    timeout: 120000  # milliseconds
```

**Implementation:**

```rust
// In CustomToolConfig
pub timeout: u64,  // Default: 120000 (2 minutes)

// Applied at HTTP transport level
// For stdio, timeout is at rmcp layer
```

**Observations:**
- ✅ Configurable per server
- ✅ Prevents indefinite hangs
- ⚠️ No per-tool timeout override
- ⚠️ Timeout behavior not well-documented

### Error Propagation

**Current Behavior:**

```rust
// Tool execution error is returned as success
if resp.is_error.is_none_or(|v| !v) {
    Ok(InvokeOutput { ... })
} else {
    warn!("Tool call for {} failed", self.name);
    Ok(InvokeOutput { ... })  // Still Ok!
}
```

**Impact:**
- Error is sent to LLM as tool result
- LLM can see error and retry or handle
- No exception thrown to caller
- Telemetry may not capture failures

**Observations:**
- ✅ LLM-friendly error handling
- ✅ Allows LLM to recover
- ⚠️ Hides errors from application
- ⚠️ May mask serious issues

### Result Formatting

**MCP Result → LLM Tool Result:**

```rust
// MCP CallToolResult
{
  "content": [
    { "type": "text", "text": "..." },
    { "type": "image", "data": "base64...", "mimeType": "image/png" }
  ],
  "isError": false
}

// Converted to InvokeOutput
InvokeOutput {
    output: OutputKind::Json(serde_json::json!({
        "content": [...],
        "isError": false
    }))
}

// Then to ToolResult for LLM
ToolResult {
    tool_use_id: "toolu_123",
    content: vec![
        ToolResultContentBlock::Json(...)
    ],
    status: ToolResultStatus::Success
}
```

**Observations:**
- ✅ Preserves MCP result structure
- ✅ Supports multiple content types
- ⚠️ Always marked as Success (even if isError=true)
- ⚠️ LLM must parse JSON to see error flag

---

## 6. Integration with agent_env Architecture

### Dependency on Tool System (Task 1.3)

**Critical Dependency:**

MCP integration MUST wait for Task 1.3 (Tool System Integration) to be completed first.

**Required from Task 1.3:**
- `Tool` trait definition
- `ToolRegistry` for tool storage
- `ToolContext` for execution context
- `ToolResult` standardized format
- `ToolDefinition` for LLM specifications
- Tool execution flow in AgentLoop
- Permission/approval mechanism

**Why This Matters:**
- MCP tools will implement the same `Tool` trait as native tools
- MCP tools will be registered in the same `ToolRegistry`
- MCP tool execution follows the same flow
- No special handling needed in AgentLoop

### Proposed Architecture

#### McpServerManager

**Purpose:** Manage all MCP server instances for a Session

**Location:** `crates/chat-cli/src/agent_env/mcp/server_manager.rs`

```rust
pub struct McpServerManager {
    servers: HashMap<String, Arc<McpServer>>,
    config: Vec<CustomToolConfig>,
}

impl McpServerManager {
    pub async fn new(configs: Vec<CustomToolConfig>) -> Result<Self>;
    
    pub async fn start_all_servers(&mut self) -> Result<()>;
    
    pub async fn discover_tools(&self) -> Vec<Arc<dyn Tool>>;
    
    pub async fn shutdown(&mut self) -> Result<()>;
    
    fn get_server(&self, name: &str) -> Option<Arc<McpServer>>;
}
```

**Responsibilities:**
- Initialize all MCP servers from config
- Manage server lifecycle
- Coordinate tool discovery
- Handle server failures
- Clean shutdown

#### McpServer

**Purpose:** Represent a single MCP server instance

**Location:** `crates/chat-cli/src/agent_env/mcp/server.rs`

```rust
pub struct McpServer {
    name: String,
    config: CustomToolConfig,
    client: Arc<Mutex<Option<RunningService>>>,
    tools: Arc<RwLock<Vec<McpToolInfo>>>,
}

impl McpServer {
    pub async fn start(name: String, config: CustomToolConfig) -> Result<Self>;
    
    pub async fn discover_tools(&self) -> Result<Vec<McpToolInfo>>;
    
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: serde_json::Value,
    ) -> Result<CallToolResult>;
    
    pub async fn shutdown(&mut self) -> Result<()>;
    
    fn is_ready(&self) -> bool;
}

pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

**Responsibilities:**
- Manage single server process
- Handle tool discovery
- Forward tool execution requests
- Track server state
- Handle reconnection

#### McpToolWrapper

**Purpose:** Implement Tool trait for MCP tools

**Location:** `crates/chat-cli/src/agent_env/mcp/tool_wrapper.rs`

```rust
pub struct McpToolWrapper {
    server_name: String,
    tool_name: String,
    full_name: String,  // "server::tool"
    description: String,
    input_schema: serde_json::Value,
    server: Arc<McpServer>,
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn input_schema(&self) -> serde_json::Value {
        self.input_schema.clone()
    }
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        // Check cancellation
        if context.cancellation_token.is_cancelled() {
            return Err(eyre!("Tool execution cancelled"));
        }
        
        // Forward to MCP server
        let result = self.server
            .execute_tool(&self.tool_name, parameters)
            .await?;
        
        // Convert CallToolResult to ToolResult
        convert_mcp_result(result)
    }
    
    fn requires_approval(&self, context: &ToolContext) -> PermissionEvalResult {
        // Check agent allowed_tools
        let agent = &context.agent;
        
        if agent.allowed_tools.contains(&self.full_name) {
            return PermissionEvalResult::Allow;
        }
        
        let server_wildcard = format!("@{}", self.server_name);
        if agent.allowed_tools.contains(&server_wildcard) {
            return PermissionEvalResult::Allow;
        }
        
        PermissionEvalResult::Ask
    }
}
```

**Observations:**
- ✅ Implements standard Tool trait
- ✅ No special handling needed in AgentLoop
- ✅ Supports cancellation
- ✅ Permission evaluation integrated
- ✅ Server reference via Arc (shared)

### Integration with Session

**Session Initialization:**

```rust
impl Session {
    pub async fn new(
        event_bus: EventBus,
        model_providers: Vec<Arc<dyn ModelProvider>>,
        agent: Option<Agent>,
    ) -> Result<Self> {
        // Create tool registry
        let tool_registry = Arc::new(ToolRegistry::new());
        
        // Register native tools
        tool_registry.register(Arc::new(FsReadTool::new()));
        tool_registry.register(Arc::new(FsWriteTool::new()));
        // ... other native tools
        
        // Initialize MCP servers if agent has them
        let mcp_manager = if let Some(agent) = &agent {
            if !agent.mcp_servers.mcp_servers.is_empty() {
                let configs: Vec<_> = agent.mcp_servers.mcp_servers
                    .values()
                    .cloned()
                    .collect();
                
                let mut manager = McpServerManager::new(configs).await?;
                manager.start_all_servers().await?;
                
                // Discover and register MCP tools
                let mcp_tools = manager.discover_tools().await;
                for tool in mcp_tools {
                    tool_registry.register(tool);
                }
                
                Some(Arc::new(manager))
            } else {
                None
            }
        } else {
            None
        };
        
        Ok(Session {
            event_bus,
            model_providers,
            tool_registry,
            mcp_manager,
            workers: Default::default(),
        })
    }
}
```

**Session Shutdown:**

```rust
impl Session {
    pub async fn shutdown(&self) -> Result<()> {
        // Shutdown all MCP servers
        if let Some(mcp_manager) = &self.mcp_manager {
            mcp_manager.shutdown().await?;
        }
        
        Ok(())
    }
}
```

**Observations:**
- ✅ MCP initialization during Session creation
- ✅ Tools registered alongside native tools
- ✅ Clean shutdown path
- ✅ Optional MCP support (graceful degradation)

### Integration with AgentLoop

**No Changes Needed:**

AgentLoop doesn't need to know about MCP. It just uses tools from ToolRegistry.

```rust
impl AgentLoop {
    async fn execute_tools(
        &self,
        requests: &[ToolRequest],
    ) -> Result<Vec<ToolResult>> {
        let tool_registry = &self.session.tool_registry;
        
        let mut results = Vec::new();
        for request in requests {
            // Get tool (could be native or MCP)
            let tool = tool_registry.get(&request.tool_name)?;
            
            // Check permissions
            let permission = tool.requires_approval(&self.tool_context);
            if !self.check_permission(permission).await? {
                continue;  // Denied
            }
            
            // Execute tool (native or MCP, same interface)
            let result = tool.execute(
                request.parameters.clone(),
                &self.tool_context,
            ).await?;
            
            results.push(result);
        }
        
        Ok(results)
    }
}
```

**Observations:**
- ✅ Transparent MCP integration
- ✅ Same code path for all tools
- ✅ No special cases
- ✅ Clean abstraction

### Event System Integration

**New Events (Optional):**

```rust
pub enum McpEvent {
    ServerStarted {
        server_name: String,
    },
    ServerFailed {
        server_name: String,
        error: String,
    },
    ToolsDiscovered {
        server_name: String,
        tool_count: usize,
    },
    ServerShutdown {
        server_name: String,
    },
}
```

**Publishing:**

```rust
// In McpServerManager
pub async fn start_all_servers(&mut self) -> Result<()> {
    for (name, config) in &self.config {
        match McpServer::start(name.clone(), config.clone()).await {
            Ok(server) => {
                self.event_bus.publish(McpEvent::ServerStarted {
                    server_name: name.clone(),
                });
                self.servers.insert(name.clone(), Arc::new(server));
            }
            Err(e) => {
                self.event_bus.publish(McpEvent::ServerFailed {
                    server_name: name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }
    Ok(())
}
```

**UI Handling:**

```rust
// In TextUi
async fn handle_event(&mut self, event: Event) {
    match event {
        Event::Mcp(McpEvent::ServerStarted { server_name }) => {
            println!("✓ MCP server '{}' started", server_name);
        }
        Event::Mcp(McpEvent::ServerFailed { server_name, error }) => {
            eprintln!("✗ MCP server '{}' failed: {}", server_name, error);
        }
        // ...
    }
}
```

**Observations:**
- ✅ Optional (not required for MVP)
- ✅ Improves user visibility
- ✅ Useful for debugging
- ⚠️ Adds complexity

### Configuration Loading

**From Agent Config:**

```rust
// In ChatArgs::execute()
let agent = if let Some(agent_name) = &self.agent {
    Some(Agent::load(os, agent_name).await?)
} else {
    None
};

// Agent has mcp_servers field
let mcp_configs = agent.as_ref()
    .map(|a| a.mcp_servers.mcp_servers.values().cloned().collect())
    .unwrap_or_default();

// Pass to Session
let session = Session::new(
    event_bus,
    model_providers,
    agent,
).await?;
```

**Observations:**
- ✅ Reuses existing Agent config loading
- ✅ MCP servers configured per agent
- ✅ Backward compatible (optional field)

### Error Handling Strategy

**Server Initialization Failures:**
- Log error
- Publish McpEvent::ServerFailed
- Continue with other servers
- Don't block Session creation

**Tool Discovery Failures:**
- Log warning
- Server marked as failed
- No tools registered from that server
- Other servers unaffected

**Tool Execution Failures:**
- Return error as ToolResult
- LLM sees error and can retry
- Don't crash AgentLoop
- Log for debugging

**Server Crashes:**
- Detect on next tool use
- Return error to LLM
- Optionally attempt reconnection
- Log incident

**Observations:**
- ✅ Graceful degradation
- ✅ Partial failures don't block system
- ✅ LLM-friendly error handling
- ⚠️ No automatic recovery (MVP)

---

## 7. Critical Issues and Challenges

### Challenge 1: Reusing Existing MCP Code

**Issue:** Current MCP implementation is tightly coupled to old ToolManager

**Components to Reuse:**
- ✅ `McpClientService` - Core protocol implementation
- ✅ `RunningService` - Client wrapper with cloning
- ✅ `CustomToolConfig` - Configuration structure
- ✅ Authentication logic - OAuth for HTTP transport
- ✅ Process management - Stdio transport handling

**Components to Adapt:**
- ⚠️ Messenger pattern - Need event-based alternative
- ⚠️ Orchestrator task - Simplify for agent_env
- ⚠️ Loading display - Adapt for EventBus
- ⚠️ Tool registration - Use ToolRegistry instead

**Strategy:**
1. Keep low-level MCP client code as-is
2. Create new wrapper layer for agent_env
3. Replace messenger with EventBus
4. Simplify orchestration logic
5. Adapt to Tool trait

**Estimated Effort:** Medium (can reuse 60-70% of code)

### Challenge 2: Async Initialization

**Issue:** MCP servers take time to initialize (process spawn, handshake, tool discovery)

**Current Approach:**
- Spawn initialization in background task
- Return JoinHandle wrapped in InitializedMcpClient::Pending
- Convert to Ready on first use

**For agent_env:**

**Option A: Block Session Creation**
- Wait for all servers to initialize
- Pros: Simple, all tools available immediately
- Cons: Slow startup, bad UX

**Option B: Lazy Initialization (Current)**
- Start servers in background
- First tool use waits for initialization
- Pros: Fast startup
- Cons: First tool use has latency

**Option C: Progressive Registration**
- Start Session immediately
- Register tools as servers initialize
- Publish events for new tools
- Pros: Best UX, fast startup
- Cons: More complex, tools appear dynamically

**Recommendation:** Option C for best UX

**Implementation:**
```rust
// In McpServerManager
pub async fn start_all_servers_async(&self) {
    for (name, config) in &self.configs {
        let event_bus = self.event_bus.clone();
        let tool_registry = self.tool_registry.clone();
        
        tokio::spawn(async move {
            match McpServer::start(name, config).await {
                Ok(server) => {
                    let tools = server.discover_tools().await?;
                    for tool in tools {
                        tool_registry.register(tool);
                    }
                    event_bus.publish(McpEvent::ServerReady { name });
                }
                Err(e) => {
                    event_bus.publish(McpEvent::ServerFailed { name, error: e });
                }
            }
        });
    }
}
```

### Challenge 3: Process Lifecycle Management

**Issue:** Child processes need proper cleanup

**Problems:**
- Orphaned processes if parent crashes
- No graceful shutdown protocol
- Stderr reading task may leak
- Process group cleanup on Unix

**Solution:**

```rust
impl McpServer {
    pub async fn shutdown(&mut self) -> Result<()> {
        // 1. Send shutdown notification (if server supports it)
        // Note: MCP protocol doesn't define shutdown, but good practice
        
        // 2. Close stdin (signals EOF to server)
        drop(self.client);
        
        // 3. Wait for process to exit (with timeout)
        tokio::time::timeout(
            Duration::from_secs(5),
            self.process.wait()
        ).await??;
        
        // 4. Force kill if still running
        if let Some(mut process) = self.process.take() {
            process.kill().await?;
        }
        
        Ok(())
    }
}
```

**For HTTP Transport:**
- No process to manage
- Just drop client connection
- OAuth token cleanup

**Observations:**
- ⚠️ MCP protocol has no standard shutdown
- ⚠️ Must handle unresponsive servers
- ⚠️ Process group cleanup on Unix

### Challenge 4: Tool Name Conflicts

**Issue:** Multiple servers may provide tools with same name

**Current Solution:** Prefix with server name (`server::tool`)

**Problems:**
- Verbose names for LLM
- User must remember server names
- Aliases help but add complexity

**Alternative Approaches:**

**Option A: Namespace by Server (Current)**
```
filesystem::read_file
git::commit
```

**Option B: Flat Namespace with Conflicts**
```
read_file  (from filesystem)
read_file_1  (from another server, renamed)
```

**Option C: Explicit Disambiguation**
```
read_file  (if unique)
filesystem::read_file  (if conflict)
```

**Recommendation:** Stick with Option A (current approach)

**Rationale:**
- Predictable naming
- No ambiguity
- Aliases can simplify for common tools

### Challenge 5: Dynamic Tool Updates

**Issue:** MCP servers can notify about tool list changes

**Current Handling:**
- Receive ToolListChangedNotification
- Re-fetch entire tool list
- Update tool registry

**For agent_env:**

**Challenges:**
- Tools may be added/removed mid-conversation
- LLM may have cached old tool list
- Need to notify LLM of changes

**Solution:**
```rust
async fn on_tool_list_changed(&self, server_name: &str) {
    // Re-discover tools
    let new_tools = self.discover_tools(server_name).await?;
    
    // Update registry
    self.tool_registry.update_server_tools(server_name, new_tools);
    
    // Publish event
    self.event_bus.publish(McpEvent::ToolsUpdated {
        server_name: server_name.to_string(),
    });
    
    // Note: LLM will see new tools in next request
    // (tools are sent with each request)
}
```

**Observations:**
- ✅ Automatic updates
- ⚠️ LLM doesn't see changes until next request
- ⚠️ May confuse LLM if tools disappear

### Challenge 6: Error Handling and Recovery

**Issue:** MCP servers can fail in various ways

**Failure Scenarios:**
1. **Initialization Failure:**
   - Process won't start
   - Handshake fails
   - Tool discovery fails

2. **Runtime Failure:**
   - Process crashes
   - Tool execution fails
   - Timeout
   - Network error (HTTP)

3. **Partial Failure:**
   - Some tools work, others don't
   - Intermittent failures

**Recovery Strategies:**

**For Initialization:**
- Log error and continue
- Don't block other servers
- Publish failure event
- Allow manual retry

**For Runtime:**
- Return error to LLM
- LLM can retry or use different tool
- Log for debugging
- Optionally attempt reconnection

**For Process Crashes:**
```rust
impl McpServer {
    async fn execute_tool(&self, name: &str, params: Value) -> Result<ToolResult> {
        let client = self.client.lock().await;
        
        match client.call_tool(...).await {
            Ok(result) => Ok(convert_result(result)),
            Err(e) if is_connection_error(&e) => {
                // Process may have crashed
                drop(client);
                
                // Attempt reconnection
                if let Ok(new_client) = self.reconnect().await {
                    *self.client.lock().await = Some(new_client);
                    
                    // Retry once
                    return self.execute_tool(name, params).await;
                }
                
                Err(e)
            }
            Err(e) => Err(e),
        }
    }
}
```

**Observations:**
- ✅ Graceful degradation
- ✅ LLM-friendly errors
- ⚠️ Reconnection adds complexity
- ⚠️ May need circuit breaker pattern

### Challenge 7: Permission Model

**Issue:** How to handle permissions for MCP tools?

**Current Approach:**
- Check `allowed_tools` in agent config
- Support server-level wildcards (`@server`)
- Default to Ask

**Limitations:**
- No per-tool settings (unlike native tools)
- No parameter-based filtering
- No path-based restrictions

**For agent_env:**

**Option A: Simple (Current)**
- Only allowed_tools check
- Server-level or tool-level
- No advanced filtering

**Option B: Extended Settings**
```yaml
toolsSettings:
  "@filesystem":
    allowedPaths: ["/workspace/**"]
    deniedPaths: ["/workspace/.git/**"]
  "@git":
    allowedOperations: ["status", "diff", "log"]
```

**Option C: Per-Tool Configuration**
```yaml
toolsSettings:
  "filesystem::read_file":
    allowedPaths: ["/workspace/**"]
  "filesystem::write_file":
    allowedPaths: ["/workspace/src/**"]
```

**Recommendation:** Option A for MVP, Option C for future

**Rationale:**
- MVP should be simple
- Can add advanced filtering later
- Most MCP servers are trusted

### Challenge 8: Testing Strategy

**Issue:** How to test MCP integration without real servers?

**Approaches:**

**1. Mock MCP Server:**
```rust
struct MockMcpServer {
    tools: Vec<McpToolInfo>,
    responses: HashMap<String, CallToolResult>,
}

impl MockMcpServer {
    fn new() -> Self { ... }
    
    fn add_tool(&mut self, name: &str, schema: Value) { ... }
    
    fn set_response(&mut self, tool: &str, result: CallToolResult) { ... }
}
```

**2. Test MCP Server:**
- Simple stdio server for testing
- Implements basic tools
- Predictable behavior
- Fast initialization

**3. Integration Tests:**
- Use real MCP servers (if available)
- Test full flow
- Slower but more realistic

**Recommendation:** All three approaches

**Test Coverage:**
- Unit tests with mocks
- Integration tests with test server
- E2E tests with real servers (optional)

### Challenge 9: Performance Considerations

**Issue:** MCP adds latency and overhead

**Latency Sources:**
1. Process spawn (stdio): 50-200ms
2. Handshake: 10-50ms
3. Tool discovery: 10-100ms per page
4. Tool execution: Variable (depends on tool)
5. JSON-RPC overhead: 1-5ms per call

**Optimization Strategies:**

**1. Lazy Initialization:**
- Don't block Session creation
- Initialize servers in background
- First use may have delay

**2. Connection Pooling (HTTP):**
- Reuse HTTP connections
- Keep-alive
- Connection limits

**3. Caching:**
- Cache tool list
- Only re-fetch on notification
- Cache server capabilities

**4. Parallel Execution:**
- Execute multiple tools concurrently
- Don't wait for sequential completion
- Respect dependencies

**5. Timeouts:**
- Configurable per server
- Prevent indefinite hangs
- Fast failure

**Observations:**
- ⚠️ MCP adds overhead vs native tools
- ✅ Acceptable for most use cases
- ✅ Flexibility outweighs cost

### Challenge 10: Backward Compatibility

**Issue:** Need to support existing MCP configurations

**Requirements:**
- Support legacy `~/.aws/amazonq/mcp.json`
- Support agent config `mcpServers` field
- Support `use_legacy_mcp_json` flag
- Maintain tool naming conventions

**Solution:**
```rust
// In Session::new()
let mut mcp_configs = Vec::new();

// Load from agent config
if let Some(agent) = &agent {
    mcp_configs.extend(
        agent.mcp_servers.mcp_servers.values().cloned()
    );
    
    // Load legacy config if flag set
    if agent.use_legacy_mcp_json {
        if let Ok(legacy) = load_legacy_mcp_config(os).await {
            mcp_configs.extend(legacy);
        }
    }
}

// Initialize with combined configs
let mcp_manager = McpServerManager::new(mcp_configs).await?;
```

**Observations:**
- ✅ Maintains compatibility
- ✅ Gradual migration path
- ⚠️ Legacy support adds complexity

---

## 8. Key Files Reference

### Existing MCP Implementation

**MCP Client:**
- `crates/chat-cli/src/mcp_client/mod.rs` - Module exports
- `crates/chat-cli/src/mcp_client/client.rs` - McpClientService, RunningService, initialization
- `crates/chat-cli/src/mcp_client/messenger.rs` - Messenger trait for event communication
- `crates/chat-cli/src/mcp_client/oauth_util.rs` - OAuth authentication for HTTP transport

**Tool Manager:**
- `crates/chat-cli/src/cli/chat/tool_manager.rs` - ToolManager orchestration (1,000+ lines)
- `crates/chat-cli/src/cli/chat/server_messenger.rs` - ServerMessenger implementation

**Custom Tools:**
- `crates/chat-cli/src/cli/chat/tools/custom_tool.rs` - CustomTool implementation
- `crates/chat-cli/src/cli/chat/tools/mod.rs` - Tool enum and common utilities

**Configuration:**
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent structure with mcp_servers field
- `crates/chat-cli/src/cli/agent/mcp_config.rs` - McpServerConfig structure
- `crates/chat-cli/src/cli/chat/tools/custom_tool.rs` - CustomToolConfig structure

**MCP CLI Commands:**
- `crates/chat-cli/src/cli/chat/cli/mcp.rs` - MCP-related CLI commands
- `crates/chat-cli/src/cli/mcp.rs` - Top-level MCP commands

### Agent Environment (Current)

**Core:**
- `crates/chat-cli/src/agent_env/mod.rs` - Module exports
- `crates/chat-cli/src/agent_env/session.rs` - Session orchestrator
- `crates/chat-cli/src/agent_env/worker.rs` - Worker state
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - AgentLoop implementation

**Events:**
- `crates/chat-cli/src/agent_env/events.rs` - Event definitions
- `crates/chat-cli/src/agent_env/event_bus.rs` - EventBus implementation

**Tool System (To be created in Task 1.3):**
- `crates/chat-cli/src/agent_env/tools/mod.rs` - Module exports
- `crates/chat-cli/src/agent_env/tools/tool_trait.rs` - Tool trait definition
- `crates/chat-cli/src/agent_env/tools/tool_registry.rs` - ToolRegistry implementation
- `crates/chat-cli/src/agent_env/tools/tool_context.rs` - ToolContext structure
- `crates/chat-cli/src/agent_env/tools/tool_result.rs` - ToolResult types

### New Files for MCP Integration

**MCP Module:**
- `crates/chat-cli/src/agent_env/mcp/mod.rs` - Module exports
- `crates/chat-cli/src/agent_env/mcp/server_manager.rs` - McpServerManager
- `crates/chat-cli/src/agent_env/mcp/server.rs` - McpServer
- `crates/chat-cli/src/agent_env/mcp/tool_wrapper.rs` - McpToolWrapper implementing Tool trait
- `crates/chat-cli/src/agent_env/mcp/events.rs` - MCP-specific events (optional)

**Tests:**
- `crates/chat-cli/src/agent_env/mcp/tests/mod.rs` - Test module
- `crates/chat-cli/src/agent_env/mcp/tests/mock_server.rs` - Mock MCP server for testing
- `crates/chat-cli/src/agent_env/mcp/tests/integration.rs` - Integration tests

### External Dependencies

**rmcp Crate:**
- Location: External crate (not in this repo)
- Purpose: MCP protocol implementation
- Key types:
  - `Service` trait
  - `RunningService`
  - `CallToolRequestParam`, `CallToolResult`
  - `ListToolsResult`, `Tool`
  - `ServerNotification`, `ServerRequest`
  - `InitializeRequestParam`

**Transport:**
- `rmcp::transport::TokioChildProcess` - Stdio transport
- HTTP transport via reqwest (in rmcp)

### Configuration Files

**Agent Config:**
```
~/.config/q/agents/{agent_name}.yaml
```

**Legacy MCP Config:**
```
~/.aws/amazonq/mcp.json
```

**Workspace MCP Config:**
```
{workspace}/.amazonq/mcp.json
```

### Documentation Files

**Architecture:**
- `codebase/agent-environment/README.md` - Agent environment overview
- `codebase/agent-environment/session.md` - Session documentation
- `codebase/agent-environment/worker.md` - Worker documentation
- `codebase/agent-environment/tasks.md` - Task system documentation

**Planning:**
- `planning/mvp-tools-mcp/mvp-tools-mcp-0-scope.md` - Scope document
- `planning/mvp-tools-mcp/mvp-tools-mcp-1-research.md` - This document
- `planning/mvp-tools-mcp/mvp-tools-mcp-2-design.md` - Design document (to be created)
- `planning/mvp-tools-mcp/mvp-tools-mcp-3-implementation-plan.md` - Implementation plan (to be created)
- `planning/mvp-tools-mcp/mvp-tools-mcp-4-implementation-log.md` - Implementation log (to be created)

### Related Tool System Files

**From Task 1.3 (mvp-tools-basic):**
- `planning/mvp-tools-basic/mvp-tools-basic-0-scope.md` - Tool system scope
- `planning/mvp-tools-basic/mvp-tools-basic-1-research.md` - Tool system research
- `planning/mvp-tools-basic/mvp-tools-basic-2-design.md` - Tool system design (to be created)

**Native Tool Implementations:**
- `crates/chat-cli/src/cli/chat/tools/fs_read.rs` - FsRead tool (reference)
- `crates/chat-cli/src/cli/chat/tools/fs_write.rs` - FsWrite tool (reference)
- `crates/chat-cli/src/cli/chat/tools/execute.rs` - Execute tool (reference)

---

## 9. Summary and Next Steps

### Key Findings

#### 1. Existing MCP Implementation is Solid

**Strengths:**
- ✅ Complete MCP protocol implementation via rmcp crate
- ✅ Supports both stdio and HTTP transports
- ✅ OAuth authentication for HTTP servers
- ✅ Robust error handling and graceful degradation
- ✅ Dynamic tool discovery and updates
- ✅ Process management for stdio servers

**Limitations:**
- ❌ Tightly coupled to old ToolManager architecture
- ❌ No explicit shutdown mechanism
- ❌ Complex orchestrator task with many responsibilities
- ❌ Terminal-specific loading display

**Reusability:** 60-70% of existing code can be reused

#### 2. Integration with agent_env is Straightforward

**Key Insight:** MCP tools can implement the same `Tool` trait as native tools

**Benefits:**
- No special handling in AgentLoop
- Same permission system
- Same execution flow
- Same error handling
- Transparent to LLM

**Requirements:**
- Task 1.3 (Tool System) must be completed first
- Need ToolRegistry for registration
- Need Tool trait definition
- Need ToolContext for execution

#### 3. Architecture is Well-Defined

**Three-Layer Design:**

1. **McpServerManager** - Session-level orchestration
   - Manages all MCP servers
   - Coordinates initialization
   - Handles shutdown

2. **McpServer** - Individual server management
   - Process lifecycle
   - Tool discovery
   - Request forwarding

3. **McpToolWrapper** - Tool trait implementation
   - Implements Tool trait
   - Forwards to MCP server
   - Permission evaluation

**Integration Points:**
- Session initialization: Start MCP servers
- Tool registration: Add to ToolRegistry
- Tool execution: Via Tool trait
- Session shutdown: Clean up servers

#### 4. Critical Challenges Identified

**Top 5 Challenges:**

1. **Async Initialization** - Progressive registration recommended
2. **Process Lifecycle** - Need explicit shutdown mechanism
3. **Error Handling** - Graceful degradation strategy defined
4. **Tool Name Conflicts** - Server prefix approach works well
5. **Testing** - Need mock servers and test infrastructure

**All challenges have proposed solutions**

#### 5. Dependency on Task 1.3 is Critical

**Cannot proceed without:**
- Tool trait definition
- ToolRegistry implementation
- ToolContext structure
- Tool execution flow in AgentLoop
- Permission/approval mechanism

**Estimated dependency:** Task 1.3 must be 80%+ complete

### Research Completeness

**Covered:**
- ✅ MCP protocol understanding
- ✅ Existing implementation analysis
- ✅ Server lifecycle management
- ✅ Tool discovery and registration
- ✅ Tool execution flow
- ✅ Integration architecture
- ✅ Critical challenges
- ✅ File references

**Not Covered (Design Phase):**
- ⚠️ Detailed API specifications
- ⚠️ Sequence diagrams
- ⚠️ Error handling state machines
- ⚠️ Testing strategy details
- ⚠️ Migration plan from old architecture

### Estimated Complexity

**Overall:** Medium-High

**Breakdown:**
- McpServerManager: Medium (4-6 hours)
- McpServer: Medium (4-6 hours)
- McpToolWrapper: Low (2-3 hours)
- Integration with Session: Low (2-3 hours)
- Testing infrastructure: Medium (4-6 hours)
- Documentation: Low (2-3 hours)

**Total Estimate:** 18-27 hours

**Risk Factors:**
- Dependency on Task 1.3 (high risk if delayed)
- Process lifecycle edge cases (medium risk)
- Dynamic tool updates (low risk)
- Performance optimization (low risk)

### Next Steps

#### Phase 1: Design (6-8 hours)

**Deliverable:** `mvp-tools-mcp-2-design.md`

**Tasks:**
1. Design McpServerManager API
2. Design McpServer API
3. Design McpToolWrapper implementation
4. Create sequence diagrams:
   - Server initialization
   - Tool discovery
   - Tool execution
   - Dynamic updates
   - Shutdown
5. Design error handling state machines
6. Design testing strategy
7. Plan code reuse from existing implementation
8. Design migration path

**Key Decisions:**
- Async initialization approach (progressive registration)
- Shutdown protocol
- Event system integration (optional vs required)
- Permission model (simple vs extended)
- Testing approach (mocks, test server, integration)

#### Phase 2: Implementation Planning (4-6 hours)

**Deliverable:** `mvp-tools-mcp-3-implementation-plan.md`

**Tasks:**
1. Break down into implementation tasks
2. Define task dependencies
3. Identify code to reuse vs rewrite
4. Plan testing for each component
5. Define acceptance criteria
6. Estimate effort per task
7. Create implementation order

#### Phase 3: Implementation (10-14 hours)

**Deliverable:** Working MCP integration

**High-Level Tasks:**
1. Create MCP module structure
2. Implement McpServerManager
3. Implement McpServer (reuse existing code)
4. Implement McpToolWrapper
5. Integrate with Session
6. Add shutdown mechanism
7. Create mock server for testing
8. Write unit tests
9. Write integration tests
10. Test with real MCP servers
11. Update documentation

#### Phase 4: Testing and Refinement (4-6 hours)

**Tasks:**
1. Test with multiple MCP servers
2. Test error scenarios
3. Test dynamic updates
4. Performance testing
5. Fix bugs
6. Optimize if needed
7. Final documentation

### Success Criteria

**MVP Complete When:**
- ✅ MCP servers can be configured in agent config
- ✅ Servers initialize during Session creation
- ✅ Tools are discovered and registered
- ✅ MCP tools can be executed via AgentLoop
- ✅ Permission system works for MCP tools
- ✅ Errors are handled gracefully
- ✅ Servers shut down cleanly
- ✅ Tests pass
- ✅ Documentation complete

**Post-MVP Enhancements:**
- Worker-specific MCP instances (Task 3.2)
- Advanced permission filtering
- Automatic reconnection
- Circuit breaker pattern
- Performance optimizations
- WebUI integration

### Risks and Mitigation

**Risk 1: Task 1.3 Delays**
- **Impact:** High (blocks MCP work)
- **Mitigation:** Monitor Task 1.3 progress, prepare design in parallel
- **Contingency:** Can start design phase before Task 1.3 complete

**Risk 2: Process Management Issues**
- **Impact:** Medium (edge cases, cleanup)
- **Mitigation:** Thorough testing, timeout mechanisms
- **Contingency:** Simplify to basic process management for MVP

**Risk 3: Performance Problems**
- **Impact:** Low (acceptable for MVP)
- **Mitigation:** Async initialization, lazy loading
- **Contingency:** Optimize in post-MVP

**Risk 4: Testing Complexity**
- **Impact:** Medium (hard to test without real servers)
- **Mitigation:** Create mock server, use test MCP servers
- **Contingency:** Focus on integration tests with real servers

### Conclusion

MCP integration is **feasible and well-understood**. The existing implementation provides a solid foundation, and the integration with agent_env is straightforward thanks to the Tool trait abstraction.

**Key Success Factors:**
1. Wait for Task 1.3 to be substantially complete
2. Reuse existing MCP client code
3. Keep architecture simple for MVP
4. Focus on graceful degradation
5. Test thoroughly with real servers

**Recommendation:** Proceed to design phase after Task 1.3 reaches 80% completion.

---

**Document Status:** ✅ Research Complete

**Next Document:** `mvp-tools-mcp-2-design.md`

**Estimated Time to Design:** 6-8 hours

**Estimated Time to Implement:** 18-27 hours (after Task 1.3)

---
