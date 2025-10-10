# EventBus - Event Distribution System

**Location**: `crates/chat-cli/src/agent_env/event_bus.rs`

## Overview

The EventBus is the central event distribution system that enables communication between all components in the Agent Environment architecture. It uses tokio's broadcast channel to efficiently multicast events to multiple subscribers.

## Design

### Core Concept

The EventBus implements a **publish-subscribe pattern** where:
- Components publish events without knowing who will receive them
- Multiple subscribers can listen to all events
- Events are delivered asynchronously without blocking publishers
- Lagged events are handled gracefully when subscribers fall behind

### Implementation

```rust
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEnvironmentEvent>,
    buffer_size: usize,
}
```

## API

### Creating an EventBus

```rust
// Default buffer size (1000 events)
let event_bus = EventBus::default();

// Custom buffer size
let event_bus = EventBus::new(5000);
```

### Publishing Events

```rust
// Publish event to all subscribers
event_bus.publish(AgentEnvironmentEvent::Worker(
    WorkerEvent::Created {
        worker_id,
        name: "main".to_string(),
        timestamp: Instant::now(),
    }
));

// Publishing never blocks - errors are ignored if no subscribers
```

### Subscribing to Events

```rust
// Get a receiver for events
let mut receiver = event_bus.subscribe();

// Receive events in a loop
while let Ok(event) = receiver.recv().await {
    match event {
        AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
            // Handle output chunk
        }
        _ => {}
    }
}
```

### Handling Lagged Events

```rust
loop {
    match receiver.recv().await {
        Ok(event) => {
            // Process event
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            // Subscriber fell behind by n events
            tracing::warn!("Event bus lagged by {} events", n);
            // Continue processing - some events were dropped
        }
        Err(broadcast::error::RecvError::Closed) => {
            // EventBus was dropped
            break;
        }
    }
}
```

## Event Types

See [events.rs](../../crates/chat-cli/src/agent_env/events.rs) for complete event definitions.

### Top-Level Event Envelope

```rust
pub enum AgentEnvironmentEvent {
    Worker(WorkerEvent),
    Job(JobEvent),
    AgentLoop(AgentLoopEvent),
    System(SystemEvent),
}
```

### Worker Events

```rust
pub enum WorkerEvent {
    Created { worker_id, name, timestamp },
    Deleted { worker_id, timestamp },
    LifecycleStateChanged { worker_id, old_state, new_state, timestamp },
}
```

### Job Events

```rust
pub enum JobEvent {
    Started { worker_id, job_id, task_type, timestamp },
    Completed { worker_id, job_id, result, timestamp },
    OutputChunk { worker_id, job_id, chunk, timestamp },
}
```

### AgentLoop Events

```rust
pub enum AgentLoopEvent {
    ResponseReceived { worker_id, job_id, text, timestamp },
    ToolUseRequestReceived { worker_id, job_id, tool_name, tool_input, timestamp },
}
```

### System Events

```rust
pub enum SystemEvent {
    ShutdownInitiated { reason, timestamp },
}
```

## Event Helper Methods

```rust
// Extract worker_id from any event
if let Some(worker_id) = event.worker_id() {
    // Filter by worker
}

// Check event type
if event.is_job_event() {
    // Handle job events
}

// Get timestamp
let timestamp = event.timestamp();
```

## Usage Patterns

### Pattern 1: Filter by Worker ID

```rust
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    if let Some(wid) = event.worker_id() {
        if wid == target_worker_id {
            handle_event(event).await;
        }
    }
}
```

### Pattern 2: Filter by Event Type

```rust
let mut receiver = event_bus.subscribe();
while let Ok(event) = receiver.recv().await {
    match event {
        AgentEnvironmentEvent::Job(JobEvent::OutputChunk { chunk, .. }) => {
            print!("{}", chunk);
        }
        AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { .. }) => {
            update_status_display();
        }
        _ => {}
    }
}
```

### Pattern 3: Multiple Subscribers

```rust
// UI subscriber
let mut ui_receiver = event_bus.subscribe();
tokio::spawn(async move {
    while let Ok(event) = ui_receiver.recv().await {
        display_event(event).await;
    }
});

// Logging subscriber
let mut log_receiver = event_bus.subscribe();
tokio::spawn(async move {
    while let Ok(event) = log_receiver.recv().await {
        log_event(event).await;
    }
});

// Test subscriber
let mut test_receiver = event_bus.subscribe();
let events: Vec<_> = collect_events(&mut test_receiver).await;
```

## Performance Considerations

### Buffer Size

- **Default**: 1000 events
- **Recommendation**: Increase for high-frequency events or slow subscribers
- **Trade-off**: Larger buffer uses more memory but reduces lag

### Lagged Events

When a subscriber falls behind:
1. EventBus drops oldest events to make room
2. Subscriber receives `RecvError::Lagged(n)` indicating n dropped events
3. Subscriber can log warning and continue processing

### Cloning

EventBus is cheap to clone (Arc internally):
```rust
let event_bus2 = event_bus.clone(); // Shares same broadcast channel
```

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_publish_subscribe() {
    let event_bus = EventBus::default();
    let mut receiver = event_bus.subscribe();
    
    event_bus.publish(test_event);
    
    let received = receiver.recv().await.unwrap();
    assert_eq!(received, test_event);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_multiple_subscribers() {
    let event_bus = EventBus::default();
    let mut r1 = event_bus.subscribe();
    let mut r2 = event_bus.subscribe();
    
    event_bus.publish(test_event);
    
    assert_eq!(r1.recv().await.unwrap(), test_event);
    assert_eq!(r2.recv().await.unwrap(), test_event);
}
```

## Design Decisions

### Why Broadcast Channel?

- **Efficient**: Single send, multiple receives
- **Non-blocking**: Publishers never wait for subscribers
- **Built-in**: Part of tokio, well-tested
- **Flexible**: Subscribers can join/leave dynamically

### Why Ignore Send Errors?

Publishing should never fail due to lack of subscribers. This allows:
- Components to publish events before UIs are created
- Tests to run without subscribing to all events
- Graceful degradation when subscribers disconnect

### Why Clone for Events?

Events must be Clone because:
- Broadcast channel requires Clone for multicasting
- Multiple subscribers need independent copies
- Events are small (mostly IDs and timestamps)

## Related Documentation

- [Events](../../crates/chat-cli/src/agent_env/events.rs) - Event type definitions
- [AgentEnvironment](./agent-environment.md) - Event multicasting coordinator
- [Session](./session.md) - Event publishing from Session
- [Worker Tasks](./tasks.md) - Event publishing from tasks
