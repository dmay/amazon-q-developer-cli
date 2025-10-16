# ToolManager Analysis

**File**: `crates/chat-cli/src/cli/chat/tool_manager.rs`  
**Lines**: ~2000 LOC

## Overview

`ToolManager` is the central orchestrator for managing tool lifecycle, MCP (Model Context Protocol) server connections, and asynchronous tool/prompt operations in the Q CLI chat system. It handles initialization, validation, name sanitization, and runtime management of both native tools and MCP server-provided tools.

---

## What Does It Do?

### Core Responsibilities

1. **MCP Server Lifecycle Management**
   - Initializes child processes for MCP servers defined in configuration
   - Manages server connection states (pending, ready, failed)
   - Handles server disconnection and cleanup
   - Supports hot-swapping of agent configurations via `swap_agent()`

2. **Tool Discovery & Registration**
   - Fetches tool lists from MCP servers asynchronously
   - Validates tool schemas against naming conventions
   - Sanitizes tool names to comply with `^[a-zA-Z][a-zA-Z0-9_]*$` regex
   - Maintains mappings between model-facing names and host tool names
   - Handles tool name conflicts and aliasing

3. **Prompt Management**
   - Caches prompt lists from MCP servers
   - Resolves ambiguous prompt names (when multiple servers expose same prompt)
   - Processes prompt arguments and schemas
   - Supports server-qualified prompt names (`server_name/prompt_name`)

4. **Async Event Orchestration**
   - Spawns background "orchestrator task" to handle server-initiated events
   - Listens for tool list updates, prompt list updates, OAuth requirements
   - Coordinates between multiple MCP clients and the main chat loop
   - Provides loading status updates to UI via channels

5. **Tool Invocation**
   - Maps assistant tool use requests to concrete tool implementations
   - Routes MCP tool calls to appropriate server clients
   - Handles native tools (fs_read, fs_write, execute_bash, etc.)
   - Returns structured tool results or errors

---

## Dependencies

### External Crates

- **`tokio`**: Async runtime, channels (`mpsc`, `broadcast`), synchronization primitives (`Mutex`, `RwLock`, `Notify`)
- **`rmcp`**: MCP protocol implementation (ServiceError, GetPromptRequestParam, Prompt, etc.)
- **`crossterm`**: Terminal UI rendering for loading spinners and status messages
- **`regex`**: Tool name validation
- **`eyre`**: Error handling with `eyre::Report`
- **`serde_json`**: Tool schema serialization/deserialization
- **`chrono`**: Timestamp generation for loading records

### Internal Modules

- **`mcp_client`**: `InitializedMcpClient`, `McpClientService`, `InnerService`
- **`cli::chat::tools`**: Native tool implementations (FsRead, FsWrite, ExecuteCommand, UseAws, etc.)
- **`cli::chat::server_messenger`**: `ServerMessengerBuilder`, `UpdateEventMessage`
- **`cli::agent`**: `Agent`, `McpServerConfig`
- **`database`**: Settings storage (e.g., `McpInitTimeout`)
- **`telemetry`**: Metrics for server initialization

---

## Key Data Structures

### ToolManager

```rust
pub struct ToolManager {
    pub conversation_id: String,
    pub clients: HashMap<String, InitializedMcpClient>,
    pub pending_clients: Arc<RwLock<HashSet<String>>>,
    pub has_new_stuff: Arc<AtomicBool>,
    prompts_sender_receiver_pair: Option<PromptsChannelPair>,
    new_tool_specs: NewToolSpecs,
    notify: Option<Arc<Notify>>,
    loading_status_sender: Option<tokio::sync::mpsc::Sender<LoadingMsg>>,
    loading_display_task: Option<JoinHandle<Result<(), Report>>>,
    pub tn_map: HashMap<ModelToolName, ToolInfo>,
    pub schema: HashMap<ModelToolName, ToolSpec>,
    is_interactive: bool,
    pub mcp_load_record: Arc<Mutex<HashMap<String, Vec<LoadingRecord>>>>,
    disabled_servers: Vec<String>,
    messenger_builder: Option<ServerMessengerBuilder>,
    pub agent: Arc<Mutex<Agent>>,
    is_first_launch: bool,
}
```

**Key Fields**:
- `clients`: Active MCP server connections
- `pending_clients`: Servers still initializing
- `has_new_stuff`: Flag indicating new tools available (checked by main loop)
- `tn_map`: Maps sanitized tool names → `ToolInfo` (server name + original name)
- `schema`: Tool specifications sent to the model
- `messenger_builder`: Factory for creating messengers that communicate with orchestrator

### ToolManagerBuilder

Builder pattern for constructing `ToolManager`. Key methods:

```rust
impl ToolManagerBuilder {
    pub fn conversation_id(mut self, conversation_id: &str) -> Self
    pub fn agent(mut self, agent: Agent) -> Self
    pub fn prompt_query_sender(mut self, sender: ...) -> Self
    pub async fn build(self, os: &mut Os, output: ..., interactive: bool) -> Result<ToolManager>
}
```

**Build Process**:
1. Separates enabled/disabled servers from agent config
2. Spawns loading display task (if interactive)
3. Spawns orchestrator task (if prompt channels provided)
4. Initializes MCP clients via `McpClientService::new().init()`
5. Returns `ToolManager` instance

### Type Aliases

```rust
type ModelToolName = String;        // Tool name as seen by the model
type HostToolName = String;         // Tool name as exposed by MCP server
type ServerName = String;           // MCP server name from config
type NewToolSpecs = Arc<Mutex<HashMap<ServerName, (HashMap<ModelToolName, ToolInfo>, Vec<ToolSpec>)>>>;
type PromptsChannelPair = (broadcast::Sender<PromptQuery>, broadcast::Receiver<PromptQueryResult>);
```

### LoadingMsg Enum

Messages sent to the loading display task:

```rust
enum LoadingMsg {
    Done { name: String, time: String },
    Error { name: String, msg: Report, time: String },
    Warn { name: String, msg: Report, time: String },
    Terminate { still_loading: Vec<String> },
    SignInNotice { name: String },
}
```

---

## How Is It Used in the Program?

### Creation Flow

1. **Entry Point**: `ConversationState::new()` in `conversation.rs`
   ```rust
   pub async fn new(
       conversation_id: &str,
       agents: Agents,
       tool_config: HashMap<String, ToolSpec>,
       tool_manager: ToolManager,  // Passed in pre-built
       ...
   ) -> Self
   ```

2. **Builder Construction**: Typically in chat initialization code
   ```rust
   let tool_manager = ToolManagerBuilder::default()
       .conversation_id(&conv_id)
       .agent(agent)
       .prompt_query_sender(tx)
       .prompt_query_receiver(rx)
       .build(&mut os, output, interactive)
       .await?;
   ```

3. **Tool Loading**: After creation, `load_tools()` is called
   ```rust
   tool_manager.load_tools(&mut os, &mut stderr).await?;
   ```

### Tool Invocation Flow

1. **Model Response**: Assistant returns tool use in chat response
2. **Parsing**: `AssistantToolUse` extracted from response
3. **Tool Resolution**: `get_tool_from_tool_use()` called
   ```rust
   pub async fn get_tool_from_tool_use(&mut self, value: AssistantToolUse) -> Result<Tool, ToolResult>
   ```
4. **Execution**:
   - Native tools: Directly instantiated (e.g., `Tool::FsRead`)
   - MCP tools: Looked up in `tn_map`, client retrieved from `clients`, wrapped in `Tool::Custom`
5. **Result**: Tool executes and returns `ToolResult`

### Agent Swapping

When user switches agents (e.g., via `/agent` command):

```rust
pub async fn swap_agent(&mut self, os: &mut Os, output: &mut impl Write, agent: &Agent) -> Result<()>
```

**Process**:
1. Drains all `clients` and spawns cleanup tasks
2. Updates `agent` field
3. Clears `mcp_load_record`
4. Builds new `ToolManager` from `ToolManagerBuilder::from(&mut *self)`
5. Swaps old with new
6. Calls `load_tools()` to reinitialize

**Key Insight**: `messenger_builder` is preserved, so the orchestrator task continues listening.

---

## Call Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      ConversationState                          │
│  - Holds ToolManager instance                                   │
│  - Calls get_tool_from_tool_use() during chat loop              │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     │ owns
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                        ToolManager                              │
│  - clients: HashMap<String, InitializedMcpClient>               │
│  - tn_map: HashMap<ModelToolName, ToolInfo>                     │
│  - schema: HashMap<ModelToolName, ToolSpec>                     │
│  - messenger_builder: ServerMessengerBuilder                    │
│  - agent: Arc<Mutex<Agent>>                                     │
└─┬───────────────────────────────────────────────────────────┬───┘
  │                                                           │
  │ created by                                                │ uses
  ▼                                                           ▼
┌──────────────────────────────────┐         ┌──────────────────────────────┐
│    ToolManagerBuilder            │         │   InitializedMcpClient       │
│  - conversation_id               │         │  - Pending(JoinHandle)       │
│  - agent                         │         │  - Ready(RunningService)     │
│  - prompt_query_sender/receiver  │         └──────────────────────────────┘
│  - messenger_builder             │
│  - pending_clients               │
│  - new_tool_specs                │
│                                  │
│  build() spawns:                 │
│   1. display_task                │
│   2. orchestrator_task           │
└──────────────────────────────────┘
           │
           │ spawns
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Orchestrator Task                            │
│  - Listens to UpdateEventMessage from MCP clients               │
│  - Handles ListToolsResult, ListPromptsResult, OauthLink        │
│  - Updates new_tool_specs, prompts cache                        │
│  - Sends LoadingMsg to display task                             │
│  - Responds to PromptQuery requests                             │
└─────────────────────────────────────────────────────────────────┘
           │
           │ receives from
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                   ServerMessenger                               │
│  - Created by messenger_builder.build_with_name(server_name)    │
│  - Each MCP client has one                                      │
│  - Sends UpdateEventMessage to orchestrator                     │
└─────────────────────────────────────────────────────────────────┘
           │
           │ owned by
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                   McpClientService                              │
│  - Wraps MCP server child process                               │
│  - Calls init() to start server                                 │
│  - Sends tool/prompt lists via messenger                        │
└─────────────────────────────────────────────────────────────────┘
```

### Consumers

- **`ConversationState`**: Primary consumer, holds `ToolManager` instance
- **Chat Loop**: Calls `get_tool_from_tool_use()` to resolve tools
- **`/mcp` command**: Accesses `mcp_load_record` for status display
- **`/prompt` command**: Calls `list_prompts()` and `get_prompt()`
- **`/agent` command**: Calls `swap_agent()` to switch configurations

---

## Async Tools Initialization

### Initialization Workflow

1. **Builder.build() Called**
   - Separates enabled/disabled servers
   - Creates `pending_clients` set with all server names
   - Spawns **display task** (if interactive)
   - Spawns **orchestrator task**
   - Initializes MCP clients asynchronously

2. **MCP Client Init**
   ```rust
   for (name, mcp_client) in pre_initialized {
       let init_res = mcp_client.init(os).await;
       match init_res {
           Ok(running_service) => clients.insert(name, running_service),
           Err(e) => /* send error to orchestrator */
       }
   }
   ```

3. **Orchestrator Task Listens**
   - Receives `UpdateEventMessage::InitStart` when client starts
   - Receives `UpdateEventMessage::ListToolsResult` when tools fetched
   - Validates and sanitizes tool names
   - Updates `new_tool_specs` (shared Arc<Mutex<...>>)
   - Sets `has_new_stuff` flag to `true`
   - Removes server from `pending_clients`
   - Sends `LoadingMsg::Done` to display task

4. **load_tools() Waits**
   ```rust
   tokio::select! {
       _ = timeout_fut => { /* timeout */ },
       _ = server_loading_fut => { /* all servers loaded */ },
       _ = ctrl_c() => { /* user interrupted */ }
   }
   ```
   - `server_loading_fut` waits on `notify.notified()`
   - Orchestrator calls `notify.notify_one()` when all servers initialized
   - After select resolves, calls `update()` to merge `new_tool_specs` into `schema`

### Signals Consumers Need to Listen To

#### 1. **has_new_stuff: Arc<AtomicBool>**
   - **Purpose**: Indicates new tools available from MCP servers
   - **Set By**: Orchestrator task when `ListToolsResult` processed
   - **Checked By**: Main chat loop before sending request
   - **Action**: Call `tool_manager.update().await` to merge new tools

#### 2. **notify: Arc<Notify>**
   - **Purpose**: Signals initial loading completion
   - **Set By**: Orchestrator when `initialized.len() >= total`
   - **Waited On**: `load_tools()` via `notify.notified().await`
   - **Lifecycle**: Consumed after first load, set to `None`

#### 3. **loading_status_sender: mpsc::Sender<LoadingMsg>**
   - **Purpose**: Sends loading status to display task
   - **Messages**: `Done`, `Error`, `Warn`, `Terminate`, `SignInNotice`
   - **Consumed By**: Display task (renders spinner, success/error messages)
   - **Lifecycle**: Dropped after `load_tools()` completes

#### 4. **pending_clients: Arc<RwLock<HashSet<String>>>**
   - **Purpose**: Tracks servers still initializing
   - **Updated By**: Orchestrator (adds on `InitStart`, removes on `ListToolsResult`)
   - **Read By**: `load_tools()` to determine which servers still loading
   - **Exposed Via**: `pending_clients()` method

#### 5. **PromptQuery/PromptQueryResult Channels**
   - **Purpose**: Request/response pattern for prompt lists
   - **Query Types**: `List` (all prompts), `Search(Option<String>)` (filtered)
   - **Handled By**: Orchestrator task
   - **Used By**: `/prompt` command implementation

### Example: Listening for New Tools

```rust
// In main chat loop
if tool_manager.has_new_stuff.load(Ordering::Acquire) {
    tool_manager.update().await;
    tool_manager.has_new_stuff.store(false, Ordering::Release);
    // Refresh tool specs sent to model
}
```

---

## Multiple ToolManagers & Shared MCP Servers

### Is It Possible to Have Multiple ToolManagers in Memory?

**Yes**, but with caveats:

1. **Cloning**: `ToolManager` implements `Clone`, but it's a **shallow clone**:
   ```rust
   impl Clone for ToolManager {
       fn clone(&self) -> Self {
           Self {
               conversation_id: self.conversation_id.clone(),
               has_new_stuff: self.has_new_stuff.clone(),  // Arc cloned
               new_tool_specs: self.new_tool_specs.clone(), // Arc cloned
               tn_map: self.tn_map.clone(),
               schema: self.schema.clone(),
               // ... other fields defaulted
               ..Default::default()
           }
       }
   }
   ```
   - Shared state: `has_new_stuff`, `new_tool_specs`, `mcp_load_record`
   - **Not cloned**: `clients`, `notify`, `loading_status_sender`, `messenger_builder`

2. **Practical Usage**: Multiple instances not typically used in practice
   - Each `ConversationState` has one `ToolManager`
   - Switching agents uses `swap_agent()` (replaces in-place)

3. **Limitations**:
   - Cloned instances lose connection to orchestrator task
   - Cannot invoke MCP tools (no `clients`)
   - Useful only for reading `schema` or `tn_map`

### Is It Possible to Have Shared MCP Servers Between Multiple ToolManagers?

**Partially**, via the orchestrator task architecture:

#### Current Design (Single ToolManager)

- One orchestrator task per `ToolManager` instance
- Orchestrator owns the `msg_rx` channel for `UpdateEventMessage`
- All `ServerMessenger` instances created by `messenger_builder` send to this channel
- MCP clients are **not shared** (each `ToolManager` owns its `clients` HashMap)

#### Theoretical Multi-ToolManager Sharing

To share MCP servers across multiple `ToolManager` instances:

1. **Shared Orchestrator**:
   - Keep single orchestrator task alive across ToolManager instances
   - Pass `messenger_builder` to new instances (already done in `swap_agent()`)
   - New MCP clients created by second ToolManager would send events to same orchestrator

2. **Shared Clients**:
   - Wrap `clients` in `Arc<RwLock<HashMap<...>>>`
   - Clone Arc when creating new ToolManager
   - Both instances could invoke tools on same MCP servers

3. **Challenges**:
   - **Conversation ID**: MCP servers may need per-conversation state
   - **Agent Filters**: Different ToolManagers may have different tool allowlists
   - **Lifecycle**: When to shut down shared servers?
   - **Telemetry**: Which conversation_id to attribute metrics to?

#### Current Implementation: swap_agent()

The `swap_agent()` method demonstrates a **replacement pattern** rather than sharing:

```rust
pub async fn swap_agent(&mut self, os: &mut Os, output: &mut impl Write, agent: &Agent) -> Result<()> {
    // 1. Drain old clients (spawns cleanup tasks)
    let to_evict = self.clients.drain().collect::<Vec<_>>();
    tokio::spawn(async move { /* cancel old clients */ });
    
    // 2. Update agent
    *self.agent.lock().await = agent.clone();
    
    // 3. Build new ToolManager (preserves messenger_builder)
    let builder = ToolManagerBuilder::from(&mut *self);
    let mut new_tool_manager = builder.build(os, Box::new(std::io::sink()), true).await?;
    
    // 4. Swap in-place
    std::mem::swap(self, &mut new_tool_manager);
    
    // 5. Load new tools
    self.load_tools(os, output).await?;
    Ok(())
}
```

**Key Insight**: `messenger_builder` is preserved via `ToolManagerBuilder::from()`, so the **orchestrator task continues running** and listens to new MCP clients.

#### Recommendation for True Sharing

If you need multiple `ToolManager` instances with shared MCP servers:

1. **Extract Orchestrator**: Make it a separate struct with its own lifecycle
2. **Shared Client Pool**: Use `Arc<RwLock<HashMap<String, InitializedMcpClient>>>`
3. **Per-Instance Filters**: Apply agent-specific tool filters in `get_tool_from_tool_use()`
4. **Reference Counting**: Track which ToolManagers are using which servers
5. **Graceful Shutdown**: Only cancel MCP clients when last ToolManager drops

**Current State**: Not implemented. Each `ToolManager` owns its MCP clients exclusively.

---

## Summary

### What ToolManager Does
- Manages MCP server lifecycle (init, connect, disconnect)
- Discovers and validates tools from servers
- Handles tool name sanitization and conflict resolution
- Orchestrates async events (tool lists, prompts, OAuth)
- Routes tool invocations to appropriate handlers

### Key Dependencies
- `tokio` for async runtime and channels
- `rmcp` for MCP protocol
- `crossterm` for terminal UI
- Internal modules for tools, agents, database

### Usage in Program
- Created via `ToolManagerBuilder` in `ConversationState::new()`
- Used in chat loop for tool resolution via `get_tool_from_tool_use()`
- Supports agent swapping via `swap_agent()`

### Async Initialization
- Spawns orchestrator task to handle server events
- Uses `Notify` to signal completion
- Exposes `has_new_stuff` flag for incremental updates
- Provides loading status via `LoadingMsg` channel

### Multiple Instances
- **Cloning**: Possible but limited (loses clients, orchestrator connection)
- **Shared Servers**: Not currently supported
- **swap_agent()**: Replaces in-place while preserving orchestrator

### Signals to Listen To
1. `has_new_stuff` - New tools available
2. `notify` - Initial load complete
3. `loading_status_sender` - UI updates
4. `pending_clients` - Servers still loading
5. Prompt query channels - Prompt list requests
