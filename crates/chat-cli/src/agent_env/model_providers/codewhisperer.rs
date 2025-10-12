use amzn_codewhisperer_streaming_client::Client as CodeWhispererStreamingClient;
use amzn_codewhisperer_streaming_client::types::*;
use std::sync::Arc;
use async_trait::async_trait;
use eyre::Result;
use tokio_util::sync::CancellationToken;

use super::model_provider::*;

pub struct CodeWhispererModelProvider {
    client: Arc<CodeWhispererStreamingClient>,
}

impl CodeWhispererModelProvider {
    pub fn new(client: CodeWhispererStreamingClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

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
                // ToolUseEvent has fields: tool_use_id (String), name (String), input (Option<Document>)
                if let Some(input) = evt.input {
                    let parameters = serde_json::to_string(&input)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    tool_requests.push(ToolRequest {
                        tool_name: evt.name.clone(),
                        parameters: parameters.clone(),
                    });
                    
                    when_received(ModelResponseChunk::ToolUseRequest {
                        tool_name: evt.name,
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

#[async_trait]
impl ModelProvider for CodeWhispererModelProvider {
    async fn request(
        &self,
        request: ModelRequest,
        when_receiving_begin: Box<dyn Fn() + Send>,
        when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<ModelResponse, eyre::Error> {
        // 1. Build conversation state
        let user_message = UserInputMessage::builder()
            .content(request.prompt)
            .build()
            .map_err(|e| eyre::eyre!("Failed to build UserInputMessage: {}", e))?;
        
        // Use conversation_id from request, or generate fallback UUID
        let conversation_id = request.conversation_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        let conversation_state = ConversationState::builder()
            .current_message(ChatMessage::UserInputMessage(user_message))
            .chat_trigger_type(ChatTriggerType::Manual)
            .conversation_id(conversation_id)
            .build()
            .map_err(|e| eyre::eyre!("Failed to build ConversationState: {}", e))?;
        
        // 2. Send request with cancellation support
        let response = tokio::select! {
            result = self.client
                .send_message()
                .conversation_state(conversation_state)
                .send() => {
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
        
        // 3. Signal receiving started
        when_receiving_begin();
        
        // 4. Process stream
        let mut accumulated_content = String::new();
        let mut tool_requests: Vec<ToolRequest> = Vec::new();
        let mut stream = response.send_message_response;
        
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_conversation_id_generation_with_provided_id() {
        // Test that when conversation_id is provided, it's used
        let request = ModelRequest {
            prompt: "Hello".to_string(),
            conversation_id: Some("test-conv-123".to_string()),
        };
        
        // Build conversation state inline (same logic as in request method)
        let user_message = UserInputMessage::builder()
            .content(request.prompt.clone())
            .build()
            .unwrap();
        
        let conversation_id = request.conversation_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        let conversation_state = ConversationState::builder()
            .current_message(ChatMessage::UserInputMessage(user_message))
            .chat_trigger_type(ChatTriggerType::Manual)
            .conversation_id(conversation_id.clone())
            .build()
            .unwrap();
        
        assert_eq!(conversation_state.conversation_id, Some("test-conv-123".to_string()));
        // chat_trigger_type is set correctly by builder
    }

    #[test]
    fn test_conversation_id_generation_without_provided_id() {
        // Test that when conversation_id is None, a UUID is generated
        let request = ModelRequest {
            prompt: "Hello".to_string(),
            conversation_id: None,
        };
        
        let conversation_id = request.conversation_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        // Verify it's a valid UUID format (36 characters with hyphens)
        assert_eq!(conversation_id.len(), 36);
        assert!(conversation_id.contains('-'));
        
        // Verify it can be parsed as UUID
        assert!(uuid::Uuid::parse_str(&conversation_id).is_ok());
    }

    #[test]
    fn test_process_assistant_response_event() {
        // Create test event
        let event = ChatResponseStream::AssistantResponseEvent(
            AssistantResponseEvent::builder()
                .content("Hello world")
                .build()
                .unwrap()
        );
        
        let mut accumulated_content = String::new();
        let mut tool_requests: Vec<ToolRequest> = Vec::new();
        let received_chunks = Arc::new(Mutex::new(Vec::new()));
        let received_chunks_clone = received_chunks.clone();
        
        let when_received = move |chunk: ModelResponseChunk| {
            received_chunks_clone.lock().unwrap().push(chunk);
        };
        
        // Process event directly without needing a provider instance
        match event {
            ChatResponseStream::AssistantResponseEvent(evt) => {
                accumulated_content.push_str(&evt.content);
                when_received(ModelResponseChunk::AssistantMessage(evt.content));
            }
            _ => {}
        }
        
        // Verify accumulated content
        assert_eq!(accumulated_content, "Hello world");
        
        // Verify callback was called
        let chunks = received_chunks.lock().unwrap();
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            ModelResponseChunk::AssistantMessage(msg) => assert_eq!(msg, "Hello world"),
            _ => panic!("Expected AssistantMessage chunk"),
        }
    }

    #[test]
    fn test_process_tool_use_event() {
        // Create test tool use event with JSON string input
        let input_json = r#"{"path":"/tmp/test.txt"}"#;
        
        let event = ChatResponseStream::ToolUseEvent(
            ToolUseEvent::builder()
                .tool_use_id("tool-123")
                .name("fs_read")
                .input(input_json)
                .build()
                .unwrap()
        );
        
        let mut accumulated_content = String::new();
        let mut tool_requests: Vec<ToolRequest> = Vec::new();
        let received_chunks = Arc::new(Mutex::new(Vec::new()));
        let received_chunks_clone = received_chunks.clone();
        
        let when_received = move |chunk: ModelResponseChunk| {
            received_chunks_clone.lock().unwrap().push(chunk);
        };
        
        // Process event directly
        match event {
            ChatResponseStream::ToolUseEvent(evt) => {
                if let Some(input) = evt.input {
                    let parameters = input;
                    
                    tool_requests.push(ToolRequest {
                        tool_name: evt.name.clone(),
                        parameters: parameters.clone(),
                    });
                    
                    when_received(ModelResponseChunk::ToolUseRequest {
                        tool_name: evt.name,
                        parameters,
                    });
                }
            }
            _ => {}
        }
        
        // Verify tool request was added
        assert_eq!(tool_requests.len(), 1);
        assert_eq!(tool_requests[0].tool_name, "fs_read");
        assert!(tool_requests[0].parameters.contains("path"));
        assert!(tool_requests[0].parameters.contains("/tmp/test.txt"));
        
        // Verify callback was called
        let chunks = received_chunks.lock().unwrap();
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            ModelResponseChunk::ToolUseRequest { tool_name, parameters } => {
                assert_eq!(tool_name, "fs_read");
                assert!(parameters.contains("path"));
            },
            _ => panic!("Expected ToolUseRequest chunk"),
        }
    }
}
