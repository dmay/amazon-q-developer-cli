# MVP CodeWhisperer - Scope

## Overview

This workflow covers the integration of CodeWhisperer as a model provider in the agent_env architecture. Currently, only Bedrock is supported. Adding CodeWhisperer support will enable users to choose between different LLM backends.

## Tasks Included

### 1.5: CodeWhisperer Model Provider

---

## Task 1.5: CodeWhisperer Model Provider

### Current State

**Existing Model Provider:**
- Only BedrockConverseStreamModelProvider exists
- ModelProvider trait defines the interface
- Bedrock uses Converse API with streaming
- Bedrock supports tool use via tool_use blocks

**CodeWhisperer Client:**
- CodeWhisperer client exists in codebase (amzn_codewhisperer_client)
- Used for code completions and other features
- Chat/conversation API needs investigation
- Streaming support needs investigation
- Tool use support needs investigation

**Current Architecture:**
```rust
pub trait ModelProvider: Send + Sync {
    async fn query(&self, request: ModelRequest) -> Result<ModelResponse>;
}

pub struct ModelRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub model_id: String,
}

pub struct ModelResponse {
    pub content: String,
    pub tool_uses: Vec<ToolUse>,
    pub stop_reason: StopReason,
}
```

### Requirements

**Core Functionality:**
- Implement CodeWhispererModelProvider
- Support streaming responses (if available)
- Support conversation/chat API
- Handle tool use (if supported by CodeWhisperer)
- Add --platform flag to select provider (Bedrock or CodeWhisperer)
- Maintain compatibility with existing ModelProvider trait

**Platform Selection:**
```bash
# Use Bedrock (default)
q chat "Hello"

# Use CodeWhisperer
q chat --platform=codewhisperer "Hello"

# Or environment variable
export Q_PLATFORM=codewhisperer
q chat "Hello"
```

**Authentication:**
- CodeWhisperer authentication (Builder ID or IAM)
- Session management
- Token refresh
- Error handling for auth failures

### Key Challenges

1. **API Discovery:**
   - What is CodeWhisperer's chat/conversation API?
   - Does it support streaming?
   - What is the request/response format?
   - How does it differ from Bedrock?

2. **Streaming Support:**
   - Does CodeWhisperer support streaming responses?
   - If yes, what is the streaming format?
   - If no, how to simulate streaming for UI?

3. **Tool Use Support:**
   - Does CodeWhisperer support tool use?
   - If yes, what is the format?
   - If no, how to handle tool requests?
   - Compatibility with Bedrock tool format?

4. **Model Selection:**
   - What models are available in CodeWhisperer?
   - How to specify model ID?
   - Default model selection?

5. **Error Handling:**
   - CodeWhisperer-specific errors
   - Rate limiting
   - Quota exceeded
   - Service unavailable

### Research Questions

**Critical Questions:**
1. **Chat API:**
   - Does CodeWhisperer have a chat/conversation API?
   - What is the endpoint and request format?
   - What models are available?

2. **Streaming:**
   - Does the API support streaming responses?
   - What is the streaming protocol? (SSE, WebSocket, chunked transfer?)

3. **Tool Use:**
   - Does CodeWhisperer support tool/function calling?
   - If yes, what is the format?
   - How does it compare to Bedrock's tool_use blocks?

4. **Authentication:**
   - What authentication methods are supported?
   - How to obtain and refresh tokens?
   - Session management?

5. **Limitations:**
   - Rate limits?
   - Token limits?
   - Concurrent request limits?

### Design Phases

#### Phase 1: Research (4-6 hours)
**Deliverable:** `mvp-codewhisperer-1-research.md`

**Research Tasks:**
- Study CodeWhisperer client in codebase
- Research CodeWhisperer API documentation
- Identify chat/conversation endpoints
- Determine streaming support
- Research tool use support
- Document authentication flow
- Identify API limitations
- Compare with Bedrock API

**Key Questions to Answer:**
- What is the CodeWhisperer chat API?
- Does it support streaming?
- Does it support tool use?
- What are the authentication requirements?
- What are the API limitations?

#### Phase 2: Design (4-6 hours)
**Deliverable:** `mvp-codewhisperer-2-design.md`

**Design Tasks:**
- Design CodeWhispererModelProvider implementation
- Design request/response mapping
- Design streaming implementation (if supported)
- Design tool use integration (if supported)
- Design authentication handling
- Design error handling
- Plan --platform flag implementation
- Create sequence diagrams

**Design Decisions:**
- How to map ModelRequest to CodeWhisperer format?
- How to map CodeWhisperer response to ModelResponse?
- How to handle streaming (native or simulated)?
- How to handle tool use (native, adapted, or unsupported)?
- Where to store platform selection? (CLI arg, config, env var?)


#### Phase 3: Implementation (8-12 hours)
**Deliverable:** Working CodeWhisperer model provider

**Implementation Tasks:**
1. Create CodeWhispererModelProvider struct
2. Implement ModelProvider trait
3. Implement request formatting
4. Implement response parsing
5. Implement streaming (native or simulated)
6. Implement tool use (if supported)
7. Implement authentication handling
8. Implement error handling
9. Add --platform flag to ChatArgs
10. Update ChatArgs::execute() to select provider
11. Test basic conversation flow
12. Test streaming output
13. Test error scenarios
14. Test authentication

### Proposed Architecture

**CodeWhispererModelProvider:**
```rust
pub struct CodeWhispererModelProvider {
    client: Arc<CodeWhispererClient>,
    model_id: String,
}

impl CodeWhispererModelProvider {
    pub fn new(client: CodeWhispererClient, model_id: String) -> Self {
        Self {
            client: Arc::new(client),
            model_id,
        }
    }
}

#[async_trait]
impl ModelProvider for CodeWhispererModelProvider {
    async fn query(&self, request: ModelRequest) -> Result<ModelResponse> {
        // Convert ModelRequest to CodeWhisperer format
        let cw_request = self.format_request(request)?;
        
        // Send request to CodeWhisperer
        let cw_response = self.client.send_message(cw_request).await?;
        
        // Convert CodeWhisperer response to ModelResponse
        let response = self.parse_response(cw_response)?;
        
        Ok(response)
    }
}
```

**Platform Selection:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Platform {
    Bedrock,
    CodeWhisperer,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct ChatArgs {
    // ... existing fields ...
    
    /// Platform to use for LLM (bedrock or codewhisperer)
    #[arg(long = "platform", value_enum)]
    pub platform: Option<Platform>,
}

impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // Select platform
        let platform = self.platform.unwrap_or(Platform::Bedrock);
        
        // Create model provider based on platform
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
                let cw_client = create_codewhisperer_client(os).await?;
                Arc::new(CodeWhispererModelProvider::new(cw_client, "default".to_string()))
            }
        };
        
        // ... rest of initialization ...
    }
}
```

**Streaming Strategies:**

**Option A: Native Streaming (if supported)**
```rust
impl CodeWhispererModelProvider {
    async fn query(&self, request: ModelRequest) -> Result<ModelResponse> {
        let mut stream = self.client.send_message_stream(request).await?;
        let mut content = String::new();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            content.push_str(&chunk.text);
            
            // Publish streaming event
            self.event_bus.publish(JobEvent::OutputChunk {
                job_id: job_id.clone(),
                chunk: chunk.text,
            });
        }
        
        Ok(ModelResponse {
            content,
            tool_uses: vec![],
            stop_reason: StopReason::EndTurn,
        })
    }
}
```

**Option B: Simulated Streaming (if not supported)**
```rust
impl CodeWhispererModelProvider {
    async fn query(&self, request: ModelRequest) -> Result<ModelResponse> {
        // Get complete response
        let response = self.client.send_message(request).await?;
        
        // Simulate streaming by chunking
        for chunk in response.text.chars().collect::<Vec<_>>().chunks(10) {
            let chunk_str: String = chunk.iter().collect();
            
            self.event_bus.publish(JobEvent::OutputChunk {
                job_id: job_id.clone(),
                chunk: chunk_str,
            });
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        Ok(ModelResponse {
            content: response.text,
            tool_uses: vec![],
            stop_reason: StopReason::EndTurn,
        })
    }
}
```

### Acceptance Criteria

**Phase 1 (Research):**
- Complete understanding of CodeWhisperer chat API
- Documentation of streaming support
- Documentation of tool use support
- Clear authentication requirements

**Phase 2 (Design):**
- Complete CodeWhispererModelProvider design
- Request/response mapping defined
- Streaming strategy decided
- Tool use strategy decided
- Design review and approval

**Phase 3 (Implementation):**
- CodeWhispererModelProvider implemented
- ModelProvider trait implemented correctly
- Streaming works (native or simulated)
- Tool use works (if supported)
- Authentication works
- --platform flag works
- Basic conversation works
- Error handling is robust
- Tests passing

### Testing Strategy

```bash
# Test basic conversation
q chat --platform=codewhisperer "What is 2+2?"

# Test streaming
q chat --platform=codewhisperer "Write a long story"
# (should see streaming output)

# Test multi-turn conversation
q chat --platform=codewhisperer
> Hello
> What is my previous message?
# (should maintain context)

# Test tool use (if supported)
q chat --platform=codewhisperer "Read the file /tmp/test.txt"

# Test error handling
q chat --platform=codewhisperer --model=invalid-model "Hello"
# (should handle error gracefully)

# Test authentication
# (test with invalid credentials)
# (should report auth error clearly)

# Test platform switching
q chat --platform=bedrock "Hello"
q chat --platform=codewhisperer "Hello"
# (both should work)
```

### Dependencies

- CodeWhisperer client (existing)
- ModelProvider trait (existing)
- Authentication system (existing)

### Estimated Effort

**Total: 16-24 hours**
- Phase 1 (Research): 4-6 hours
- Phase 2 (Design): 4-6 hours
- Phase 3 (Implementation): 8-12 hours

**Note:** Effort depends heavily on CodeWhisperer API capabilities. If streaming or tool use are not supported, implementation may be simpler but require workarounds.

---

## Success Metrics

- CodeWhisperer provider works for basic chat
- Streaming responses display correctly (native or simulated)
- --platform flag switches between providers
- Authentication works correctly
- Error handling is robust
- Multi-turn conversations work
- Tool use works (if supported by CodeWhisperer)

---

## Risks and Mitigation

**Risk 1: Limited API Capabilities**
- CodeWhisperer may not support chat/conversation API
- Mitigation: Research thoroughly, consider alternatives, graceful degradation

**Risk 2: No Streaming Support**
- May impact user experience
- Mitigation: Implement simulated streaming

**Risk 3: No Tool Use Support**
- May limit functionality
- Mitigation: Document limitation, consider future enhancement

**Risk 4: Authentication Complexity**
- CodeWhisperer auth may be complex
- Mitigation: Leverage existing auth code, thorough testing

**Risk 5: API Differences**
- CodeWhisperer API may differ significantly from Bedrock
- Mitigation: Abstract differences in ModelProvider implementation

---

## Alternative Approaches

**If CodeWhisperer doesn't support chat:**

**Option A: Use Code Completion API**
- Adapt code completion API for chat
- May have limitations
- Not ideal but functional

**Option B: Proxy Through Another Service**
- Use CodeWhisperer indirectly
- Adds complexity
- May not be feasible

**Option C: Document as Unsupported**
- Clearly document that CodeWhisperer is not supported for chat
- Focus on Bedrock for MVP
- Revisit in future

---

## Related Files

**Existing Code:**
- `crates/amzn_codewhisperer_client/` - CodeWhisperer client
- `crates/chat-cli/src/agent_env/model_providers/model_provider.rs` - ModelProvider trait
- `crates/chat-cli/src/agent_env/model_providers/bedrock_converse_stream.rs` - Bedrock implementation
- `crates/chat-cli/src/cli/chat/mod.rs` - ChatArgs::execute()

**New Files (to be created):**
- `crates/chat-cli/src/agent_env/model_providers/codewhisperer.rs` - CodeWhisperer implementation

---

## Documentation Updates

After implementation:
- Document --platform flag
- Document CodeWhisperer setup and authentication
- Document differences between Bedrock and CodeWhisperer
- Add troubleshooting guide
- Update README with platform options
- Document limitations (if any)

---

## Future Enhancements

- Support for additional platforms (Claude, GPT, etc.)
- Platform-specific features
- Model selection per platform
- Platform-specific optimizations
- Fallback between platforms
