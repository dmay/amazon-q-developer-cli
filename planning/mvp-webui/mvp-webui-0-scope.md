# MVP WebUI - Scope

## Overview

This workflow covers the implementation of a web-based user interface for the agent_env architecture. The WebUI will provide a visual interface for interacting with workers, viewing streaming output, and managing conversations. This is a "fancy feature" that demonstrates the flexibility of the event-driven architecture.

## Tasks Included

### 2.1: Web UI with Minimal Worker Presentation

---

## Task 2.1: Web UI with Minimal Worker Presentation

### Current State

**Reference Implementation:**
- Reference web UI exists at `/Volumes/workplace/web-q/`
- Contains public/ folder with static assets
- Contains src/ folder with frontend code
- Provides design patterns and UI components

**Agent Environment:**
- EventBus supports multiple UI subscribers
- AgentEnvironment can manage multiple UIs (main + headless)
- Events are broadcast to all subscribers
- No web UI implementation exists

**Current UIs:**
- TextUi: Terminal-based interactive UI
- StructuredIO: JSON I/O for scripting
- Both implement event handling and command sending

### Requirements

**Core Functionality:**
- One-page web application
- Display list of workers with their states
- Show streaming output from jobs
- Send prompts to workers
- Cancel running jobs
- Display worker lifecycle events
- Real-time updates via WebSocket or SSE

**UI Features:**
- Worker list with status indicators (Idle, Busy, IdleFailed)
- Worker details (ID, current task, metadata)
- Streaming output display (like terminal)
- Input field for sending prompts
- Cancel button for active jobs
- Event log (optional)
- Responsive design

**Architecture:**
- HTTP server (axum or similar)
- WebSocket endpoint for event streaming
- REST API for commands
- Static file serving
- Headless UI for event forwarding

### Key Challenges

1. **Web Server Integration:**
   - Run HTTP server in same process or separate?
   - Port configuration and conflicts
   - Shutdown coordination

2. **Event Streaming:**
   - WebSocket vs Server-Sent Events (SSE)?
   - Event serialization (JSON)
   - Connection management
   - Reconnection handling

3. **Command Handling:**
   - How to send commands from web UI to AgentEnvironment?
   - REST API vs WebSocket messages?
   - Authentication and security

4. **State Synchronization:**
   - Initial state loading (existing workers)
   - Real-time updates (events)
   - Handling missed events (reconnection)

5. **Security:**
   - Authentication (if needed)
   - CORS configuration
   - Input validation
   - XSS prevention

6. **Frontend Framework:**
   - Vanilla JS vs React/Vue/Svelte?
   - Build process
   - Asset bundling

### Design Phases

#### Phase 1: Research & Design (4-6 hours)
**Deliverable:** `mvp-webui-1-design.md`

**Research Tasks:**
- Study /Volumes/workplace/web-q/ reference implementation
- Evaluate web framework options (axum, warp, actix-web)
- Evaluate WebSocket vs SSE for event streaming
- Design HTTP server integration
- Design event streaming protocol
- Design command handling
- Design static file serving
- Plan security model (auth, CORS)
- Choose frontend approach (vanilla JS vs framework)

**Design Decisions:**
- Web framework: axum (recommended for tokio integration)
- Event streaming: WebSocket (bidirectional, real-time)
- Command handling: WebSocket messages (same connection)
- Frontend: Vanilla JS or minimal framework (simplicity)
- Security: Local-only by default, optional auth

**Architecture Components:**
```
┌─────────────────┐
│   Browser       │
│  (Frontend)     │
└────────┬────────┘
         │ HTTP/WebSocket
         │
┌────────▼────────┐
│  Web Server     │
│   (axum)        │
├─────────────────┤
│ Static Files    │
│ WebSocket       │
│ REST API        │
└────────┬────────┘
         │
┌────────▼────────┐
│  WebUI          │
│ (Headless UI)   │
└────────┬────────┘
         │ Events
         │
┌────────▼────────┐
│  EventBus       │
└─────────────────┘
```

#### Phase 2: Backend Implementation (8-12 hours)
**Deliverable:** Working web server with event streaming

**Implementation Tasks:**
1. Add axum dependency
2. Create WebServer struct
3. Implement HTTP server setup
4. Implement WebSocket endpoint
5. Create WebUI (headless UI implementation)
6. Implement event forwarding to WebSocket
7. Implement command handling from WebSocket
8. Implement static file serving
9. Add CORS configuration
10. Test event streaming
11. Test command execution
12. Test connection handling

#### Phase 3: Frontend Implementation (8-12 hours)
**Deliverable:** Working web UI

**Implementation Tasks:**
1. Port web-q design to new structure
2. Implement WebSocket client
3. Implement worker list display
4. Implement worker state indicators
5. Implement streaming output display
6. Implement prompt input
7. Implement cancel button
8. Style and polish UI
9. Test event handling
10. Test command sending
11. Test reconnection
12. Test responsive design

### Proposed Architecture

**WebServer:**
```rust
pub struct WebServer {
    addr: SocketAddr,
    session: Arc<Session>,
    event_bus: EventBus,
}

impl WebServer {
    pub fn new(
        addr: SocketAddr,
        session: Arc<Session>,
        event_bus: EventBus,
    ) -> Self {
        Self { addr, session, event_bus }
    }
    
    pub async fn run(self) -> Result<()> {
        let app = Router::new()
            .route("/ws", get(websocket_handler))
            .route("/api/workers", get(list_workers))
            .route("/api/workers/:id/prompt", post(send_prompt))
            .route("/api/workers/:id/cancel", post(cancel_job))
            .nest_service("/", ServeDir::new("web/public"))
            .layer(CorsLayer::permissive())
            .with_state(AppState {
                session: self.session,
                event_bus: self.event_bus,
            });
        
        axum::Server::bind(&self.addr)
            .serve(app.into_make_service())
            .await?;
        
        Ok(())
    }
}
```

**WebUI (Headless UI):**
```rust
pub struct WebUI {
    session: Arc<Session>,
    worker_id: String,
    event_tx: broadcast::Sender<WebUIEvent>,
}

impl WebUI {
    pub fn new(session: Arc<Session>, worker_id: String) -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        Self { session, worker_id, event_tx }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<WebUIEvent> {
        self.event_tx.subscribe()
    }
}

#[async_trait]
impl UserInterface for WebUI {
    async fn handle_event(&self, event: Event) {
        // Convert Event to WebUIEvent (JSON-serializable)
        let web_event = WebUIEvent::from(event);
        
        // Broadcast to all WebSocket connections
        let _ = self.event_tx.send(web_event);
    }
    
    async fn run(&self) -> Result<()> {
        // WebUI doesn't have its own run loop
        // Events are handled via handle_event
        Ok(())
    }
}
```

**WebSocket Handler:**
```rust
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(
    socket: WebSocket,
    state: AppState,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to events
    let mut event_rx = state.event_bus.subscribe();
    
    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let json = serde_json::to_string(&event).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages (commands)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(command) = serde_json::from_str::<WebCommand>(&text) {
                    handle_web_command(command, &state).await;
                }
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

**WebCommand:**
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebCommand {
    Prompt {
        worker_id: String,
        message: String,
    },
    Cancel {
        worker_id: String,
    },
    CreateWorker {
        worker_id: String,
    },
    DeleteWorker {
        worker_id: String,
    },
}
```

**WebUIEvent:**
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebUIEvent {
    WorkerCreated {
        worker_id: String,
        timestamp: String,
    },
    WorkerDeleted {
        worker_id: String,
        timestamp: String,
    },
    WorkerStateChanged {
        worker_id: String,
        state: String,
        timestamp: String,
    },
    JobStarted {
        job_id: String,
        worker_id: String,
        timestamp: String,
    },
    JobCompleted {
        job_id: String,
        worker_id: String,
        success: bool,
        timestamp: String,
    },
    OutputChunk {
        job_id: String,
        worker_id: String,
        chunk: String,
    },
}
```


**Frontend (HTML/JS):**
```html
<!DOCTYPE html>
<html>
<head>
    <title>Q CLI - Web UI</title>
    <style>
        /* Styles from web-q reference */
        body { font-family: sans-serif; margin: 0; padding: 20px; }
        .worker-list { display: flex; flex-direction: column; gap: 10px; }
        .worker-card { border: 1px solid #ccc; padding: 15px; border-radius: 5px; }
        .worker-card.idle { border-color: green; }
        .worker-card.busy { border-color: orange; }
        .worker-card.failed { border-color: red; }
        .output { background: #f5f5f5; padding: 10px; font-family: monospace; white-space: pre-wrap; }
        .input-area { margin-top: 10px; display: flex; gap: 10px; }
        .input-area input { flex: 1; padding: 8px; }
        .input-area button { padding: 8px 16px; }
    </style>
</head>
<body>
    <h1>Q CLI - Workers</h1>
    <div id="workers" class="worker-list"></div>
    
    <script>
        const ws = new WebSocket('ws://localhost:8080/ws');
        const workers = new Map();
        
        ws.onmessage = (event) => {
            const msg = JSON.parse(event.data);
            handleEvent(msg);
        };
        
        ws.onclose = () => {
            console.log('WebSocket closed, reconnecting...');
            setTimeout(() => location.reload(), 1000);
        };
        
        function handleEvent(event) {
            switch (event.type) {
                case 'WorkerCreated':
                    createWorkerCard(event.worker_id);
                    break;
                case 'WorkerDeleted':
                    deleteWorkerCard(event.worker_id);
                    break;
                case 'WorkerStateChanged':
                    updateWorkerState(event.worker_id, event.state);
                    break;
                case 'OutputChunk':
                    appendOutput(event.worker_id, event.chunk);
                    break;
                case 'JobStarted':
                    clearOutput(event.worker_id);
                    break;
            }
        }
        
        function createWorkerCard(workerId) {
            const card = document.createElement('div');
            card.className = 'worker-card idle';
            card.id = `worker-${workerId}`;
            card.innerHTML = `
                <h3>Worker: ${workerId}</h3>
                <div class="status">Status: <span class="state">Idle</span></div>
                <div class="output" id="output-${workerId}"></div>
                <div class="input-area">
                    <input type="text" id="input-${workerId}" placeholder="Enter prompt...">
                    <button onclick="sendPrompt('${workerId}')">Send</button>
                    <button onclick="cancelJob('${workerId}')">Cancel</button>
                </div>
            `;
            document.getElementById('workers').appendChild(card);
        }
        
        function deleteWorkerCard(workerId) {
            const card = document.getElementById(`worker-${workerId}`);
            if (card) card.remove();
        }
        
        function updateWorkerState(workerId, state) {
            const card = document.getElementById(`worker-${workerId}`);
            if (!card) return;
            
            card.className = `worker-card ${state.toLowerCase()}`;
            card.querySelector('.state').textContent = state;
        }
        
        function appendOutput(workerId, chunk) {
            const output = document.getElementById(`output-${workerId}`);
            if (output) {
                output.textContent += chunk;
                output.scrollTop = output.scrollHeight;
            }
        }
        
        function clearOutput(workerId) {
            const output = document.getElementById(`output-${workerId}`);
            if (output) output.textContent = '';
        }
        
        function sendPrompt(workerId) {
            const input = document.getElementById(`input-${workerId}`);
            const message = input.value.trim();
            if (!message) return;
            
            ws.send(JSON.stringify({
                type: 'Prompt',
                worker_id: workerId,
                message: message
            }));
            
            input.value = '';
        }
        
        function cancelJob(workerId) {
            ws.send(JSON.stringify({
                type: 'Cancel',
                worker_id: workerId
            }));
        }
    </script>
</body>
</html>
```

### Integration with ChatArgs

```rust
impl ChatArgs {
    pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
        // ... existing initialization ...
        
        // Check if web UI is requested
        let enable_web_ui = self.ui_mode == Some(UiMode::Web) 
            || std::env::var("Q_WEB_UI").is_ok();
        
        if enable_web_ui {
            // Start web server
            let web_addr = "127.0.0.1:8080".parse().unwrap();
            let web_server = WebServer::new(web_addr, session.clone(), event_bus.clone());
            
            tokio::spawn(async move {
                if let Err(e) = web_server.run().await {
                    error!("Web server error: {}", e);
                }
            });
            
            info!("Web UI available at http://127.0.0.1:8080");
        }
        
        // ... rest of initialization ...
    }
}
```

### Acceptance Criteria

**Phase 1 (Design):**
- Complete web UI architecture
- Technology choices made
- Security model defined
- Design review and approval

**Phase 2 (Backend):**
- Web server runs successfully
- WebSocket endpoint works
- Events are streamed to clients
- Commands are received and executed
- Static files are served
- Connection handling is robust

**Phase 3 (Frontend):**
- Worker list displays correctly
- Worker states update in real-time
- Streaming output displays correctly
- Prompt input works
- Cancel button works
- UI is responsive
- Reconnection works

### Testing Strategy

```bash
# Start with web UI
q chat --ui-mode=web

# Or environment variable
export Q_WEB_UI=1
q chat

# Open browser to http://localhost:8080

# Test worker display
# (should see main worker)

# Test prompt sending
# (type in input field, click Send)
# (should see streaming output)

# Test cancel
# (start long-running task, click Cancel)
# (should cancel job)

# Test reconnection
# (close browser tab, reopen)
# (should reconnect and show current state)

# Test multiple workers (future)
# (create additional workers via API)
# (should see all workers in UI)
```

### Dependencies

- EventBus (existing)
- AgentEnvironment (existing)
- Session (existing)
- axum (new dependency)
- tokio-tungstenite (WebSocket, new dependency)
- tower-http (static files, new dependency)

### Estimated Effort

**Total: 20-30 hours**
- Phase 1 (Design): 4-6 hours
- Phase 2 (Backend): 8-12 hours
- Phase 3 (Frontend): 8-12 hours

---

## Success Metrics

- Web UI is accessible via browser
- Workers are displayed with correct states
- Streaming output works in real-time
- Commands can be sent from UI
- Jobs can be cancelled from UI
- UI is responsive and polished
- Connection handling is robust
- No security vulnerabilities

---

## Risks and Mitigation

**Risk 1: WebSocket Complexity**
- Mitigation: Use established libraries (axum, tokio-tungstenite), thorough testing

**Risk 2: State Synchronization**
- Mitigation: Send initial state on connection, handle reconnection gracefully

**Risk 3: Security Vulnerabilities**
- Mitigation: Local-only by default, input validation, CORS configuration

**Risk 4: Performance Impact**
- Mitigation: Async architecture, efficient event broadcasting, connection limits

**Risk 5: Browser Compatibility**
- Mitigation: Use standard WebSocket API, test on major browsers

---

## Future Enhancements

- Multiple worker support (create/delete workers from UI)
- Worker configuration UI
- Conversation history view
- Tool use visualization
- MCP server status display
- Authentication and multi-user support
- Remote access (with security)
- Mobile-responsive design
- Dark mode
- Syntax highlighting for code output
- File upload/download
- Export conversation

---

## Related Files

**Existing Code:**
- `crates/chat-cli/src/agent_env/agent_environment.rs` - AgentEnvironment
- `crates/chat-cli/src/agent_env/event_bus.rs` - EventBus
- `crates/chat-cli/src/agent_env/events.rs` - Event definitions
- `crates/chat-cli/src/cli/chat/mod.rs` - ChatArgs::execute()

**Reference Implementation:**
- `/Volumes/workplace/web-q/public/` - Static assets
- `/Volumes/workplace/web-q/src/` - Frontend code

**New Files (to be created):**
- `crates/chat-cli/src/cli/chat/web_server/mod.rs` - Web server module
- `crates/chat-cli/src/cli/chat/web_server/server.rs` - WebServer implementation
- `crates/chat-cli/src/cli/chat/web_server/handlers.rs` - HTTP/WebSocket handlers
- `crates/chat-cli/src/cli/chat/agent_env_ui/web_ui.rs` - WebUI (headless UI)
- `web/public/index.html` - Frontend HTML
- `web/public/style.css` - Frontend CSS
- `web/public/app.js` - Frontend JavaScript

---

## Documentation Updates

After implementation:
- Document web UI setup and usage
- Document WebSocket protocol
- Document REST API endpoints
- Add screenshots to README
- Create troubleshooting guide
- Document security considerations
- Add development guide for frontend

---

## Optional Features (Nice to Have)

### Advanced Features
- **Syntax Highlighting:** Use highlight.js for code output
- **Markdown Rendering:** Render markdown in output
- **File Upload:** Upload files for context
- **Export:** Export conversation as markdown/JSON
- **Themes:** Light/dark mode toggle
- **Notifications:** Browser notifications for job completion

### Multi-Worker Features
- **Worker Creation:** Create new workers from UI
- **Worker Configuration:** Configure worker settings
- **Worker Deletion:** Delete workers from UI
- **Worker Filtering:** Filter/search workers

### Advanced Visualization
- **Tool Use Graph:** Visualize tool execution flow
- **Token Usage:** Display token usage per worker
- **Performance Metrics:** Show response times, etc.
- **Event Timeline:** Visual timeline of events

These features can be added incrementally after the MVP is complete.
