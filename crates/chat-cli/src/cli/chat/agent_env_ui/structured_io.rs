use crate::agent_env::{
    AgentEnvironmentCommand, AgentEnvironmentEvent, AgentLoopEvent, PromptResult, Session,
    UserInterface, WorkerEvent, WorkerLifecycleState,
};
use async_trait::async_trait;
use eyre::Result;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// StructuredIO UI implementation
///
/// Reads single-line prompts from stdin and outputs structured JSON events.
/// Suitable for scripting and automation where commands may be piped in.
pub struct StructuredIO {
    session: Arc<Session>,
    main_worker_id: Uuid,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<TokioMutex<Option<mpsc::Receiver<PromptResult>>>>,
    output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>,
    interactive: bool,
}

impl StructuredIO {
    /// Create new StructuredIO instance
    ///
    /// Returns tuple of (StructuredIO, Receiver) following Option C pattern from design.
    /// The receiver should be passed to AgentEnvironment.
    pub fn new(session: Arc<Session>, main_worker_id: Uuid, interactive: bool) -> Result<Self> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);

        Ok(Self {
            session,
            main_worker_id,
            cmd_sender,
            cmd_receiver: Arc::new(TokioMutex::new(Some(cmd_receiver))),
            output_writer: Arc::new(TokioMutex::new(Box::new(std::io::stdout()))),
            interactive,
        })
    }

    /// Spawn input reader task (always reading)
    fn spawn_input_reader(&self) -> JoinHandle<()> {
        let cmd_sender = self.cmd_sender.clone();
        let main_worker_id = self.main_worker_id;

        tokio::spawn(async move {
            tracing::info!("StructuredIO input reader: task started");
            let stdin = tokio::io::stdin();
            let reader = BufReader::new(stdin);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                // Try to parse as JSON command, otherwise treat as plain text prompt
                let result = if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(cmd_str) = json.get("command").and_then(|v| v.as_str()) {
                        match cmd_str {
                            "quit" => PromptResult::Shutdown,
                            _ => {
                                // Unknown command - treat whole line as prompt
                                PromptResult::Command(AgentEnvironmentCommand::Prompt {
                                    worker_id: main_worker_id,
                                    text: line.to_string(),
                                })
                            }
                        }
                    } else {
                        // No command field - treat as prompt
                        PromptResult::Command(AgentEnvironmentCommand::Prompt {
                            worker_id: main_worker_id,
                            text: line.to_string(),
                        })
                    }
                } else {
                    // Not JSON - treat as plain text prompt
                    PromptResult::Command(AgentEnvironmentCommand::Prompt {
                        worker_id: main_worker_id,
                        text: line.to_string(),
                    })
                };

                if cmd_sender.send(result).await.is_err() {
                    break; // Channel closed
                }
            }
        })
    }
}

#[async_trait]
impl UserInterface for StructuredIO {
    async fn start(&self) -> Result<()> {
        if self.interactive {
            tracing::info!("StructuredIO: Starting input reader");
            let _handle = self.spawn_input_reader();
        } else {
            tracing::info!("StructuredIO: Non-interactive mode, skipping input reader");
        }
        Ok(())
    }

    fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
        // Use try_lock since we're in sync context but called from async
        self.cmd_receiver
            .try_lock()
            .expect("command_receiver() called while locked")
            .take()
            .expect("command_receiver() called more than once")
    }

    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Filter to main worker
        if let Some(wid) = event.worker_id() {
            if wid != self.main_worker_id {
                return;
            }
        }

        // Handle events
        match event {
            AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged {
                worker_id,
                new_state,
                ..
            }) => {
                let state_str = match new_state {
                    WorkerLifecycleState::Idle => "idle",
                    WorkerLifecycleState::Busy => "busy",
                    WorkerLifecycleState::IdleFailed => "idle_failed",
                };

                let json = json!({
                    "worker_id": worker_id,
                    "lifecycle_state": state_str,
                });

                let mut writer = self.output_writer.lock().await;
                writeln!(writer, "{}", json).unwrap();
                writer.flush().unwrap();
            }
            AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived {
                worker_id,
                text,
                ..
            }) => {
                let json = json!({
                    "worker_id": worker_id,
                    "assistant_response": text,
                });

                let mut writer = self.output_writer.lock().await;
                writeln!(writer, "{}", json).unwrap();
                writer.flush().unwrap();
            }
            AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ToolUseRequestReceived {
                worker_id,
                tool_name,
                tool_input,
                ..
            }) => {
                let json = json!({
                    "worker_id": worker_id,
                    "tool_use_request": {
                        "tool_name": tool_name,
                        "tool_input": tool_input,
                    }
                });

                let mut writer = self.output_writer.lock().await;
                writeln!(writer, "{}", json).unwrap();
                writer.flush().unwrap();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_env::{
        events::AgentLoopEvent, model_providers::ModelProvider, EventBus, ModelResponseChunk,
    };
    use async_trait::async_trait;
    use eyre::Result;

    // Mock ModelProvider for testing
    struct MockModelProvider;

    #[async_trait]
    impl ModelProvider for MockModelProvider {
        async fn request(
            &self,
            _request: crate::agent_env::ModelRequest,
            _when_receiving_begin: Box<dyn Fn() + Send>,
            _when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
            _cancellation_token: tokio_util::sync::CancellationToken,
        ) -> Result<crate::agent_env::ModelResponse, eyre::Error> {
            Ok(crate::agent_env::ModelResponse {
                content: String::new(),
                tool_requests: vec![],
            })
        }
    }

    fn create_test_session() -> Arc<Session> {
        let event_bus = EventBus::default();
        let model_provider: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
        Arc::new(Session::new(event_bus, vec![model_provider]))
    }

    #[tokio::test]
    async fn test_structured_io_filters_events_by_worker_id() {
        let session = create_test_session();
        let main_worker = session.build_worker("main".to_string());
        let other_worker = session.build_worker("other".to_string());

        let structured_io = StructuredIO::new(session.clone(), main_worker.id).unwrap();

        // Event for main worker - should be processed
        let event1 = AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived {
            worker_id: main_worker.id,
            job_id: Uuid::new_v4(),
            text: "Response for main".to_string(),
            timestamp: std::time::Instant::now(),
        });

        // Event for other worker - should be filtered out
        let event2 = AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived {
            worker_id: other_worker.id,
            job_id: Uuid::new_v4(),
            text: "Response for other".to_string(),
            timestamp: std::time::Instant::now(),
        });

        // Both should complete without error (filtering happens internally)
        structured_io.handle_event(event1).await;
        structured_io.handle_event(event2).await;
    }

    #[tokio::test]
    async fn test_structured_io_outputs_json_for_response() {
        let session = create_test_session();
        let main_worker = session.build_worker("main".to_string());

        let structured_io = StructuredIO::new(session.clone(), main_worker.id).unwrap();

        let event = AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ResponseReceived {
            worker_id: main_worker.id,
            job_id: Uuid::new_v4(),
            text: "Test response".to_string(),
            timestamp: std::time::Instant::now(),
        });

        // Should output JSON without error
        structured_io.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_structured_io_outputs_json_for_tool_use() {
        let session = create_test_session();
        let main_worker = session.build_worker("main".to_string());

        let structured_io = StructuredIO::new(session.clone(), main_worker.id).unwrap();

        let event = AgentEnvironmentEvent::AgentLoop(AgentLoopEvent::ToolUseRequestReceived {
            worker_id: main_worker.id,
            job_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            tool_input: serde_json::json!({"arg": "value"}),
            timestamp: std::time::Instant::now(),
        });

        // Should output JSON without error
        structured_io.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_command_receiver_single_use() {
        let session = create_test_session();
        let main_worker = session.build_worker("main".to_string());

        let structured_io = StructuredIO::new(session.clone(), main_worker.id).unwrap();

        // First call should succeed
        let _receiver = structured_io.command_receiver();

        // Test passes - we verified first call works
        // Second call would panic but we can't easily test that
    }
}
