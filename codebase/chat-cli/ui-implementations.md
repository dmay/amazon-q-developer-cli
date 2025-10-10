# UI Implementations

## Overview

The Agent Environment architecture supports multiple UI implementations through the `UserInterface` and `HeadlessInterface` traits. This document describes the available UI implementations and how to create new ones.

## UI Trait Hierarchy

### UserInterface (Main Interactive UI)

**Location**: `crates/chat-cli/src/agent_env/agent_environment.rs`

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

**Purpose**: Interactive UIs that accept user input and send commands to AgentEnvironment.

**Characteristics**:
- Spawns tasks for input reading and event processing
- Provides command channel for sending commands
- Handles events for display/interaction
- Only one main UI per AgentEnvironment

### HeadlessInterface (Non-Interactive UI)

```rust
#[async_trait]
pub trait HeadlessInterface: Send + Sync {
    /// Handle event from EventBus
    async fn handle_event(&self, event: AgentEnvironmentEvent);
}
```

**Purpose**: Non-interactive UIs that only observe events (logging, web APIs, metrics, etc.)

**Characteristics**:
- No user input
- Only receives events
- Multiple headless UIs can run concurrently
- Useful for background processing

## Available Implementations

### TextUi - Text-Based Interactive UI

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/text_ui.rs`

**Purpose**: Basic text-based interactive UI with readline-style input and streaming output.

#### Features

- **Readline-style input**: Uses rustyline for command history and editing
- **Streaming output**: Displays LLM responses as they arrive
- **Prompt queue pattern**: Only reads input when worker is Idle
- **UI commands**: Handles /usage, /context, /status, /workers internally
- **Agent commands**: Forwards /prompt, /compact, /quit to AgentEnvironment

#### Structure

```rust
pub struct TextUi {
    session: Arc<Session>,
    main_worker_id: Uuid,
    input_handler: Arc<InputHandler>,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<Mutex<Option<mpsc::Receiver<PromptResult>>>>,
    prompt_ready: Arc<Notify>,
    shutdown_signal: Arc<Notify>,
}
```

#### Constructor

```rust
pub fn new(
    session: Arc<Session>,
    main_worker_id: Uuid,
    history_path: Option<PathBuf>,
) -> Result<Self>
```

**Parameters**:
- `session`: Session for accessing worker state
- `main_worker_id`: Worker to display events for
- `history_path`: Optional path for command history file

**Returns**: TextUi instance (receiver is stored internally)

#### Event Handling

TextUi handles these events:

**OutputChunk Events**:
- `AssistantResponse(text)`: Prints text with flush
- `ToolUse { tool_name, .. }`: Prints "[Using tool: name]"
- `ToolResult { tool_name, .. }`: Prints "[Tool name completed]"

**Lifecycle State Events**:
- `Idle`: Prints newline, signals prompt_ready
- `IdleFailed`: Prints "[Task failed]", signals prompt_ready
- `Busy`: No action (worker started job)

#### Prompt Loop

TextUi uses a **prompt queue pattern**:
1. Waits for `prompt_ready.notified()` signal
2. Reads user input with `InputHandler.read_line()`
3. Parses command with `CommandParser`
4. Handles UI commands internally (re-signals prompt_ready)
5. Sends Agent commands to AgentEnvironment
6. Waits for next prompt_ready signal

This prevents confusing UX where user types while agent is working.

#### Usage Example

```rust
// Create TextUi
let text_ui = TextUi::new(
    session.clone(),
    worker.id,
    Some(PathBuf::from(".q_history")),
)?;

// Create AgentEnvironment with TextUi
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![],
);

// Run (blocks until shutdown)
agent_env.run().await?;
```

#### UI Commands

- `/usage`: Display token usage for worker
- `/context`: Display context information
- `/status`: Display worker status
- `/workers`: List all workers
- `/compact [instruction]`: Compact conversation history
- `/quit` or `/q`: Shutdown

---

### StructuredIO - JSON I/O for Scripting

**Location**: `crates/chat-cli/src/cli/chat/agent_env_ui/structured_io.rs`

**Purpose**: Structured JSON I/O for scripting and automation. Reads single-line prompts from stdin, outputs structured JSON events.

#### Features

- **Always-reading pattern**: Continuously reads from stdin (no prompt queue)
- **JSON output**: Outputs structured JSON for easy parsing
- **Worker lifecycle events**: Outputs lifecycle state changes
- **AgentLoop events**: Outputs responses and tool use requests
- **Suitable for piping**: Can pipe commands from scripts

#### Structure

```rust
pub struct StructuredIO {
    session: Arc<Session>,
    main_worker_id: Uuid,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<Mutex<Option<mpsc::Receiver<PromptResult>>>>,
    output_writer: Arc<TokioMutex<Box<dyn Write + Send>>>,
}
```

#### Constructor

```rust
pub fn new(
    session: Arc<Session>,
    main_worker_id: Uuid,
) -> Result<Self>
```

**Parameters**:
- `session`: Session for accessing worker state
- `main_worker_id`: Worker to display events for

**Returns**: StructuredIO instance (receiver is stored internally)

#### Event Handling

StructuredIO handles these events:

**AgentLoop Events**:
```json
{"worker_id": "...", "assistant_response": "..."}
{"worker_id": "...", "tool_use_request": {"tool_name": "...", "tool_input": {...}}}
```

**Worker Lifecycle Events**:
```json
{"worker_id": "...", "lifecycle_state": "idle"}
{"worker_id": "...", "lifecycle_state": "busy"}
{"worker_id": "...", "lifecycle_state": "idle_failed"}
```

#### Input Reading

StructuredIO uses an **always-reading pattern**:
1. Continuously reads lines from stdin
2. Creates Prompt command for each non-empty line
3. Sends commands to AgentEnvironment
4. No waiting for worker state (suitable for piped input)

#### Usage Example

```rust
// Create StructuredIO
let structured_io = StructuredIO::new(
    session.clone(),
    worker.id,
)?;

// Create AgentEnvironment with StructuredIO
let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(structured_io)),
    vec![],
);

// Run (blocks until shutdown)
agent_env.run().await?;
```

#### Scripting Example

```bash
# Pipe commands to Q CLI
echo "What is 2+2?" | q chat --ui-mode structured

# Process output with jq
q chat --ui-mode structured < prompts.txt | jq -r '.assistant_response'

# Interactive with structured output
q chat --ui-mode structured
```

---

## UI Selection

### Command-Line Argument

```bash
# Text UI (default)
q chat

# Structured IO
q chat --ui-mode structured

# Headless mode (no UI)
q chat --ui-mode none
```

### In Code

```rust
let ui_mode = UiMode::Text; // or Structured, or None

let main_ui: Option<Arc<dyn UserInterface>> = match ui_mode {
    UiMode::Text => {
        let text_ui = TextUi::new(session.clone(), worker.id, history_path)?;
        Some(Arc::new(text_ui))
    }
    UiMode::Structured => {
        let structured_io = StructuredIO::new(session.clone(), worker.id)?;
        Some(Arc::new(structured_io))
    }
    UiMode::None => None,
};

let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    main_ui,
    vec![], // Headless UIs
);
```

## Creating New UI Implementations

### Step 1: Implement UserInterface Trait

```rust
pub struct MyCustomUi {
    session: Arc<Session>,
    main_worker_id: Uuid,
    cmd_sender: mpsc::Sender<PromptResult>,
    cmd_receiver: Arc<Mutex<Option<mpsc::Receiver<PromptResult>>>>,
    // ... other fields
}

#[async_trait]
impl UserInterface for MyCustomUi {
    async fn start(&self) -> Result<()> {
        // Spawn tasks for input reading, event processing, etc.
        self.spawn_input_reader();
        Ok(())
    }
    
    fn command_receiver(&self) -> mpsc::Receiver<PromptResult> {
        // Return receiver (can only be called once)
        self.cmd_receiver
            .lock()
            .unwrap()
            .take()
            .expect("command_receiver() called more than once")
    }
    
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        // Filter by worker_id
        if let Some(wid) = event.worker_id() {
            if wid != self.main_worker_id {
                return;
            }
        }
        
        // Handle event
        match event {
            AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
                // Display output
            }
            _ => {}
        }
    }
}
```

### Step 2: Create Constructor

```rust
impl MyCustomUi {
    pub fn new(
        session: Arc<Session>,
        main_worker_id: Uuid,
    ) -> Result<Self> {
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        
        Ok(Self {
            session,
            main_worker_id,
            cmd_sender,
            cmd_receiver: Arc::new(Mutex::new(Some(cmd_receiver))),
        })
    }
}
```

### Step 3: Spawn Input Reader (if interactive)

```rust
impl MyCustomUi {
    fn spawn_input_reader(&self) -> JoinHandle<()> {
        let cmd_sender = self.cmd_sender.clone();
        let main_worker_id = self.main_worker_id;
        
        tokio::spawn(async move {
            loop {
                // Read input (blocking or async)
                let input = read_user_input().await;
                
                // Parse command
                let cmd = AgentEnvironmentCommand::Prompt {
                    worker_id: main_worker_id,
                    text: input,
                };
                
                // Send to AgentEnvironment
                if cmd_sender.send(PromptResult::Command(cmd)).await.is_err() {
                    break; // Channel closed
                }
            }
        })
    }
}
```

### Step 4: Add to UI Selection

```rust
pub enum UiMode {
    Text,
    Structured,
    MyCustom, // Add new variant
    None,
}

// In ChatArgs::execute()
let main_ui = match ui_mode {
    UiMode::MyCustom => {
        let ui = MyCustomUi::new(session.clone(), worker.id)?;
        Some(Arc::new(ui))
    }
    // ... other modes
};
```

## Headless UI Example

```rust
pub struct EventLogger {
    log_file: Arc<TokioMutex<File>>,
}

#[async_trait]
impl HeadlessInterface for EventLogger {
    async fn handle_event(&self, event: AgentEnvironmentEvent) {
        let json = serde_json::to_string(&event).unwrap();
        let mut file = self.log_file.lock().await;
        writeln!(file, "{}", json).unwrap();
    }
}

// Usage
let logger = Arc::new(EventLogger::new("events.log")?);

let agent_env = AgentEnvironment::new(
    session,
    event_bus,
    Some(Arc::new(text_ui)),
    vec![logger], // Headless UI
);
```

## Design Patterns

### Pattern 1: Prompt Queue (TextUi)

**Use when**: User should only input when worker is ready

```rust
// Wait for worker to be Idle
prompt_ready.notified().await;

// Read input
let input = input_handler.read_line("You").await?;

// Send command
cmd_sender.send(PromptResult::Command(cmd)).await?;
```

### Pattern 2: Always Reading (StructuredIO)

**Use when**: Commands may be piped in, no waiting needed

```rust
// Continuously read lines
let stdin = tokio::io::stdin();
let reader = BufReader::new(stdin);
let mut lines = reader.lines();

while let Ok(Some(line)) = lines.next_line().await {
    // Send command immediately
    cmd_sender.send(PromptResult::Command(cmd)).await?;
}
```

### Pattern 3: Event Filtering

**Use when**: UI only cares about specific worker

```rust
async fn handle_event(&self, event: AgentEnvironmentEvent) {
    // Filter by worker_id
    if let Some(wid) = event.worker_id() {
        if wid != self.main_worker_id {
            return; // Ignore events for other workers
        }
    }
    
    // Process event
    // ...
}
```

## Related Documentation

- [AgentEnvironment](../agent-environment/agent-environment.md) - UI coordination
- [EventBus](../agent-environment/event-bus.md) - Event system
- [Commands](../agent-environment/commands.md) - Command types
- [UI Utils](../../crates/chat-cli/src/cli/chat/agent_env_ui/ui_utils.rs) - Shared utilities
