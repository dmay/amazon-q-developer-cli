# MVP Agent and Context - Scope (v2)

## Feedback Addressed

This scope document has been rewritten from the ground up to address the following feedback:

1. ✅ **Reuse existing Agent configuration model** - Uses `crates/chat-cli/src/cli/agent/mod.rs` and `Agent::get_agent_by_name()`
2. ✅ **Reuse existing infrastructure** - Leverages existing agent config loading and identification
3. ✅ **Introduce WorkerBuilder** - New class to encapsulate worker creation, taking ChatArgs fields (agent, platform, model, input)
4. ✅ **Focus on ContextContainer** - WorkerBuilder builds and populates ContextContainer with agent context
5. ✅ **Introduce ContextBuilder** - AgentLoopTask delegates to ContextBuilder to build ModelRequest from ContextContainer
6. ✅ **Update ModelRequest structure** - Enhanced to support full conversation history, system prompts, and context
7. ✅ **Support both ModelProviders** - Both Bedrock and CodeWhisperer providers updated to handle new ModelRequest
8. ✅ **Align with EventBus architecture** - All changes work within the current EventBus-centered architecture

The original scope focused on simple history accumulation. This version provides a proper architectural foundation for context management that aligns with the current codebase structure.

## Overview

This workflow implements context management and agent configuration integration for the Agent Environment architecture. It enables:

1. **Multi-turn conversations** - Full conversation history is maintained and sent to LLMs, allowing agents to reference previous exchanges
2. **Agent-specific context** - Agent configurations can specify system prompts and resource files that provide domain-specific knowledge
3. **Proper architecture** - Introduces WorkerBuilder for encapsulated worker creation and ContextBuilder for clean ModelRequest construction

The workflow addresses two core user needs:
- Users want agents to remember previous messages in a conversation ("What was the number I mentioned earlier?")
- Users want to configure agents with specific knowledge and behavior ("Act as a Rust expert with access to project guidelines")

This is foundational work that enables future features like conversation persistence, context management commands, and advanced agent behaviors.

## Current State

### Architecture Overview

The Agent Environment uses an EventBus-centered architecture where:
- **Session** orchestrates Workers and Jobs, publishes lifecycle events
- **Worker** represents an agent instance with ContextContainer and model provider
- **AgentLoop** (WorkerTask) executes agent reasoning loops
- **ModelProvider** trait abstracts LLM communication (Bedrock, CodeWhisperer)
- **ContextContainer** holds conversation history and contextual information

### Worker Creation

Currently in `ChatArgs::execute()`:
```rust
let session = Arc::new(Session::new(event_bus.clone(), model_providers));
let main_worker = session.build_worker("main".to_string());
```

`Session::build_worker()` is minimal:
- Takes only a name string
- Uses first available model provider
- Creates empty ContextContainer
- No agent config integration
- No resource loading

### Conversation History

`ContextContainer` has `ConversationHistory`:
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
}
```

`ConversationHistory` stores entries but **only the last entry is used**:
```rust
// AgentLoop::query_llm() - CURRENT
let last_entry = worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .get_entries()
    .last();
```

### ModelRequest Structure

Current `ModelRequest` is simple:
```rust
pub struct ModelRequest {
    pub prompt: String,
    pub conversation_id: Option<String>,
}
```

Both providers (Bedrock, CodeWhisperer) expect this structure. No support for:
- Multiple messages with roles
- System prompts
- Additional context

### Agent Configuration

Agent config system exists in `crates/chat-cli/src/cli/agent/mod.rs`:
- `Agent` struct with fields: name, description, prompt, resources, tools, model, etc.
- `Agent::get_agent_by_name()` loads configs from local/global directories
- `resources` field: `Vec<ResourcePath>` supporting `file://` URLs and glob patterns
- `prompt` field: `Option<String>` for system-level instructions
- `model` field: `Option<String>` for model selection

**Not integrated with agent_env architecture** - exists only in legacy chat flow.

### Gap Summary

1. **No agent config integration** - `--agent` flag not used in agent_env flow
2. **No resource loading** - Agent resources not loaded into ContextContainer
3. **History not used** - Only last message sent to LLM
4. **No system prompts** - Agent prompts not sent to LLM
5. **Simple ModelRequest** - Can't represent conversation history or context
6. **Monolithic worker creation** - Logic scattered in ChatArgs::execute()

## Requirements

### R1: WorkerBuilder

Create `WorkerBuilder` class to encapsulate worker creation logic:

**Inputs:**
- `agent_name: Option<String>` - from `--agent` flag
- `platform: Platform` - from `--platform` flag
- `model: Option<String>` - from `--model` flag
- `initial_input: Option<String>` - from positional argument
- `session: Arc<Session>` - for creating worker
- `os: &Os` - for file operations

**Responsibilities:**
1. Load Agent config (or use default if none specified)
2. Load agent resources from file:// URLs and glob patterns
3. Select appropriate model provider based on platform and agent.model
4. Create ContextContainer with agent context populated
5. Create Worker through Session
6. Add initial input message if provided

**Output:**
- `Arc<Worker>` ready for task execution

**Location:** `crates/chat-cli/src/agent_env/worker_builder.rs`

### R2: Enhanced ContextContainer

Extend `ContextContainer` to store agent-specific context:

```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_prompt: Arc<Mutex<Option<String>>>,      // NEW
    pub agent_resources: Arc<Mutex<Option<String>>>,   // NEW
}
```

**agent_prompt:** System-level instructions from Agent config
**agent_resources:** Concatenated content from resource files

### R3: ContextBuilder

Create `ContextBuilder` class to construct ModelRequest from ContextContainer:

**Inputs:**
- `context_container: &ContextContainer` - source of all context

**Responsibilities:**
1. Read full conversation history (all entries, not just last)
2. Read agent prompt
3. Read agent resources
4. Format into ModelRequest structure
5. Handle edge cases (empty history, no context)

**Output:**
- `ModelRequest` with complete context

**Location:** `crates/chat-cli/src/agent_env/context_builder.rs`

### R4: Enhanced ModelRequest

Update `ModelRequest` to support structured context:

```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // Full conversation history
    pub system_prompt: Option<String>,       // Agent prompt
    pub context: Option<String>,             // Agent resources
    pub conversation_id: Option<String>,     // Existing field
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

**Backward compatibility:** Providers can concatenate fields if needed for their APIs.

### R5: Updated ModelProviders

Both `BedrockConverseStreamModelProvider` and `CodeWhispererModelProvider` must:

1. Accept new ModelRequest structure
2. Format messages appropriately for their respective APIs
3. Handle system_prompt and context fields
4. Maintain existing streaming and cancellation behavior

**Bedrock:** Use system parameter and messages array in converse API
**CodeWhisperer:** Concatenate context into conversation state appropriately

### R6: AgentLoop Integration

Update `AgentLoop::query_llm()` to use ContextBuilder:

```rust
// OLD
let last_entry = worker.context_container.conversation_history.lock().unwrap().last_entry();
let request = ModelRequest { prompt: last_entry, conversation_id: None };

// NEW
let request = ContextBuilder::build_request(&worker.context_container)?;
```

### R7: Response Accumulation

After receiving LLM response, append assistant message to conversation history:

```rust
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_assistant_message(response.content);
```

### R8: Resource Loading

Support loading resources from Agent config:

**File URLs:** `file:///path/to/file.md` - load single file
**Glob patterns:** `file:///path/**/*.rs` - expand and load multiple files
**Error handling:** Graceful handling of missing files, permission errors
**Format:** Concatenate with file path headers for clarity

Example output:
```
--- /path/to/guidelines.md ---
<file content>

--- /path/to/project/src/main.rs ---
<file content>
```

### R9: Entry Point Integration

Update `ChatArgs::execute()` to use WorkerBuilder:

```rust
// OLD
let main_worker = session.build_worker("main".to_string());

// NEW
let main_worker = WorkerBuilder::new()
    .agent(self.agent.clone())
    .platform(self.platform.unwrap_or_default())
    .model(self.model.clone())
    .initial_input(self.input.clone())
    .build(session.clone(), os)
    .await?;
```

### Non-Requirements (Future Work)

- **Conversation persistence** - Loading history from database (future: --resume flag)
- **Tool configuration** - Setting up tools provider (future: trust_tools handling)
- **Context commands** - /context command enhancements
- **Context truncation** - Handling very large contexts
- **Dynamic context** - Reloading resources during conversation

## Implementation Approach

### Phase 1: Core Infrastructure

#### Task 1.1: Update ContextContainer

**File:** `crates/chat-cli/src/agent_env/context_container/context_container.rs`

Add fields for agent context:
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_prompt: Arc<Mutex<Option<String>>>,
    pub agent_resources: Arc<Mutex<Option<String>>>,
}

impl ContextContainer {
    pub fn new() -> Self {
        Self {
            conversation_history: Arc::new(Mutex::new(ConversationHistory::new())),
            agent_prompt: Arc::new(Mutex::new(None)),
            agent_resources: Arc::new(Mutex::new(None)),
        }
    }
    
    pub fn set_agent_prompt(&self, prompt: String) {
        *self.agent_prompt.lock().unwrap() = Some(prompt);
    }
    
    pub fn set_agent_resources(&self, resources: String) {
        *self.agent_resources.lock().unwrap() = Some(resources);
    }
}
```

#### Task 1.2: Update ModelRequest

**File:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

Replace simple ModelRequest with structured version:
```rust
#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,
    pub system_prompt: Option<String>,
    pub context: Option<String>,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}
```

#### Task 1.3: Update ConversationHistory

**File:** `crates/chat-cli/src/agent_env/context_container/conversation_history.rs`

Ensure methods exist for:
- `push_assistant_message(content: String)` - Add assistant response
- `get_entries() -> &[ConversationEntry]` - Read all entries
- Proper serialization for future persistence

### Phase 2: ContextBuilder

#### Task 2.1: Create ContextBuilder

**File:** `crates/chat-cli/src/agent_env/context_builder.rs`

```rust
use super::context_container::ContextContainer;
use super::model_providers::{ModelRequest, ConversationMessage, MessageRole};

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_request(
        context_container: &ContextContainer,
    ) -> Result<ModelRequest, eyre::Error> {
        // 1. Read conversation history
        let messages = {
            let history = context_container.conversation_history.lock().unwrap();
            let entries = history.get_entries();
            
            if entries.is_empty() {
                return Err(eyre::eyre!("No messages in conversation history"));
            }
            
            // Convert entries to messages
            let mut messages = Vec::new();
            for entry in entries {
                if let Some(user_msg) = &entry.user {
                    messages.push(ConversationMessage {
                        role: MessageRole::User,
                        content: user_msg.content_as_string(),
                    });
                }
                if let Some(assistant_msg) = &entry.assistant {
                    messages.push(ConversationMessage {
                        role: MessageRole::Assistant,
                        content: assistant_msg.content.clone(),
                    });
                }
            }
            messages
        };
        
        // 2. Read agent prompt
        let system_prompt = context_container.agent_prompt.lock().unwrap().clone();
        
        // 3. Read agent resources
        let context = context_container.agent_resources.lock().unwrap().clone();
        
        // 4. Build request
        Ok(ModelRequest {
            messages,
            system_prompt,
            context,
            conversation_id: None, // TODO: Extract from ContextContainer when implemented
        })
    }
}
```

**Module export:** Add to `crates/chat-cli/src/agent_env/mod.rs`

### Phase 3: WorkerBuilder

#### Task 3.1: Create WorkerBuilder Structure

**File:** `crates/chat-cli/src/agent_env/worker_builder.rs`

```rust
use std::sync::Arc;
use eyre::Result;

use super::Session;
use super::Worker;
use crate::cli::agent::Agent;
use crate::cli::chat::Platform;
use crate::os::Os;

pub struct WorkerBuilder {
    agent_name: Option<String>,
    platform: Platform,
    model: Option<String>,
    initial_input: Option<String>,
}

impl WorkerBuilder {
    pub fn new() -> Self {
        Self {
            agent_name: None,
            platform: Platform::default(),
            model: None,
            initial_input: None,
        }
    }
    
    pub fn agent(mut self, agent_name: Option<String>) -> Self {
        self.agent_name = agent_name;
        self
    }
    
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
    
    pub fn model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }
    
    pub fn initial_input(mut self, input: Option<String>) -> Self {
        self.initial_input = input;
        self
    }
    
    pub async fn build(
        self,
        session: Arc<Session>,
        os: &Os,
    ) -> Result<Arc<Worker>> {
        // Implementation in next task
        todo!()
    }
}
```

#### Task 3.2: Implement Agent Loading

Add to `WorkerBuilder::build()`:

```rust
// 1. Load agent config
let agent = if let Some(agent_name) = &self.agent_name {
    let (agent, _path) = Agent::get_agent_by_name(os, agent_name).await?;
    agent
} else {
    Agent::default()
};
```

#### Task 3.3: Implement Resource Loading

Add helper method and use in `build()`:

```rust
async fn load_resources(
    resources: &[crate::cli::agent::wrapper_types::ResourcePath],
    os: &Os,
) -> Result<String> {
    use glob::glob;
    
    let mut content = String::new();
    
    for resource in resources {
        let resource_str = resource.as_str();
        
        if !resource_str.starts_with("file://") {
            continue; // Skip non-file resources for MVP
        }
        
        let path = resource_str.strip_prefix("file://").unwrap();
        
        // Handle glob patterns
        if path.contains('*') {
            for entry in glob(path)? {
                let file_path = entry?;
                if let Ok(file_content) = os.fs.read_to_string(&file_path).await {
                    content.push_str(&format!("\n--- {} ---\n", file_path.display()));
                    content.push_str(&file_content);
                }
            }
        } else {
            // Single file
            if let Ok(file_content) = os.fs.read_to_string(path).await {
                content.push_str(&format!("\n--- {} ---\n", path));
                content.push_str(&file_content);
            }
        }
    }
    
    Ok(content)
}
```

#### Task 3.4: Implement Worker Creation

Complete `build()` method:

```rust
pub async fn build(
    self,
    session: Arc<Session>,
    os: &Os,
) -> Result<Arc<Worker>> {
    // 1. Load agent config
    let agent = if let Some(agent_name) = &self.agent_name {
        let (agent, _path) = Agent::get_agent_by_name(os, agent_name).await?;
        agent
    } else {
        Agent::default()
    };
    
    // 2. Load resources
    let resources_content = Self::load_resources(&agent.resources, os).await?;
    
    // 3. Create worker through session
    let worker = session.build_worker("main".to_string());
    
    // 4. Populate context container
    if let Some(prompt) = &agent.prompt {
        worker.context_container.set_agent_prompt(prompt.clone());
    }
    
    if !resources_content.is_empty() {
        worker.context_container.set_agent_resources(resources_content);
    }
    
    // 5. Add initial input if provided
    if let Some(input) = &self.initial_input {
        worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_input_message(input.clone());
    }
    
    Ok(worker)
}
```

**Module export:** Add to `crates/chat-cli/src/agent_env/mod.rs`

### Phase 4: ModelProvider Updates

#### Task 4.1: Update BedrockConverseStreamModelProvider

**File:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

Update `request()` method to handle new ModelRequest:

```rust
async fn request(
    &self,
    request: ModelRequest,
    when_receiving_begin: Box<dyn Fn() + Send>,
    when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
    cancellation_token: CancellationToken,
) -> Result<ModelResponse, eyre::Error> {
    use aws_sdk_bedrockruntime::types::{Message, ConversationRole, ContentBlock, SystemContentBlock};
    
    // 1. Build system prompt (combine agent prompt and context)
    let mut system_blocks = Vec::new();
    if let Some(prompt) = request.system_prompt {
        system_blocks.push(SystemContentBlock::Text(prompt));
    }
    if let Some(context) = request.context {
        system_blocks.push(SystemContentBlock::Text(context));
    }
    
    // 2. Convert messages to Bedrock format
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
                .unwrap()
        })
        .collect();
    
    // 3. Build request
    let mut request_builder = self.client
        .converse_stream()
        .model_id(&self.model_id)
        .set_messages(Some(messages));
    
    if !system_blocks.is_empty() {
        request_builder = request_builder.set_system(Some(system_blocks));
    }
    
    // 4. Send and process (existing streaming logic)
    let response = tokio::select! {
        result = request_builder.send() => result?,
        _ = cancellation_token.cancelled() => {
            return Err(eyre::eyre!("Request cancelled"));
        }
    };
    
    // ... rest of streaming logic unchanged
}
```

#### Task 4.2: Update CodeWhispererModelProvider

**File:** `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`

Update `request()` method:

```rust
async fn request(
    &self,
    request: ModelRequest,
    when_receiving_begin: Box<dyn Fn() + Send>,
    when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
    cancellation_token: CancellationToken,
) -> Result<ModelResponse, eyre::Error> {
    // 1. Build user message content (concatenate context + last user message)
    let mut content = String::new();
    
    // Add system prompt and context as preamble
    if let Some(prompt) = request.system_prompt {
        content.push_str(&prompt);
        content.push_str("\n\n");
    }
    if let Some(ctx) = request.context {
        content.push_str(&ctx);
        content.push_str("\n\n");
    }
    
    // Add conversation history
    for msg in &request.messages {
        match msg.role {
            MessageRole::User => {
                content.push_str("User: ");
                content.push_str(&msg.content);
                content.push_str("\n\n");
            }
            MessageRole::Assistant => {
                content.push_str("Assistant: ");
                content.push_str(&msg.content);
                content.push_str("\n\n");
            }
        }
    }
    
    // 2. Build CodeWhisperer request
    let user_message = UserInputMessage::builder()
        .content(content)
        .build()
        .map_err(|e| eyre::eyre!("Failed to build UserInputMessage: {}", e))?;
    
    let conversation_id = request.conversation_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    let conversation_state = ConversationState::builder()
        .current_message(ChatMessage::UserInputMessage(user_message))
        .chat_trigger_type(ChatTriggerType::Manual)
        .conversation_id(conversation_id)
        .build()
        .map_err(|e| eyre::eyre!("Failed to build ConversationState: {}", e))?;
    
    // 3. Send and process (existing logic unchanged)
    // ... rest of implementation
}
```

### Phase 5: AgentLoop Integration

#### Task 5.1: Update AgentLoop::query_llm()

**File:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

Replace direct history access with ContextBuilder:

```rust
async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
    self.check_cancellation()?;
    
    // Build request using ContextBuilder
    let request = crate::agent_env::ContextBuilder::build_request(
        &self.worker.context_container
    )?;
    
    self.worker.set_state(WorkerStates::Requesting);
    
    // ... rest of method unchanged
}
```

#### Task 5.2: Add Response Accumulation

In `AgentLoop::run()`, after receiving response:

```rust
// Append assistant response to history
self.worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_assistant_message(response.content.clone());
```

### Phase 6: Entry Point Integration

#### Task 6.1: Update ChatArgs::execute()

**File:** `crates/chat-cli/src/cli/chat/mod.rs`

Replace direct worker creation with WorkerBuilder:

```rust
// OLD
let main_worker = session.build_worker("main".to_string());
if let Some(initial_input) = &self.input {
    main_worker.context_container
        .conversation_history
        .lock()
        .unwrap()
        .push_input_message(initial_input.clone());
}

// NEW
use crate::agent_env::WorkerBuilder;

let main_worker = WorkerBuilder::new()
    .agent(self.agent.clone())
    .platform(self.platform.unwrap_or_default())
    .model(self.model.clone())
    .initial_input(self.input.clone())
    .build(session.clone(), os)
    .await?;
```

## Testing Strategy

### Unit Tests

#### ContextBuilder Tests
**File:** `crates/chat-cli/src/agent_env/context_builder.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_request_with_full_context() {
        // Test with conversation history, prompt, and resources
    }
    
    #[test]
    fn test_build_request_empty_history() {
        // Should return error
    }
    
    #[test]
    fn test_build_request_no_agent_context() {
        // Should work with just conversation history
    }
}
```

#### WorkerBuilder Tests
**File:** `crates/chat-cli/src/agent_env/worker_builder.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_build_with_default_agent() {
        // Test default agent creation
    }
    
    #[tokio::test]
    async fn test_build_with_agent_config() {
        // Test loading specific agent
    }
    
    #[tokio::test]
    async fn test_resource_loading() {
        // Test file:// URL loading
    }
    
    #[tokio::test]
    async fn test_glob_pattern_loading() {
        // Test glob pattern expansion
    }
}
```

#### ModelProvider Tests
Update existing tests to handle new ModelRequest structure.

### Integration Tests

#### Test 1: Multi-turn Conversation
```bash
q chat --no-interactive "What is 2+2?"
# Verify response
q chat --no-interactive "What about the previous number plus 1?"
# Verify agent remembers "4" from previous exchange
```

#### Test 2: Agent with Prompt
```bash
# Create test agent
cat > ~/.config/q/agents/test-agent.json << EOF
{
  "name": "test-agent",
  "prompt": "You are a helpful assistant who always responds in haiku format."
}
EOF

q chat --agent test-agent --no-interactive "Tell me about the weather"
# Verify response follows haiku format
```

#### Test 3: Agent with Resources
```bash
# Create test resource
echo "Project uses Rust 2021 edition" > /tmp/guidelines.txt

# Create agent with resource
cat > ~/.config/q/agents/rust-agent.json << EOF
{
  "name": "rust-agent",
  "resources": ["file:///tmp/guidelines.txt"]
}
EOF

q chat --agent rust-agent --no-interactive "What Rust edition does this project use?"
# Verify agent references the guideline
```

#### Test 4: Glob Pattern Resources
```bash
# Create multiple files
echo "Rule 1" > /tmp/rule-1.txt
echo "Rule 2" > /tmp/rule-2.txt

# Create agent with glob
cat > ~/.config/q/agents/glob-agent.json << EOF
{
  "name": "glob-agent",
  "resources": ["file:///tmp/rule-*.txt"]
}
EOF

q chat --agent glob-agent --no-interactive "List all rules"
# Verify both files loaded
```

#### Test 5: Platform Selection
```bash
q chat --platform bedrock --no-interactive "Hello"
# Verify Bedrock provider used

q chat --platform codewhisperer --no-interactive "Hello"
# Verify CodeWhisperer provider used
```

### Manual Testing

1. **Interactive multi-turn:**
   ```bash
   q chat
   > My name is Alice
   > What is my name?
   # Should respond "Alice"
   ```

2. **Agent switching:**
   ```bash
   q chat --agent rust-expert
   > Help me with Rust
   # Verify rust-specific context
   ```

3. **Error handling:**
   ```bash
   q chat --agent nonexistent
   # Should show clear error message
   ```

## Dependencies

### Internal Dependencies
- Existing Agent config system (`crates/chat-cli/src/cli/agent/mod.rs`)
- Existing ConversationHistory implementation
- Existing ModelProvider trait and implementations
- Session and Worker infrastructure

### External Dependencies
- `glob` crate - for glob pattern expansion (likely already in dependencies)
- No new external dependencies required

### Blocked By
None - can be implemented immediately

### Blocks
- **mvp-tools-basic** - Tools will need access to agent config for tool permissions
- **Conversation persistence** - Future --resume flag will build on this foundation
- **Context management commands** - /context command enhancements

## Estimated Effort

### By Phase
- **Phase 1: Core Infrastructure** - 2 hours
  - Update ContextContainer, ModelRequest, ConversationHistory
- **Phase 2: ContextBuilder** - 2 hours
  - Create ContextBuilder with request building logic
- **Phase 3: WorkerBuilder** - 4 hours
  - Create WorkerBuilder, implement agent loading, resource loading
- **Phase 4: ModelProvider Updates** - 4 hours
  - Update both Bedrock and CodeWhisperer providers
- **Phase 5: AgentLoop Integration** - 2 hours
  - Update AgentLoop to use ContextBuilder, add response accumulation
- **Phase 6: Entry Point Integration** - 1 hour
  - Update ChatArgs::execute()

### Testing
- **Unit tests** - 3 hours
- **Integration tests** - 2 hours
- **Manual testing** - 2 hours

### Total: 22 hours (~3 days)

## Success Criteria

1. ✅ Multi-turn conversations maintain full context
2. ✅ Agent configs load successfully with --agent flag
3. ✅ Agent prompts are sent to LLM
4. ✅ Agent resources are loaded and sent to LLM
5. ✅ Glob patterns expand correctly
6. ✅ Both Bedrock and CodeWhisperer providers work with new ModelRequest
7. ✅ WorkerBuilder encapsulates all worker creation logic
8. ✅ ContextBuilder cleanly separates request construction
9. ✅ Error handling is robust (missing files, invalid agents)
10. ✅ No regressions in existing functionality

## Related Files

### New Files
- `crates/chat-cli/src/agent_env/worker_builder.rs` - WorkerBuilder implementation
- `crates/chat-cli/src/agent_env/context_builder.rs` - ContextBuilder implementation

### Modified Files
- `crates/chat-cli/src/agent_env/context_container/context_container.rs` - Add agent context fields
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - Update ModelRequest structure
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Handle new ModelRequest
- `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs` - Handle new ModelRequest
- `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs` - Use ContextBuilder, add response accumulation
- `crates/chat-cli/src/cli/chat/mod.rs` - Use WorkerBuilder in ChatArgs::execute()
- `crates/chat-cli/src/agent_env/mod.rs` - Export new modules

### Referenced Files
- `crates/chat-cli/src/cli/agent/mod.rs` - Existing Agent config system (reused)
- `crates/chat-cli/src/agent_env/context_container/conversation_history.rs` - Existing history management
- `crates/chat-cli/src/agent_env/session.rs` - Existing Session implementation

## Future Enhancements

### Conversation Persistence
- Load conversation history from database with --resume flag
- Save conversation history after each turn
- WorkerBuilder integration point already designed

### Tool Configuration
- Load tool permissions from agent config
- Pass to tools provider layer
- WorkerBuilder can handle this in future iteration

### Context Management
- /context command to view current context
- /context clear to reset conversation
- /context reload to refresh resources

### Context Optimization
- Truncate very large contexts
- Summarize old conversation history
- Smart resource loading (only relevant files)

### Dynamic Context
- Reload resources during conversation
- Update agent prompt mid-conversation
- Context versioning and rollback

