# MCP Server Instance Tracking

## Components That Maintain Server Lists

### 1. **ToolManager** (Primary Owner)

**Location**: `crates/chat-cli/src/cli/chat/tool_manager.rs`

#### Connected Servers
```rust
pub clients: HashMap<String, InitializedMcpClient>
```
- **Purpose**: Stores actual MCP client instances (connected servers)
- **Key**: Server name (as recognized by Q CLI, may differ from config due to sanitization)
- **Value**: `InitializedMcpClient` enum:
  - `Pending(JoinHandle)` - Server initializing
  - `Ready(RunningService)` - Server ready for tool invocations
- **Visibility**: Public field, directly accessible
- **Lifecycle**: 
  - Added during `ToolManagerBuilder::build()`
  - Removed during `swap_agent()` or manual cleanup
  - Used by `get_tool_from_tool_use()` for tool invocation

#### Initializing Servers
```rust
pub pending_clients: Arc<RwLock<HashSet<String>>>
```
- **Purpose**: Tracks servers still in initialization phase
- **Type**: Shared reference (Arc) - accessible by orchestrator task
- **Operations**:
  - Added when server starts init
  - Removed when `ListToolsResult` received
- **Visibility**: Public field
- **Used by**: 
  - `load_tools()` to determine when to stop waiting
  - `pending_clients()` method to expose current state
  - Orchestrator task (shared ownership)

#### Disabled Servers
```rust
disabled_servers: Vec<String>
```
- **Purpose**: List of server names that are disabled in config
- **Visibility**: Private field
- **Used for**: Display purposes only (not functional tracking)

---

### 2. **Orchestrator Task** (Event Handler)

**Location**: `spawn_orchestrator_task()` in `tool_manager.rs`

#### Local State (Task-Owned)

```rust
let mut initialized = HashSet::<String>::new();
let mut loading_servers = HashMap<String, Instant>::new();
```

**`initialized`**:
- **Purpose**: Tracks servers that have completed initialization
- **Lifecycle**: 
  - Added when `ListToolsResult` received
  - Used to check if `initialized.len() >= total` for completion notification
- **Scope**: Local to orchestrator task, not shared

**`loading_servers`**:
- **Purpose**: Maps server name → start time (for calculating load duration)
- **Lifecycle**:
  - Added on `InitStart` event
  - Removed on `ListToolsResult` event (to calculate time taken)
- **Scope**: Local to orchestrator task, not shared

#### Shared State (via Arc)

The orchestrator has **read/write access** to:
- `pending: Arc<RwLock<HashSet<String>>>` (same as `ToolManager::pending_clients`)
- `new_tool_specs: Arc<Mutex<HashMap<...>>>` (same as `ToolManager::new_tool_specs`)
- `load_record: Arc<Mutex<HashMap<...>>>` (same as `ToolManager::mcp_load_record`)

---

### 3. **Agent** (Configuration Source)

**Location**: `crates/chat-cli/src/cli/agent/mod.rs`

```rust
pub struct Agent {
    pub mcp_servers: McpServerConfig,
    // ...
}

pub struct McpServerConfig {
    pub mcp_servers: Vec<(String, CustomToolConfig)>,
}
```

- **Purpose**: Defines which servers SHOULD be loaded (configuration)
- **Not a runtime tracker**: This is the source of truth for configuration, not actual connection state
- **Used by**: `ToolManagerBuilder::build()` to determine which servers to initialize

---

## Summary Table

| Component | Field | Type | Purpose | Shared? |
|-----------|-------|------|---------|---------|
| **ToolManager** | `clients` | `HashMap<String, InitializedMcpClient>` | Connected server instances | No |
| **ToolManager** | `pending_clients` | `Arc<RwLock<HashSet<String>>>` | Servers initializing | Yes (with orchestrator) |
| **ToolManager** | `disabled_servers` | `Vec<String>` | Disabled server names | No |
| **Orchestrator** | `initialized` | `HashSet<String>` | Completed initialization | No (local) |
| **Orchestrator** | `loading_servers` | `HashMap<String, Instant>` | Loading start times | No (local) |
| **Agent** | `mcp_servers` | `Vec<(String, CustomToolConfig)>` | Configuration (not runtime) | No |

---

## Data Flow

```
Agent (config)
    │
    │ read during build()
    ▼
ToolManagerBuilder
    │
    │ creates clients
    ▼
ToolManager::clients ◄─────────────┐
    │                              │
    │ shares pending_clients       │ updates
    ▼                              │
Orchestrator Task                  │
    │                              │
    ├─ initialized (local)         │
    ├─ loading_servers (local)     │
    └─ pending_clients (shared) ───┘
```

---

## Access Patterns

### To Get Connected Servers
```rust
// From ToolManager
let connected: Vec<&String> = tool_manager.clients.keys().collect();
```

### To Get Initializing Servers
```rust
// From ToolManager
let initializing = tool_manager.pending_clients().await;
```

### To Get All Configured Servers
```rust
// From Agent
let agent = tool_manager.agent.lock().await;
let configured: Vec<String> = agent.mcp_servers.mcp_servers
    .iter()
    .map(|(name, _)| name.clone())
    .collect();
```

### To Check If Server Is Ready
```rust
// From ToolManager
match tool_manager.clients.get(server_name) {
    Some(InitializedMcpClient::Ready(_)) => { /* ready */ }
    Some(InitializedMcpClient::Pending(_)) => { /* still loading */ }
    None => { /* not found */ }
}
```

---

## Key Insights

1. **ToolManager is the single source of truth** for runtime server state
   - `clients` = connected servers
   - `pending_clients` = initializing servers

2. **Orchestrator maintains parallel tracking** for event handling
   - Local state (`initialized`, `loading_servers`) for its own logic
   - Shared state (`pending_clients`) synchronized with ToolManager

3. **Agent is configuration, not state**
   - Defines what SHOULD be loaded
   - Not updated when servers connect/disconnect

4. **No centralized "all servers" list**
   - Must combine `clients.keys()` + `pending_clients` to get complete picture
   - Disabled servers tracked separately

5. **Shared ownership via Arc** enables concurrent access
   - Orchestrator can update `pending_clients` while ToolManager reads it
   - Both components have consistent view of initialization state
