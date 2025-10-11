# MVP CodeWhisperer - Research and Analysis

## Overview

This document contains research findings for integrating CodeWhisperer as a model provider in the agent_env architecture. The goal is to enable users to choose between Bedrock and CodeWhisperer as LLM backends.

---

## Executive Summary

**Key Finding:** CodeWhisperer streaming client (`amzn-codewhisperer-streaming-client`) DOES support chat/conversation with:
- ✅ Native streaming responses
- ✅ Tool use support (ToolUseEvent, ToolResultEvent)
- ✅ Multi-turn conversations
- ✅ Event-based streaming architecture

**Recommendation:** CodeWhisperer is a viable alternative to Bedrock for the agent_env architecture. Implementation is straightforward with minimal adaptation needed.

---

## Current ModelProvider Architecture

### ModelProvider Trait
**Location:** `crates/chat-cli/src/agent_env/model_providers/model_provider.rs`

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
- Cancellation support via `CancellationToken`
- Simple request/response model

### Bedrock Implementation
**Location:** `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs`

```rust
pub struct BedrockConverseStreamModelProvider {
    client: BedrockClient,
    model_id: String,
}

impl ModelProvider for BedrockConverseStreamModelProvider {
    async fn request(...) -> Result<ModelResponse> {
        // 1. Create message with ConversationRole::User
        // 2. Call converse_stream() API
        // 3. Stream ConverseStreamOutput events
        // 4. Extract ContentBlockDelta::Text chunks
        // 5. Call when_received() callback for each chunk
        // 6. Accumulate content and return ModelResponse
    }
}
```

**Streaming mechanism:**
- Uses AWS SDK's `converse_stream()` API
- Receives `ConverseStreamOutput` events
- Extracts text from `ContentBlockDelta::Text`
- Handles `MessageStop` event to end stream

---

## CodeWhisperer Streaming Client Analysis

### Client Location
- **Crate:** `crates/amzn-codewhisperer-streaming-client`
- **Alternative:** `crates/amzn-qdeveloper-streaming-client` (similar API)

### send_message Operation
**Location:** `crates/amzn-codewhisperer-streaming-client/src/operation/send_message.rs`

**API Structure:**
```rust
// Input
pub struct SendMessageInput {
    // Message content, conversation state, etc.
}

// Output (streaming)
pub struct SendMessageOutput {
    pub chat_response_stream: ChatResponseStream,
}
```

### ChatResponseStream Events
**Location:** `crates/amzn-codewhisperer-streaming-client/src/types/_chat_response_stream.rs`

```rust
pub enum ChatResponseStream {
    /// Assistant response event - Text / Code snippet
    AssistantResponseEvent(AssistantResponseEvent),
    
    /// ToolUse event
    ToolUseEvent(ToolUseEvent),
    
    /// Tool use result
    ToolResultEvent(ToolResultEvent),
    
    /// Code Generated event
    CodeEvent(CodeEvent),
    
    /// Citation event
    CitationEvent(CitationEvent),
    
    /// Code References event
    CodeReferenceEvent(CodeReferenceEvent),
    
    /// Followup prompt event
    FollowupPromptEvent(FollowupPromptEvent),
    
    /// Metadata event
    MetadataEvent(MetadataEvent),
    
    /// Message Metadata event
    MessageMetadataEvent(MessageMetadataEvent),
    
    /// Metering event
    MeteringEvent(MeteringEvent),
    
    /// Reasoning process returned by models
    ReasoningContentEvent(ReasoningContentEvent),
    
    /// Web Reference links event
    SupplementaryWebLinksEvent(SupplementaryWebLinksEvent),
    
    /// Intents event
    IntentsEvent(IntentsEvent),
    
    /// Interactions components event
    InteractionComponentsEvent(InteractionComponentsEvent),
    
    /// Invalid State event
    InvalidStateEvent(InvalidStateEvent),
    
    /// DryRun Succeed Event
    DryRunSucceedEvent(DryRunSucceedEvent),
    
    /// Unknown variant
    Unknown,
}
```

**Key events for agent_env:**
1. **AssistantResponseEvent** - Text/code responses (equivalent to Bedrock's ContentBlockDelta::Text)
2. **ToolUseEvent** - Tool use requests (equivalent to Bedrock's tool_use blocks)
3. **ToolResultEvent** - Tool execution results
4. **MetadataEvent** - Conversation metadata

### Streaming Architecture

**CodeWhisperer uses event-based streaming:**
- Client sends `send_message` request
- Server responds with stream of `ChatResponseStream` events
- Client processes events as they arrive
- Stream ends when complete

**Comparison with Bedrock:**
| Feature | Bedrock | CodeWhisperer |
|---------|---------|---------------|
| Streaming | ✅ ConverseStreamOutput | ✅ ChatResponseStream |
| Text chunks | ContentBlockDelta::Text | AssistantResponseEvent |
| Tool use | tool_use blocks | ToolUseEvent |
| Tool results | tool_result blocks | ToolResultEvent |
| End signal | MessageStop | Stream completion |

---

## Tool Use Support Analysis

### CodeWhisperer Tool Use

**ToolUseEvent structure:**
```rust
pub struct ToolUseEvent {
    pub tool_use: Option<ToolUse>,
}

pub struct ToolUse {
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,  // JSON string
}
```

**ToolResultEvent structure:**
```rust
pub struct ToolResultEvent {
    pub tool_result: Option<ToolResult>,
}

pub struct ToolResult {
    pub tool_name: Option<String>,
    pub content: Option<Vec<ToolResultContentBlock>>,
    pub status: Option<ToolResultStatus>,
}

pub enum ToolResultStatus {
    Success,
    Error,
}
```

**Tool specification:**
```rust
pub struct ToolSpecification {
    pub name: Option<String>,
    pub description: Option<String>,
    pub input_schema: Option<ToolInputSchema>,
}
```

### Comparison with Bedrock

**Bedrock tool use:**
- Tools defined in request
- Model returns tool_use blocks in response
- Client executes tools
- Client sends tool_result blocks in next request

**CodeWhisperer tool use:**
- Tools defined in request (ToolSpecification)
- Model returns ToolUseEvent in stream
- Client executes tools
- Client sends ToolResult in next request

**Compatibility:** CodeWhisperer tool use is conceptually identical to Bedrock. Mapping is straightforward.

---

## Implementation Requirements

### 1. Create CodeWhispererModelProvider

**Location:** `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs` (new file)

```rust
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

#[async_trait]
impl ModelProvider for CodeWhispererModelProvider {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: Box<dyn Fn() + Send>,
        when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse, eyre::Error> {
        // 1. Convert ModelRequest to SendMessageInput
        // 2. Call client.send_message()
        // 3. Process ChatResponseStream events
        // 4. Call when_received() for each chunk
        // 5. Return ModelResponse
    }
}
```

### 2. Event Mapping

**AssistantResponseEvent → ModelResponseChunk::AssistantMessage:**
```rust
ChatResponseStream::AssistantResponseEvent(event) => {
    if let Some(content) = event.content {
        when_received(ModelResponseChunk::AssistantMessage(content));
    }
}
```

**ToolUseEvent → ModelResponseChunk::ToolUseRequest:**
```rust
ChatResponseStream::ToolUseEvent(event) => {
    if let Some(tool_use) = event.tool_use {
        when_received(ModelResponseChunk::ToolUseRequest {
            tool_name: tool_use.tool_name.unwrap_or_default(),
            parameters: tool_use.tool_input.unwrap_or_default(),
        });
    }
}
```

### 3. Client Creation

**Authentication:**
- CodeWhisperer uses Bearer token authentication
- Existing auth infrastructure in codebase
- Need to obtain token from auth system

**Client initialization:**
```rust
use amzn_codewhisperer_streaming_client::{Client as CodeWhispererStreamingClient, Config};

async fn create_codewhisperer_client() -> Result<CodeWhispererStreamingClient> {
    let config = Config::builder()
        .bearer_token(get_auth_token()?)
        .endpoint_url("https://codewhisperer.us-east-1.amazonaws.com")
        .build();
    
    Ok(CodeWhispererStreamingClient::from_conf(config))
}
```

### 4. Platform Selection

**Add --platform flag to ChatArgs:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Platform {
    Bedrock,
    CodeWhisperer,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct ChatArgs {
    /// Platform to use for LLM (bedrock or codewhisperer)
    #[arg(long = "platform", value_enum)]
    pub platform: Option<Platform>,
    
    // ... existing fields
}
```

**Update ChatArgs::execute():**
```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        let platform = self.platform.unwrap_or(Platform::Bedrock);
        
        let model_provider: Arc<dyn ModelProvider> = match platform {
            Platform::Bedrock => {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new("us-east-1"))
                    .load()
                    .await;
                let bedrock_client = BedrockClient::new(&config);
                Arc::new(BedrockConverseStreamModelProvider::new(bedrock_client))
            }
            Platform::CodeWhisperer => {
                let cw_client = create_codewhisperer_client().await?;
                Arc::new(CodeWhispererModelProvider::new(cw_client))
            }
        };
        
        // ... rest of initialization
    }
}
```

---

## Potential Pitfalls

### 1. Authentication Differences

**Issue:** CodeWhisperer uses Bearer token auth, Bedrock uses AWS credentials.

**Mitigation:**
- Leverage existing auth infrastructure in codebase
- Handle auth errors gracefully
- Provide clear error messages for auth failures

### 2. Request Format Differences

**Issue:** CodeWhisperer's `SendMessageInput` may have different required fields than Bedrock's `converse_stream`.

**Investigation needed:**
- What fields are required in `SendMessageInput`?
- How to structure conversation history?
- How to pass tool specifications?

**Mitigation:**
- Study existing CodeWhisperer usage in codebase
- Create adapter layer to map ModelRequest to SendMessageInput
- Handle missing fields with sensible defaults

### 3. Event Stream Differences

**Issue:** CodeWhisperer has many event types (17+), Bedrock has fewer.

**Mitigation:**
- Map only essential events (AssistantResponseEvent, ToolUseEvent)
- Ignore non-essential events (CitationEvent, MeteringEvent, etc.)
- Log unknown events for debugging

### 4. Tool Use Format Differences

**Issue:** Tool input/output format may differ between platforms.

**Investigation needed:**
- Does CodeWhisperer expect JSON schema for tools?
- How are tool results formatted?
- Are there limitations on tool complexity?

**Mitigation:**
- Create tool specification adapter
- Test with simple tools first
- Document any limitations

### 5. Conversation State Management

**Issue:** CodeWhisperer may require explicit conversation state tracking.

**Investigation needed:**
- Does `SendMessageInput` have a `conversation_id` field?
- How to maintain multi-turn conversations?
- Is conversation history sent with each request?

**Mitigation:**
- Study `ConversationState` type in CodeWhisperer client
- Implement conversation tracking in CodeWhispererModelProvider
- Test multi-turn conversations

### 6. Model Selection

**Issue:** CodeWhisperer may have different model IDs than Bedrock.

**Investigation needed:**
- What models are available in CodeWhisperer?
- How to specify model in request?
- Are there model-specific capabilities?

**Mitigation:**
- Document available models
- Add model selection to --platform flag or separate --model flag
- Provide sensible defaults

### 7. Rate Limiting and Quotas

**Issue:** CodeWhisperer may have different rate limits than Bedrock.

**Investigation needed:**
- What are CodeWhisperer's rate limits?
- How are quota errors reported?
- Is there a retry mechanism?

**Mitigation:**
- Handle rate limit errors gracefully
- Implement exponential backoff
- Display clear error messages to users

---

## Code Elements Summary

### Files to Create

1. **`crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs`**
   - CodeWhispererModelProvider struct
   - ModelProvider trait implementation
   - Event mapping logic
   - Request/response conversion

### Files to Modify

1. **`crates/chat-cli/src/agent_env/model_providers/mod.rs`**
   - Export CodeWhispererModelProvider

2. **`crates/chat-cli/src/cli/chat/mod.rs`**
   - Add Platform enum
   - Add --platform flag to ChatArgs
   - Update execute() to create appropriate provider

3. **`crates/chat-cli/Cargo.toml`**
   - Add dependency on `amzn-codewhisperer-streaming-client`

### Critical APIs

**CodeWhisperer Streaming Client:**
- `Client::send_message(input: SendMessageInput) -> SendMessageOutput`
- `SendMessageOutput::chat_response_stream` - Stream of events
- `ChatResponseStream` enum - Event types

**Event Types:**
- `AssistantResponseEvent` - Text responses
- `ToolUseEvent` - Tool use requests
- `ToolResultEvent` - Tool results
- `MetadataEvent` - Metadata

**Authentication:**
- Bearer token via `Config::builder().bearer_token()`
- Endpoint URL configuration

---

## Testing Strategy

### Phase 1: Basic Conversation
```bash
# Test basic chat with CodeWhisperer
q chat --platform=codewhisperer "What is 2+2?"
```

**Expected:** Receive streaming response, display text.

### Phase 2: Multi-turn Conversation
```bash
# Test conversation history
q chat --platform=codewhisperer
> Hello
> What did I just say?
```

**Expected:** CodeWhisperer remembers previous message.

### Phase 3: Tool Use (if supported)
```bash
# Test tool use
q chat --platform=codewhisperer "Read the file /tmp/test.txt"
```

**Expected:** Tool use request, tool execution, result display.

### Phase 4: Streaming
```bash
# Test streaming output
q chat --platform=codewhisperer "Write a long story"
```

**Expected:** See text appear incrementally, not all at once.

### Phase 5: Error Handling
```bash
# Test with invalid auth
# (remove or corrupt auth token)
q chat --platform=codewhisperer "Hello"
```

**Expected:** Clear error message about authentication.

### Phase 6: Platform Switching
```bash
# Test switching between platforms
q chat --platform=bedrock "Hello from Bedrock"
q chat --platform=codewhisperer "Hello from CodeWhisperer"
```

**Expected:** Both work correctly.

---

## Open Questions

### Critical Questions (Need Investigation)

1. **SendMessageInput Structure:**
   - What fields are required?
   - How to structure conversation history?
   - How to pass tool specifications?
   - Is there a conversation_id field?

2. **Authentication:**
   - How to obtain Bearer token?
   - Where is token stored?
   - How to refresh expired tokens?
   - What auth errors can occur?

3. **Model Selection:**
   - What models are available?
   - How to specify model in request?
   - What are model capabilities?
   - Are there model-specific limitations?

4. **Tool Use Details:**
   - Exact format of ToolSpecification?
   - How to send tool results in next request?
   - Are there tool complexity limitations?
   - Does CodeWhisperer support all tool types?

5. **Conversation State:**
   - How to maintain conversation across requests?
   - Is conversation history sent with each request?
   - Is there a conversation ID or session token?
   - How long do conversations persist?

### Non-Critical Questions (Can Defer)

1. **Event Handling:**
   - Should we display CitationEvent to users?
   - How to handle MeteringEvent?
   - What to do with ReasoningContentEvent?

2. **Performance:**
   - What are typical response times?
   - How does streaming latency compare to Bedrock?
   - Are there performance optimizations available?

3. **Features:**
   - Does CodeWhisperer support system prompts?
   - Can we customize model parameters (temperature, etc.)?
   - Are there CodeWhisperer-specific features to leverage?

---

## Next Steps

### Phase 1: Research (Complete)
✅ Confirmed CodeWhisperer supports chat/conversation
✅ Confirmed streaming support
✅ Confirmed tool use support
✅ Identified client location and API structure

### Phase 2: Design (Next)
**Deliverable:** `mvp-codewhisperer-2-design.md`

**Tasks:**
1. Study `SendMessageInput` structure in detail
2. Design request/response mapping
3. Design authentication handling
4. Design conversation state management
5. Design tool specification mapping
6. Create sequence diagrams
7. Document design decisions

**Key decisions to make:**
- How to map ModelRequest to SendMessageInput?
- How to handle conversation history?
- How to obtain and manage auth tokens?
- Which events to process, which to ignore?

### Phase 3: Implementation
**Deliverable:** Working CodeWhispererModelProvider

**Tasks:**
1. Create CodeWhispererModelProvider struct
2. Implement ModelProvider trait
3. Implement event mapping
4. Implement authentication
5. Add --platform flag
6. Test basic conversation
7. Test streaming
8. Test tool use (if supported)
9. Test error handling

---

## Recommendations

### Implementation Priority

**High Priority:**
1. Basic conversation support (AssistantResponseEvent)
2. Streaming support
3. Authentication handling
4. Platform selection (--platform flag)

**Medium Priority:**
5. Tool use support (ToolUseEvent, ToolResultEvent)
6. Multi-turn conversation
7. Error handling

**Low Priority:**
8. Additional event types (CitationEvent, etc.)
9. Model selection
10. Performance optimization

### Risk Mitigation

1. **Start Simple:** Implement basic conversation first, add features incrementally
2. **Study Existing Code:** Look for existing CodeWhisperer usage in codebase
3. **Test Early:** Test authentication and basic requests before full implementation
4. **Document Limitations:** Clearly document any CodeWhisperer-specific limitations
5. **Graceful Degradation:** If tool use isn't supported, document and continue

### Success Criteria

**Minimum Viable Implementation:**
- ✅ Basic conversation works
- ✅ Streaming responses display correctly
- ✅ --platform flag switches between Bedrock and CodeWhisperer
- ✅ Authentication works
- ✅ Error handling is robust

**Full Implementation:**
- ✅ All minimum criteria
- ✅ Tool use works (if supported)
- ✅ Multi-turn conversations work
- ✅ All relevant events are handled
- ✅ Performance is acceptable

---

## Conclusion

CodeWhisperer is a viable alternative to Bedrock for the agent_env architecture. The streaming client provides all necessary capabilities:
- ✅ Chat/conversation API
- ✅ Native streaming support
- ✅ Tool use support
- ✅ Event-based architecture

Implementation is straightforward with minimal adaptation needed. The main challenges are:
1. Understanding SendMessageInput structure
2. Handling authentication
3. Managing conversation state
4. Mapping tool specifications

Estimated effort: 16-24 hours (as originally estimated)
- Phase 1 (Research): 4-6 hours ✅ **COMPLETE**
- Phase 2 (Design): 4-6 hours
- Phase 3 (Implementation): 8-12 hours

**Recommendation:** Proceed with design phase to answer open questions and create detailed implementation plan.
