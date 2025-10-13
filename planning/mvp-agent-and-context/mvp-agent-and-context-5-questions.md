# MVP Agent and Context - Questions and Answers

## Q: What's the difference between `Agent::get_agent_by_name(os, agent_name)` and `Agents::load(os, self.agent.as_deref(), skip_migration, &mut stderr, mcp_enabled)`?

### Answer

These are two different approaches to loading agent configurations, serving different purposes:

#### `Agent::get_agent_by_name(os, agent_name)` 
**Location:** `crates/chat-cli/src/cli/agent/mod.rs:290`

**Purpose:** Load a single, specific agent by name

**Signature:**
```rust
pub async fn get_agent_by_name(os: &Os, agent_name: &str) -> eyre::Result<(Agent, PathBuf)>
```

**Behavior:**
- Takes a specific agent name as input
- Searches local directory first (`./.amazonq/agents/`)
- Falls back to global directory (`~/.config/q/agents/`)
- Returns the first matching agent found
- Returns `Result<(Agent, PathBuf)>` - either success with agent + path, or error
- Simple, focused operation for loading one agent

**Use Case:** When you know exactly which agent you want (e.g., `--agent rust-expert`)

#### `Agents::load(os, agent_name, skip_migration, output, mcp_enabled)`
**Location:** `crates/chat-cli/src/cli/agent/mod.rs:448`

**Purpose:** Load and manage the entire agent ecosystem with migration and validation

**Signature:**
```rust
pub async fn load(
    os: &mut Os,
    agent_name: Option<&str>,
    skip_migration: bool,
    output: &mut impl Write,
    mcp_enabled: bool,
) -> (Self, AgentsLoadMetadata)
```

**Behavior:**
- Loads ALL agents from both local and global directories
- Performs legacy profile migration if `skip_migration` is false
- Validates all agent configs and reports errors to `output`
- Handles MCP (Model Context Protocol) configuration
- Returns `Agents` collection + metadata about the load operation
- Tracks migration status, load failures, etc.
- More complex operation with side effects (migration, validation, output)

**Use Case:** 
- Legacy chat flow that needs full agent ecosystem
- Agent management commands (`q agent list`, `q agent validate`)
- When you need migration and validation of all agents

### Key Differences

| Aspect | `Agent::get_agent_by_name()` | `Agents::load()` |
|--------|------------------------------|------------------|
| **Scope** | Single agent | All agents |
| **Migration** | No | Yes (optional) |
| **Validation** | Basic (parse only) | Full validation with reporting |
| **Output** | Silent | Writes warnings/errors to output |
| **Return Type** | `Result<(Agent, PathBuf)>` | `(Agents, AgentsLoadMetadata)` |
| **Error Handling** | Fails on error | Collects errors, continues loading |
| **MCP Handling** | No | Yes |
| **Use in agent_env** | ✅ Used by WorkerBuilder | ❌ Not used |

### Why WorkerBuilder Uses `Agent::get_agent_by_name()`

The new agent_env architecture uses `Agent::get_agent_by_name()` because:

1. **Simplicity** - Only needs one agent at a time
2. **Fast** - Doesn't load unnecessary agents
3. **Clean errors** - Fails fast with clear error message
4. **No side effects** - No migration, no output writing
5. **Async-friendly** - Simple async function, easy to await

The `Agents::load()` approach is more suitable for the legacy chat flow where:
- Multiple agents might be managed in a session
- Migration from old profiles is needed
- Full validation and error reporting is required
- MCP configuration needs to be loaded globally

### Recommendation

Continue using `Agent::get_agent_by_name()` in WorkerBuilder. It's the right choice for the agent_env architecture.
