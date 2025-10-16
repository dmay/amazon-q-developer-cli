# MVP WebUI - Implementation Plan

## Overview

This document provides a detailed, task-oriented implementation plan for the WebUI feature. The plan is organized into phases, with each task referencing the relevant section of the design document.

**Design Document:** `mvp-webui-2-design.md`

**Estimated Total Effort:** 26-34 hours

---

## Phase 1: Backend Infrastructure

**Estimated Effort:** 8-10 hours

This phase establishes the core backend components: serializable event types, WebUI component, and web server structure.

### 1.1 Setup Module Structure

- [x] **Create web_server module directory** (Design Doc §5)
  - Create `crates/chat-cli/src/cli/chat/web_server/` directory
  - Create `mod.rs` with module exports
  - Add module declaration to `crates/chat-cli/src/cli/chat/mod.rs`

- [x] **Create module files** (Design Doc §5)
  - Create `events.rs` for WebUIEvent types
  - Create `web_ui.rs` for WebUI component
  - Create `server.rs` for WebServer implementation
  - Create `websocket.rs` for WebSocket handlers
  - Create `api.rs` for REST API handlers

- [x] **Add dependencies to Cargo.toml** (Design Doc §5)
  - Add `axum = { version = "0.7", features = ["ws"] }`
  - Add `tower = "0.4"`
  - Add `tower-http = { version = "0.5", features = ["fs", "cors"] }`
  - Verify existing dependencies: tokio, serde, serde_json, uuid

### 1.2 Implement WebUIEvent Types

- [x] **Define WorkerLifecycleState enum** (Design Doc §1)
  - Create serializable enum with Idle, Busy, IdleFailed variants
  - Add serde attributes: `#[serde(rename_all = "snake_case")]`
  - Implement conversion from internal WorkerLifecycleState type

- [x] **Define JobResult enum** (Design Doc §1)
  - Create Success, Cancelled, Failed variants with appropriate fields
  - Add serde tag attribute: `#[serde(tag = "status")]`
  - Implement conversion from internal JobCompletionResult type

- [x] **Define OutputChunkData enum** (Design Doc §1)
  - Create AssistantResponse, ToolUse, ToolResult variants
  - Add serde tag attribute: `#[serde(tag = "chunk_type")]`
  - Implement conversion from internal OutputChunk type

- [x] **Define WebUIEvent enum** (Design Doc §1)
  - Create all event variants: WorkerCreated, WorkerDeleted, WorkerStateChanged, JobStarted, JobCompleted, OutputChunk, ResponseReceived, ToolUseRequested, ShutdownInitiated
  - Add serde tag attribute: `#[serde(tag = "type", rename_all = "snake_case")]`
  - Use f64 for timestamps, String for IDs

- [x] **Implement time conversion utilities** (Design Doc §1)
  - Create global OnceLock for PROCESS_START_INSTANT and PROCESS_START_SYSTEM_TIME
  - Implement `init_time_conversion()` function
  - Implement `instant_to_unix_timestamp()` function
  - Add documentation explaining Instant to SystemTime conversion

- [x] **Implement WebUIEvent::from_agent_event()** (Design Doc §1)
  - Implement conversion from AgentEnvironmentEvent
  - Handle Worker, Job, AgentLoop, System event types
  - Convert Instant to f64 timestamps
  - Convert Uuid to String

- [x] **Add helper types** (Design Doc §1)
  - Create WorkerStateSnapshot struct for initial state
  - Create CurrentJobInfo struct for job details
  - Create ConversationSummary struct (optional for MVP)
  - Add serde derives and type field

### 1.3 Implement WebUI Component

- [x] **Define WebUI struct** (Design Doc §4)
  - Add session: Arc<Session> field
  - Add event_tx: broadcast::Sender<WebUIEvent> field
  - Create with buffer size of 10,000 events

- [x] **Implement WebUI::new()** (Design Doc §4)
  - Create broadcast channel with large buffer
  - Store session reference
  - Return WebUI instance

- [x] **Implement WebUI::subscribe()** (Design Doc §4)
  - Return broadcast::Receiver<WebUIEvent> for WebSocket handlers
  - Document usage pattern

- [x] **Implement WebUI::session()** (Design Doc §4)
  - Return reference to Arc<Session>
  - Provide access for WebSocket handlers to query state

- [x] **Implement HeadlessInterface for WebUI** (Design Doc §4)
  - Implement async handle_event() method
  - Convert AgentEnvironmentEvent to WebUIEvent
  - Broadcast to all subscribers via event_tx
  - Ignore send errors (no subscribers is OK)

### 1.4 Implement AppState and WebServer Structure

- [x] **Define AppState struct** (Design Doc §5)
  - Add session: Arc<Session> field
  - Add web_ui: Arc<WebUI> field
  - Derive Clone for use with Axum extractors

- [x] **Define WebServer struct** (Design Doc §5)
  - Add addr: SocketAddr field
  - Add state: AppState field
  - Document purpose and lifecycle

- [x] **Implement WebServer::new()** (Design Doc §5)
  - Accept addr, session, web_ui parameters
  - Create AppState
  - Return WebServer instance

- [x] **Implement WebServer::build_router()** (Design Doc §5)
  - Create Axum Router
  - Add placeholder routes (will be implemented in later phases)
  - Add CORS layer with permissive settings for development
  - Add state with with_state()

- [x] **Implement WebServer::run()** (Design Doc §5)
  - Build router
  - Create TcpListener on addr
  - Start axum::serve()
  - Log server address on startup

- [x] **Implement WebServer::run_with_shutdown()** (Design Doc §5, §8)
  - Accept shutdown_signal: Arc<Notify> parameter
  - Build router and listener
  - Use with_graceful_shutdown() with 5-second timeout
  - Log shutdown message

### 1.5 Implement REST API Handlers (Skeleton)

- [x] **Implement health_check handler** (Design Doc §3)
  - Return JSON with status: "ok" and version
  - Use env!("CARGO_PKG_VERSION") for version
  - Add route to router: GET /api/health

- [x] **Implement list_workers handler** (Design Doc §3)
  - Extract AppState from request
  - Query session.get_workers()
  - Convert to WorkersResponse with WorkerInfo structs
  - Return JSON response

- [x] **Implement get_worker handler** (Design Doc §3)
  - Extract worker_id from path parameter
  - Parse Uuid, return 400 on invalid format
  - Query session.get_worker(), return 404 if not found
  - Convert to WorkerDetailResponse
  - Return JSON response

- [x] **Define response types** (Design Doc §3)
  - Create WorkersResponse struct
  - Create WorkerInfo struct
  - Create WorkerDetailResponse struct
  - Create ErrorResponse struct
  - Add serde derives
  - Return JSON response

### 1.6 Add Static File Serving

- [x] **Create web/public directory** (Design Doc §5)
  - Create directory structure: web/public/
  - Add placeholder index.html (will be implemented in Phase 3)
  - Document directory purpose

- [x] **Add static file route** (Design Doc §5)
  - Use tower_http::services::ServeDir
  - Add route: nest_service("/", ServeDir::new("web/public"))
  - Test that static files are served correctly

---

## Phase 2: WebSocket Protocol Implementation

**Estimated Effort:** 6-8 hours

This phase implements the WebSocket protocol for real-time communication between backend and frontend.

### 2.1 Define WebSocket Message Types

- [x] **Define WebSocketCommand enum** (Design Doc §2)
  - Create Prompt, Cancel, Ping variants
  - Add serde tag attribute: `#[serde(tag = "type", rename_all = "snake_case")]`
  - Add Deserialize derive for parsing client messages

- [x] **Add command validation** (Design Doc §2)
  - Validate prompt text is not empty
  - Document command format in code comments
  - Add examples in documentation

### 2.2 Implement State Snapshot Generation

- [x] **Implement send_state_snapshot() function** (Design Doc §2, §6)
  - Accept sender, worker_id, and AppState parameters
  - Query worker from session
  - Create WorkerStateSnapshot with current state
  - Include lifecycle_state, current_job, conversation_summary
  - Serialize to JSON and send via WebSocket
  - Handle errors gracefully with logging

- [x] **Add worker existence check** (Design Doc §2)
  - Verify worker exists before sending snapshot
  - Return appropriate error if worker not found
  - Log worker_id for debugging

- [x] **Handle current job information** (Design Doc §6)
  - Check if worker has active job
  - Include job_id, task_type, started_at if present
  - Convert Instant to timestamp for started_at

### 2.3 Implement WebSocket Handler

- [x] **Implement websocket_handler() function** (Design Doc §2)
  - Extract worker_id from path parameter
  - Parse Uuid, return 400 on invalid format
  - Extract AppState from request state
  - Check if worker exists, return 404 if not found
  - Upgrade to WebSocket connection
  - Call handle_websocket() on upgrade

- [x] **Implement handle_websocket() function** (Design Doc §2, §6)
  - Split WebSocket into sender and receiver
  - Subscribe to WebUI events BEFORE sending snapshot (prevent race)
  - Send initial state snapshot
  - Spawn event streaming task
  - Spawn command handling task
  - Use tokio::select! to wait for either task or shutdown

- [x] **Implement event streaming task** (Design Doc §2, §6)
  - Loop on event_rx.recv()
  - Filter events by worker_id
  - Convert AgentEnvironmentEvent to WebUIEvent (already done by WebUI)
  - Serialize to JSON
  - Send via WebSocket sender
  - Break on send error (connection closed)
  - Handle RecvError::Lagged by sending fresh snapshot

- [x] **Implement command handling task** (Design Doc §2)
  - Loop on receiver.next()
  - Parse JSON to WebSocketCommand
  - Call handle_command() for each command
  - Log errors but continue processing
  - Break on connection close

### 2.4 Implement Command Handling

- [x] **Implement handle_command() function** (Design Doc §2)
  - Accept command, worker_id, and session parameters
  - Match on WebSocketCommand variants
  - Return Result for error handling

- [x] **Handle Prompt command** (Design Doc §2)
  - Get worker from session
  - Add message to conversation history
  - Launch agent loop via session.run_task__agent_loop()
  - Log command execution

- [x] **Handle Cancel command** (Design Doc §2)
  - Call session.cancel_worker_job(worker_id)
  - Log cancellation
  - Handle errors if no job is running

- [x] **Handle Ping command** (Design Doc §2)
  - No-op, just keep connection alive
  - Optionally log ping for debugging

### 2.5 Implement Connection Lifecycle

- [x] **Add connection logging** (Design Doc §2)
  - Log WebSocket connection established
  - Log worker_id for each connection
  - Log disconnection with reason

- [x] **Implement graceful closure** (Design Doc §8)
  - Send ShutdownInitiated event before closing
  - Send WebSocket close frame
  - Clean up resources
  - Log shutdown completion

- [x] **Handle abnormal closure** (Design Doc §2)
  - Detect connection errors
  - Log error details
  - Clean up resources properly

### 2.6 Add WebSocket Route to Router

- [x] **Add WebSocket route** (Design Doc §2, §5)
  - Add route to WebServer::build_router()
  - Route: GET /ws/worker/:worker_id
  - Handler: websocket_handler
  - Test route is accessible

---

## Phase 3: Frontend Implementation

**Estimated Effort:** 8-10 hours

This phase implements the browser-based user interface using vanilla JavaScript.

### 3.1 Create HTML Structure

- [ ] **Create index.html** (Design Doc §7)
  - Create file at web/public/index.html
  - Add DOCTYPE, html, head, body structure
  - Set viewport meta tag for responsive design
  - Link to style.css and app.js

- [ ] **Add header section** (Design Doc §7)
  - Add h1 title: "Q CLI - Web UI"
  - Add connection status indicator (span with id)
  - Add connection status text (span with id)
  - Style with flexbox for layout

- [ ] **Add worker container** (Design Doc §7)
  - Create main container div
  - Add worker header with name and state badge
  - Add IDs for dynamic content: worker-name, worker-state

- [ ] **Add output container** (Design Doc §7)
  - Create scrollable output div
  - Set fixed height (500px)
  - Add ID: output
  - Style for overflow-y: auto

- [ ] **Add input container** (Design Doc §7)
  - Create input field with ID: prompt-input
  - Add send button with ID: send-button
  - Add cancel button with ID: cancel-button
  - Set initial disabled state
  - Use flexbox for layout

### 3.2 Create CSS Styles

- [ ] **Create style.css** (Design Doc §7)
  - Create file at web/public/style.css
  - Add CSS reset (margin, padding, box-sizing)
  - Set base font family and colors

- [ ] **Style header** (Design Doc §7)
  - Flexbox layout for title and status
  - White background with border-radius
  - Box shadow for depth
  - Padding and margins

- [ ] **Style connection status** (Design Doc §7)
  - Circular indicator (12px, border-radius: 50%)
  - Color states: gray (default), green (connected), red (disconnected)
  - Flexbox for indicator + text layout

- [ ] **Style worker container** (Design Doc §7)
  - White background with border-radius
  - Box shadow for depth
  - Overflow hidden for clean edges

- [ ] **Style worker header** (Design Doc §7)
  - Flexbox for name and state badge
  - Border-bottom separator
  - Padding

- [ ] **Style state badge** (Design Doc §7)
  - Pill shape with border-radius
  - Color variants: green (idle), orange (busy), red (idle_failed)
  - Uppercase text
  - Padding

- [ ] **Style output container** (Design Doc §7)
  - Fixed height with scroll
  - Light gray background
  - Padding
  - Monospace font for output

- [ ] **Style output chunks** (Design Doc §7)
  - White background with border-radius
  - Left border for type indication (blue: assistant, orange: tool-use, green: tool-result)
  - Padding and margins
  - Pre-wrap for text wrapping

- [ ] **Style input container** (Design Doc §7)
  - Flexbox layout for input + buttons
  - Border-top separator
  - Padding and gaps

- [ ] **Style input field** (Design Doc §7)
  - Flex: 1 to fill space
  - Border and border-radius
  - Padding
  - Focus state with blue border

- [ ] **Style buttons** (Design Doc §7)
  - Padding and border-radius
  - Color variants: blue (send), red (cancel)
  - Hover states
  - Disabled state (opacity, cursor)

### 3.3 Implement JavaScript Application

- [ ] **Create app.js** (Design Doc §7)
  - Create file at web/public/app.js
  - Add DOMContentLoaded event listener
  - Initialize QWebUI class

- [ ] **Define QWebUI class** (Design Doc §7)
  - Add constructor with initialization
  - Store workerId, ws, reconnect state
  - Cache DOM element references
  - Define max reconnect attempts and delay

- [ ] **Implement init() method** (Design Doc §7)
  - Fetch worker list from /api/workers
  - Select first worker (MVP: single worker)
  - Set worker name in UI
  - Setup event listeners
  - Call connect()

- [ ] **Implement fetchWorkers() method** (Design Doc §7)
  - Fetch from /api/workers endpoint
  - Parse JSON response
  - Return workers array
  - Handle fetch errors with console.error

- [ ] **Implement setupEventListeners() method** (Design Doc §7)
  - Add click listener to send button → sendPrompt()
  - Add click listener to cancel button → cancelJob()
  - Add keypress listener to input (Enter key) → sendPrompt()
  - Prevent default on Enter to avoid form submission

### 3.4 Implement WebSocket Client

- [ ] **Implement connect() method** (Design Doc §7)
  - Update connection status to "connecting"
  - Create WebSocket with URL: ws://{host}/ws/worker/{workerId}
  - Set up onopen, onmessage, onclose, onerror handlers

- [ ] **Implement onopen handler** (Design Doc §7)
  - Log connection success
  - Update connection status to "connected"
  - Reset reconnect attempts and delay

- [ ] **Implement onmessage handler** (Design Doc §7)
  - Parse JSON message
  - Call handleEvent() with parsed data
  - Log errors if JSON parsing fails

- [ ] **Implement onclose handler** (Design Doc §7)
  - Log disconnection
  - Update connection status to "disconnected"
  - Check close code (1000 = normal, don't reconnect)
  - Call reconnect() for abnormal closure

- [ ] **Implement onerror handler** (Design Doc §7)
  - Log WebSocket error
  - Error details for debugging

- [ ] **Implement reconnect() method** (Design Doc §7)
  - Check max reconnect attempts
  - Calculate exponential backoff delay
  - Log reconnection attempt
  - Use setTimeout to call connect() after delay

### 3.5 Implement Event Handling

- [ ] **Implement handleEvent() method** (Design Doc §7)
  - Switch on event.type
  - Route to appropriate handler method
  - Log unknown event types

- [ ] **Implement handleSnapshot() method** (Design Doc §7)
  - Update worker name from snapshot
  - Update worker state
  - Clear output for fresh start
  - Log snapshot received

- [ ] **Implement updateWorkerState() method** (Design Doc §7)
  - Update state badge text and class
  - Enable/disable input controls based on state
  - Idle/IdleFailed: enable input and send, disable cancel
  - Busy: disable input and send, enable cancel

- [ ] **Implement handleJobStarted() method** (Design Doc §7)
  - Clear previous output
  - Log job started

- [ ] **Implement handleJobCompleted() method** (Design Doc §7)
  - Check result status
  - Display error message if failed
  - Log job completion

- [ ] **Implement handleOutputChunk() method** (Design Doc §7)
  - Switch on chunk.chunk_type
  - Call appendOutput() with appropriate text and type
  - Format tool-use and tool-result with JSON.stringify

### 3.6 Implement User Actions

- [ ] **Implement appendOutput() method** (Design Doc §7)
  - Create div element with class: output-chunk {type}
  - Set textContent to provided text
  - Append to output container
  - Scroll to bottom (scrollTop = scrollHeight)

- [ ] **Implement sendPrompt() method** (Design Doc §7)
  - Get input value and trim
  - Return early if empty
  - Create command object: {type: "prompt", text: ...}
  - Send via WebSocket as JSON
  - Clear input field

- [ ] **Implement cancelJob() method** (Design Doc §7)
  - Create command object: {type: "cancel"}
  - Send via WebSocket as JSON
  - Log cancellation

- [ ] **Implement updateConnectionStatus() method** (Design Doc §7)
  - Update status indicator class (connecting, connected, disconnected)
  - Update status text
  - Use status text map for display

- [ ] **Implement showError() method** (Design Doc §7)
  - Clear output container
  - Display error message with error styling
  - Log error to console

---

## Phase 4: Integration & Testing

**Estimated Effort:** 4-6 hours

This phase integrates the WebUI with the existing Q CLI and performs comprehensive testing.

### 4.1 CLI Integration

- [ ] **Add CLI arguments to ChatArgs** (Design Doc §5)
  - Add `web_ui: bool` field with `#[arg(long)]`
  - Add `web_port: Option<u16>` field with `#[arg(long)]`
  - Document arguments in help text

- [ ] **Initialize time conversion in ChatArgs::execute()** (Design Doc §1, §5)
  - Call `web_server::events::init_time_conversion()` at startup
  - Place before any event generation
  - Document why this is needed (Instant to SystemTime conversion)

- [ ] **Create WebUI instance in ChatArgs::execute()** (Design Doc §5)
  - Create `Arc<WebUI>` with session reference
  - Place after session creation
  - Store for use in AgentEnvironment and WebServer

- [ ] **Check web UI enable flag** (Design Doc §5)
  - Check `self.web_ui` flag or `Q_WEB_UI` environment variable
  - Determine if web server should start
  - Log decision for debugging

- [ ] **Start WebServer in background task** (Design Doc §5)
  - Parse web_port (default: 8080)
  - Create SocketAddr with 127.0.0.1 (localhost only)
  - Create WebServer instance
  - Spawn tokio task with run_with_shutdown()
  - Pass shutdown_signal for coordination
  - Log web UI URL: http://127.0.0.1:{port}

- [ ] **Add WebUI to AgentEnvironment headless UIs** (Design Doc §5)
  - Create headless_uis vector
  - Add web_ui if enabled
  - Pass to AgentEnvironment::new()
  - Ensure WebUI receives all events

- [ ] **Handle web server errors** (Design Doc §5)
  - Log errors from web server task
  - Detect "Address already in use" error
  - Suggest --web-port flag in error message
  - Don't crash main process on web server error

### 4.2 Unit Testing

- [ ] **Test WebUIEvent conversion** (Design Doc §1)
  - Create test for each AgentEnvironmentEvent variant
  - Verify correct WebUIEvent is produced
  - Check timestamp conversion
  - Check Uuid to String conversion

- [ ] **Test time conversion utilities** (Design Doc §1)
  - Test init_time_conversion()
  - Test instant_to_unix_timestamp()
  - Verify timestamps are reasonable (not negative, not far future)

- [ ] **Test WebUI component** (Design Doc §4)
  - Test WebUI::new() creates instance
  - Test subscribe() returns receiver
  - Test handle_event() broadcasts to subscribers
  - Test multiple subscribers receive same event

- [ ] **Test WebSocketCommand parsing** (Design Doc §2)
  - Test parsing valid Prompt command
  - Test parsing valid Cancel command
  - Test parsing valid Ping command
  - Test parsing invalid JSON returns error

- [ ] **Test response type serialization** (Design Doc §3)
  - Test WorkersResponse serializes correctly
  - Test WorkerDetailResponse serializes correctly
  - Test ErrorResponse serializes correctly

### 4.3 Integration Testing

- [ ] **Test WebServer startup** (Design Doc §5)
  - Start WebServer on test port
  - Verify server is listening
  - Verify health endpoint responds
  - Shutdown gracefully

- [ ] **Test REST API endpoints** (Design Doc §3)
  - Test GET /api/health returns 200
  - Test GET /api/workers returns worker list
  - Test GET /api/workers/:id returns worker details
  - Test GET /api/workers/invalid returns 404

- [ ] **Test WebSocket connection** (Design Doc §2)
  - Connect to /ws/worker/:id
  - Verify connection succeeds
  - Verify initial snapshot is received
  - Close connection gracefully

- [ ] **Test WebSocket event streaming** (Design Doc §2, §6)
  - Connect WebSocket
  - Trigger events (job start, output, completion)
  - Verify events are received in correct order
  - Verify events are filtered by worker_id

- [ ] **Test WebSocket command handling** (Design Doc §2)
  - Send Prompt command
  - Verify agent loop starts
  - Send Cancel command
  - Verify job is cancelled

- [ ] **Test reconnection** (Design Doc §6)
  - Connect WebSocket
  - Close connection
  - Reconnect
  - Verify fresh snapshot is received
  - Verify events continue streaming

- [ ] **Test shutdown coordination** (Design Doc §8)
  - Start WebServer with shutdown signal
  - Trigger shutdown
  - Verify WebSocket connections close gracefully
  - Verify server stops accepting connections
  - Verify server exits within timeout

### 4.4 Manual Testing

- [ ] **Test basic workflow** (Design Doc §7)
  - Start Q CLI with --web-ui flag
  - Open browser to http://localhost:8080
  - Verify UI loads correctly
  - Verify worker is displayed
  - Send prompt via UI
  - Verify streaming output appears
  - Verify job completes

- [ ] **Test cancel functionality** (Design Doc §7)
  - Start long-running task
  - Click cancel button
  - Verify job is cancelled
  - Verify worker returns to idle state

- [ ] **Test reconnection** (Design Doc §7)
  - Connect to UI
  - Close browser tab
  - Reopen tab
  - Verify UI reconnects
  - Verify current state is displayed

- [ ] **Test multiple browser tabs** (Design Doc §7)
  - Open UI in two browser tabs
  - Send prompt from one tab
  - Verify output appears in both tabs
  - Verify state is synchronized

- [ ] **Test error handling** (Design Doc §7)
  - Send invalid worker ID in URL
  - Verify 404 error
  - Stop Q CLI while UI is connected
  - Verify reconnection attempts
  - Verify error message after max attempts

- [ ] **Test different browsers** (Design Doc §7)
  - Test on Chrome
  - Test on Firefox
  - Test on Safari (macOS)
  - Verify consistent behavior

- [ ] **Test responsive design** (Design Doc §7)
  - Resize browser window
  - Verify layout adapts
  - Test on mobile viewport (browser dev tools)
  - Verify usability on small screens

### 4.5 Documentation

- [ ] **Update README with web UI instructions** (Design Doc §5)
  - Document --web-ui flag
  - Document --web-port flag
  - Document Q_WEB_UI environment variable
  - Add screenshot of web UI

- [ ] **Document WebSocket protocol** (Design Doc §2)
  - Document message format
  - Document event types
  - Document command types
  - Add examples

- [ ] **Document REST API** (Design Doc §3)
  - Document endpoints
  - Document request/response formats
  - Add curl examples

- [ ] **Add troubleshooting guide** (Design Doc §5)
  - Document port conflict resolution
  - Document connection issues
  - Document browser compatibility
  - Add FAQ section

- [ ] **Update architecture documentation** (Design Doc §5)
  - Add WebUI to architecture diagram
  - Document component interactions
  - Update file structure documentation

### 4.6 Final Verification

- [ ] **Run all tests** (Design Doc §4)
  - Run `cargo test` for unit tests
  - Run integration tests
  - Verify all tests pass

- [ ] **Run lints** (Design Doc §4)
  - Run `cargo clippy`
  - Fix any warnings
  - Run `cargo +nightly fmt`

- [ ] **Test build** (Design Doc §4)
  - Run `cargo build --release`
  - Verify no compilation errors
  - Test release binary

- [ ] **Verify static files are included** (Design Doc §5)
  - Check web/public/ directory exists
  - Verify index.html, style.css, app.js are present
  - Test static file serving

- [ ] **End-to-end smoke test** (Design Doc §7)
  - Start Q CLI with --web-ui
  - Open browser
  - Complete full interaction workflow
  - Verify no errors in logs
  - Shutdown gracefully

---

## Summary

### Total Task Count

- **Phase 1:** 28 tasks (Backend Infrastructure)
- **Phase 2:** 19 tasks (WebSocket Protocol)
- **Phase 3:** 43 tasks (Frontend Implementation)
- **Phase 4:** 38 tasks (Integration & Testing)

**Total:** 128 tasks

### Estimated Effort

- **Phase 1:** 8-10 hours
- **Phase 2:** 6-8 hours
- **Phase 3:** 8-10 hours
- **Phase 4:** 4-6 hours

**Total:** 26-34 hours

### Dependencies

**Phase Dependencies:**
- Phase 2 requires Phase 1 (Backend Infrastructure)
- Phase 3 can be done in parallel with Phase 2
- Phase 4 requires Phases 1, 2, and 3

**External Dependencies:**
- EventBus (existing)
- AgentEnvironment (existing)
- Session (existing)
- Worker (existing)

### Key Deliverables

1. **Backend Components:**
   - WebUIEvent types with serialization
   - WebUI component (HeadlessInterface)
   - WebServer with Axum
   - WebSocket handlers
   - REST API handlers

2. **Frontend Components:**
   - HTML structure
   - CSS styles
   - JavaScript application
   - WebSocket client

3. **Integration:**
   - CLI flags (--web-ui, --web-port)
   - ChatArgs integration
   - Shutdown coordination

4. **Documentation:**
   - README updates
   - API documentation
   - Troubleshooting guide

### Completion Criteria

The implementation is complete when:

- ✅ All 128 tasks are completed
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Manual testing scenarios pass
- ✅ Web server starts with `q chat --web-ui`
- ✅ Browser can connect to http://localhost:8080
- ✅ Worker state displays correctly in UI
- ✅ Streaming output appears in real-time
- ✅ Prompts can be sent from UI
- ✅ Jobs can be cancelled from UI
- ✅ Reconnection works after disconnect
- ✅ Shutdown is graceful and coordinated
- ✅ Documentation is updated
- ✅ Code passes lints and formatting checks

### Risk Mitigation

**High Priority Risks:**
1. WebSocket connection stability → Robust reconnection logic
2. Event lag and buffer overflow → Large buffer + snapshot on lag
3. Port conflicts → Configurable port with clear error messages

**Medium Priority Risks:**
4. Browser compatibility → Test on major browsers
5. Security vulnerabilities → Local-only binding, input validation

**Low Priority Risks:**
6. Time conversion accuracy → Consistent conversion function

### Next Steps

1. Review this implementation plan
2. Get approval from team
3. Begin Phase 1: Backend Infrastructure
4. Track progress in implementation log

---

## Implementation Notes

### Code Style

- Follow existing Q CLI code style
- Use descriptive variable names
- Add documentation comments for public APIs
- Keep functions small and focused
- Use Result for error handling

### Testing Strategy

- Write unit tests for pure functions
- Write integration tests for API endpoints
- Use manual testing for UI interactions
- Test error paths and edge cases

### Logging

- Use tracing macros (info!, warn!, error!)
- Log important events (connection, disconnection, errors)
- Include context (worker_id, job_id) in logs
- Use appropriate log levels

### Error Handling

- Use eyre::Result for error propagation
- Provide context with .wrap_err()
- Log errors before returning
- Return user-friendly error messages

### Performance Considerations

- Use async/await for I/O operations
- Avoid blocking operations in async context
- Use broadcast channel for efficient multi-client distribution
- Keep event payloads small

### Security Considerations

- Bind to 127.0.0.1 only (no remote access)
- Validate all input from WebSocket commands
- Use serde_json for safe JSON parsing
- Escape output in frontend (textContent, not innerHTML)

---

## References

- **Design Document:** `mvp-webui-2-design.md`
- **Research Document:** `mvp-webui-1-research.md`
- **Scope Document:** `mvp-webui-0-scope.md`
- **Architecture Documentation:** `codebase/agent-environment/README.md`
- **EventBus Documentation:** `codebase/agent-environment/event-bus.md`
- **Web-q Reference:** `/Volumes/workplace/web-q/`

---

*This implementation plan was generated on 2025-10-15 and is ready for execution.*
