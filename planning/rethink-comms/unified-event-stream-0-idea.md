# Idea: Unified Event Stream Architecture

## Core Concept

Create a single, typed event stream that carries all events from Session, Workers, Jobs, and Tasks. UI implementations subscribe to this stream and filter/handle events they care about.

## Key Components

1. **Unified Event Enum**: Single enum type covering all possible events
   - `SessionEvent::WorkerCreated(worker_id)`
   - `SessionEvent::JobStarted(worker_id, job_id, ...)`
   - `WorkerEvent::StateChanged(worker_id, state)`
   - `TaskEvent::AgentLoopStatusChanged(worker_id, status)`
   - `StreamEvent::ResponseChunk(worker_id, chunk)`

2. **Event Publisher**: Session owns a broadcast channel, publishes all events
   - Workers/Jobs/Tasks publish through Session
   - Single source of truth for event distribution

3. **UI Subscriber**: UI implementations subscribe to event stream
   - Filter events relevant to their display
   - Multiple UIs can subscribe simultaneously
   - Each UI maintains its own state based on events

4. **Synchronous Interactions**: Separate channel for blocking operations
   - Tool approval requests
   - User input prompts
   - Use oneshot channels for request/response

## Benefits

- Clean separation: Core doesn't know about UI implementations
- Easy to add new UIs: Just subscribe to event stream
- Multiple UIs simultaneously: Each gets same events
- Testable: Can record/replay event streams

## Challenges

- Event ordering guarantees
- Backpressure handling if UI is slow
- Synchronous operations (tool approval) need separate mechanism
