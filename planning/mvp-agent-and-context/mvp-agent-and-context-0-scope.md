# MVP Agent and Context - Scope

## Corrections provided by the team's senior SDE - pay attention, update this scope doc

- There is existing Agent configuration model crates/chat-cli/src/cli/agent/mod.rs that MUST be reused
- There is existing code infrastructure that can identify available Agent Configs and obtain one by name, or provide default value. It MUST be reused.
- Entry point code must delegate building a default worker to new WorkerBuilder class, that would take some of the ChatArgs (agent, platform, model, input) and build a worker. Main focus will be on building ContextContainer and its content
    - in the following iterations we will also pass 'resume' to load ConversationHistory from the database, and 'trust_tools' to setul tools provider layer (no trust_all_tools, this one is a mistake)
- History accumulation AND context collection must work properly on AgentLoopTask level, and sent to ModelProvides through updated ModelRequest structure.
- AgetLoopTask must delegate that to new class ContextBuilder, that takes ContextContainer and produces ModelRequest
- BOTH existing ModelProviders must be able to handle updated ModelRequest properly - Bedrock and Codewhisperer

----


## Overview

This workflow covers context management and agent configuration integration. It includes accumulating conversation history for multi-turn interactions and loading agent-specific context from configuration files.


## Tasks Included

### 1.4: --agent Context Loading
### 1.2: History Accumulation (passing history to LLM)

---

## Task 1.2: History Accumulation

### Current State

- AgentLoop queries conversation_history for the last entry only
- ModelRequest is built with only the most recent user message
- No mechanism to append assistant responses back to conversation_history
- ConversationHistory has push methods but they're not integrated with AgentLoop
- Multi-turn conversations don't maintain context

**Current Flow:**
```rust
// AgentLoop::query_llm() - CURRENT
let last_entry = worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .last_entry();

// Only sends last message to LLM
let request = ModelRequest {
    messages: vec![last_entry],
    ...
};
```

### Requirements

**Core Functionality:**
- Read entire conversation_history (not just last entry)
- Format all entries into ModelRequest for LLM
- Append assistant responses to conversation_history after completion
- Maintain proper user/assistant message sequence
- Preserve history across multiple agent loop invocations
- Support multi-turn conversations with full context

**Conversation Flow:**
```
User: "What is 2+2?"
Assistant: "2+2 equals 4."
User: "What about 3+3?"
Assistant: "3+3 equals 6." (should have context from previous exchange)
```

**History Format:**
- User messages: ConversationEntry with user content
- Assistant messages: ConversationEntry with assistant content
- Proper ordering: chronological sequence maintained
- Thread safety: proper locking for concurrent access

### Implementation Approach

1. **Modify AgentLoop::query_llm():**
   - Read entire conversation_history instead of last entry
   - Format all entries into message array for ModelRequest
   - Ensure proper message role sequence (user/assistant alternation)

2. **Modify AgentLoop::run():**
   - After receiving ModelResponse, extract assistant message
   - Create ConversationEntry for assistant response
   - Append to Worker's conversation_history
   - Ensure proper mutex locking

3. **Update ModelProvider Interface:**
   - Ensure ModelRequest supports message arrays
   - Verify BedrockConverseStreamModelProvider handles multiple messages
   - Format messages correctly for Bedrock API

4. **Handle Edge Cases:**
   - Empty history (first message)
   - System messages vs user/assistant messages
   - Tool use messages in history
   - Large history (potential truncation strategy)

### Technical Details

**ConversationHistory Structure:**
```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,
}

pub enum ConversationEntry {
    UserMessage(String),
    AssistantMessage(String),
    // Future: ToolUse, ToolResult, etc.
}
```

**Updated AgentLoop Flow:**
```rust
// Read entire history
let history = worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .get_all_entries();

// Format for ModelRequest
let messages: Vec<Message> = history.iter()
    .map(|entry| match entry {
        ConversationEntry::UserMessage(text) => Message::User(text.clone()),
        ConversationEntry::AssistantMessage(text) => Message::Assistant(text.clone()),
    })
    .collect();

// Send to LLM
let request = ModelRequest { messages, ... };
let response = model_provider.query(request).await?;

// Append response to history
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_assistant_message(response.content);
```

### Acceptance Criteria

- Entire conversation history is sent to LLM on each request
- Assistant responses are appended to conversation_history
- Multi-turn conversations maintain context correctly
- /context command shows full conversation history
- No race conditions or deadlocks with mutex locking
- History persists across multiple agent loop invocations
- Proper message ordering maintained

### Testing Strategy

```bash
# Test multi-turn conversation
q chat
> What is 2+2?
# (wait for response)
> What about the previous number plus 1?
# (should understand "previous number" refers to 4)

# Test history persistence
q chat
> My name is Alice
# (wait for response)
> What is my name?
# (should respond with Alice)

# Test /context command
q chat
> Hello
> How are you?
> /context
# (should show both user messages and assistant responses)
```

### Dependencies

None - can be implemented immediately

### Estimated Effort

3-4 hours

---

## Task 1.4: --agent Context Loading

### Current State

- Agent config system exists in `cli/agent/mod.rs`
- Agent has `resources` field for context files
- Agent config supports file:// paths and glob patterns
- No integration with agent_env Worker
- Context loading happens in old chat flow (not agent_env)

**Agent Config Example:**
```yaml
name: "rust-expert"
description: "Expert in Rust programming"
resources:
  - file:///path/to/rust-guidelines.md
  - file:///path/to/project/**/*.rs
mcp_servers:
  - name: "filesystem"
    command: "mcp-server-filesystem"
```

### Requirements

**Core Functionality:**
- Load Agent config when --agent flag is provided
- Read and parse resource files from Agent config
- Support file:// paths (single files)
- Support glob patterns (multiple files)
- Inject context into Worker's ContextContainer
- Make context available to LLM in requests

**Context Injection Options:**

**Option A: System Prompt**
- Add context as system message at start of conversation
- Pros: Clear separation, always present
- Cons: Counts against token limit, not part of conversation flow

**Option B: Initial User Message**
- Prepend context to first user message
- Pros: Part of conversation flow
- Cons: May confuse conversation structure

**Option C: Separate Context Field**
- Add dedicated context field to ContextContainer
- ModelProvider includes context in every request
- Pros: Clean separation, flexible formatting
- Cons: Requires ModelProvider changes

### Implementation Approach

1. **Load Agent Config:**
   - In ChatArgs::execute(), check for --agent flag
   - Load Agent from config file using existing Agent::load()
   - Extract resources list from Agent

2. **Read Resource Files:**
   - Iterate through resources list
   - Handle file:// URLs (strip prefix, resolve path)
   - Handle glob patterns (expand to file list)
   - Read file contents
   - Handle errors (file not found, permission denied)

3. **Format Context:**
   - Combine multiple files into single context string
   - Add file path headers for clarity
   - Consider truncation for large files
   - Format as markdown or plain text

4. **Inject Context:**
   - Add to Worker's ContextContainer (Option C preferred)
   - Or prepend to conversation_history (Option B)
   - Or add as system prompt (Option A)

5. **Update ModelProvider:**
   - Ensure context is included in ModelRequest
   - Format appropriately for Bedrock API
   - Handle context + conversation history combination

### Technical Details

**Resource Loading:**
```rust
// Load agent config
let agent = if let Some(agent_name) = &self.agent {
    Some(Agent::load(agent_name, os)?)
} else {
    None
};

// Read resources
if let Some(agent) = &agent {
    let context = load_agent_resources(&agent.resources)?;
    
    // Inject into worker
    main_worker.context_container
        .set_context(context);
}

fn load_agent_resources(resources: &[String]) -> Result<String> {
    let mut context = String::new();
    
    for resource in resources {
        if resource.starts_with("file://") {
            let path = resource.strip_prefix("file://").unwrap();
            
            // Handle glob patterns
            if path.contains('*') {
                for entry in glob::glob(path)? {
                    let file_path = entry?;
                    let content = std::fs::read_to_string(&file_path)?;
                    context.push_str(&format!("\n--- {} ---\n", file_path.display()));
                    context.push_str(&content);
                }
            } else {
                // Single file
                let content = std::fs::read_to_string(path)?;
                context.push_str(&format!("\n--- {} ---\n", path));
                context.push_str(&content);
            }
        }
    }
    
    Ok(context)
}
```

**ContextContainer Enhancement:**
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_context: Arc<Mutex<Option<String>>>, // NEW
}

impl ContextContainer {
    pub fn set_context(&self, context: String) {
        *self.agent_context.lock().unwrap() = Some(context);
    }
    
    pub fn get_context(&self) -> Option<String> {
        self.agent_context.lock().unwrap().clone()
    }
}
```

### Research Questions

1. **Context Placement:**
   - Where should context be injected in the request?
   - System prompt vs user message vs separate field?
   - How does Bedrock handle system prompts?

2. **Context Size:**
   - What are token limits for context?
   - Should we truncate large files?
   - Should we summarize or chunk context?

3. **Context Updates:**
   - Should context be reloaded on each request?
   - Should context be cached?
   - How to handle file changes during conversation?

4. **Glob Patterns:**
   - What glob library to use? (glob crate)
   - How to handle large numbers of matched files?
   - Should we limit number of files?

### Acceptance Criteria

- --agent flag loads correct Agent config
- Resource files are read and parsed
- Glob patterns expand correctly
- Context is injected into Worker
- Context appears in LLM requests
- /context command shows agent context
- Error handling for missing files
- Error handling for invalid glob patterns

### Testing Strategy

```bash
# Create test agent config
cat > ~/.config/q/agents/test-agent.yaml << EOF
name: "test-agent"
description: "Test agent with context"
resources:
  - file:///tmp/test-context.txt
EOF

# Create test context file
echo "This is test context" > /tmp/test-context.txt

# Test agent loading
q chat --agent test-agent "What context do you have?"

# Test glob patterns
cat > ~/.config/q/agents/glob-agent.yaml << EOF
name: "glob-agent"
resources:
  - file:///tmp/test-*.txt
EOF

echo "File 1" > /tmp/test-1.txt
echo "File 2" > /tmp/test-2.txt

q chat --agent glob-agent "List the files you have context from"

# Test /context command
q chat --agent test-agent
> /context
# (should show agent context)
```

### Dependencies

- Agent config loading (existing)
- Glob pattern support (glob crate)
- File reading (std::fs)

### Estimated Effort

4-6 hours (includes research on context placement)

---

## Task Dependencies

- Task 1.2 (History) is independent - can be implemented first
- Task 1.4 (Context) is independent - can be implemented in parallel
- Both tasks enhance ContextContainer but don't conflict

---

## Implementation Order

**Recommended:**
1. **Task 1.2 first** - Foundation for multi-turn conversations
2. **Task 1.4 second** - Builds on conversation foundation

**Alternative (parallel):**
- Both tasks can be implemented simultaneously by different developers
- They modify different parts of the system

---

## Success Metrics

### Task 1.2 Success
- Multi-turn conversations work correctly
- History is maintained across invocations
- /context shows full conversation
- No memory leaks or race conditions

### Task 1.4 Success
- Agent configs load successfully
- Resources are read and injected
- Context is available to LLM
- Glob patterns work correctly
- Error handling is robust

---

## Related Files

**Task 1.2:**
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - AgentLoop implementation
- `crates/chat-cli/src/agent_env/context_container/conversation_history.rs` - History management
- `crates/chat-cli/src/agent_env/context_container/conversation_entry.rs` - Entry types
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelProvider trait
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Bedrock implementation

**Task 1.4:**
- `crates/chat-cli/src/cli/chat/mod.rs` - ChatArgs::execute()
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config loading
- `crates/chat-cli/src/agent_env/context_container/context_container.rs` - Context storage
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelProvider trait

---

## Documentation Updates

After implementation:
- Update Agent config documentation with resource examples
- Document context injection strategy
- Add examples of multi-turn conversations
- Update ContextContainer documentation
- Add troubleshooting guide for context loading
