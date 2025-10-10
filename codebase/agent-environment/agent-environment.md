# AgentEnvironment - Top-Level Coordinator

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

## Overview

AgentEnvironment is the top-level coordinator that manages event multicasting, UI coordination, and command processing. It supports multiple concurrent UIs (one main interactive UI + multiple headless UIs) and coordinates the entire application lifecycle.

## Architecture

```
AgentEnvironment
├─ Event Multicast Task (always running)
│  └─ Broadcasts events to all UIs
├─ Main UI (optional, interactive)
│  ├─ Event processor (via handle_event)
│  ├─ Prompt loop task
│  └─ Command channel
└─ Headless UIs (multiple, non-interactive)
   └─ Event processors (via handle_event)
```

## Structure

```rust
pub struct AgentEnvironment {
    session: Arc<Session>,
    event_bus: EventBus,
    main_ui: Option<Arc<dyn UserInterface>>,
    headless_uis: Vec<Arc<dyn HeadlessInterface>>,
    shutdown_signal: Arc<Notify>,
}
```

## UI Trait Definitions

### UserInterface (Main Interactive UI)

```rust
#[async_trait]
pub trait UserInterface: Send + Sync {
    /// Start UI (spawns tasks, returns immediately)
    async fn start(&self) -> Result<()>;
    
    /// Get receiver for commands from this UI
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult>;
    
    /// Handle event from EventBus (called by AgentEnvironment)
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

### HeadlessInterface (Non-Interactive UI)

```rust
#[async_trait]
pub trait HeadlessInterface: Send + Sync {
    /// Handle event from EventBus
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

## API

### Creating AgentEnvironment

```rust
// With main UI
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![], // No headless UIs
);

// Headless mode (no main UI)
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    None,
    vec![Arc::new(web_api)], // Headless UIs only
);

// Multiple UIs
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![Arc::new(web_api), Arc::new(logger)],
);
```

### Running AgentEnvironment

```rust
// Blocks until shutdown
agent_env.run().await?;
```

### Triggering Shutdown

```rust
// From another task
agent_env.shutdown();

// From UI
cmd_sender.send(PromptResult::Shutdown).await?;
```

## Main Run Loop

### With Main UI

```rust
pub async fn run(&self) -> Result<()> {
    // Spawn event multicast task
    let multicast_handle = self.spawn_event_multicast();
    
    if let Some(ui) = &self.main_ui {
        // Start UI (spawns its own tasks)
        ui.start().await?;
        
        // Get command receiver from UI
        let mut cmd_receiver = ui.command_receiver();
        
        // Main loop: process commands
        loop {
            tokio::select! {
                Some(result) = cmd_receiver.recv() => {
                    match result {
                        PromptResult::Command(cmd) => {
                            self.handle_command(cmd).await?;
                        }
                        PromptResult::Shutdown => break,
                    }
                }
                _ = self.shutdown_signal.notified() => break,
            }
        }
    } else {
        // Headless mode - just wait for shutdown
        self.shutdown_signal.notified().await;
    }
    
    // Cleanup
    multicast_handle.abort();
    self.session.cancel_all_jobs();
    Ok(())
}
```

### Headless Mode

When no main UI is provided:
- AgentEnvironment waits for shutdown signal
- Headless UIs continue receiving events
- Useful for background processing, web APIs, logging

## Event Multicasting

### Implementation

```rust
fn spawn_event_multicast(&self) -> JoinHandle<()> {
    let mut receiver = self.event_bus.subscribe();
    let headless_uis = self.headless_uis.clone();
    let main_ui = self.main_ui.clone();
    let shutdown = self.shutdown_signal.clone();
    
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(event) = receiver.recv() => {
                    // Forward to main UI
                    if let Some(ui) = &main_ui {
                        ui.handle_event(event.clone()).await;
                    }
                    
                    // Forward to headless UIs
                    for headless_ui in &headless_uis {
                        headless_ui.handle_event(event.clone()).await;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Event bus lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
                _ = shutdown.notified() => break,
            }
        }
    })
}
```

### Key Features

- **Non-blocking**: Runs in separate task, never blocks event publishing
- **Resilient**: Handles lagged events gracefully
- **Flexible**: Supports any number of UIs
- **Coordinated**: Single point for event distribution

## Command Processing

### Command Types

```rust
pub enum AgentEnvironmentCommand {
    Prompt { worker_id: Uuid, text: String },
    Compact { worker_id: Uuid, instruction: Option<String> },
    Quit,
}
```

### Command Handler

```rust
async fn handle_command(&self, cmd: AgentEnvironmentCommand) -> Result<()> {
    match cmd {
        AgentEnvironmentCommand::Prompt { worker_id, text } => {
            let worker = self.session.get_worker(worker_id)?;
            
            // Add message to conversation history
            worker.context_container
                .conversation_history
                .lock()
                .unwrap()
                .push_input_message(text);
            
            // Launch agent loop
            self.session.run_task__agent_loop(worker, AgentLoopInput {})?;
        }
        
        AgentEnvironmentCommand::Compact { worker_id, instruction } => {
            let worker = self.session.get_worker(worker_id)?;
            self.session.run_task__compact_conversation(
                worker,
                CompactInput { instruction }
            )?;
        }
        
        AgentEnvironmentCommand::Quit => {
            self.shutdown_signal.notify_waiters();
        }
    }
    Ok(())
}
```

## Lifecycle Management

### Startup Sequence

1. Create EventBus, Session, Workers
2. Create UI implementations
3. Create AgentEnvironment with Session, EventBus, UIs
4. Call `agent_env.run()` (blocks)
5. AgentEnvironment spawns event multicast task
6. AgentEnvironment starts main UI (if present)
7. Main loop begins processing commands

### Shutdown Sequence

1. Shutdown signal received (Ctrl+C, /quit, or programmatic)
2. Main loop exits
3. Event multicast task aborted
4. All jobs cancelled via `session.cancel_all_jobs()`
5. Resources cleaned up
6. `run()` returns

## Usage Examples

### Example 1: Text-Based Chat

```rust
// Create components
let event_bus = EventBus::default();
let session = Arc::new(Session::new(event_bus.clone(), model_providers));
let worker = session.build_worker("main".to_string());

// Create TextUi
let (text_ui, cmd_receiver) = TextUi::new(
    session.clone(),
    worker.id,
    Some(history_path),
)?;

// Create AgentEnvironment
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![],
);

// Run (blocks until shutdown)
agent_env.run().await?;
```

### Example 2: Structured I/O for Scripting

```rust
// Create StructuredIO instead of TextUi
let (structured_io, cmd_receiver) = StructuredIO::new(
    session.clone(),
    worker.id,
)?;

let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(structured_io)),
    vec![],
);

agent_env.run().await?;
```

### Example 3: Headless with Web API

```rust
// Create web API as headless UI
let web_api = Arc::new(WebApi::new(session.clone(), event_bus.clone()));

// No main UI, only headless
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    None,
    vec![web_api],
);

agent_env.run().await?;
```

### Example 4: Multiple UIs

```rust
// Main interactive UI
let (text_ui, _) = TextUi::new(session.clone(), worker.id, None)?;

// Headless UIs
let web_api = Arc::new(WebApi::new(session.clone(), event_bus.clone()));
let logger = Arc::new(EventLogger::new());

// Combine them
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![web_api, logger],
);

agent_env.run().await?;
```

## Design Benefits

### Non-Blocking Architecture

- Event multicast runs independently of command processing
- UIs never block event delivery
- Commands processed asynchronously
- Responsive user experience

### Flexible UI Support

- Any combination of main UI + headless UIs
- Easy to add new UI types
- UIs can be swapped without changing core logic
- Supports headless mode for automation

### Clean Separation of Concerns

- AgentEnvironment: Coordination and lifecycle
- Session: Worker and job management
- UIs: Display and user interaction
- EventBus: Communication

### Testable

- Can run without any UI (headless mode)
- Mock UIs for testing
- Event-driven makes testing easier
- Clear boundaries between components

## Related Documentation

- [EventBus](./event-bus.md) - Event distribution system
- [Session](./session.md) - Worker and job orchestration
- [Commands](./commands.md) - Command system
- [UI Implementations](../chat-cli/ui-implementations.md) - TextUi, StructuredIO, etc.
