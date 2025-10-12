# MVP Agent and Context - Technical Design

## Overview

This document provides the technical design for implementing Task 1.2 (History Accumulation) and Task 1.4 (Agent Context Loading) in the agent_env architecture. These tasks enable multi-turn conversations with full context and agent-specific configuration loading.

### Goals

**Task 1.2: History Accumulation**
- Enable multi-turn conversations by sending entire conversation history to LLM
- Maintain context across multiple agent loop invocations
- Support proper user/assistant message sequencing

**Task 1.4: Agent Context Loading**
- Load agent configuration when --agent flag is provided
- Read and inject resource files from agent config
- Support file:// paths and glob patterns
- Make context available to LLM via system prompts

### Design Principles

1. **Minimal Changes**: Modify only necessary components to achieve functionality
2. **Clean Separation**: Keep conversation history and agent context separate
3. **Bedrock Native**: Use Bedrock Converse API features (system prompts, message arrays)
4. **MVP Scope**: Defer advanced features (truncation, summarization) to future iterations

## Architecture Overview

### Component Interaction

```
ChatArgs::execute()
    ├─> Load Agent config (if --agent specified)
    ├─> Read agent resources (files, globs)
    ├─> Create Worker with ContextContainer
    │   ├─> conversation_history (existing)
    │   ├─> agent_context (NEW)
    │   └─> system_prompt (NEW)
    └─> Launch AgentLoop

AgentLoop::query_llm()
    ├─> Read entire conversation_history
    ├─> Convert to ConversationMessage array
    ├─> Get agent_context and system_prompt
    ├─> Build ModelRequest
    │   ├─> messages: Vec<ConversationMessage>
    │   └─> system: Option<String>
    └─> Send to ModelProvider

BedrockConverseStreamModelProvider
    ├─> Convert ConversationMessage to Bedrock Message
    ├─> Build system ContentBlock array
    └─> Call converse_stream() with messages + system
```

### Data Flow

**Conversation History Flow:**
```
User Input → ConversationHistory → AgentLoop → ModelRequest → Bedrock
                                         ↓
                                  Assistant Response
                                         ↓
                                  ConversationHistory (append)
```

**Agent Context Flow:**
```
Agent Config → Resource Files → ContextContainer → ModelRequest → Bedrock
                                                                      ↓
                                                              (system prompt)
```

## Bedrock API Research Findings

### System Prompt Support

Bedrock Converse API supports system prompts via the `system` field:
- Type: Array of `SystemContentBlock` objects
- SystemContentBlock can contain `text` field for system prompts
- System prompts provide instructions/context separate from conversation messages
- Not counted as part of message history

### Message Format

Messages in Bedrock Converse API:
- Array of `Message` objects
- Each Message has:
  - `role`: "user" or "assistant"
  - `content`: Array of `ContentBlock` objects (text, images, documents)
- No explicit requirement for alternating roles (but recommended practice)

### AWS SDK Types Available

From `aws_sdk_bedrockruntime::types`:
- `Message` - Message structure
- `ConversationRole` - Enum: User, Assistant
- `ContentBlock` - Content types (Text, Image, Document, ToolUse, ToolResult)
- `SystemContentBlock` - System prompt content

## Task 1.2: History Accumulation Design

### Problem Statement

Current implementation only sends the last user message to the LLM, ignoring all previous conversation context. This prevents multi-turn conversations where the model needs to reference earlier exchanges.

### Solution Overview

1. Update `ModelRequest` to support message arrays instead of single prompt string
2. Modify `AgentLoop::query_llm()` to read entire conversation history
3. Convert `ConversationEntry` objects to `ConversationMessage` format
4. Update `BedrockConverseStreamModelProvider` to send all messages

### Data Structures

#### New: ConversationMessage

```rust
// Location: crates/chat-cli/src/agent_env/model_providers/model_provider.rs

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
- Simple, focused structure for MVP
- Separates concerns from complex UserMessage/AssistantMessage types
- Easy to convert to Bedrock Message format
- Future-proof for additional fields (images, tool use)

#### Updated: ModelRequest

```rust
// Location: crates/chat-cli/src/agent_env/model_providers/model_provider.rs

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // CHANGED: was `prompt: String`
    pub system: Option<String>,              // NEW: for Task 1.4
    pub conversation_id: Option<String>,     // EXISTING
}
```

**Changes:**
- Replace `prompt: String` with `messages: Vec<ConversationMessage>`
- Add `system: Option<String>` for agent context (Task 1.4)
- Keep `conversation_id` for future use

**Impact:**
- Breaking change to ModelProvider interface
- All ModelProvider implementations must be updated
- Currently only BedrockConverseStreamModelProvider exists

### Message Extraction Logic

#### Extracting from UserMessage

```rust
fn extract_user_message_content(user_msg: &UserMessage) -> Option<String> {
    match user_msg.content() {
        UserMessageContent::Prompt { prompt } => Some(prompt.clone()),
        UserMessageContent::ToolUseResults { .. } => None,  // Skip for MVP
        UserMessageContent::CancelledToolUses { .. } => None,  // Skip for MVP
    }
}
```

**Rationale:**
- For MVP, only extract Prompt messages
- Tool use messages will be handled in mvp-tools-basic
- Cancelled tool uses are not relevant for history

#### Extracting from AssistantMessage

```rust
fn extract_assistant_message_content(assistant_msg: &AssistantMessage) -> String {
    // AssistantMessage has a text() method that returns the response text
    assistant_msg.text().to_string()
}
```

**Note:** Need to verify AssistantMessage API - may need to extract from response field.

### AgentLoop Changes

#### Updated query_llm() Method

```rust
// Location: crates/chat-cli/src/agent_env/worker_tasks/agent_loop.rs

async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
    self.check_cancellation()?;
    
    // Read entire conversation history
    let messages = {
        let history = self.worker.context_container
            .conversation_history
            .lock()
            .unwrap();
        
        let mut messages = Vec::new();
        
        for entry in history.get_entries() {
            // Extract user message
            if let Some(user_msg) = &entry.user {
                if let Some(content) = extract_user_message_content(user_msg) {
                    messages.push(ConversationMessage {
                        role: MessageRole::User,
                        content,
                    });
                }
            }
            
            // Extract assistant message
            if let Some(assistant_msg) = &entry.assistant {
                let content = extract_assistant_message_content(assistant_msg);
                messages.push(ConversationMessage {
                    role: MessageRole::Assistant,
                    content,
                });
            }
        }
        
        if messages.is_empty() {
            return Err(eyre::eyre!("No messages in history"));
        }
        
        messages
    };  // Lock dropped here
    
    // Get system prompt (Task 1.4)
    let system = self.worker.context_container.get_combined_system_prompt();
    
    let request = ModelRequest {
        messages,
        system,
        conversation_id: None,  // TODO: Extract from ContextContainer
    };
    
    // ... rest of method unchanged ...
}
```

**Key Points:**
- Lock conversation_history only during extraction
- Build messages vector from all entries
- Skip tool use messages for MVP
- Validate non-empty message array
- Get system prompt for Task 1.4 integration

### BedrockConverseStreamModelProvider Changes

#### Updated request() Method

```rust
// Location: crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs

async fn request(
    &self,
    request: ModelRequest,
    when_receiving_begin: Box<dyn Fn() + Send>,
    when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
    cancellation_token: CancellationToken,
) -> Result<ModelResponse, eyre::Error> {
    // Convert ConversationMessage to Bedrock Message
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
    
    // Build request
    let mut builder = self.client
        .converse_stream()
        .model_id(&self.model_id)
        .set_messages(Some(messages));
    
    // Add system prompt if provided (Task 1.4)
    if let Some(system_text) = request.system {
        use aws_sdk_bedrockruntime::types::SystemContentBlock;
        
        let system_block = SystemContentBlock::Text(system_text);
        builder = builder.system(system_block);
    }
    
    let response = tokio::select! {
        result = builder.send() => {
            // ... error handling unchanged ...
        },
        _ = cancellation_token.cancelled() => {
            return Err(eyre::eyre!("Request cancelled"));
        }
    };
    
    // ... streaming logic unchanged ...
}
```

**Key Points:**
- Convert all messages to Bedrock format
- Use `set_messages(Some(messages))` for array
- Add system prompt if provided
- Error handling remains the same

### Edge Cases and Validation

#### Empty History
- Return error if no messages in history
- Should not happen in normal flow (user message added before AgentLoop)

#### Non-Alternating Messages
- Bedrock API doesn't strictly require alternation
- Our extraction naturally produces alternating messages
- If tool use messages are skipped, may have consecutive user messages
- Bedrock should handle this gracefully

#### Large History
- For MVP: Send entire history without truncation
- Let Bedrock handle token limit errors
- Future: Implement sliding window or summarization

### Testing Strategy

#### Unit Tests

1. **Message Extraction:**
   - Test extracting Prompt from UserMessage
   - Test extracting text from AssistantMessage
   - Test skipping ToolUseResults messages

2. **Message Conversion:**
   - Test ConversationMessage to Bedrock Message conversion
   - Test role mapping (User/Assistant)

3. **History Building:**
   - Test building messages from empty history (error)
   - Test building messages from single exchange
   - Test building messages from multiple exchanges

#### Integration Tests

1. **Multi-turn Conversation:**
   ```bash
   q chat
   > What is 2+2?
   # (wait for response: "4")
   > What about the previous number plus 1?
   # (should respond: "5")
   ```

2. **Context Preservation:**
   ```bash
   q chat
   > My name is Alice
   # (wait for response)
   > What is my name?
   # (should respond: "Alice")
   ```

3. **Long Conversation:**
   ```bash
   q chat
   > Message 1
   > Message 2
   > Message 3
   > Summarize our conversation
   # (should reference all previous messages)
   ```

## Task 1.4: Agent Context Loading Design

### Problem Statement

Agent configuration system exists but is not integrated with agent_env architecture. Need to load agent resources (context files) and make them available to the LLM via system prompts.

### Solution Overview

1. Enhance `ContextContainer` with agent context storage
2. Implement resource loading (file reading, glob expansion)
3. Integrate agent loading in `ChatArgs::execute()`
4. Inject context into `ModelRequest` as system prompt
5. Update `BedrockConverseStreamModelProvider` to use system field

### Data Structures

#### Updated: ContextContainer

```rust
// Location: crates/chat-cli/src/agent_env/context_container/context_container.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
    
    #[serde(skip)]
    pub agent_context: Arc<Mutex<Option<String>>>,  // NEW
    
    #[serde(skip)]
    pub system_prompt: Arc<Mutex<Option<String>>>,  // NEW
}

impl ContextContainer {
    pub fn new() -> Self {
        Self {
            conversation_history: Arc::new(Mutex::new(ConversationHistory::new())),
            agent_context: Arc::new(Mutex::new(None)),
            system_prompt: Arc::new(Mutex::new(None)),
        }
    }
    
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
    
    pub fn get_combined_system_prompt(&self) -> Option<String> {
        let system = self.system_prompt.lock().unwrap().clone();
        let context = self.agent_context.lock().unwrap().clone();
        
        match (system, context) {
            (Some(s), Some(c)) => Some(format!("{}\n\n{}", s, c)),
            (Some(s), None) => Some(s),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        }
    }
}
```

**Rationale:**
- Separate fields for system_prompt (agent.prompt) and agent_context (resources)
- Thread-safe with Arc<Mutex<>>
- Combined getter for convenience
- Skip serialization (context is loaded at startup)

### Resource Loading

#### Resource Loading Function

```rust
// Location: crates/chat-cli/src/cli/chat/mod.rs (or new module)

use glob::glob;
use std::path::{Path, PathBuf};

fn load_agent_resources(
    agent: &Agent,
) -> Result<String, eyre::Error> {
    let base_path = agent.path.as_ref()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    
    let mut context = String::new();
    
    for resource in &agent.resources {
        // Strip file:// prefix
        let path_str = resource.as_str()
            .strip_prefix("file://")
            .unwrap_or(resource.as_str());
        
        // Resolve path (relative to agent config directory)
        let resolved = if Path::new(path_str).is_relative() {
            base_path.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        
        // Handle glob patterns
        if path_str.contains('*') {
            let pattern = resolved.to_str()
                .ok_or_else(|| eyre::eyre!("Invalid path: {:?}", resolved))?;
            
            for entry in glob(pattern)? {
                let file_path = entry?;
                if file_path.is_file() {
                    let content = std::fs::read_to_string(&file_path)
                        .map_err(|e| eyre::eyre!("Failed to read {}: {}", file_path.display(), e))?;
                    
                    context.push_str(&format!("\n--- {} ---\n", file_path.display()));
                    context.push_str(&content);
                }
            }
        } else {
            // Single file
            if resolved.is_file() {
                let content = std::fs::read_to_string(&resolved)
                    .map_err(|e| eyre::eyre!("Failed to read {}: {}", resolved.display(), e))?;
                
                context.push_str(&format!("\n--- {} ---\n", resolved.display()));
                context.push_str(&content);
            } else {
                // Warning: file not found (don't error, just skip)
                eprintln!("Warning: Resource file not found: {}", resolved.display());
            }
        }
    }
    
    Ok(context)
}
```

**Key Points:**
- Resolve paths relative to agent config directory
- Support both single files and glob patterns
- Add file path headers for clarity
- Warn on missing files (don't error)
- Return combined context string

#### Path Resolution Strategy

**Base Path:** Parent directory of agent config file
- Agent config: `~/.config/q/agents/my-agent.yaml`
- Base path: `~/.config/q/agents/`
- Relative resource: `file://context.md` → `~/.config/q/agents/context.md`

**Absolute Paths:** Used as-is
- Resource: `file:///tmp/context.md` → `/tmp/context.md`

**Glob Patterns:** Expanded relative to base path
- Resource: `file://.amazonq/rules/**/*.md`
- Expands from: `~/.config/q/agents/.amazonq/rules/**/*.md`

### ChatArgs Integration

#### Updated execute() Method

```rust
// Location: crates/chat-cli/src/cli/chat/mod.rs

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
            let context = load_agent_resources(agent)?;
            if !context.is_empty() {
                main_worker.context_container.set_agent_context(context);
            }
            
            // Set system prompt if provided
            if let Some(prompt) = &agent.prompt {
                main_worker.context_container.set_system_prompt(prompt.clone());
            }
        }
        
        // ... rest of initialization unchanged ...
    }
}
```

**Key Points:**
- Load agent config early (before worker creation)
- Inject context into worker's ContextContainer
- Set both agent_context and system_prompt
- Continue with normal flow

### Error Handling

#### Resource Loading Errors

**File Not Found:**
- Warn and continue (don't fail)
- Allow partial context loading
- Log warning to stderr

**Permission Denied:**
- Warn and continue
- Log error details

**Invalid Glob Pattern:**
- Return error (fail fast)
- Invalid pattern indicates config error

**Large Files:**
- For MVP: Load entire file
- Future: Add size limits and warnings

### Testing Strategy

#### Unit Tests

1. **Resource Loading:**
   - Test single file loading
   - Test glob pattern expansion
   - Test relative path resolution
   - Test absolute path handling
   - Test missing file handling

2. **Context Injection:**
   - Test set_agent_context()
   - Test set_system_prompt()
   - Test get_combined_system_prompt()

#### Integration Tests

1. **Agent with Context:**
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

2. **Glob Patterns:**
   ```bash
   cat > ~/.config/q/agents/glob-agent.yaml << EOF
   name: "glob-agent"
   resources:
     - file:///tmp/docs/*.md
   EOF
   
   mkdir -p /tmp/docs
   echo "# Doc 1" > /tmp/docs/doc1.md
   echo "# Doc 2" > /tmp/docs/doc2.md
   
   q chat --agent glob-agent "List the documents you have"
   # Should mention both doc1 and doc2
   ```

3. **Missing Files:**
   ```bash
   cat > ~/.config/q/agents/missing-agent.yaml << EOF
   name: "missing-agent"
   resources:
     - file:///tmp/nonexistent.txt
   EOF
   
   q chat --agent missing-agent "Hello"
   # Should work with warning, not error
   ```

## Integration Design

### Combined ModelRequest Structure

```rust
#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<ConversationMessage>,  // Task 1.2
    pub system: Option<String>,              // Task 1.4
    pub conversation_id: Option<String>,
}
```

**Usage:**
- `messages`: Conversation history (Task 1.2)
- `system`: Combined agent context + system prompt (Task 1.4)
- Both fields work together seamlessly

### Request Building Flow

```rust
// In AgentLoop::query_llm()

// 1. Build messages from history (Task 1.2)
let messages = build_messages_from_history(&history);

// 2. Get combined system prompt (Task 1.4)
let system = self.worker.context_container.get_combined_system_prompt();

// 3. Build request
let request = ModelRequest {
    messages,
    system,
    conversation_id: None,
};
```

### Bedrock Request Format

```rust
// In BedrockConverseStreamModelProvider::request()

let mut builder = self.client
    .converse_stream()
    .model_id(&self.model_id)
    .set_messages(Some(messages));  // Task 1.2

if let Some(system_text) = request.system {  // Task 1.4
    let system_block = SystemContentBlock::Text(system_text);
    builder = builder.system(system_block);
}

let response = builder.send().await?;
```

## Implementation Order

### Recommended Sequence

1. **Task 1.2 First** (History Accumulation)
   - Update ModelRequest structure
   - Implement message extraction
   - Update AgentLoop::query_llm()
   - Update BedrockConverseStreamModelProvider
   - Test multi-turn conversations

2. **Task 1.4 Second** (Agent Context Loading)
   - Enhance ContextContainer
   - Implement resource loading
   - Integrate in ChatArgs::execute()
   - Update AgentLoop to use system prompt
   - Test agent context injection

**Rationale:**
- Task 1.2 is more fundamental (multi-turn conversations)
- Task 1.4 builds on Task 1.2's ModelRequest changes
- Testing is easier with working conversations first
- Both tasks modify ModelRequest, so doing 1.2 first establishes the structure

### Alternative: Parallel Implementation

Tasks can be implemented in parallel by different developers:
- Task 1.2: Focus on message array and history
- Task 1.4: Focus on context loading and system prompt
- Integration: Merge both changes to ModelRequest

**Coordination Required:**
- Agree on ModelRequest structure upfront
- Both tasks update model_provider.rs
- Both tasks update agent_loop.rs
- Both tasks update bedrock_converse_stream.rs

## Dependencies

### External Crates

**glob** (for Task 1.4):
```toml
[dependencies]
glob = "0.3"
```

**Purpose:** Glob pattern expansion for resource loading

### Internal Dependencies

**Task 1.2:**
- ConversationHistory (existing)
- ConversationEntry (existing)
- UserMessage, AssistantMessage (existing)
- ModelProvider trait (existing)

**Task 1.4:**
- Agent config system (existing)
- ContextContainer (existing, enhanced)
- ChatArgs (existing, enhanced)

## Future Enhancements

### Task 1.2 Future Work

1. **History Truncation:**
   - Implement sliding window (keep last N messages)
   - Token-based truncation
   - Summarization of old messages

2. **Tool Use Messages:**
   - Include tool use/result messages in history
   - Format for Bedrock API
   - Handle tool use in multi-turn conversations

3. **Message Validation:**
   - Enforce alternating user/assistant messages
   - Merge consecutive same-role messages
   - Validate message content

### Task 1.4 Future Work

1. **Context Caching:**
   - Cache loaded resources
   - Reload on file changes
   - Invalidate cache on agent reload

2. **Context Size Management:**
   - Warn on large contexts
   - Limit file sizes
   - Truncate or summarize large files

3. **Advanced Resource Types:**
   - Support URLs (http://, https://)
   - Support environment variables
   - Support dynamic context (commands)

4. **Context Scope:**
   - Per-worker context
   - Shared context across workers
   - Context inheritance

## Risk Assessment

### Task 1.2 Risks

**Risk: Breaking ModelProvider Interface**
- Impact: High (all providers must be updated)
- Mitigation: Only one provider exists (Bedrock)
- Likelihood: Certain (intentional breaking change)

**Risk: Large History Token Limits**
- Impact: Medium (errors on long conversations)
- Mitigation: Let Bedrock handle errors for MVP
- Likelihood: Low (most conversations fit in limits)

**Risk: Message Extraction Complexity**
- Impact: Low (simple extraction for MVP)
- Mitigation: Skip tool use messages
- Likelihood: Low (well-defined structures)

### Task 1.4 Risks

**Risk: File System Errors**
- Impact: Low (warn and continue)
- Mitigation: Graceful error handling
- Likelihood: Medium (missing files, permissions)

**Risk: Large Context Files**
- Impact: Medium (token limits, memory)
- Mitigation: Accept for MVP, add warnings
- Likelihood: Medium (users may include large files)

**Risk: Glob Pattern Complexity**
- Impact: Low (well-defined behavior)
- Mitigation: Use standard glob crate
- Likelihood: Low (glob is well-tested)

**Risk: Path Resolution Confusion**
- Impact: Medium (files not found)
- Mitigation: Clear documentation, warnings
- Likelihood: Medium (relative vs absolute paths)

## Success Criteria

### Task 1.2 Success

- Multi-turn conversations work correctly
- History is maintained across invocations
- Context is preserved in responses
- No memory leaks or race conditions
- Tests pass

### Task 1.4 Success

- Agent configs load successfully
- Resources are read and injected
- Context is available to LLM
- Glob patterns work correctly
- Error handling is robust
- Tests pass

### Integration Success

- Both features work together
- System prompt and messages coexist
- No conflicts or regressions
- Performance is acceptable
- Documentation is complete

## Conclusion

This design provides a minimal, clean implementation of history accumulation and agent context loading. Both tasks integrate seamlessly through the ModelRequest structure and leverage Bedrock's native support for system prompts and message arrays.

The design prioritizes simplicity and correctness for MVP, deferring advanced features (truncation, caching, validation) to future iterations. Implementation can proceed sequentially (1.2 then 1.4) or in parallel with proper coordination.
