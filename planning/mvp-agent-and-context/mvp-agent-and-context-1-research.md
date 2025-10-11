# MVP Agent and Context - Research and Analysis

## Overview

This document contains research findings and analysis for implementing Task 1.2 (History Accumulation) and Task 1.4 (Agent Context Loading) in the agent_env architecture.

---

## Task 1.2: History Accumulation - Research

### Current Implementation Analysis

#### ConversationHistory Structure

**Location:** `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`

```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,
}
```

**Key Methods:**
- `push_input_message(content: String)` - Adds user message
- `push_assistant_message(assistant: AssistantMessage)` - Adds assistant message
- `get_entries() -> &[ConversationEntry]` - Returns all entries
- `len()` / `is_empty()` - Size queries

**Observations:**
- ✅ History storage is already structured for multi-turn conversations
- ✅ Both user and assistant messages can be stored
- ✅ Thread-safe with Arc<Mutex<>>
- ✅ AgentLoop already appends assistant responses after completion

#### ConversationEntry Structure

**Location:** `crates/chat-cli/src/agent_env/context_container/conversation_entry.rs`

```rust
pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

**Observations:**
- Each entry can contain either user or assistant message (not both)
- UserMessage and AssistantMessage are rich structures from the old chat system
- Contains metadata like timestamps, images, tool use results

#### AgentLoop Current Behavior

**Location:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Current query_llm() implementation:**
```rust
let last_entry = history.get_entries().last()
    .ok_or_else(|| eyre::eyre!("No messages in history"))?;

match &last_entry.user {
    Some(user_msg) => match user_msg.content() {
        UserMessageContent::Prompt { prompt } => prompt.clone(),
        _ => return Err(eyre::eyre!("Expected prompt message")),
    },
    None => return Err(eyre::eyre!("Last entry is not a user message")),
}
```

**Observations:**
- ❌ Only reads the last entry
- ❌ Only extracts the prompt string from the last user message
- ❌ Ignores all previous conversation context
- ✅ Already appends assistant responses to history after completion

#### ModelProvider Interface

**Location:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

**Current ModelRequest structure:**
```rust
pub struct ModelRequest {
    pub prompt: String,  // ❌ Single string, not message array
}
```

**ModelProvider trait:**
```rust
async fn request(
    &self,
    request: ModelRequest,
    when_receiving_begin: Box<dyn Fn() + Send>,
    when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
    cancellation_token: CancellationToken,
) -> Result<ModelResponse, eyre::Error>;
```

**Observations:**
- ❌ ModelRequest only supports a single prompt string
- ❌ No support for message arrays or conversation history
- ⚠️ This is a fundamental interface - changes affect all implementations

#### BedrockConverseStreamModelProvider

**Location:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Current implementation:**
```rust
let message = Message::builder()
    .role(ConversationRole::User)
    .content(ContentBlock::Text(request.prompt))
    .build()?;

let response = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .messages(message)  // ❌ Single message only
    .send()
```

**Observations:**
- ❌ Only sends a single user message to Bedrock
- ❌ No support for conversation history
- ⚠️ Bedrock Converse API supports multiple messages via `.messages()` method
- ⚠️ Bedrock requires alternating user/assistant roles

### Critical Issues Identified

#### Issue 1: ModelRequest Structure Too Simple

**Problem:** ModelRequest only contains a single prompt string, not a message array.

**Impact:** 
- Cannot pass conversation history to model providers
- Fundamental interface change required
- Affects all ModelProvider implementations

**Solution Approach:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // NEW
}

pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum MessageRole {
    User,
    Assistant,
    System,  // For future use
}
```

#### Issue 2: Bedrock API Message Format Requirements

**Problem:** Bedrock Converse API has specific requirements for message sequences.

**Requirements:**
- Messages must alternate between user and assistant roles
- Cannot have consecutive messages with the same role
- System messages (if supported) have special handling

**Research Needed:**
- Does Bedrock Converse API support system messages?
- How to handle system prompts for agent context?
- What happens if history doesn't alternate properly?

**Mitigation:**
- Validate message sequence before sending to Bedrock
- Merge consecutive same-role messages if needed
- Add system message support investigation

#### Issue 3: UserMessage and AssistantMessage Complexity

**Problem:** ConversationEntry uses complex message structures from old chat system.

**Current structures:**
- UserMessage has: additional_context, env_context, content (enum), timestamp, images
- AssistantMessage has: tool uses, responses, metadata
- UserMessageContent is an enum: Prompt, CancelledToolUses, ToolUseResults

**Impact:**
- Need to extract text content from complex structures
- Tool use messages need special handling
- Images and metadata need to be preserved or discarded

**Solution Approach:**
- For MVP: Extract only text content from Prompt messages
- Ignore tool use messages for now (will be handled in mvp-tools-basic)
- Future: Properly format tool use/result messages for Bedrock

#### Issue 4: Token Limits and History Truncation

**Problem:** Long conversation histories can exceed model token limits.

**Considerations:**
- Bedrock models have token limits (e.g., Claude Sonnet: 200K input tokens)
- Need strategy for handling long conversations
- Should we truncate, summarize, or error?

**Solution Approach (for MVP):**
- Send entire history without truncation
- Let Bedrock handle token limit errors
- Future: Implement sliding window or summarization

### Implementation Requirements

#### 1. Update ModelRequest Structure

**File:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

**Changes:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
}

pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum MessageRole {
    User,
    Assistant,
}
```

#### 2. Update AgentLoop::query_llm()

**File:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Changes:**
- Read entire conversation history (not just last entry)
- Convert ConversationEntry entries to ConversationMessage format
- Extract text content from UserMessage and AssistantMessage
- Build ModelRequest with message array
- Validate message sequence (alternating roles)

**Pseudo-code:**
```rust
let entries = history.get_entries();
let mut messages = Vec::new();

for entry in entries {
    if let Some(user_msg) = &entry.user {
        if let UserMessageContent::Prompt { prompt } = user_msg.content() {
            messages.push(ConversationMessage {
                role: MessageRole::User,
                content: prompt.clone(),
            });
        }
    }
    if let Some(assistant_msg) = &entry.assistant {
        // Extract text from assistant message
        let content = extract_assistant_text(assistant_msg);
        messages.push(ConversationMessage {
            role: MessageRole::Assistant,
            content,
        });
    }
}

let request = ModelRequest { messages };
```

#### 3. Update BedrockConverseStreamModelProvider

**File:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Changes:**
- Accept ModelRequest with message array
- Convert ConversationMessage to Bedrock Message format
- Send all messages to Bedrock Converse API
- Handle role conversion (MessageRole -> ConversationRole)

**Pseudo-code:**
```rust
let messages: Vec<Message> = request.messages
    .iter()
    .map(|msg| {
        let role = match msg.role {
            MessageRole::User => ConversationRole::User,
            MessageRole::Assistant => ConversationRole::Assistant,
        };
        Message::builder()
            .role(role)
            .content(ContentBlock::Text(msg.content.clone()))
            .build()
    })
    .collect::<Result<Vec<_>, _>>()?;

let response = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .set_messages(Some(messages))  // Send all messages
    .send()
```

### Testing Strategy

#### Unit Tests

1. **Test history accumulation:**
   - Add user message, get response, add another user message
   - Verify second request includes first exchange

2. **Test message extraction:**
   - Create ConversationEntry with UserMessage
   - Verify correct text extraction
   - Test with AssistantMessage

3. **Test role alternation:**
   - Create history with alternating messages
   - Verify ModelRequest has correct sequence

#### Integration Tests

1. **Multi-turn conversation:**
   ```bash
   q chat
   > What is 2+2?
   # (wait for response: "4")
   > What about the previous number plus 1?
   # (should respond: "5")
   ```

2. **Context preservation:**
   ```bash
   q chat
   > My name is Alice
   # (wait for response)
   > What is my name?
   # (should respond: "Alice")
   ```

### Potential Pitfalls

1. **Message Role Validation:**
   - Bedrock may reject non-alternating messages
   - Need to handle edge cases (empty history, consecutive same-role messages)

2. **Content Extraction:**
   - UserMessage has complex content enum
   - AssistantMessage may have multiple response parts
   - Need robust extraction logic

3. **Thread Safety:**
   - ConversationHistory is behind Arc<Mutex<>>
   - Need to minimize lock duration
   - Avoid deadlocks with proper lock scoping

4. **Backward Compatibility:**
   - ModelRequest change affects all ModelProvider implementations
   - Need to update all providers (currently only Bedrock)
   - Future providers must support message arrays

---

## Task 1.4: Agent Context Loading - Research

### Current Implementation Analysis

#### Agent Configuration System

**Location:** `crates/chat-cli/src/cli/agent/mod.rs`

**Agent structure:**
```rust
pub struct Agent {
    pub name: String,
    pub description: Option<String>,
    pub prompt: Option<String>,
    pub resources: Vec<ResourcePath>,  // ✅ Context files
    pub mcp_servers: McpServerConfig,
    pub tools: Vec<String>,
    pub tool_aliases: HashMap<OriginalToolName, String>,
    pub allowed_tools: HashSet<String>,
    pub hooks: HashMap<HookTrigger, Vec<Hook>>,
    pub tools_settings: HashMap<ToolSettingTarget, serde_json::Value>,
    pub use_legacy_mcp_json: bool,
    pub model: Option<String>,
    pub path: Option<PathBuf>,
}
```

**Observations:**
- ✅ Agent config system is mature and well-structured
- ✅ `resources` field supports file paths for context
- ✅ `prompt` field for high-level context (like system prompt)
- ✅ Agent loading infrastructure exists
- ❌ No integration with agent_env architecture

#### ResourcePath Type

**Location:** `crates/chat-cli/src/cli/agent/wrapper_types.rs` (inferred)

**Format:**
- Supports `file://` URLs
- Supports glob patterns (e.g., `file://.amazonq/rules/**/*.md`)
- Examples from default agent:
  ```rust
  resources: vec![
      "file://AmazonQ.md",
      "file://AGENTS.md",
      "file://README.md",
      "file://.amazonq/rules/**/*.md",
  ]
  ```

**Observations:**
- ✅ Flexible path specification
- ✅ Glob pattern support for multiple files
- ⚠️ Need glob expansion library (likely `glob` crate)
- ⚠️ Relative paths need resolution (relative to what?)

#### ContextContainer Structure

**Location:** `crates/chat-cli/src/agent_env/context_container/context_container.rs`

**Current structure:**
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    // ❌ No field for agent context
}
```

**Observations:**
- ❌ No dedicated field for agent context
- ❌ No system prompt storage
- ⚠️ Need to add context storage capability

#### ChatArgs Integration

**Location:** `crates/chat-cli/src/cli/chat/mod.rs`

**Current ChatArgs::execute():**
```rust
pub struct ChatArgs {
    pub agent: Option<String>,  // ✅ --agent flag exists
    // ...
}

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Creates EventBus, Session, Worker
        // ❌ No agent loading
        // ❌ No resource reading
        // ❌ No context injection
    }
}
```

**Observations:**
- ✅ --agent flag is defined
- ❌ Not used in agent_env flow
- ❌ Agent loading not integrated

### Critical Issues Identified

#### Issue 1: No Agent Context Storage in ContextContainer

**Problem:** ContextContainer has no field for storing agent context.

**Impact:**
- Cannot store loaded context from agent resources
- Cannot pass context to model provider

**Solution Approach:**
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_context: Arc<Mutex<Option<String>>>,  // NEW
    pub system_prompt: Arc<Mutex<Option<String>>>,  // NEW (for agent.prompt)
}
```

#### Issue 2: Context Injection Strategy Unclear

**Problem:** Multiple options for where to inject context, each with tradeoffs.

**Option A: System Prompt**
- Add context as system message in ModelRequest
- Pros: Clear separation, always present, doesn't pollute conversation
- Cons: Bedrock API system prompt support unclear, counts against token limit
- Research needed: Does Bedrock Converse API support system messages?

**Option B: Prepend to First User Message**
- Add context before first user prompt
- Pros: Simple, works with current API
- Cons: Pollutes conversation history, confusing structure

**Option C: Separate Context Field in ModelRequest**
- Add dedicated context field to ModelRequest
- ModelProvider includes it in every request
- Pros: Clean separation, flexible formatting
- Cons: Requires ModelRequest changes (already needed for Task 1.2)

**Recommendation:** Option C (separate context field) or Option A (system prompt) depending on Bedrock API capabilities.

#### Issue 3: Bedrock System Prompt Support Unknown

**Problem:** Need to research if Bedrock Converse API supports system prompts.

**Research Questions:**
1. Does `converse_stream()` accept system messages?
2. Is there a separate system prompt parameter?
3. How are system messages formatted?
4. Are system messages counted in token limits?

**Investigation Needed:**
- Check AWS SDK documentation for BedrockRuntime
- Look for `system` or `system_prompt` parameters
- Test with sample system message

**Fallback:** If system prompts not supported, use Option C (context field in ModelRequest).

#### Issue 4: Resource Loading Implementation

**Problem:** Need to implement file reading and glob expansion.

**Requirements:**
1. Parse `file://` URLs (strip prefix)
2. Resolve relative paths (relative to what? CWD? Agent config location?)
3. Expand glob patterns
4. Read file contents
5. Handle errors (file not found, permission denied, invalid glob)
6. Combine multiple files into single context string

**Dependencies:**
- `glob` crate for pattern expansion
- `std::fs` for file reading
- Error handling for I/O failures

**Implementation Approach:**
```rust
fn load_agent_resources(resources: &[ResourcePath], base_path: &Path) -> Result<String> {
    let mut context = String::new();
    
    for resource in resources {
        let path_str = resource.strip_prefix("file://")
            .ok_or_else(|| eyre::eyre!("Invalid resource path"))?;
        
        // Resolve relative to base_path (agent config directory)
        let resolved_path = if Path::new(path_str).is_relative() {
            base_path.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        
        // Handle glob patterns
        if path_str.contains('*') {
            for entry in glob::glob(resolved_path.to_str().unwrap())? {
                let file_path = entry?;
                let content = std::fs::read_to_string(&file_path)?;
                context.push_str(&format!("\n--- {} ---\n", file_path.display()));
                context.push_str(&content);
            }
        } else {
            // Single file
            let content = std::fs::read_to_string(&resolved_path)?;
            context.push_str(&format!("\n--- {} ---\n", resolved_path.display()));
            context.push_str(&content);
        }
    }
    
    Ok(context)
}
```

#### Issue 5: Context Size and Token Limits

**Problem:** Large context files could exceed token limits.

**Considerations:**
- Agent resources could include large codebases
- Glob patterns could match hundreds of files
- Need strategy for handling large contexts

**Solution Approach (for MVP):**
- Load all resources without truncation
- Let model provider handle token limits
- Future: Add file size limits, truncation, or summarization

**Recommendations:**
- Warn if context exceeds certain size (e.g., 100KB)
- Consider limiting number of files from glob patterns
- Document best practices for resource selection

### Implementation Requirements

#### 1. Enhance ContextContainer

**File:** `crates/chat-cli/src/agent_env/context_container/context_container.rs`

**Changes:**
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_context: Arc<Mutex<Option<String>>>,
    pub system_prompt: Arc<Mutex<Option<String>>>,
}

impl ContextContainer {
    pub fn set_agent_context(&self, context: String) {
        *self.agent_context.lock().unwrap() = Some(context);
    }
    
    pub fn get_agent_context(&self) -> Option<String> {
        self.agent_context.lock().unwrap().clone()
    }
    
    pub fn set_system_prompt(&self, prompt: String) {
        *self.system_prompt.lock().unwrap() = Some(prompt);
    }
    
    pub fn get_system_prompt(&self) -> Option<String> {
        self.system_prompt.lock().unwrap().clone()
    }
}
```

#### 2. Update ModelRequest (if using Option C)

**File:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

**Changes:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,  // NEW
    pub agent_context: Option<String>,  // NEW (or combine with system_prompt)
}
```

**Note:** This combines with Task 1.2 changes.

#### 3. Implement Resource Loading

**File:** `crates/chat-cli/src/cli/chat/mod.rs` (or new module)

**New function:**
```rust
fn load_agent_resources(
    agent: &Agent,
    os: &Os,
) -> Result<String> {
    let base_path = agent.path.as_ref()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    
    let mut context = String::new();
    
    for resource in &agent.resources {
        // Strip file:// prefix
        let path_str = resource.as_str()
            .strip_prefix("file://")
            .unwrap_or(resource.as_str());
        
        // Resolve path
        let resolved = if Path::new(path_str).is_relative() {
            base_path.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        
        // Handle globs
        if path_str.contains('*') {
            for entry in glob::glob(resolved.to_str().unwrap())? {
                let file_path = entry?;
                if file_path.is_file() {
                    let content = std::fs::read_to_string(&file_path)?;
                    context.push_str(&format!("\n--- {} ---\n", file_path.display()));
                    context.push_str(&content);
                }
            }
        } else {
            if resolved.is_file() {
                let content = std::fs::read_to_string(&resolved)?;
                context.push_str(&format!("\n--- {} ---\n", resolved.display()));
                context.push_str(&content);
            }
        }
    }
    
    Ok(context)
}
```

#### 4. Integrate Agent Loading in ChatArgs::execute()

**File:** `crates/chat-cli/src/cli/chat/mod.rs`

**Changes:**
```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // ... existing EventBus, Session creation ...
        
        // NEW: Load agent if specified
        let agent = if let Some(agent_name) = &self.agent {
            Some(Agent::load(agent_name, os)?)
        } else {
            None
        };
        
        // Create main Worker
        let main_worker = session.build_worker("main".to_string());
        
        // NEW: Load and inject agent context
        if let Some(agent) = &agent {
            // Load resources
            let context = load_agent_resources(agent, os)?;
            main_worker.context_container.set_agent_context(context);
            
            // Set system prompt if provided
            if let Some(prompt) = &agent.prompt {
                main_worker.context_container.set_system_prompt(prompt.clone());
            }
        }
        
        // ... rest of initialization ...
    }
}
```

#### 5. Update AgentLoop to Include Context

**File:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Changes in query_llm():**
```rust
// Build messages from history (Task 1.2)
let messages = build_messages_from_history(&history);

// Get agent context and system prompt
let agent_context = self.worker.context_container.get_agent_context();
let system_prompt = self.worker.context_container.get_system_prompt();

// Combine system prompt and agent context
let combined_system = match (system_prompt, agent_context) {
    (Some(prompt), Some(context)) => Some(format!("{}\n\n{}", prompt, context)),
    (Some(prompt), None) => Some(prompt),
    (None, Some(context)) => Some(context),
    (None, None) => None,
};

let request = ModelRequest {
    messages,
    system_prompt: combined_system,
};
```

#### 6. Update BedrockConverseStreamModelProvider

**File:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Research needed:** Check if Bedrock supports system prompts.

**If supported:**
```rust
let mut builder = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .set_messages(Some(messages));

if let Some(system) = request.system_prompt {
    builder = builder.system(/* format system message */);
}

let response = builder.send().await?;
```

**If not supported:** Prepend system prompt to first user message as workaround.

### Testing Strategy

#### Unit Tests

1. **Test resource loading:**
   - Create test files
   - Load with file:// paths
   - Verify content

2. **Test glob expansion:**
   - Create multiple test files
   - Use glob pattern
   - Verify all files loaded

3. **Test context injection:**
   - Set agent context in ContextContainer
   - Verify retrieval
   - Test with system prompt

#### Integration Tests

1. **Agent with context:**
   ```bash
   # Create test agent
   cat > ~/.config/q/agents/test-agent.yaml << EOF
   name: "test-agent"
   prompt: "You are a helpful assistant."
   resources:
     - file:///tmp/test-context.txt
   EOF
   
   echo "Important context: The answer is 42" > /tmp/test-context.txt
   
   q chat --agent test-agent "What is the answer?"
   # Should reference context in response
   ```

2. **Glob patterns:**
   ```bash
   cat > ~/.config/q/agents/glob-agent.yaml << EOF
   name: "glob-agent"
   resources:
     - file:///tmp/docs/*.md
   EOF
   
   echo "# Doc 1" > /tmp/docs/doc1.md
   echo "# Doc 2" > /tmp/docs/doc2.md
   
   q chat --agent glob-agent "List the documents you have"
   # Should mention both doc1 and doc2
   ```

### Potential Pitfalls

1. **Path Resolution:**
   - Relative paths need proper base directory
   - Agent config location vs CWD
   - Cross-platform path handling (Windows vs Unix)

2. **Glob Pattern Issues:**
   - Invalid patterns should error gracefully
   - Large number of matches could cause performance issues
   - Recursive patterns (**) need careful handling

3. **File Reading Errors:**
   - File not found
   - Permission denied
   - Binary files vs text files
   - Large files causing memory issues

4. **Context Formatting:**
   - Multiple files need clear separation
   - File path headers for clarity
   - Markdown formatting considerations

5. **Token Limits:**
   - Large contexts could exceed model limits
   - Need monitoring and warnings
   - Consider truncation strategy

---

## Cross-Task Considerations

### ModelRequest Changes

Both tasks require changes to ModelRequest:
- Task 1.2: Add messages array
- Task 1.4: Add system_prompt/agent_context

**Combined structure:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,
}
```

### Implementation Order

**Recommendation:**
1. **Task 1.2 first** - Establishes message array foundation
2. **Task 1.4 second** - Adds context on top of working multi-turn conversations

**Rationale:**
- Task 1.2 is more fundamental (multi-turn conversations)
- Task 1.4 builds on Task 1.2's ModelRequest changes
- Testing is easier with working conversations first

### Shared Code

Both tasks will modify:
- `model_provider.rs` - ModelRequest structure
- `agent_loop.rs` - Request building
- `bedrock_converse_stream.rs` - Request sending

Need careful coordination to avoid conflicts.

---

## Research Questions for Design Phase

### Bedrock API Questions

1. **System Prompt Support:**
   - Does Bedrock Converse API support system messages?
   - What is the parameter name and format?
   - Are system messages counted in token limits?

2. **Message Format:**
   - Are there restrictions on message sequences?
   - Can we send empty messages?
   - How are tool use messages formatted?

3. **Token Limits:**
   - What are the exact token limits for Claude Sonnet?
   - How are tokens counted (input vs output)?
   - What error is returned when limit exceeded?

### Architecture Questions

1. **Context Caching:**
   - Should agent context be cached?
   - Should it be reloaded on file changes?
   - How to handle context updates during conversation?

2. **Context Scope:**
   - Should context be per-worker or per-session?
   - Can different workers have different contexts?
   - How to handle context in multi-agent scenarios?

3. **Error Handling:**
   - How to handle resource loading failures?
   - Should missing files be warnings or errors?
   - How to report context size issues?

---

## Key Files for Design Phase

### Task 1.2 Files

- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelRequest changes
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - History reading
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Message sending
- `crates/chat-cli/src/agent_env/context_container/conversation_history.rs` - History access
- `crates/chat-cli/src/agent_env/context_container/conversation_entry.rs` - Entry structure

### Task 1.4 Files

- `crates/chat-cli/src/agent_env/context_container/context_container.rs` - Context storage
- `crates/chat-cli/src/cli/chat/mod.rs` - Agent loading integration
- `crates/chat-cli/src/cli/agent/mod.rs` - Agent config structure
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - Context injection
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - System prompt handling

---

## Summary

### Task 1.2: History Accumulation

**Current State:** AgentLoop only sends last message; history storage exists but unused.

**Key Changes Needed:**
1. Update ModelRequest to support message arrays
2. Modify AgentLoop to read entire history
3. Update BedrockConverseStreamModelProvider to send all messages
4. Handle message role alternation

**Main Challenge:** ModelRequest interface change affects all providers.

**Estimated Complexity:** Medium (fundamental interface change, but clear path forward)

### Task 1.4: Agent Context Loading

**Current State:** Agent config system exists; no integration with agent_env.

**Key Changes Needed:**
1. Add context storage to ContextContainer
2. Implement resource loading (file reading, glob expansion)
3. Integrate agent loading in ChatArgs::execute()
4. Inject context into ModelRequest (system prompt or separate field)
5. Update BedrockConverseStreamModelProvider to handle context

**Main Challenge:** Bedrock system prompt support unclear; resource loading implementation.

**Estimated Complexity:** Medium (multiple components, but mostly additive changes)

### Combined Effort

Both tasks are independent but share ModelRequest changes. Implementing Task 1.2 first provides foundation for Task 1.4. Total estimated effort: 7-10 hours for both tasks.
