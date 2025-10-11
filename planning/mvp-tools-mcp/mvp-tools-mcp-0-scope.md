# MVP Tools MCP - Scope

## Overview

This workflow covers Model Context Protocol (MCP) integration into the agent_env architecture. MCP enables external tools and resources to be provided by separate server processes. This includes both shared MCP instances (all workers) and worker-specific MCP instances (future).

## Tasks Included

### 3.1: MCP Integration (Shared Instances)
### 3.2: Worker-Specific MCP (Future - Out of MVP Scope)

---

## Task 3.1: MCP Integration (Shared Instances)

### Current State

**Existing MCP Support:**
- MCP client code exists in codebase (rmcp crate)
- Agent config has `mcp_servers` field
- ToolManager handles MCP tool discovery
- MCP servers are initialized in old chat flow
- Tools from MCP servers are available alongside built-in tools

**Agent Config Example:**
```yaml
name: "developer"
mcp_servers:
  - name: "filesystem"
    command: "mcp-server-filesystem"
    args: ["/workspace"]
  - name: "git"
    command: "mcp-server-git"
    args: ["--repo", "/workspace"]
```

**Agent Environment:**
- No MCP integration in agent_env
- Tool system needs to be implemented first (Task 1.3)
- Session could manage shared MCP instances
- Workers would access MCP tools through ToolRegistry

### Requirements

**Core Functionality:**
- Initialize MCP servers at application startup
- Share MCP instances across all workers
- Discover tools from MCP servers
- Make MCP tools available through ToolRegistry
- Handle MCP server lifecycle (startup, shutdown, reconnection)
- Support multiple MCP servers simultaneously

**MCP Server Lifecycle:**
```
1. Startup: Launch MCP server processes
2. Initialization: Handshake and capability negotiation
3. Tool Discovery: Query available tools from server
4. Registration: Add MCP tools to ToolRegistry
5. Execution: Forward tool requests to MCP server
6. Shutdown: Clean termination of MCP processes
```

**Tool Naming:**
- MCP tools use delimiter: `mcp_server_name::tool_name`
- Example: `filesystem::read_file`, `git::commit`
- Prevents naming conflicts between MCP servers

**Error Handling:**
- MCP server startup failures
- MCP server crashes during operation
- Tool execution failures
- Reconnection strategies

### Key Challenges

1. **MCP Server Management:**
   - Process lifecycle management
   - Multiple concurrent MCP servers
   - Health monitoring and reconnection
   - Resource cleanup on shutdown

2. **Tool Discovery:**
   - Dynamic tool registration from MCP servers
   - Tool schema translation (MCP format → agent_env format)
   - Handling tool updates from MCP servers

3. **Tool Execution:**
   - Forwarding tool requests to correct MCP server
   - Parameter serialization/deserialization
   - Result formatting
   - Timeout handling

4. **Integration Points:**
   - Where to initialize MCP servers? (Session? AgentEnvironment?)
   - How to share MCP instances across workers?
   - How to handle worker-specific MCP configurations? (future)

5. **Permission System:**
   - Should MCP tools require approval?
   - Per-server approval vs per-tool approval?
   - How to handle untrusted MCP servers?

### Dependencies

**Critical Dependency:**
- **Task 1.3 (Tool System Integration)** must be completed first
- MCP tools will implement the agent_env Tool trait
- ToolRegistry must exist to register MCP tools
- Tool execution flow must be working

**Other Dependencies:**
- Agent config loading (for mcp_servers field)
- Process management (tokio::process)
- JSON-RPC communication (rmcp crate)

### Research Questions

1. **MCP Server Initialization:**
   - How are MCP servers currently initialized?
   - What is the startup sequence?
   - How is the handshake performed?
   - What capabilities are negotiated?

2. **Tool Discovery:**
   - How are tools discovered from MCP servers?
   - What is the tool schema format?
   - How to map MCP tool schema to agent_env Tool trait?

3. **Tool Execution:**
   - How are tool requests sent to MCP servers?
   - What is the request/response format?
   - How are errors communicated?

4. **Lifecycle Management:**
   - How to detect MCP server crashes?
   - What is the reconnection strategy?
   - How to handle partial failures (some servers down)?

5. **Configuration:**
   - Where should MCP servers be configured?
   - Agent config only, or also CLI flags?
   - How to override MCP server settings?

### Design Phases

#### Phase 1: Research (4-6 hours)
**Deliverable:** `mvp-tools-mcp-1-research.md`

**Research Tasks:**
- Study existing MCP client code (rmcp crate)
- Analyze MCP server initialization in ToolManager
- Research MCP protocol specification
- Document MCP tool discovery process
- Document MCP tool execution flow
- Identify error handling patterns
- Study MCP server lifecycle management

**Key Questions to Answer:**
- What is the MCP protocol format?
- How are MCP servers started and managed?
- How are tools discovered and registered?
- How are tool requests forwarded?
- What are the error scenarios?

#### Phase 2: Design (6-8 hours)
**Deliverable:** `mvp-tools-mcp-2-design.md`

**Design Tasks:**
- Design MCP server manager for Session
- Design MCP tool wrapper (implements Tool trait)
- Design tool discovery and registration flow
- Design tool execution forwarding
- Design error handling and reconnection
- Design shutdown and cleanup
- Create sequence diagrams
- Plan integration with ToolRegistry

**Design Decisions:**
- Where to initialize MCP servers? (Session initialization)
- How to share MCP instances? (Arc<McpServerManager>)
- How to handle MCP tool naming? (server::tool delimiter)
- How to handle MCP server failures? (reconnection strategy)
- Permission model for MCP tools? (per-server or per-tool)

**Architecture Components:**
```rust
pub struct McpServerManager {
    servers: HashMap<String, Arc<McpServer>>,
    tool_registry: Arc<ToolRegistry>,
}

pub struct McpServer {
    name: String,
    process: Child,
    client: McpClient,
    tools: Vec<McpToolWrapper>,
}

pub struct McpToolWrapper {
    server_name: String,
    tool_name: String,
    schema: ToolSchema,
    client: Arc<McpClient>,
}

impl Tool for McpToolWrapper {
    // Forwards execution to MCP server
}
```

#### Phase 3: Implementation (10-14 hours)
**Deliverable:** Working MCP integration

**Implementation Tasks:**
1. Create McpServerManager
2. Implement MCP server initialization
3. Implement tool discovery from MCP servers
4. Create McpToolWrapper implementing Tool trait
5. Integrate with ToolRegistry
6. Add MCP server lifecycle management
7. Implement error handling and reconnection
8. Add MCP server shutdown
9. Update Session to initialize MCP servers
10. Load MCP servers from Agent config
11. Test with filesystem MCP server
12. Test with multiple MCP servers
13. Test error scenarios
14. Test shutdown and cleanup

### Proposed Architecture

**McpServerManager:**
```rust
pub struct McpServerManager {
    servers: HashMap<String, Arc<McpServer>>,
}

impl McpServerManager {
    pub async fn new(configs: Vec<McpServerConfig>) -> Result<Self>;
    pub async fn start_server(&mut self, config: McpServerConfig) -> Result<()>;
    pub async fn stop_server(&mut self, name: &str) -> Result<()>;
    pub async fn discover_tools(&self) -> Vec<Arc<dyn Tool>>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

**McpServer:**
```rust
pub struct McpServer {
    name: String,
    config: McpServerConfig,
    process: Child,
    client: McpClient,
    tools: Vec<McpToolInfo>,
}

impl McpServer {
    pub async fn start(config: McpServerConfig) -> Result<Self>;
    pub async fn discover_tools(&mut self) -> Result<Vec<McpToolInfo>>;
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: serde_json::Value,
    ) -> Result<ToolResult>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

**McpToolWrapper:**
```rust
pub struct McpToolWrapper {
    server_name: String,
    tool_name: String,
    full_name: String, // "server::tool"
    description: String,
    schema: serde_json::Value,
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
    
    fn parameters_schema(&self) -> serde_json::Value {
        self.schema.clone()
    }
    
    async fn execute(
        &self,
        parameters: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        self.server.execute_tool(&self.tool_name, parameters).await
    }
    
    fn requires_approval(&self) -> bool {
        true // MCP tools require approval by default
    }
}
```

**Integration with Session:**
```rust
impl Session {
    pub fn new(
        event_bus: EventBus,
        model_providers: Vec<Arc<dyn ModelProvider>>,
        agent: Option<Agent>,
    ) -> Result<Self> {
        // Initialize tool registry
        let tool_registry = Arc::new(ToolRegistry::new());
        
        // Register built-in tools
        tool_registry.register(Arc::new(FsReadTool::new()));
        tool_registry.register(Arc::new(FsWriteTool::new()));
        
        // Initialize MCP servers if agent config has them
        let mcp_manager = if let Some(agent) = &agent {
            let manager = McpServerManager::new(agent.mcp_servers.clone()).await?;
            
            // Discover and register MCP tools
            let mcp_tools = manager.discover_tools().await;
            for tool in mcp_tools {
                tool_registry.register(tool);
            }
            
            Some(Arc::new(manager))
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

### Acceptance Criteria

**Phase 1 (Research):**
- Complete understanding of MCP protocol
- Documentation of existing MCP integration
- Clear requirements for agent_env integration

**Phase 2 (Design):**
- Complete MCP integration architecture
- Sequence diagrams for MCP lifecycle
- Clear integration with ToolRegistry
- Design review and approval

**Phase 3 (Implementation):**
- McpServerManager implemented
- MCP servers can be started and stopped
- Tools are discovered from MCP servers
- MCP tools are registered in ToolRegistry
- MCP tools can be executed
- Error handling works correctly
- Shutdown is clean
- Tests passing

### Testing Strategy

```bash
# Create test agent with MCP server
cat > ~/.config/q/agents/mcp-test.yaml << EOF
name: "mcp-test"
mcp_servers:
  - name: "filesystem"
    command: "mcp-server-filesystem"
    args: ["/tmp"]
EOF

# Test MCP tool discovery
q chat --agent mcp-test
> /tools
# (should show filesystem::read_file, filesystem::write_file, etc.)

# Test MCP tool execution
q chat --agent mcp-test "Use the filesystem server to read /tmp/test.txt"

# Test multiple MCP servers
cat > ~/.config/q/agents/multi-mcp.yaml << EOF
name: "multi-mcp"
mcp_servers:
  - name: "filesystem"
    command: "mcp-server-filesystem"
  - name: "git"
    command: "mcp-server-git"
EOF

q chat --agent multi-mcp
> /tools
# (should show tools from both servers)

# Test MCP server failure
# (kill MCP server process during execution)
# (should handle gracefully and report error)

# Test shutdown
q chat --agent mcp-test "hello"
# (exit with Ctrl+C or /quit)
# (verify MCP server processes are terminated)
```

### Estimated Effort

**Total: 20-28 hours**
- Phase 1 (Research): 4-6 hours
- Phase 2 (Design): 6-8 hours
- Phase 3 (Implementation): 10-14 hours

**Note:** This assumes Task 1.3 (Tool System) is already complete.

---

## Task 3.2: Worker-Specific MCP Instances

### Status

**OUT OF MVP SCOPE** - Documented for future work only.

### Overview

Allow individual workers to have their own MCP server instances, separate from shared instances. This enables:
- Isolated environments per worker
- Different tool sets per worker
- Worker-specific configurations
- Parallel execution without conflicts

### Requirements

**Core Functionality:**
- Workers can specify their own MCP servers
- Worker-specific MCP instances are isolated
- Shared MCP instances remain available
- Tool naming distinguishes worker-specific tools
- Lifecycle tied to worker lifecycle

**Use Cases:**
- Worker A uses filesystem MCP for /workspace/project-a
- Worker B uses filesystem MCP for /workspace/project-b
- Both workers operate independently without conflicts

### Challenges

1. **Lifecycle Management:**
   - Start MCP servers when worker is created
   - Stop MCP servers when worker is deleted
   - Handle worker cancellation

2. **Tool Naming:**
   - Distinguish worker-specific tools from shared tools
   - Prevent naming conflicts
   - Example: `worker_id::server::tool`

3. **Resource Management:**
   - Multiple MCP server instances (resource intensive)
   - Process limits
   - Memory usage

4. **Configuration:**
   - How to specify worker-specific MCP servers?
   - API for creating workers with MCP configs?
   - Dynamic MCP server addition/removal?

### Design Considerations

**Worker MCP Configuration:**
```rust
pub struct WorkerConfig {
    pub id: String,
    pub mcp_servers: Vec<McpServerConfig>,
    // Other config
}

impl Session {
    pub fn build_worker_with_config(&self, config: WorkerConfig) -> Arc<Worker> {
        // Initialize worker-specific MCP servers
        let worker_mcp_manager = McpServerManager::new(config.mcp_servers).await?;
        
        // Create worker with dedicated MCP manager
        let worker = Worker {
            id: config.id,
            mcp_manager: Some(Arc::new(worker_mcp_manager)),
            // Other fields
        };
        
        Arc::new(worker)
    }
}
```

**Tool Registry Strategy:**
- Option A: Worker has its own ToolRegistry (shared + worker-specific tools)
- Option B: Single ToolRegistry with worker-scoped tools
- Option C: Hierarchical ToolRegistry (session → worker)

### Estimated Effort

**Total: 15-20 hours** (after Task 3.1 is complete)
- Design: 5-6 hours
- Implementation: 10-14 hours

### Dependencies

- Task 3.1 (MCP Integration) must be complete
- Worker lifecycle management
- Tool system (Task 1.3)

---

## Implementation Order

1. **Task 3.1 first** - Shared MCP instances (MVP)
2. **Task 3.2 later** - Worker-specific MCP (post-MVP)

Task 3.2 is explicitly out of scope for MVP but documented for future planning.

---

## Success Metrics

### Task 3.1 Success
- MCP servers can be initialized from Agent config
- Tools are discovered and registered
- MCP tools can be executed
- Multiple MCP servers work simultaneously
- Error handling is robust
- Clean shutdown

### Task 3.2 Success (Future)
- Workers can have dedicated MCP instances
- Worker-specific tools are isolated
- Lifecycle management works correctly
- No resource leaks

---

## Risks and Mitigation

**Risk 1: MCP Protocol Complexity**
- Mitigation: Leverage existing rmcp crate, thorough research

**Risk 2: Process Management**
- Mitigation: Use tokio::process, proper cleanup, health monitoring

**Risk 3: Tool Schema Translation**
- Mitigation: Flexible schema mapping, validation

**Risk 4: Performance Impact**
- Mitigation: Async execution, connection pooling, lazy initialization

**Risk 5: MCP Server Reliability**
- Mitigation: Error handling, reconnection, graceful degradation

---

## Related Files

**Existing MCP Code:**
- `crates/rmcp/` - MCP client library
- `crates/chat-cli/src/cli/chat/tool_manager.rs` - Current MCP integration
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config with mcp_servers

**Agent Environment:**
- `crates/chat-cli/src/agent_env/session.rs` - Session orchestration
- `crates/chat-cli/src/agent_env/worker.rs` - Worker state
- `crates/chat-cli/src/agent_env/tools/` - Tool system (to be created in Task 1.3)

**New Files (to be created):**
- `crates/chat-cli/src/agent_env/mcp/mod.rs` - MCP integration module
- `crates/chat-cli/src/agent_env/mcp/server_manager.rs` - McpServerManager
- `crates/chat-cli/src/agent_env/mcp/server.rs` - McpServer
- `crates/chat-cli/src/agent_env/mcp/tool_wrapper.rs` - McpToolWrapper

---

## Documentation Updates

After implementation:
- Document MCP integration architecture
- Create MCP server configuration guide
- Document MCP tool naming conventions
- Add MCP troubleshooting guide
- Update Agent config documentation
- Document MCP server development guide

---

## WebUI Considerations

The WebUI (Task 2.1) may need to display MCP-related information:
- List of active MCP servers
- MCP server status (running, failed, reconnecting)
- MCP tools available
- MCP tool execution events

This should be considered in the WebUI design phase.
