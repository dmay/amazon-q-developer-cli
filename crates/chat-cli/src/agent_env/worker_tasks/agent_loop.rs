use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{info, debug, error};

use crate::agent_env::{
    Worker, WorkerTask, WorkerStates,
    ModelRequest, ModelResponse,
    EventBus, AgentEnvironmentEvent, JobEvent, AgentLoopEvent, OutputChunk,
};
use crate::agent_env::worker::task_metadata_keys;
use crate::cli::chat::message::{AssistantMessage, UserMessageContent};

pub struct AgentLoopInput {
    // Empty - all context comes from Worker
}

pub struct AgentLoop {
    worker: Arc<Worker>,
    event_bus: EventBus,
    cancellation_token: CancellationToken,
}

impl AgentLoop {
    pub fn new(
        worker: Arc<Worker>,
        _input: AgentLoopInput,
        event_bus: EventBus,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            worker,
            event_bus,
            cancellation_token,
        }
    }

    fn check_cancellation(&self) -> Result<(), eyre::Error> {
        if self.cancellation_token.is_cancelled() {
            debug!(worker_id = %self.worker.id, "Cancelled");
            Err(eyre::eyre!("Cancelled"))
        } else {
            Ok(())
        }
    }

    async fn query_llm(&self) -> Result<ModelResponse, eyre::Error> {
        self.check_cancellation()?;
        
        // Get prompt from worker's context
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
        };  // Lock dropped here

        let request = ModelRequest { prompt };

        self.worker.set_state(WorkerStates::Requesting);
        
        let worker = self.worker.clone();
        let worker_id = self.worker.id;
        
        let model_provider = self.worker.model_provider.as_ref()
            .ok_or_else(|| eyre::eyre!("model_provider not available"))?;
        
        let response = model_provider.request(
            request,
            Box::new(move || {
                worker.set_state(WorkerStates::Receiving);
            }),
            Box::new(move |_chunk| {
                // Chunk handling stubbed out - will be replaced by EventBus
            }),
            self.cancellation_token.clone(),
        ).await.map_err(|e| {
            if !self.cancellation_token.is_cancelled() {
                let error_msg = format!("LLM request failed: {}", e);
                error!(worker_id = %self.worker.id, error = %e, "LLM request failed");
                self.worker.set_failure(error_msg);
                self.worker.set_state(WorkerStates::InactiveFailed);
            } else {
                self.worker.set_state(WorkerStates::Inactive);
            }
            e
        })?;

        debug!(
            worker_id = %self.worker.id,
            content_len = response.content.len(),
            tool_count = response.tool_requests.len(),
            "LLM response received"
        );

        Ok(response)
    }
}

#[async_trait::async_trait]
impl WorkerTask for AgentLoop {
    fn get_worker(&self) -> &Worker {
        &self.worker
    }

    async fn run(&self) -> Result<(), eyre::Error> {
        let start = Instant::now();
        let job_id = uuid::Uuid::new_v4(); // TODO: Get from WorkerJob
        info!(worker_id = %self.worker.id, "Agent loop started");

        self.check_cancellation()?;
        self.worker.set_failure("".to_string());
        self.worker.set_state(WorkerStates::Working);

        let response = self.query_llm().await?;

        // Publish OutputChunk event for assistant response text
        if !response.content.is_empty() {
            self.event_bus.publish(AgentEnvironmentEvent::Job(
                JobEvent::OutputChunk {
                    worker_id: self.worker.id,
                    job_id,
                    chunk: OutputChunk::AssistantResponse(response.content.clone()),
                    timestamp: Instant::now(),
                }
            ));
        }

        // Publish AgentLoopEvent for complete response
        self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
            AgentLoopEvent::ResponseReceived {
                worker_id: self.worker.id,
                job_id,
                text: response.content.clone(),
                timestamp: Instant::now(),
            }
        ));

        // Publish events for tool use requests
        for tool_request in &response.tool_requests {
            // Parse parameters as JSON
            let tool_input: serde_json::Value = serde_json::from_str(&tool_request.parameters)
                .unwrap_or_else(|_| serde_json::Value::String(tool_request.parameters.clone()));

            // Publish OutputChunk event for tool use
            self.event_bus.publish(AgentEnvironmentEvent::Job(
                JobEvent::OutputChunk {
                    worker_id: self.worker.id,
                    job_id,
                    chunk: OutputChunk::ToolUse {
                        tool_name: tool_request.tool_name.clone(),
                        tool_input: tool_input.clone(),
                    },
                    timestamp: Instant::now(),
                }
            ));

            // Publish AgentLoopEvent for tool use request
            self.event_bus.publish(AgentEnvironmentEvent::AgentLoop(
                AgentLoopEvent::ToolUseRequestReceived {
                    worker_id: self.worker.id,
                    job_id,
                    tool_name: tool_request.tool_name.clone(),
                    tool_input,
                    timestamp: Instant::now(),
                }
            ));
        }

        // Create assistant message and add to history
        let assistant_message = if response.tool_requests.is_empty() {
            AssistantMessage::new_response(None, response.content.clone())
        } else {
            // For now, create a simple response. Tool support will be added later.
            AssistantMessage::new_response(None, response.content.clone())
        };

        self.worker.context_container
            .conversation_history
            .lock()
            .unwrap()
            .push_assistant_message(assistant_message);

        // Set completion state metadata
        if !response.tool_requests.is_empty() {
            // Tool approval needed (for future implementation)
            self.worker.set_task_metadata(
                task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
                serde_json::Value::String("completed_with_tool_request".to_string()),
            );
            info!(
                worker_id = %self.worker.id,
                tool_count = response.tool_requests.len(),
                "Tool requests accumulated - approval needed"
            );
        } else {
            // Normal completion - ready for new prompt
            self.worker.set_task_metadata(
                task_metadata_keys::AGENT_LOOP_COMPLETION_STATE,
                serde_json::Value::String("completed_ready_for_prompt".to_string()),
            );
        }

        self.worker.set_state(WorkerStates::Inactive);
        
        let elapsed = start.elapsed();
        info!(
            worker_id = %self.worker.id,
            duration_ms = elapsed.as_millis(),
            "Agent loop completed"
        );

        Ok(())
    }
}
