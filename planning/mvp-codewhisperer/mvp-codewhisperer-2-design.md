# MVP CodeWhisperer - Technical Design

## 1. Overview & Goals

### Purpose
Integrate CodeWhisperer as an alternative model provider in the agent_env architecture, enabling users to choose between Bedrock and CodeWhisperer as LLM backends.

### Success Criteria
- CodeWhisperer works as a drop-in replacement for Bedrock
- Streaming responses display correctly
- Tool use support (if available)
- Platform selection via `--platform` flag
- Minimal changes to existing architecture

### Non-Goals (MVP)
- Multi-turn conversation history (use empty history for MVP)
- Advanced CodeWhisperer-specific features
- Model selection per platform
- Conversation persistence across sessions

---

## 2. Architecture Summary

### Current ModelProvider Architecture

The agent_env architecture uses a `ModelProvider` trait for LLM abstraction:

```rust
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: Box<dyn Fn() + Send>,
        when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse, eyre::Error>;
}

pub struct ModelRequest {
    pub prompt: String,
}

pub enum ModelResponseChunk {
    AssistantMessage(String),
    ToolUseRequest { tool_name: String, parameters: String },
}

pub struct ModelResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,
}
```

**Key characteristics:**
- Streaming via callbacks (`when_received`)
- Tool use support via `ToolUseRequest` chunks
- Cancellation support
- Simple request/response model

### Existing Implementation: BedrockConverseStreamModelProvider

- Uses AWS SDK's `converse_stream()` API
- Streams `ConverseStreamOutput` events
- Extracts text from `ContentBlockDelta::Text`
- Handles `MessageStop` to end stream

---

## 3. CodeWhisperer API Analysis

### Key Types

**SendMessageInput:**
```rust
pub struct SendMessageInput {
    pub conversation_state: Option<ConversationState>,
    pub profile_arn: Option<String>,
    pub source: Option<Origin>,
    pub dry_run: Option<bool>,
}
```

**ConversationState:**
```rust
pub struct ConversationState {
    pub conversation_id: Option<String>,
    pub workspace_id: Option<String>,
    pub history: Option<Vec<ChatMessage>>,
    pub current_message: ChatMessage,  // Required
    pub chat_trigger_type: ChatTriggerType,  // Required
    pub customization_arn: Option<String>,
    pub agent_continuation_id: Option<String>,
    pub agent_task_type: Option<AgentTaskType>,
}
```

**ChatMessage (enum):**
- `UserInputMessage(UserInputMessage)` - User messages
- `AssistantResponseMessage(AssistantResponseMessage)` - Assistant messages

**UserInputMessage:**
```rust
pub struct UserInputMessage {
    pub content: String,  // Required
    pub user_input_message_context: Option<UserInputMessageContext>,
    pub user_intent: Option<UserIntent>,
    pub origin: Option<Origin>,
    pub images: Option<Vec<ImageBlock>>,
    pub model_id: Option<String>,
    pub cache_point: Option<CachePoint>,
    pub client_cache_config: Option<ClientCacheConfig>,
}
```

**ChatResponseStream (enum):**
- `AssistantResponseEvent` - Text chunks
- `ToolUseEvent` - Tool use requests
- `ToolResultEvent` - Tool results
- 13+ other event types (citations, metadata, etc.)

**ToolUse:**
```rust
pub struct ToolUse {
    pub tool_use_id: String,  // Required
    pub name: String,  // Required
    pub input: Document,  // Required (JSON-like structure)
}
```

### API Comparison

| Feature | Bedrock | CodeWhisperer |
|---------|---------|---------------|
| Streaming | ✅ ConverseStreamOutput | ✅ ChatResponseStream |
| Text chunks | ContentBlockDelta::Text | AssistantResponseEvent |
| Tool use | tool_use blocks | ToolUseEvent |
| Conversation history | Messages array | ConversationState.history |
| Authentication | AWS credentials | Bearer token |

---

## 4. Design Decisions

### 4.1 Request Mapping

**Challenge:** Convert `ModelRequest` (simple prompt) to `SendMessageInput` (complex conversation state).

**Design:**

**Step 1: Extend ModelRequest to include conversation_id**
```rust
pub struct ModelRequest {
    pub prompt: String,
    pub conversation_id: Option<String>,  // NEW
}
```

**Step 2: AgentLoop gets conversation_id from ContextContainer**
```rust
// In AgentLoop::query_llm()
let conversation_id = self.worker.context_container
    .get_conversation_id()
    .map(|s| s.to_string());

let request = ModelRequest { 
    prompt,
    conversation_id,
};
```

**Step 3: CodeWhispererModelProvider uses conversation_id from request**
```rust
impl CodeWhispererModelProvider {
    fn build_send_message_input(&self, request: ModelRequest) -> Result<SendMessageInput> {
        // Create UserInputMessage from prompt
        let user_message = UserInputMessage::builder()
            .content(request.prompt)
            .build()?;
        
        // Use conversation_id from request, or generate new one
        let conversation_id = request.conversation_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        // Create ConversationState
        let conversation_state = ConversationState::builder()
            .current_message(ChatMessage::UserInputMessage(user_message))
            .chat_trigger_type(ChatTriggerType::Manual)
            .conversation_id(conversation_id)
            .build()?;
        
        // Create SendMessageInput
        let input = SendMessageInput::builder()
            .conversation_state(conversation_state)
            .build()?;
        
        Ok(input)
    }
}
```

**Key decisions:**
- Add `conversation_id: Option<String>` to `ModelRequest`
- AgentLoop extracts conversation_id from ContextContainer
- CodeWhispererModelProvider uses provided ID or generates fallback
- Leave `history` empty for MVP (no multi-turn context)
- Use `ChatTriggerType::Manual` for all requests

**Rationale:**
- Conversation ID flows naturally from context source
- Enables conversation tracking across requests
- Fallback UUID ensures robustness
- Minimal breaking change (Option field)

### 4.2 Response Mapping

**Challenge:** Map CodeWhisperer's 16+ event types to `ModelResponseChunk`.

**Design:**

```rust
impl CodeWhispererModelProvider {
    async fn process_stream_event(
        &self,
        event: ChatResponseStream,
        accumulated_content: &mut String,
        tool_requests: &mut Vec<ToolRequest>,
        when_received: &dyn Fn(ModelResponseChunk),
    ) -> Result<()> {
        match event {
            ChatResponseStream::AssistantResponseEvent(evt) => {
                let content = evt.content;
                accumulated_content.push_str(&content);
                when_received(ModelResponseChunk::AssistantMessage(content));
            }
            
            ChatResponseStream::ToolUseEvent(evt) => {
                if let Some(tool_use) = evt.tool_use {
                    let parameters = serde_json::to_string(&tool_use.input)?;
                    
                    tool_requests.push(ToolRequest {
                        tool_name: tool_use.name.clone(),
                        parameters: parameters.clone(),
                    });
                    
                    when_received(ModelResponseChunk::ToolUseRequest {
                        tool_name: tool_use.name,
                        parameters,
                    });
                }
            }
            
            // Ignore other events for MVP
            _ => {}
        }
        
        Ok(())
    }
}
```

**Key decisions:**
- Map only `AssistantResponseEvent` and `ToolUseEvent`
- Ignore 14+ other event types (citations, metadata, etc.)
- Convert `Document` to JSON string for parameters
- Accumulate content and tool_requests for final response

**Rationale:**
- MVP focuses on core functionality
- Other events can be added incrementally
- Matches Bedrock's behavior (text + tool use)

### 4.3 Tool Use Integration

**Challenge:** Map CodeWhisperer's `ToolUse` (with Document input) to `ToolRequest` (with String parameters).

**Design:**

```rust
// ToolUse has:
// - tool_use_id: String
// - name: String
// - input: Document

// Convert to ToolRequest:
let parameters = serde_json::to_string(&tool_use.input)?;

tool_requests.push(ToolRequest {
    tool_name: tool_use.name,
    parameters,  // JSON string
});
```

**Key decisions:**
- Serialize `Document` to JSON string
- Discard `tool_use_id` (not used by ModelProvider trait)
- Store `tool_use_id` internally for future ToolResult support

**Rationale:**
- `Document` is JSON-compatible, serialization is straightforward
- `tool_use_id` not needed for MVP (no tool result responses yet)
- Can be enhanced when tool execution is implemented

**Future Enhancement:**
When tool execution is added, store `tool_use_id` mapping:
```rust
struct CodeWhispererModelProvider {
    tool_use_id_map: Arc<Mutex<HashMap<String, String>>>,  // tool_name -> tool_use_id
}
```

### 4.4 Conversation State Management

**Challenge:** ModelProvider trait doesn't provide conversation history.

**Design Options:**

#### Option A: Empty History (MVP Choice)
```rust
// Always use empty history
let conversation_state = ConversationState::builder()
    .current_message(user_message)
    .chat_trigger_type(ChatTriggerType::Manual)
    .conversation_id(self.conversation_id.clone())
    // No history field
    .build()?;
```

**Pros:**
- Simple implementation
- No trait changes needed
- Works for single-turn interactions

**Cons:**
- No multi-turn conversation support
- CodeWhisperer won't have context from previous messages

#### Option B: Extend ModelProvider Trait (Future)
```rust
pub struct ModelRequest {
    pub prompt: String,
    pub conversation_history: Vec<ConversationEntry>,  // NEW
}
```

**Pros:**
- Full conversation context
- Platform-agnostic

**Cons:**
- Breaking change to trait
- Affects all implementations

#### Option C: Pass Worker Reference (Future)
```rust
pub trait ModelProvider: Send + Sync {
    async fn request(
        &self,
        request: ModelRequest,
        worker: &Worker,  // NEW
        // ...
    ) -> Result<ModelResponse>;
}
```

**Pros:**
- Access to full context
- No request structure changes

**Cons:**
- Tight coupling to Worker
- Breaking change

**Decision:** Use Option A for MVP, plan Option B for future.

**Rationale:**
- MVP focuses on basic functionality
- Empty history sufficient for testing
- Can be enhanced without major refactoring

### 4.5 Authentication & Client Creation

**Challenge:** CodeWhisperer uses Bearer token auth, different from Bedrock's AWS credentials.

**Design: Reuse Existing ApiClient Infrastructure**

The codebase already has `ApiClient` that creates `CodewhispererStreamingClient` with proper configuration. We should reuse this instead of creating a new client.

**Existing Implementation (in `api_client/mod.rs`):**

```rust
// ApiClient::new() creates streaming client with:
streaming_client = Some(CodewhispererStreamingClient::from_conf(
    amzn_codewhisperer_streaming_client::config::Builder::from(&bearer_sdk_config)
        .http_client(crate::aws_common::http_client::client())
        .interceptor(OptOutInterceptor::new(database))
        .interceptor(UserAgentOverrideInterceptor::new())
        .interceptor(DelayTrackingInterceptor::new())
        .bearer_token_resolver(BearerResolver)
        .app_name(app_name())
        .endpoint_url(endpoint.url())  // From Endpoint::configured_value(database)
        .retry_classifier(retry_classifier::QCliRetryClassifier::new())
        .stalled_stream_protection(stalled_stream_protection_config())
        .build(),
));
```

**Key Configuration:**
- **Endpoint URL**: From `Endpoint::configured_value(database)` - handles default endpoints and custom endpoints
- **Bearer Token**: Via `BearerResolver` (existing auth infrastructure)
- **Interceptors**: OptOut, UserAgent, DelayTracking
- **Retry/Timeout**: Configured via database settings
- **Stalled Stream Protection**: 5-minute grace period

**Design Decision: Extract Client from ApiClient**

```rust
// In ChatArgs::execute()
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        let platform = self.platform.unwrap_or_default();
        
        let model_provider: Arc<dyn ModelProvider> = match platform {
            Platform::Bedrock => {
                // Bedrock setup (unchanged)
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new("us-east-1"))
                    .load()
                    .await;
                let bedrock_client = BedrockClient::new(&config);
                Arc::new(BedrockConverseStreamModelProvider::new(bedrock_client))
            }
            Platform::CodeWhisperer => {
                // Create ApiClient (existing infrastructure)
                let api_client = ApiClient::new(
                    &os.env,
                    &os.fs,
                    &mut os.database,
                    None,  // Use configured endpoint
                ).await?;
                
                // Extract streaming client
                let streaming_client = api_client.streaming_client()
                    .ok_or_else(|| eyre::eyre!("CodeWhisperer streaming client not available"))?;
                
                Arc::new(CodeWhispererModelProvider::new(streaming_client))
            }
        };
        
        // ... rest of initialization
    }
}
```

**Add Accessor Method to ApiClient:**

```rust
// In api_client/mod.rs
impl ApiClient {
    pub fn streaming_client(&self) -> Option<CodewhispererStreamingClient> {
        self.streaming_client.clone()
    }
}
```

**Rationale:**
- Reuses all existing configuration (endpoint, auth, interceptors, retry logic)
- Respects database settings (timeout, custom endpoints)
- Handles both default and custom endpoints automatically
- Uses existing `BearerResolver` for authentication
- Includes all production interceptors (opt-out, telemetry, etc.)
- No duplication of complex configuration logic

**Error Handling:**
```rust
match api_client.streaming_client() {
    Some(client) => Arc::new(CodeWhispererModelProvider::new(client)),
    None => {
        eprintln!("CodeWhisperer streaming client not available");
        eprintln!("This may occur if:");
        eprintln!("  - Authentication failed (run 'q login')");
        eprintln!("  - AMAZON_Q_SIGV4 environment variable is set (not supported yet)");
        return Err(eyre::eyre!("CodeWhisperer client unavailable"));
    }
}
```

**Note on SIGV4:**
The existing code has a `AMAZON_Q_SIGV4` environment variable that switches to `QDeveloperStreamingClient` instead of `CodewhispererStreamingClient`. For MVP, we only support the bearer token path. SIGV4 support can be added later if needed.

### 4.6 Platform Selection

**Challenge:** Allow users to choose between Bedrock and CodeWhisperer.

**Design:**

```rust
// Add Platform enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Platform {
    Bedrock,
    CodeWhisperer,
}

impl Default for Platform {
    fn default() -> Self {
        Platform::CodeWhisperer  // Default to CodeWhisperer
    }
}

// Add to ChatArgs
#[derive(Debug, Clone, Args)]
pub struct ChatArgs {
    /// Platform to use for LLM (bedrock or codewhisperer)
    #[arg(long = "platform", value_enum)]
    pub platform: Option<Platform>,
    
    // ... existing fields
}

// Update execute() - extract provider creation to separate method
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        let platform = self.platform.unwrap_or_default();
        let model_provider = Self::create_model_provider(platform, os).await?;
        
        // ... rest of initialization
    }
    
    async fn create_model_provider(
        platform: Platform,
        os: &mut Os,
    ) -> Result<Arc<dyn ModelProvider>> {
        match platform {
            Platform::Bedrock => {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new("us-east-1"))
                    .load()
                    .await;
                let bedrock_client = BedrockClient::new(&config);
                Ok(Arc::new(BedrockConverseStreamModelProvider::new(bedrock_client)))
            }
            Platform::CodeWhisperer => {
                let api_client = ApiClient::new(&os.env, &os.fs, &mut os.database, None).await?;
                let streaming_client = api_client.streaming_client()
                    .ok_or_else(|| eyre::eyre!(
                        "CodeWhisperer streaming client not available. \
                         Please run 'q login' or check AMAZON_Q_SIGV4 is not set."
                    ))?;
                Ok(Arc::new(CodeWhispererModelProvider::new(streaming_client)))
            }
        }
    }
}
```

**Key decisions:**
- Use clap's `ValueEnum` for type-safe CLI parsing
- **Default to CodeWhisperer** (not Bedrock)
- Extract provider creation to `create_model_provider()` method for readability
- Create provider based on platform selection

**Usage:**
```bash
# Use CodeWhisperer (default)
q chat "Hello"

# Use Bedrock explicitly
q chat --platform=bedrock "Hello"

# Use CodeWhisperer explicitly
q chat --platform=codewhisperer "Hello"
```

**Rationale:**
- Simple, explicit selection
- Type-safe enum prevents typos
- CodeWhisperer as default aligns with primary use case
- Separate method improves code organization and readability

### 4.7 Error Handling

**Challenge:** Map CodeWhisperer SDK errors to user-friendly messages.

**Design:**

```rust
impl CodeWhispererModelProvider {
    async fn request(&self, ...) -> Result<ModelResponse, eyre::Error> {
        let response = tokio::select! {
            result = self.client.send_message().send() => {
                match result {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("CodeWhisperer request failed:");
                        eprintln!("  Error: {}", e);
                        
                        // Provide context-specific guidance
                        if e.to_string().contains("authentication") {
                            eprintln!("  Likely cause: Invalid or expired token");
                            eprintln!("  Solution: Run 'q login' to re-authenticate");
                        } else if e.to_string().contains("rate limit") {
                            eprintln!("  Likely cause: Rate limit exceeded");
                            eprintln!("  Solution: Wait a moment and try again");
                        }
                        
                        return Err(eyre::eyre!("CodeWhisperer request failed: {}", e));
                    }
                }
            },
            _ = cancellation_token.cancelled() => {
                return Err(eyre::eyre!("Request cancelled"));
            }
        };
        
        // ... process stream
    }
}
```

**Error Categories:**
1. **Authentication errors** - Invalid/expired token
2. **Rate limiting** - Too many requests
3. **Network errors** - Connection failures
4. **Cancellation** - User interrupted
5. **Stream errors** - Malformed responses

**Key decisions:**
- Use `tokio::select!` for cancellation support
- Provide actionable error messages
- Log errors for debugging
- Propagate errors with context

**Rationale:**
- Consistent with Bedrock error handling
- User-friendly messages improve UX
- Cancellation support matches existing behavior

---

## 5. Implementation Details

### 5.1 CodeWhispererModelProvider Structure

```rust
use amzn_codewhisperer_streaming_client::Client as CodeWhispererStreamingClient;
use std::sync::Arc;

pub struct CodeWhispererModelProvider {
    client: Arc<CodeWhispererStreamingClient>,
}

impl CodeWhispererModelProvider {
    pub fn new(client: CodeWhispererStreamingClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}
```

**Fields:**
- `client`: Shared reference to CodeWhisperer client

**Note:** `conversation_id` is no longer stored in the provider. It flows through `ModelRequest` from `ContextContainer`.

### 5.2 ModelProvider Trait Implementation

```rust
#[async_trait::async_trait]
impl ModelProvider for CodeWhispererModelProvider {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: Box<dyn Fn() + Send>,
        when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse, eyre::Error> {
        // 1. Build SendMessageInput
        let input = self.build_send_message_input(request)?;
        
        // 2. Send request with cancellation support
        let response = tokio::select! {
            result = self.client.send_message().set_input(Some(input)).send() => {
                result.map_err(|e| eyre::eyre!("CodeWhisperer request failed: {}", e))?
            },
            _ = cancellation_token.cancelled() => {
                return Err(eyre::eyre!("Request cancelled"));
            }
        };
        
        // 3. Signal receiving started
        when_receiving_begin();
        
        // 4. Process stream
        let mut accumulated_content = String::new();
        let mut tool_requests = Vec::new();
        let mut stream = response.chat_response_stream;
        
        loop {
            let event = tokio::select! {
                event = stream.recv() => event,
                _ = cancellation_token.cancelled() => {
                    return Err(eyre::eyre!("Request cancelled"));
                }
            };
            
            match event {
                Ok(Some(stream_event)) => {
                    if cancellation_token.is_cancelled() {
                        return Err(eyre::eyre!("Request cancelled"));
                    }
                    
                    self.process_stream_event(
                        stream_event,
                        &mut accumulated_content,
                        &mut tool_requests,
                        &when_received,
                    )?;
                }
                Ok(None) => break,  // Stream ended
                Err(e) => return Err(eyre::eyre!("Stream error: {}", e)),
            }
        }
        
        // 5. Return final response
        Ok(ModelResponse {
            content: accumulated_content,
            tool_requests,
        })
    }
}
```

### 5.3 Event Processing Logic

```rust
impl CodeWhispererModelProvider {
    fn process_stream_event(
        &self,
        event: ChatResponseStream,
        accumulated_content: &mut String,
        tool_requests: &mut Vec<ToolRequest>,
        when_received: &dyn Fn(ModelResponseChunk),
    ) -> Result<()> {
        match event {
            ChatResponseStream::AssistantResponseEvent(evt) => {
                accumulated_content.push_str(&evt.content);
                when_received(ModelResponseChunk::AssistantMessage(evt.content));
            }
            
            ChatResponseStream::ToolUseEvent(evt) => {
                if let Some(tool_use) = evt.tool_use {
                    let parameters = serde_json::to_string(&tool_use.input)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    tool_requests.push(ToolRequest {
                        tool_name: tool_use.name.clone(),
                        parameters: parameters.clone(),
                    });
                    
                    when_received(ModelResponseChunk::ToolUseRequest {
                        tool_name: tool_use.name,
                        parameters,
                    });
                }
            }
            
            // Log and ignore other events
            _ => {
                // Optional: Log for debugging
                // debug!("Ignoring CodeWhisperer event: {:?}", event);
            }
        }
        
        Ok(())
    }
    
    fn build_send_message_input(&self, request: ModelRequest) -> Result<SendMessageInput> {
        use amzn_codewhisperer_streaming_client::types::*;
        
        let user_message = UserInputMessage::builder()
            .content(request.prompt)
            .build()
            .map_err(|e| eyre::eyre!("Failed to build UserInputMessage: {}", e))?;
        
        // Use conversation_id from request, or generate fallback
        let conversation_id = request.conversation_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        let conversation_state = ConversationState::builder()
            .current_message(ChatMessage::UserInputMessage(user_message))
            .chat_trigger_type(ChatTriggerType::Manual)
            .conversation_id(conversation_id)
            .build()
            .map_err(|e| eyre::eyre!("Failed to build ConversationState: {}", e))?;
        
        let input = SendMessageInput::builder()
            .conversation_state(conversation_state)
            .build()
            .map_err(|e| eyre::eyre!("Failed to build SendMessageInput: {}", e))?;
        
        Ok(input)
    }
}
```

### 5.4 Platform Selection in ChatArgs

**File:** `crates/chat-cli/src/cli/chat/mod.rs`

```rust
use clap::{Args, ValueEnum};
use crate::api_client::ApiClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Platform {
    #[value(name = "bedrock")]
    Bedrock,
    #[value(name = "codewhisperer")]
    CodeWhisperer,
}

impl Default for Platform {
    fn default() -> Self {
        Platform::CodeWhisperer  // Default to CodeWhisperer
    }
}

#[derive(Debug, Clone, Args)]
pub struct ChatArgs {
    /// Platform to use for LLM
    #[arg(long = "platform", value_enum)]
    pub platform: Option<Platform>,
    
    // ... existing fields
}

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        let platform = self.platform.unwrap_or_default();
        let model_provider = Self::create_model_provider(platform, os).await?;
        
        // ... rest of initialization (unchanged)
    }
    
    async fn create_model_provider(
        platform: Platform,
        os: &mut Os,
    ) -> Result<Arc<dyn ModelProvider>> {
        match platform {
            Platform::Bedrock => {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new("us-east-1"))
                    .load()
                    .await;
                let bedrock_client = BedrockClient::new(&config);
                Ok(Arc::new(BedrockConverseStreamModelProvider::new(bedrock_client)))
            }
            Platform::CodeWhisperer => {
                let api_client = ApiClient::new(
                    &os.env,
                    &os.fs,
                    &mut os.database,
                    None,  // Use configured endpoint from database
                ).await
                    .context("Failed to create CodeWhisperer API client")?;
                
                let streaming_client = api_client.streaming_client()
                    .ok_or_else(|| eyre::eyre!(
                        "CodeWhisperer streaming client not available. \
                         Please run 'q login' or check AMAZON_Q_SIGV4 is not set."
                    ))?;
                
                Ok(Arc::new(CodeWhispererModelProvider::new(streaming_client)))
            }
        }
    }
}
```

**Add Accessor to ApiClient:**

**File:** `crates/chat-cli/src/api_client/mod.rs`

```rust
impl ApiClient {
    /// Get the CodeWhisperer streaming client (bearer token auth)
    pub fn streaming_client(&self) -> Option<CodewhispererStreamingClient> {
        self.streaming_client.clone()
    }
}
```

---

## 6. Sequence Diagrams

### Request Flow

```
User → ChatArgs::execute()
  ├─ Create CodeWhispererModelProvider
  ├─ Create Session with provider
  └─ Launch AgentLoop

AgentLoop → ModelProvider::request()
  ├─ Get conversation_id from ContextContainer
  ├─ Build ModelRequest
  │   ├─ prompt: from conversation history
  │   └─ conversation_id: from ContextContainer
  ├─ Build SendMessageInput
  │   ├─ Create UserInputMessage(prompt)
  │   ├─ Create ConversationState
  │   │   ├─ current_message: UserInputMessage
  │   │   ├─ chat_trigger_type: Manual
  │   │   └─ conversation_id: from ModelRequest
  │   └─ Create SendMessageInput
  ├─ Send request to CodeWhisperer
  ├─ Call when_receiving_begin()
  └─ Process stream events
      ├─ AssistantResponseEvent → when_received(AssistantMessage)
      ├─ ToolUseEvent → when_received(ToolUseRequest)
      └─ Other events → ignore
  
ModelProvider → AgentLoop
  └─ Return ModelResponse
      ├─ content: accumulated text
      └─ tool_requests: Vec<ToolRequest>
```

### Streaming Flow

```
CodeWhisperer API → ChatResponseStream
  ├─ AssistantResponseEvent
  │   └─ CodeWhispererModelProvider
  │       ├─ Accumulate content
  │       └─ Call when_received(AssistantMessage)
  │           └─ EventBus.publish(OutputChunk)
  │               └─ UI displays chunk
  │
  ├─ ToolUseEvent
  │   └─ CodeWhispererModelProvider
  │       ├─ Convert Document to JSON
  │       ├─ Store in tool_requests
  │       └─ Call when_received(ToolUseRequest)
  │           └─ EventBus.publish(ToolUse)
  │               └─ UI displays tool request
  │
  └─ Other events (ignored)
```

---

## 7. Open Questions & Future Work

### Open Questions

1. **ContextContainer API:**
   - Does ContextContainer have `get_conversation_id()` method?
   - If not, where should conversation_id be stored?
   - Should it be generated once per Worker or per conversation?

2. **ModelRequest Breaking Change:**
   - Adding `conversation_id` field affects BedrockConverseStreamModelProvider
   - Should Bedrock also use conversation_id?
   - Is this the right time to make this change?

3. **Conversation History:**
   - How to pass conversation history to ModelProvider?
   - Should we extend the trait or use a different approach?
   - Impact on other providers (Bedrock)?

4. **Model Selection:**
   - What models are available in CodeWhisperer?
   - How to specify model ID?
   - Should model selection be per-platform or global?

5. **Tool Result Responses:**
   - How to send tool results back to CodeWhisperer?
   - Do we need to track `tool_use_id` mappings?
   - When will tool execution be implemented?

6. **SIGV4 Authentication:**
   - Should we support AMAZON_Q_SIGV4 mode (QDeveloperStreamingClient)?
   - Is bearer token auth sufficient for MVP?
   - What's the use case for SIGV4?

### Future Enhancements

**Chore: Refactor ChatArgs::execute()**
- Extract remaining initialization logic to separate methods
- Goal: Make execute() as straightforward and simple as possible
- Candidates for extraction:
  - EventBus creation
  - Session creation
  - Worker creation
  - UI initialization
  - AgentEnvironment setup
- Target structure:
  ```rust
  pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
      let platform = self.platform.unwrap_or_default();
      let model_provider = Self::create_model_provider(platform, os).await?;
      let event_bus = Self::create_event_bus();
      let session = Self::create_session(event_bus.clone(), model_provider);
      let worker = Self::create_worker(&session);
      let ui = Self::create_ui(&session, &worker, &self)?;
      let agent_env = Self::create_agent_environment(session, event_bus, ui);
      
      agent_env.run().await
  }
  ```

**Phase 1: Conversation History**
- Extend `ModelRequest` to include conversation history
- Update CodeWhispererModelProvider to populate `ConversationState.history`
- Update BedrockConverseStreamModelProvider to use history

**Phase 2: Tool Execution**
- Implement tool result responses
- Track `tool_use_id` mappings
- Send `ToolResultEvent` in subsequent requests

**Phase 3: Model Selection**
- Add `--model` flag to ChatArgs
- Support platform-specific model IDs
- Document available models per platform

**Phase 4: Advanced Features**
- Support CodeWhisperer-specific events (citations, reasoning)
- Implement conversation persistence
- Add metrics and telemetry

**Phase 5: Additional Platforms**
- Abstract platform selection further
- Support Claude API, GPT, etc.
- Plugin architecture for custom providers

---

## 8. Testing Strategy

### Unit Tests

**CodeWhispererModelProvider:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_build_send_message_input() {
        let provider = CodeWhispererModelProvider::new(mock_client());
        let request = ModelRequest { prompt: "Hello".to_string() };
        let input = provider.build_send_message_input(request).unwrap();
        
        assert!(input.conversation_state.is_some());
        let state = input.conversation_state.unwrap();
        assert_eq!(state.chat_trigger_type, ChatTriggerType::Manual);
    }
    
    #[test]
    fn test_process_assistant_response_event() {
        // Test event processing logic
    }
    
    #[test]
    fn test_process_tool_use_event() {
        // Test tool use conversion
    }
}
```

### Integration Tests

**Basic Conversation:**
```bash
q chat --platform=codewhisperer "What is 2+2?"
# Expected: Streaming response, displays answer
```

**Multi-turn (Limited):**
```bash
q chat --platform=codewhisperer
> Hello
> What is Rust?
# Expected: Both messages work, but no context between them (MVP limitation)
```

**Tool Use (If Supported):**
```bash
q chat --platform=codewhisperer "Read file /tmp/test.txt"
# Expected: Tool use request displayed (execution not implemented yet)
```

**Error Handling:**
```bash
# Test with invalid auth
q chat --platform=codewhisperer "Hello"
# Expected: Clear error message about authentication

# Test cancellation
q chat --platform=codewhisperer "Write a long story"
# Press Ctrl+C
# Expected: Graceful cancellation
```

**Platform Switching:**
```bash
q chat --platform=bedrock "Hello from Bedrock"
q chat --platform=codewhisperer "Hello from CodeWhisperer"
# Expected: Both work correctly
```

### Manual Testing Checklist

- [ ] Basic conversation works
- [ ] Streaming displays correctly
- [ ] Tool use requests appear (if supported)
- [ ] Cancellation works (Ctrl+C)
- [ ] Authentication errors are clear
- [ ] Platform switching works
- [ ] Default platform is Bedrock
- [ ] Long responses stream smoothly
- [ ] Error messages are helpful

---

## Summary

This design provides a minimal, pragmatic approach to integrating CodeWhisperer as an alternative model provider:

**Key Design Choices:**
1. **Conversation ID from ContextContainer** - Flows through ModelRequest for proper tracking
2. **Empty conversation history** - Simplifies MVP, can be enhanced later
3. **Event filtering** - Process only essential events (text, tool use)
4. **Platform enum** - Type-safe, explicit selection with CodeWhisperer as default
5. **Reuse ApiClient infrastructure** - Leverage existing configuration and auth
6. **Consistent error handling** - Match Bedrock's approach
7. **Extracted provider creation** - Separate method improves code readability

**Implementation Scope:**
- Modified: `model_provider.rs` (add conversation_id to ModelRequest)
- Modified: `agent_loop.rs` (extract conversation_id from ContextContainer)
- Modified: `bedrock_converse_stream.rs` (handle new ModelRequest field)
- Modified: `api_client/mod.rs` (add streaming_client() accessor)
- New file: `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`
- Modified file: `crates/chat-cli/src/cli/chat/mod.rs` (Platform enum, ChatArgs)
- Modified file: `crates/chat-cli/src/agent_env/model_providers/mod.rs` (export)

**Estimated Effort:** 8-12 hours
- ModelRequest changes: 1-2 hours
- ApiClient accessor: 0.5 hours
- Core implementation: 4-5 hours
- Platform selection: 2-3 hours
- Testing and refinement: 2-3 hours

**Next Steps:**
1. Add `conversation_id` field to `ModelRequest`
2. Update `AgentLoop` to extract conversation_id from ContextContainer
3. Update `BedrockConverseStreamModelProvider` to handle new field
4. Add `streaming_client()` accessor to `ApiClient`
5. Implement `CodeWhispererModelProvider`
6. Add `Platform` enum and `--platform` flag
7. Test basic conversation flow
8. Test streaming and tool use
9. Document limitations and future work
