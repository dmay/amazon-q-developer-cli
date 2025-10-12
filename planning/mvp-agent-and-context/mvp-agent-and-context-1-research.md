# MVP Agent and Context - Research Document

## 1. Executive Summary

This research document analyzes the existing codebase to inform the implementation of agent configuration and context management for the Agent Environment architecture. The workflow aims to enable:

1. **Multi-turn conversations** - Full conversation history sent to LLMs
2. **Agent-specific context** - System prompts and resource files providing domain knowledge
3. **Proper architecture** - WorkerBuilder for worker creation, ContextBuilder for request construction

### Current State

The agent_env architecture has a solid foundation with EventBus, Session, Worker, and AgentLoop components. However:

- Agent configuration system exists but is NOT integrated with agent_env
- Only the last message is sent to LLMs (no conversation history)
- ModelRequest is too simple (single prompt string)
- Worker creation is scattered and doesn't consider agent config
- No resource loading or system prompt support

### Key Findings

1. **Agent system is mature** - Comprehensive config with resources, prompts, tools, model selection
2. **Async/sync mismatch** - Resource loading is async but worker creation is sync
3. **Breaking changes required** - ModelRequest needs expansion, affects both providers
4. **Clear integration points** - ChatArgs::execute(), AgentLoop::query_llm(), ModelProvider implementations

### Recommended Approach

1. Introduce **WorkerBuilder** (async) to encapsulate worker creation with agent config
2. Introduce **ContextBuilder** to construct ModelRequest from ContextContainer
3. Extend **ContextContainer** with agent_prompt and agent_resources fields
4. Update **ModelRequest** to support messages array, system prompt, and context
5. Update **both ModelProviders** to handle new ModelRequest structure
6. Integrate at **ChatArgs::execute()** and **AgentLoop::query_llm()**

## 2. Agent Configuration System Analysis

**Location:** `crates/chat-cli/src/cli/agent/mod.rs`

### Agent Struct

The Agent configuration system is comprehensive and mature:

```rust
pub struct Agent {
    pub name: String,
    pub description: Option<String>,
    pub prompt: Option<String>,                    // System-level instructions
    pub resources: Vec<ResourcePath>,              // file:// URLs and globs
    pub model: Option<String>,                     // Model selection
    pub mcp_servers: McpServerConfig,
    pub tools: Vec<String>,
    pub tool_aliases: HashMap<OriginalToolName, String>,
    pub allowed_tools: HashSet<String>,
    pub tools_settings: HashMap<ToolSettingTarget, serde_json::Value>,
    pub hooks: HashMap<HookTrigger, Vec<Hook>>,
    pub use_legacy_mcp_json: bool,
    pub path: Option<PathBuf>,                     // Populated at load time
}
```

### Key Features

1. **Loading:** `Agent::get_agent_by_name(os, name)` loads from local/global directories
2. **Resources:** Supports `file://` URLs and glob patterns (e.g., `file://.amazonq/rules/**/*.md`)
3. **State Management:** "Cold" (as written) vs "Warm" (runtime) states
   - `thaw()` method populates runtime fields (path, merges legacy MCP config)
   - `freeze()` method reverts to writable state
4. **Default Agent:** Includes resources for AmazonQ.md, AGENTS.md, README.md, .amazonq/rules/**/*.md
5. **Model Selection:** Optional model field for per-agent model override

### Loading Process

```rust
// Async loading with error handling
let (agent, path) = Agent::get_agent_by_name(os, agent_name).await?;

// Thaw process:
// 1. Populates path field
// 2. Merges legacy MCP config if use_legacy_mcp_json is true
// 3. Validates against JSON schema
```

### Integration Status

❌ **NOT integrated with agent_env architecture**
- Exists only in legacy chat flow
- ChatArgs has --agent flag but doesn't use it in agent_env flow
- No connection between Agent config and Worker creation

### Implications for Implementation

1. ✅ **Reusable:** Agent loading logic is solid and can be reused as-is
2. ⚠️ **Async:** Loading is async (uses Os.fs) - creates async/sync mismatch with worker creation
3. ⚠️ **Glob expansion:** Requires glob crate for pattern matching
4. ⚠️ **Error handling:** Need graceful handling of missing files, permission errors
5. ✅ **Model selection:** Agent.model field provides per-agent model override capability

## 3. Context Management Analysis

**Location:** `crates/chat-cli/src/agent_env/context_container/`

### ContextContainer

Currently minimal structure:

```rust
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
}
```

**Needs extension:**
```rust
pub struct ContextContainer {
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    pub agent_prompt: Arc<Mutex<Option<String>>>,      // NEW
    pub agent_resources: Arc<Mutex<Option<String>>>,   // NEW
}
```

### ConversationHistory

Stores conversation as entries:

```rust
pub struct ConversationHistory {
    entries: Vec<ConversationEntry>,  // All messages
}

pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

**Methods:**
- `push_input_message(String)` - Add user message
- `push_assistant_message(AssistantMessage)` - Add assistant response
- `get_entries() -> &[ConversationEntry]` - Get all entries

### UserMessage Structure

Complex message with rich context:

```rust
pub struct UserMessage {
    pub additional_context: String,           // Extra context
    pub env_context: UserEnvContext,          // OS, CWD, etc.
    pub content: UserMessageContent,          // Prompt/ToolResults/Cancelled
    pub timestamp: Option<DateTime>,
    pub images: Option<Vec<ImageBlock>>,
}

pub enum UserMessageContent {
    Prompt { prompt: String },
    ToolUseResults { tool_use_results: Vec<ToolUseResult> },
    CancelledToolUses { prompt: Option<String>, tool_use_results: Vec<ToolUseResult> },
}
```

**Key method:** `content_with_context()` - Formats message with timestamp and additional_context

### AssistantMessage Structure

```rust
pub enum AssistantMessage {
    Response { message_id: Option<String>, content: String },
    ToolUse { message_id: Option<String>, content: String, tool_uses: Vec<AssistantToolUse> },
}
```

### Current Usage Pattern

```rust
// Adding messages
worker.context_container
    .conversation_history
    .lock()
    .unwrap()
    .push_input_message("Hello".to_string());

// Reading (currently only last entry used)
let last_entry = history.get_entries().last();
```

### Implications for Implementation

1. ⚠️ **Context separation:** UserMessage already has `additional_context` - agent context should be separate
2. ⚠️ **Formatting conflict:** `content_with_context()` already formats context - avoid duplication
3. ⚠️ **Entry structure:** ConversationEntry has separate user/assistant - need conversion to messages array
4. ✅ **History available:** Full history is stored, just not currently used
5. ⚠️ **Serialization:** ContextContainer is serializable but conversation_history is skipped
6. ✅ **Thread-safe:** Arc<Mutex<>> pattern allows safe concurrent access

## 4. Model Provider Analysis

**Location:** `crates/chat-cli/src/agent_env/model_providers/`

### Current ModelRequest

Too simple for multi-turn conversations:

```rust
pub struct ModelRequest {
    pub prompt: String,                    // Single prompt only
    pub conversation_id: Option<String>,
}
```

**Needs expansion:**
```rust
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // Full history
    pub system_prompt: Option<String>,       // Agent prompt
    pub context: Option<String>,             // Agent resources
    pub conversation_id: Option<String>,
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

### BedrockConverseStreamModelProvider

**Current implementation:**
```rust
let message = Message::builder()
    .role(ConversationRole::User)
    .content(ContentBlock::Text(request.prompt))  // Single message
    .build()?;

let response = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .messages(message)
    .send()
    .await?;
```

**Bedrock API capabilities:**
- ✅ Supports `system` parameter for system prompts
- ✅ Supports `messages` array for conversation history
- ✅ Each message has role (User/Assistant) and content
- ✅ Supports SystemContentBlock for system-level context

**Required changes:**
1. Build messages array from ModelRequest.messages
2. Add system parameter with combined agent_prompt + context
3. Convert MessageRole to ConversationRole

### CodeWhispererModelProvider

**Current implementation:**
```rust
let user_message = UserInputMessage::builder()
    .content(request.prompt)  // Single prompt
    .build()?;

let conversation_state = ConversationState::builder()
    .current_message(ChatMessage::UserInputMessage(user_message))
    .conversation_id(conversation_id)
    .build()?;
```

**CodeWhisperer API characteristics:**
- Uses ConversationState with current_message
- Generates UUID for conversation_id if not provided
- Processes AssistantResponseEvent and ToolUseEvent
- Less clear on multi-turn history support

**Required changes:**
1. Concatenate agent_prompt + context + conversation history into content
2. Format as "User: ...\nAssistant: ...\n" pattern
3. Maintain conversation_id for session continuity

### Implications for Implementation

1. ❌ **Breaking change:** ModelRequest structure change affects both providers
2. ⚠️ **Different approaches:** Bedrock uses native multi-message, CodeWhisperer needs concatenation
3. ✅ **Bedrock ready:** AWS SDK already supports what we need
4. ⚠️ **CodeWhisperer workaround:** Need to format history as text
5. ⚠️ **Coordination required:** Both providers must be updated simultaneously
6. ✅ **Streaming preserved:** Changes don't affect streaming architecture

## 5. Worker Creation Analysis

**Location:** `crates/chat-cli/src/agent_env/session.rs` and `worker.rs`

### Current Session::build_worker()

Too simple for agent integration:

```rust
pub fn build_worker(&self, name: String) -> Arc<Worker> {
    let model_provider = self.model_providers.first()
        .expect("At least one model provider required")
        .clone();
    
    let worker = Arc::new(Worker::new(name.clone(), model_provider));
    self.workers.lock().unwrap().push(worker.clone());
    
    // Publish WorkerEvent::Created
    self.event_bus.publish(AgentEnvironmentEvent::Worker(
        WorkerEvent::Created { worker_id: worker.id, name, timestamp: Instant::now() }
    ));
    
    worker
}
```

**Limitations:**
- Takes only a name string
- Uses first model provider (no selection logic)
- Creates empty ContextContainer
- No agent config integration
- No resource loading

### Worker Structure

```rust
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub context_container: ContextContainer,
    pub lifecycle_state: Arc<Mutex<WorkerLifecycleState>>,
    pub task_metadata: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub model_provider: Option<Arc<dyn ModelProvider>>,
    pub state: Arc<Mutex<WorkerStates>>,  // Legacy
    pub last_failure: Arc<Mutex<Option<String>>>,
}
```

**Features:**
- Serializable (skips runtime fields)
- Thread-safe with Arc<Mutex<>>
- Extensible via task_metadata
- Lifecycle state management

### Current Usage in ChatArgs::execute()

```rust
// Create Session
let session = Arc::new(Session::new(event_bus.clone(), model_providers));

// Create Worker (simple)
let main_worker = session.build_worker("main".to_string());

// Add initial input (manual)
if let Some(initial_input) = &self.input {
    main_worker.context_container
        .conversation_history
        .lock()
        .unwrap()
        .push_input_message(initial_input.clone());
}
```

### Implications for Implementation

1. ❌ **Insufficient:** Current build_worker() can't handle agent config
2. ⚠️ **Sync constraint:** Worker creation is synchronous but resource loading is async
3. ⚠️ **Scattered logic:** Worker setup split between Session and ChatArgs
4. ⚠️ **No provider selection:** Can't choose provider based on platform/agent.model
5. ✅ **Event publishing:** Already publishes WorkerEvent::Created
6. ✅ **Extensible:** Worker structure can accommodate new fields

### Need for WorkerBuilder

A WorkerBuilder is needed to:
1. Load agent config (async)
2. Load and process resources (async with glob expansion)
3. Select appropriate model provider
4. Populate ContextContainer with agent context
5. Add initial input if provided
6. Create Worker through Session
7. Encapsulate all creation logic in one place

## 6. AgentLoop Integration Analysis

**Location:** `crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs`

### Current query_llm() Implementation

Only uses last message:

```rust
async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
    // Get prompt from LAST entry only
    let prompt = {
        let history = self.worker.context_container
            .conversation_history
            .lock()
            .unwrap();
        
        let last_entry = history.get_entries().last()
            .ok_or_else(|| eyre::eyre!("No messages in history"))?;
        
        match &last_entry.user {
            Some(user_msg) => match user_msg.content() {
                UserMessageContent::Prompt { prompt } => prompt.clone(),
                _ => return Err(eyre::eyre!("Expected prompt message")),
            },
            None => return Err(eyre::eyre!("Last entry is not a user message")),
        }
    };

    let request = ModelRequest { 
        prompt,
        conversation_id: None,
    };

    // Send to model provider...
}
```

**Problem:** Full conversation history is available but not used!

### Response Handling

Already accumulates responses correctly:

```rust
async fn run(&self) -> Result<(), eyre::Error> {
    let response = self.query_llm().await?;
    
    // Publish events
    self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
        AgentLoopEvent::ResponseReceived { ... }
    ));
    
    // Add to history
    let assistant_message = AssistantMessage::new_response(None, response.content.clone());
    self.worker.context_container
        .conversation_history
        .lock()
        .unwrap()
        .push_assistant_message(assistant_message);
    
    Ok(())
}
```

### Integration Point

This is the **key integration point** for ContextBuilder:

```rust
// BEFORE (current)
let prompt = extract_last_message_prompt();
let request = ModelRequest { prompt, conversation_id: None };

// AFTER (with ContextBuilder)
let request = ContextBuilder::build_request(&self.worker.context_container)?;
```

### Implications for Implementation

1. ✅ **Clean integration:** Single line change in query_llm()
2. ✅ **Response handling works:** Already adds assistant messages to history
3. ✅ **Event publishing works:** Already publishes appropriate events
4. ⚠️ **Error handling:** Need to handle empty history, malformed entries
5. ✅ **Cancellation preserved:** No impact on cancellation logic
6. ✅ **Tool support exists:** Already handles tool_requests in response

## 7. Entry Point Analysis

**Location:** `crates/chat-cli/src/cli/chat/mod.rs`

### ChatArgs Structure

```rust
pub struct ChatArgs {
    pub resume: bool,
    pub agent: Option<String>,              // --agent flag (NOT USED)
    pub model: Option<String>,              // --model flag (NOT USED)
    pub trust_all_tools: bool,
    pub trust_tools: Option<Vec<String>>,
    pub no_interactive: bool,
    pub input: Option<String>,              // Initial prompt
    pub wrap: Option<WrapMode>,
    pub ui_mode: Option<UiMode>,
    pub platform: Option<Platform>,         // Bedrock/CodeWhisperer
}
```

### Current execute() Flow

```rust
pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
    // 1. Create EventBus
    let event_bus = EventBus::default();
    
    // 2. Create model provider based on platform
    let platform = self.platform.unwrap_or_default();
    let model_provider = Self::create_model_provider(platform, os).await?;
    let model_providers = vec![model_provider];
    
    // 3. Create Session
    let session = Arc::new(Session::new(event_bus.clone(), model_providers));
    
    // 4. Create Worker (SIMPLE - no agent config)
    let main_worker = session.build_worker("main".to_string());
    
    // 5. Add initial input (MANUAL)
    if let Some(initial_input) = &self.input {
        main_worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_input_message(initial_input.clone());
    }
    
    // 6. Create UI
    let main_ui = /* TextUi or StructuredIO */;
    
    // 7. Create AgentEnvironment and run
    let agent_env = AgentEnvironment::new(session, event_bus, main_ui, vec![]);
    agent_env.run().await
}
```

### Problems with Current Approach

1. ❌ **Agent flag ignored:** `self.agent` is parsed but not used
2. ❌ **Model flag ignored:** `self.model` is parsed but not used
3. ❌ **No agent config:** Worker created without agent context
4. ❌ **No resources:** No resource loading
5. ❌ **Manual setup:** Initial input added manually
6. ❌ **Platform only:** Provider selection only considers platform, not agent.model

### Integration Point

Replace simple worker creation with WorkerBuilder:

```rust
// BEFORE (current)
let main_worker = session.build_worker("main".to_string());
if let Some(initial_input) = &self.input {
    main_worker.context_container
        .conversation_history
        .lock()
        .unwrap()
        .push_input_message(initial_input.clone());
}

// AFTER (with WorkerBuilder)
let main_worker = WorkerBuilder::new()
    .agent(self.agent.clone())
    .platform(self.platform.unwrap_or_default())
    .model(self.model.clone())
    .initial_input(self.input.clone())
    .build(session.clone(), os)
    .await?;
```

### Implications for Implementation

1. ✅ **Single change point:** Replace worker creation with WorkerBuilder
2. ⚠️ **Async required:** Need to await WorkerBuilder::build()
3. ✅ **All flags used:** Agent, model, platform, input all passed to builder
4. ✅ **Clean encapsulation:** All setup logic moves to WorkerBuilder
5. ⚠️ **Error handling:** Need to handle agent loading errors gracefully

## 8. Architectural Constraints

### 1. Async/Sync Mismatch

**Constraint:** Worker creation is synchronous but resource loading is async

```rust
// Session::build_worker() is NOT async
pub fn build_worker(&self, name: String) -> Arc<Worker> { ... }

// But Agent::get_agent_by_name() IS async
pub async fn get_agent_by_name(os: &Os, agent_name: &str) -> Result<(Agent, PathBuf)> { ... }

// And file loading is async
os.fs.read_to_string(&path).await?
```

**Impact:**
- WorkerBuilder::build() must be async
- ChatArgs::execute() must await worker creation
- Session::build_worker() remains sync (for simple cases)

**Solution:** WorkerBuilder is a separate async builder, doesn't replace Session::build_worker()

### 2. Breaking Changes to ModelRequest

**Constraint:** ModelRequest structure change affects all code using it

**Current usage:**
- BedrockConverseStreamModelProvider
- CodeWhispererModelProvider
- AgentLoop::query_llm()
- Any tests

**Impact:**
- Both providers must be updated simultaneously
- Tests must be updated
- Cannot be done incrementally

**Solution:** Update all providers in same PR, ensure comprehensive testing

### 3. Serialization Compatibility

**Constraint:** ContextContainer is serializable for future persistence

```rust
#[derive(Serialize, Deserialize)]
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    // New fields must be serializable or skipped
}
```

**Impact:**
- New fields (agent_prompt, agent_resources) must be serializable
- Arc<Mutex<>> pattern requires #[serde(skip)]
- Need default values for deserialization

**Solution:** Use Arc<Mutex<Option<String>>> with #[serde(skip)] and default functions

### 4. Context Formatting Conflict

**Constraint:** UserMessage already has context formatting

```rust
impl UserMessage {
    fn content_with_context(&self) -> String {
        // Already formats timestamp, additional_context, env_context
    }
}
```

**Impact:**
- Agent context (prompt/resources) should NOT be in UserMessage
- Agent context should be separate in ContextContainer
- ContextBuilder must handle both types of context

**Solution:** Keep agent context separate, ContextBuilder combines them appropriately

### 5. ConversationEntry Structure

**Constraint:** Entries have separate user/assistant fields, not a flat message list

```rust
pub struct ConversationEntry {
    pub user: Option<UserMessage>,
    pub assistant: Option<AssistantMessage>,
}
```

**Impact:**
- Need to flatten to messages array for ModelRequest
- Must handle entries with only user or only assistant
- Order must be preserved

**Solution:** ContextBuilder iterates entries and builds messages array in order

### 6. Event Publishing Requirements

**Constraint:** Session publishes WorkerEvent::Created when worker is created

**Impact:**
- WorkerBuilder must create worker through Session to maintain event flow
- Cannot bypass Session::build_worker() entirely
- Must preserve event publishing behavior

**Solution:** WorkerBuilder calls Session::build_worker(), then populates context

## 9. Critical Pitfalls and Solutions

### Pitfall 1: Async/Sync Mismatch in Worker Creation

**Problem:** Resource loading is async but worker creation is sync

**Risk:** Cannot load resources during worker creation without making it async

**Solution:**
```rust
// WorkerBuilder::build() is async
pub async fn build(self, session: Arc<Session>, os: &Os) -> Result<Arc<Worker>> {
    // 1. Load agent config (async)
    let agent = Agent::get_agent_by_name(os, &agent_name).await?;
    
    // 2. Load resources (async)
    let resources = Self::load_resources(&agent.resources, os).await?;
    
    // 3. Create worker through Session (sync)
    let worker = session.build_worker("main".to_string());
    
    // 4. Populate context (sync)
    worker.context_container.set_agent_prompt(agent.prompt);
    worker.context_container.set_agent_resources(resources);
    
    Ok(worker)
}
```

### Pitfall 2: Context Duplication

**Problem:** UserMessage already has additional_context and env_context

**Risk:** Agent context might be duplicated or conflict with message context

**Solution:**
- Agent context (prompt/resources) stored in ContextContainer
- Message context (env, timestamp) stays in UserMessage
- ContextBuilder combines them appropriately for each provider:
  - Bedrock: Agent context in system parameter, message context in content
  - CodeWhisperer: All context concatenated in content

### Pitfall 3: Glob Pattern Expansion Errors

**Problem:** Glob patterns might match no files, or files might not exist

**Risk:** Worker creation fails if resources can't be loaded

**Solution:**
```rust
async fn load_resources(resources: &[ResourcePath], os: &Os) -> Result<String> {
    let mut content = String::new();
    
    for resource in resources {
        let path = resource.strip_prefix("file://");
        
        if path.contains('*') {
            // Glob pattern - collect all matches
            for entry in glob(path)? {
                if let Ok(file_path) = entry {
                    // Gracefully handle missing files
                    if let Ok(file_content) = os.fs.read_to_string(&file_path).await {
                        content.push_str(&format!("\n--- {} ---\n{}", file_path.display(), file_content));
                    }
                }
            }
        } else {
            // Single file - gracefully handle missing
            if let Ok(file_content) = os.fs.read_to_string(path).await {
                content.push_str(&format!("\n--- {} ---\n{}", path, file_content));
            }
        }
    }
    
    Ok(content)
}
```

### Pitfall 4: Model Provider Selection Logic

**Problem:** Need to select provider based on both --platform flag and agent.model

**Risk:** Conflicting or unclear provider selection

**Solution:**
```rust
// Priority order:
// 1. --platform flag (explicit user choice)
// 2. agent.model field (agent config)
// 3. Default (CodeWhisperer)

let platform = self.platform.unwrap_or_else(|| {
    if let Some(model) = &agent.model {
        if model.contains("bedrock") || model.contains("claude") {
            Platform::Bedrock
        } else {
            Platform::CodeWhisperer
        }
    } else {
        Platform::default()
    }
});
```

### Pitfall 5: Breaking ModelRequest Change

**Problem:** Changing ModelRequest structure breaks both providers

**Risk:** Incomplete update leaves system in broken state

**Solution:**
- Update ModelRequest, ContextBuilder, both providers, and AgentLoop in single PR
- Add comprehensive tests before and after
- Consider temporary backward compatibility if needed

### Pitfall 6: Empty Conversation History

**Problem:** ContextBuilder might be called with empty history

**Risk:** Cannot build ModelRequest without messages

**Solution:**
```rust
impl ContextBuilder {
    pub fn build_request(context_container: &ContextContainer) -> Result<ModelRequest> {
        let messages = /* build from history */;
        
        if messages.is_empty() {
            return Err(eyre::eyre!("No messages in conversation history"));
        }
        
        // Continue building request...
    }
}
```

### Pitfall 7: ConversationEntry Conversion

**Problem:** ConversationEntry has separate user/assistant, need flat message list

**Risk:** Incorrect message ordering or missing messages

**Solution:**
```rust
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
            content: assistant_msg.content().to_string(),
        });
    }
}
```

## 10. Design Recommendations

### 1. WorkerBuilder Design

**Recommendation:** Create async builder that encapsulates all worker creation logic

```rust
pub struct WorkerBuilder {
    agent_name: Option<String>,
    platform: Platform,
    model: Option<String>,
    initial_input: Option<String>,
}

impl WorkerBuilder {
    pub fn new() -> Self { ... }
    
    pub fn agent(mut self, agent_name: Option<String>) -> Self { ... }
    pub fn platform(mut self, platform: Platform) -> Self { ... }
    pub fn model(mut self, model: Option<String>) -> Self { ... }
    pub fn initial_input(mut self, input: Option<String>) -> Self { ... }
    
    pub async fn build(self, session: Arc<Session>, os: &Os) -> Result<Arc<Worker>> {
        // 1. Load agent config or use default
        // 2. Load resources with glob expansion
        // 3. Create worker through Session
        // 4. Populate ContextContainer
        // 5. Add initial input if provided
    }
}
```

**Benefits:**
- Encapsulates all creation logic
- Async for resource loading
- Builder pattern for flexibility
- Reuses Session::build_worker() for event publishing

### 2. ContextBuilder Design

**Recommendation:** Static builder that converts ContextContainer to ModelRequest

```rust
pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_request(context_container: &ContextContainer) -> Result<ModelRequest> {
        // 1. Read conversation history
        // 2. Convert entries to messages array
        // 3. Read agent_prompt
        // 4. Read agent_resources
        // 5. Build ModelRequest
    }
}
```

**Benefits:**
- Clean separation of concerns
- Stateless (no fields)
- Easy to test
- Single responsibility

### 3. ContextContainer Extension

**Recommendation:** Add agent context fields with proper serialization

```rust
#[derive(Serialize, Deserialize)]
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    
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
    pub fn set_agent_prompt(&self, prompt: String) {
        *self.agent_prompt.lock().unwrap() = Some(prompt);
    }
    
    pub fn set_agent_resources(&self, resources: String) {
        *self.agent_resources.lock().unwrap() = Some(resources);
    }
}
```

**Benefits:**
- Maintains serialization compatibility
- Thread-safe with Arc<Mutex<>>
- Default values for deserialization
- Clean API

### 4. ModelRequest Structure

**Recommendation:** Expand to support full conversation context

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

**Benefits:**
- Supports multi-turn conversations
- Separates system prompt from messages
- Allows additional context
- Clean structure

### 5. Provider Update Strategy

**Recommendation:** Update both providers to handle new ModelRequest

**Bedrock:**
```rust
// Use native multi-message support
let messages: Vec<Message> = request.messages
    .iter()
    .map(|msg| {
        Message::builder()
            .role(match msg.role {
                MessageRole::User => ConversationRole::User,
                MessageRole::Assistant => ConversationRole::Assistant,
            })
            .content(ContentBlock::Text(msg.content.clone()))
            .build()
    })
    .collect();

// Combine system prompt and context
let mut system_blocks = Vec::new();
if let Some(prompt) = request.system_prompt {
    system_blocks.push(SystemContentBlock::Text(prompt));
}
if let Some(context) = request.context {
    system_blocks.push(SystemContentBlock::Text(context));
}

let response = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .set_messages(Some(messages))
    .set_system(Some(system_blocks))
    .send()
    .await?;
```

**CodeWhisperer:**
```rust
// Concatenate all context into content
let mut content = String::new();

// Add system prompt and context
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
        MessageRole::User => content.push_str("User: "),
        MessageRole::Assistant => content.push_str("Assistant: "),
    }
    content.push_str(&msg.content);
    content.push_str("\n\n");
}

let user_message = UserInputMessage::builder()
    .content(content)
    .build()?;
```

**Benefits:**
- Bedrock uses native capabilities
- CodeWhisperer uses text concatenation
- Both support full context
- Maintains streaming behavior

### 6. Integration Points

**Recommendation:** Minimal changes at integration points

**ChatArgs::execute():**
```rust
// Replace this:
let main_worker = session.build_worker("main".to_string());

// With this:
let main_worker = WorkerBuilder::new()
    .agent(self.agent.clone())
    .platform(self.platform.unwrap_or_default())
    .model(self.model.clone())
    .initial_input(self.input.clone())
    .build(session.clone(), os)
    .await?;
```

**AgentLoop::query_llm():**
```rust
// Replace this:
let prompt = extract_last_message();
let request = ModelRequest { prompt, conversation_id: None };

// With this:
let request = ContextBuilder::build_request(&self.worker.context_container)?;
```

**Benefits:**
- Minimal code changes
- Clear intent
- Easy to review
- Maintains existing behavior

### 7. Testing Strategy

**Recommendation:** Comprehensive testing at each layer

1. **Unit tests:**
   - ContextBuilder with various history states
   - WorkerBuilder with different agent configs
   - Resource loading with glob patterns
   - ModelRequest conversion

2. **Integration tests:**
   - Multi-turn conversations
   - Agent with prompt and resources
   - Both Bedrock and CodeWhisperer providers
   - Error handling (missing files, invalid agents)

3. **Manual tests:**
   - Interactive multi-turn chat
   - Agent switching
   - Resource loading verification

**Benefits:**
- Catches issues early
- Documents expected behavior
- Enables confident refactoring
- Prevents regressions
