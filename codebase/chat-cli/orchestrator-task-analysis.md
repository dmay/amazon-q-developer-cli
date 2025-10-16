# Orchestrator Task Deep Dive

## Overview

The orchestrator task is a long-running async task spawned by `ToolManagerBuilder::build()` that serves as the central event hub for all MCP server lifecycle events. It runs for the lifetime of the conversation and handles asynchronous server-initiated events.

**Location**: `spawn_orchestrator_task()` in `tool_manager.rs` (lines ~1250-1750)

---

## Architecture

### Task Lifecycle

```
ToolManagerBuilder::build()
    │
    ├─> ServerMessengerBuilder::new(20)  // Creates (msg_rx, builder)
    │
    └─> spawn_orchestrator_task(...)     // Spawns background task
            │
            └─> tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            Ok(query) = prompt_list_receiver.recv() => { ... }
                            Some(msg) = msg_rx.recv() => { ... }
                            else => break  // Exit when all senders dropped
                        }
                    }
                })
```

### Communication Channels

**Inputs:**
1. **`msg_rx: mpsc::Receiver<UpdateEventMessage>`**
   - Receives events from all MCP server messengers
   - Channel capacity: 20 messages
   - Events: `InitStart`, `ListToolsResult`, `ListPromptsResult`, `OauthLink`, `Deinit`

2. **`prompt_list_receiver: broadcast::Receiver<PromptQuery>`**
   - Receives prompt list/search requests from chat commands
   - Queries: `List`, `Search(Option<String>)`

**Outputs:**
1. **`prompt_list_sender: broadcast::Sender<PromptQueryResult>`**
   - Sends prompt list/search results back to requesters
   - Results: `List(HashMap<...>)`, `Search(Vec<String>)`

2. **`loading_status_sender: mpsc::Sender<LoadingMsg>`**
   - Sends loading status to display task
   - Messages: `Done`, `Error`, `Warn`, `SignInNotice`

**Shared State (via Arc):**
- `pending: Arc<RwLock<HashSet<String>>>` - Servers currently loading
- `new_tool_specs: Arc<Mutex<HashMap<...>>>` - Newly discovered tools
- `has_new_stuff: Arc<AtomicBool>` - Flag for main loop
- `load_record: Arc<Mutex<HashMap<...>>>` - Loading history
- `agent: Arc<Mutex<Agent>>` - Agent configuration (for filters)
- `notify_weak: Weak<Notify>` - Initial load completion signal

---

## Event Handling

### 1. InitStart

**Triggered**: When MCP client begins initialization

```rust
UpdateEventMessage::InitStart { server_name, .. } => {
    pending.write().await.insert(server_name.clone());
    loading_servers.insert(server_name, std::time::Instant::now());
}
```

**Actions**:
- Adds server to `pending` set
- Records start time in `loading_servers` HashMap

**Dynamic Capability**: ✅ **YES** - Can handle servers starting at any time

---

### 2. ListToolsResult

**Triggered**: When MCP server returns tool list (success or failure)

**Actions**:
1. Remove server from `loading_servers` (calculate time taken)
2. Remove server from `pending` set
3. Extract agent's tool filter and aliases
4. Validate and sanitize tool names via `process_tool_specs()`
5. Insert into `new_tool_specs` (shared with ToolManager)
6. Set `has_new_stuff = true`
7. Send `LoadingMsg::Done/Warn/Error` to display task
8. Record in `load_record`
9. Check if `initialized.len() >= total`, notify if complete

**Dynamic Capability**: ⚠️ **PARTIAL** - The `total` parameter is fixed at spawn time

**Problem**:
```rust
if initialized.len() >= total {
    notify.notify_one();  // Only fires when reaching initial count
}
```

The `total` is passed as a parameter and never updated. If servers are added dynamically after spawn, the orchestrator won't know to wait for them.

---

### 3. ListPromptsResult

**Triggered**: When MCP server returns prompt list

**Actions**:
1. Clear existing prompts from this server
2. Insert new prompts into `prompts` HashMap
3. Handle name collisions (multiple servers with same prompt name)

**Dynamic Capability**: ✅ **YES** - Fully dynamic, replaces server's prompts on each update

---

### 4. OauthLink

**Triggered**: When MCP server requires OAuth authentication

**Actions**:
1. Record OAuth link in `load_record`
2. Send `LoadingMsg::SignInNotice` to display task

**Dynamic Capability**: ✅ **YES** - Can handle at any time

---

### 5. Deinit

**Triggered**: When MCP server disconnects/shuts down

```rust
UpdateEventMessage::Deinit { server_name, .. } => {
    // Remove prompts from this server
    for (_prompt_name, bundles) in prompts.iter_mut() {
        bundles.retain(|bundle| bundle.server_name != server_name);
    }
    prompts.retain(|_, bundles| !bundles.is_empty());
    has_new_stuff.store(true, Ordering::Release);
}
```

**Actions**:
1. Remove all prompts from disconnected server
2. Set `has_new_stuff = true` (triggers tool list refresh)

**Problem**: Tools are NOT removed here! Comment says:
> "Only prompts are stored here so we'll just be clearing that. In the future if we are also storing tools, we need to make sure that the tools are also pruned."

**Dynamic Capability**: ⚠️ **INCOMPLETE** - Prompts removed, but tools persist

---

## Can It Handle Dynamic MCP Server Lists?

### Current State Analysis

| Capability | Status | Notes |
|------------|--------|-------|
| **Add server at runtime** | ⚠️ Partial | Orchestrator handles events, but `total` is fixed |
| **Remove server at runtime** | ⚠️ Partial | Prompts removed, tools NOT removed |
| **Server loading UI** | ❌ No | Display task terminates after initial load |
| **Dynamic tool updates** | ✅ Yes | `has_new_stuff` flag triggers updates |
| **Dynamic prompt updates** | ✅ Yes | Fully supported |
| **Completion notification** | ❌ No | `notify` only fires when `initialized.len() >= total` |

---

## Modifications Needed for Full Dynamic Support

### 1. Remove Fixed `total` Parameter

**Current Problem**:
```rust
fn spawn_orchestrator_task(
    // ... other params
    total: usize,  // ❌ Fixed at spawn time
    conv_id: String,
)
```

**Solution**: Track expected servers dynamically

```rust
// Add to orchestrator state
let mut expected_servers = Arc::new(RwLock<HashSet<String>>>::new();

// On InitStart
UpdateEventMessage::InitStart { server_name, .. } => {
    expected_servers.write().await.insert(server_name.clone());
    pending.write().await.insert(server_name.clone());
    loading_servers.insert(server_name, Instant::now());
}

// On ListToolsResult
if initialized.len() >= expected_servers.read().await.len() {
    if let Some(notify) = notify_weak.upgrade() {
        notify.notify_one();
    }
}
```

### 2. Handle Tool Removal on Deinit

**Current Problem**: Tools persist after server disconnects

**Solution**: Add tool cleanup

```rust
UpdateEventMessage::Deinit { server_name, .. } => {
    // Remove prompts (existing)
    for (_prompt_name, bundles) in prompts.iter_mut() {
        bundles.retain(|bundle| bundle.server_name != server_name);
    }
    prompts.retain(|_, bundles| !bundles.is_empty());
    
    // NEW: Remove tools
    new_tool_specs.lock().await.remove(&server_name);
    
    // Remove from tracking
    pending.write().await.remove(&server_name);
    loading_servers.remove(&server_name);
    initialized.remove(&server_name);
    
    has_new_stuff.store(true, Ordering::Release);
}
```

### 3. Support Persistent Loading Display

**Current Problem**: `loading_status_sender` is `Option<&Sender>` and gets `.take()` on errors, display task terminates after initial load

**Solution**: Keep display task alive or support re-spawning

**Option A**: Never terminate display task
```rust
// In display task, don't break on Terminate
LoadingMsg::Terminate { still_loading } => {
    // Show completion message but keep listening
    execute!(output, style::Print("Initial load complete\n"))?;
    // Don't break - keep listening for new servers
}
```

**Option B**: Support dynamic display task spawning
```rust
// Add method to ToolManager
pub fn spawn_loading_display(&mut self, output: Box<dyn Write>) {
    let (task, sender) = spawn_display_task(true, 0, vec![], output);
    self.loading_display_task = task;
    self.loading_status_sender = sender;
}
```

### 4. Add Dynamic Server Management API

**New UpdateEventMessage variants**:

```rust
pub enum UpdateEventMessage {
    // ... existing variants
    
    /// Request to add a new MCP server dynamically
    AddServer {
        server_name: String,
        config: CustomToolConfig,
    },
    
    /// Request to remove an MCP server
    RemoveServer {
        server_name: String,
    },
}
```

**Handler in orchestrator**:

```rust
UpdateEventMessage::AddServer { server_name, config } => {
    // Spawn new MCP client
    let messenger = messenger_builder.build_with_name(server_name.clone());
    let client = McpClientService::new(server_name.clone(), config, messenger);
    
    // Init will trigger InitStart -> ListToolsResult flow
    tokio::spawn(async move {
        if let Err(e) = client.init(&mut os).await {
            error!("Failed to init dynamic server {}: {}", server_name, e);
        }
    });
}

UpdateEventMessage::RemoveServer { server_name } => {
    // Trigger cleanup (same as Deinit)
    // Client cancellation handled by ToolManager
}
```

---

## Proposed Architecture for Dynamic Support

### Modified Orchestrator State

```rust
struct OrchestratorState {
    // Existing
    initialized: HashSet<String>,
    loading_servers: HashMap<String, Instant>,
    prompts: HashMap<String, Vec<PromptBundle>>,
    
    // NEW: Dynamic tracking
    expected_servers: HashSet<String>,  // Servers we're waiting for
    active_servers: HashSet<String>,    // Servers currently connected
    
    // NEW: Persistent display
    display_active: bool,               // Whether to show loading UI
}
```

### Modified Notification Logic

```rust
// Replace fixed total with dynamic check
fn check_loading_complete(
    initialized: &HashSet<String>,
    expected: &HashSet<String>,
    notify_weak: &Weak<Notify>,
) {
    if initialized.len() >= expected.len() && !expected.is_empty() {
        if let Some(notify) = notify_weak.upgrade() {
            notify.notify_one();
        }
    }
}
```

### Integration with ToolManager

**New method**:

```rust
impl ToolManager {
    /// Dynamically add an MCP server at runtime
    pub async fn add_server(
        &mut self,
        server_name: String,
        config: CustomToolConfig,
        os: &mut Os,
    ) -> Result<()> {
        // Create messenger (uses existing builder)
        let messenger = self.messenger_builder
            .as_ref()
            .ok_or(eyre::eyre!("No messenger builder"))?
            .build_with_name(server_name.clone());
        
        // Create and init client
        let client = McpClientService::new(server_name.clone(), config, messenger);
        let running = client.init(os).await?;
        
        // Add to clients map
        self.clients.insert(server_name, running);
        
        Ok(())
    }
    
    /// Dynamically remove an MCP server at runtime
    pub async fn remove_server(&mut self, server_name: &str) -> Result<()> {
        if let Some(client) = self.clients.remove(server_name) {
            // Trigger Deinit event
            // Client drop will handle cleanup
            tokio::spawn(async move {
                drop(client);
            });
        }
        Ok(())
    }
}
```

---

## Summary

### Current Capabilities

✅ **Already Dynamic**:
- Handles `InitStart` events at any time
- Processes `ListToolsResult` from any server
- Updates prompts dynamically
- Responds to `Deinit` events

⚠️ **Partially Dynamic**:
- Fixed `total` parameter limits completion detection
- Tools not removed on `Deinit`
- Display task terminates after initial load

❌ **Not Dynamic**:
- No API to add/remove servers at runtime
- Completion notification tied to initial server count

### Required Modifications

**Minimal changes for dynamic support**:

1. **Replace `total` with dynamic tracking** (~10 lines)
   - Track expected servers in orchestrator state
   - Update completion check to use dynamic count

2. **Add tool cleanup on Deinit** (~5 lines)
   - Remove from `new_tool_specs`
   - Remove from tracking sets

3. **Keep display task alive** (~3 lines)
   - Don't break on `Terminate` message
   - Show status updates for new servers

**Full dynamic API** (optional):
4. Add `AddServer`/`RemoveServer` to `UpdateEventMessage`
5. Add `add_server()`/`remove_server()` methods to `ToolManager`
6. Handle dynamic server management in orchestrator

### Conclusion

**Yes, the orchestrator task CAN be modified to handle dynamic MCP server lists**, including the loading stage. The architecture is already event-driven and most of the infrastructure exists. The main limitations are:

1. Fixed `total` parameter (easy fix)
2. Missing tool cleanup on disconnect (easy fix)
3. Display task lifecycle (moderate fix)

The orchestrator's event-driven design with `UpdateEventMessage` makes it naturally suited for dynamic server management. The required changes are localized and don't require architectural overhaul.
