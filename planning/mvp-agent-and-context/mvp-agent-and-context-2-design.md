# MVP Agent and Context - Technical Design

## Overview

This design implements agent configuration and context management for the Agent Environment architecture, enabling:

1. **Multi-turn conversations** - Full conversation history sent to LLMs
2. **Agent-specific context** - System prompts and resource files providing domain knowledge
3. **Clean architecture** - WorkerBuilder for worker creation, ContextBuilder for request construction

## Design Goals

- Enable agents to remember previous messages in conversations
- Support agent configurations with system prompts and resource files
- Maintain clean separation of concerns
- Preserve existing EventBus architecture and event publishing
- Support both Bedrock and CodeWhisperer providers
- Graceful error handling for missing files and invalid configs

## Architecture Overview

### Component Relationships

```
ChatArgs::execute()
    ↓
WorkerBuilder (NEW)
    ├─ Load Agent config
    ├─ Load resources (glob patterns)
    ├─ Create Worker via Session
    └─ Populate ContextContainer
        ├─ agent_prompt (NEW)
        ├─ agent_resources (NEW)
        └─ conversation_history (existing)
    ↓
AgentLoop::query_llm()
    ↓
ContextBuilder (NEW)
    ├─ Read conversation_history
    ├─ Read agent_prompt
    ├─ Read agent_resources
    └─ Build ModelRequest (UPDATED)
        ├─ messages: Vec<ConversationMessage> (NEW)
        ├─ system_prompt: Option<String> (NEW)
        ├─ context: Option<String> (NEW)
        └─ conversation_id: Option<String> (existing)
    ↓
ModelProvider::request()
    ├─ BedrockConverseStreamModelProvider (UPDATED)
    │   ├─ Use native multi-message API
    │   └─ System blocks for agent context
    └─ CodeWhispererModelProvider (UPDATED)
        └─ Concatenate all context into content
```

### Key Design Decisions

1. **WorkerBuilder is async** - Required for resource loading with file I/O
2. **ContextBuilder is stateless** - Pure conversion logic, no state
3. **Breaking ModelRequest change** - All providers updated simultaneously
4. **Graceful resource loading** - Missing files don't fail worker creation
5. **Reuse Session::build_worker()** - Maintains event publishing behavior

## Component Designs



### 1. ContextContainer Extension

**File:** `crates/chat-cli/src/agent_env/context_container/context_container.rs`

**Changes:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    
    // NEW: Agent-specific context
    #[serde(skip, default = "default_agent_prompt")]
    pub agent_prompt: Arc<Mutex<Option<String>>>,
    
    #[serde(skip, default = "default_agent_resources")]
    pub agent_resources: Arc<Mutex<Option<String>>>,
}

fn default_agent_prompt() -> Arc<Mutex<Option<String>>> {
    Arc::new(Mutex::new(None))
}

fn default_agent_resources() -> Arc<Mutex<Option<String>>> {
    Arc::new(Mutex::new(None))
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
    
    pub fn get_agent_prompt(&self) -> Option<String> {
        self.agent_prompt.lock().unwrap().clone()
    }
    
    pub fn set_agent_resources(&self, resources: String) {
        *self.agent_resources.lock().unwrap() = Some(resources);
    }
    
    pub fn get_agent_resources(&self) -> Option<String> {
        self.agent_resources.lock().unwrap().clone()
    }
}
```

**Rationale:**
- Separate agent context from conversation history
- Thread-safe with Arc<Mutex<>>
- Serialization-compatible with skip and default functions
- Simple String storage for concatenated resources

### 2. Enhanced ModelRequest

**File:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

**Changes:**
```rust
#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // NEW: Full conversation
    pub system_prompt: Option<String>,       // NEW: Agent instructions
    pub context: Option<String>,             // NEW: Agent resources
    pub conversation_id: Option<String>,     // Existing
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

**Rationale:**
- Supports multi-turn conversations with messages array
- Separates system-level context from conversation
- Clean role-based message structure
- Breaking change - requires coordinated provider updates

### 3. ContextBuilder

**File:** `crates/chat-cli/src/agent_env/context_builder.rs` (NEW)

**Implementation:**
```rust
use super::context_container::ContextContainer;
use super::model_providers::{ModelRequest, ConversationMessage, MessageRole};
use super::context_container::conversation_entry::AssistantMessage;
use eyre::Result;

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_request(
        context_container: &ContextContainer,
    ) -> Result<ModelRequest> {
        let messages = Self::build_messages(context_container)?;
        let system_prompt = context_container.get_agent_prompt();
        let context = context_container.get_agent_resources();
        
        Ok(ModelRequest {
            messages,
            system_prompt,
            context,
            conversation_id: None,
        })
    }
    
    fn build_messages(
        context_container: &ContextContainer,
    ) -> Result<Vec<ConversationMessage>> {
        let history = context_container.conversation_history.lock().unwrap();
        let entries = history.get_entries();
        
        if entries.is_empty() {
            return Err(eyre::eyre!("No messages in conversation history"));
        }
        
        let mut messages = Vec::new();
        
        for entry in entries {
            if let Some(user_msg) = &entry.user {
                messages.push(ConversationMessage {
                    role: MessageRole::User,
                    content: user_msg.content_as_string(),
                });
            }
            
            if let Some(assistant_msg) = &entry.assistant {
                let content = match assistant_msg {
                    AssistantMessage::Response { content, .. } => content.clone(),
                    AssistantMessage::ToolUse { content, .. } => content.clone(),
                };
                
                messages.push(ConversationMessage {
                    role: MessageRole::Assistant,
                    content,
                });
            }
        }
        
        Ok(messages)
    }
}
```

**Rationale:**
- Stateless converter (no fields)
- Single responsibility: ContextContainer → ModelRequest
- Flattens ConversationEntry structure to messages array
- Error handling for empty history

### 4. WorkerBuilder

**File:** `crates/chat-cli/src/agent_env/worker_builder.rs` (NEW)

**Implementation:**
```rust
use std::sync::Arc;
use eyre::Result;

use super::{Session, Worker};
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
        // 1. Load agent config or use default
        let agent = if let Some(agent_name) = &self.agent_name {
            let (agent, _path) = Agent::get_agent_by_name(os, agent_name).await?;
            agent
        } else {
            Agent::default()
        };
        
        // 2. Load resources
        let resources_content = Self::load_resources(&agent.resources, os).await?;
        
        // 3. Create worker through Session (maintains event publishing)
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
    
    async fn load_resources(
        resources: &[crate::cli::agent::wrapper_types::ResourcePath],
        os: &Os,
    ) -> Result<String> {
        use glob::glob;
        
        let mut content = String::new();
        
        for resource in resources {
            let resource_str = resource.as_str();
            
            if !resource_str.starts_with("file://") {
                continue;
            }
            
            let path = resource_str.strip_prefix("file://").unwrap();
            
            if path.contains('*') {
                // Glob pattern
                match glob(path) {
                    Ok(entries) => {
                        for entry in entries {
                            match entry {
                                Ok(file_path) => {
                                    if let Ok(file_content) = os.fs.read_to_string(&file_path).await {
                                        content.push_str(&format!("\n--- {} ---\n", file_path.display()));
                                        content.push_str(&file_content);
                                        content.push('\n');
                                    }
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                    Err(_) => continue,
                }
            } else {
                // Single file
                if let Ok(file_content) = os.fs.read_to_string(path).await {
                    content.push_str(&format!("\n--- {} ---\n", path));
                    content.push_str(&file_content);
                    content.push('\n');
                }
            }
        }
        
        Ok(content)
    }
}
```

**Rationale:**
- Builder pattern for flexible configuration
- Async for resource loading
- Reuses Session::build_worker() for event publishing
- Graceful error handling (missing files don't fail)
- Encapsulates all worker creation logic

**Resource Format:**
```
--- /path/to/file1.md ---
<file1 content>

--- /path/to/file2.rs ---
<file2 content>
```



### 5. BedrockConverseStreamModelProvider Updates

**File:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

**Changes to request() method:**
```rust
use aws_sdk_bedrockruntime::types::SystemContentBlock;

async fn request(&self, request: ModelRequest, ...) -> Result<ModelResponse, eyre::Error> {
    // 1. Build system content blocks
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
    
    // 4. Send and stream (existing logic unchanged)
    // ... rest unchanged
}
```

**Rationale:**
- Uses Bedrock's native multi-message API
- System blocks for agent context
- Preserves streaming and cancellation logic

### 6. CodeWhispererModelProvider Updates

**File:** `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`

**Changes to request() method:**
```rust
async fn request(&self, request: ModelRequest, ...) -> Result<ModelResponse, eyre::Error> {
    // 1. Concatenate all context
    let mut content = String::new();
    
    if let Some(prompt) = request.system_prompt {
        content.push_str(&prompt);
        content.push_str("\n\n");
    }
    
    if let Some(ctx) = request.context {
        content.push_str(&ctx);
        content.push_str("\n\n");
    }
    
    for msg in &request.messages {
        match msg.role {
            MessageRole::User => content.push_str("User: "),
            MessageRole::Assistant => content.push_str("Assistant: "),
        }
        content.push_str(&msg.content);
        content.push_str("\n\n");
    }
    
    // 2. Build user message
    let user_message = UserInputMessage::builder()
        .content(content)
        .build()?;
    
    // ... rest unchanged
}
```

**Rationale:**
- Concatenates all context (no native multi-message support)
- Format as "User: ... Assistant: ..." pattern



### 7. AgentLoop Integration

**File:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

**Changes to query_llm():**
```rust
async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
    self.check_cancellation()?;
    
    // NEW: Use ContextBuilder instead of manual extraction
    let request = crate::agent_env::ContextBuilder::build_request(
        &self.worker.context_container
    )?;
    
    self.worker.set_state(WorkerStates::Requesting);
    
    // ... rest unchanged (send to provider, handle streaming)
}
```

**No changes to run()** - response accumulation already works correctly.

**Rationale:**
- Single line change
- ContextBuilder handles full history + agent context
- Existing response handling preserved

### 8. ChatArgs::execute() Integration

**File:** `crates/chat-cli/src/cli/chat/mod.rs`

**Changes:**
```rust
pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
    // ... EventBus, model providers, Session creation unchanged
    
    // NEW: Use WorkerBuilder instead of direct creation
    let main_worker = crate::agent_env::WorkerBuilder::new()
        .agent(self.agent.clone())
        .platform(platform)
        .model(self.model.clone())
        .initial_input(self.input.clone())
        .build(session.clone(), os)
        .await?;
    
    // ... rest unchanged (UI creation, AgentEnvironment, run)
}
```

**Rationale:**
- Replaces simple worker creation + manual input
- All complexity moved to WorkerBuilder
- Maintains existing flow

## Implementation Notes

### Breaking Changes

**ModelRequest structure change affects:**
- `model_provider.rs` - ModelRequest definition
- `bedrock_converse_stream.rs` - request() method
- `codewhisperer.rs` - request() method
- `agent_loop.rs` - query_llm() method
- Any tests using ModelRequest

**All changes must be done in single PR.**

### Error Handling

**WorkerBuilder:**
- Agent loading errors → fail worker creation
- Missing resource files → silently skip
- Invalid glob patterns → silently skip
- Permission errors → silently skip

**ContextBuilder:**
- Empty conversation history → return error
- Missing agent context → OK (None values)

### Module Exports

**Add to `crates/chat-cli/src/agent_env/mod.rs`:**
```rust
mod context_builder;
mod worker_builder;

pub use context_builder::ContextBuilder;
pub use worker_builder::WorkerBuilder;
```

### Dependencies

**Likely already present:**
- `glob` crate for pattern matching
- `uuid` crate for conversation IDs

**Check Cargo.toml and add if needed.**

## Testing Strategy

### Unit Tests

**ContextBuilder:**
- Full context (history + prompt + resources)
- Empty history (should error)
- No agent context (should work)
- Mixed entries (user only, assistant only)

**WorkerBuilder:**
- Default agent
- Specific agent config
- Single file resource
- Glob pattern resource
- Missing files (graceful)
- Invalid agent (error)

**ModelProvider updates:**
- Update existing tests for new ModelRequest structure
- Test with/without system_prompt and context

### Integration Tests

1. **Multi-turn conversation:**
   ```bash
   q chat --no-interactive "What is 2+2?"
   q chat --no-interactive "What about the previous number plus 1?"
   ```

2. **Agent with prompt:**
   ```bash
   q chat --agent test-agent --no-interactive "Tell me about the weather"
   ```

3. **Agent with resources:**
   ```bash
   q chat --agent rust-agent --no-interactive "What Rust edition?"
   ```

4. **Glob patterns:**
   ```bash
   q chat --agent glob-agent --no-interactive "List all rules"
   ```

### Manual Testing

- Interactive multi-turn conversations
- Agent switching
- Error handling (nonexistent agent, missing files)
- Both Bedrock and CodeWhisperer platforms

## Success Criteria

1. ✅ Multi-turn conversations maintain full context
2. ✅ Agent configs load with --agent flag
3. ✅ Agent prompts sent to LLM
4. ✅ Agent resources loaded and sent to LLM
5. ✅ Glob patterns expand correctly
6. ✅ Both providers work with new ModelRequest
7. ✅ WorkerBuilder encapsulates creation logic
8. ✅ ContextBuilder cleanly separates request construction
9. ✅ Graceful error handling
10. ✅ No regressions in existing functionality

## Future Enhancements

- Conversation persistence (--resume flag)
- Tool configuration from agent config
- Context management commands (/context)
- Context truncation for large histories
- Dynamic resource reloading
