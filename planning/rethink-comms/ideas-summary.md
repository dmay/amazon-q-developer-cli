# Architecture Ideas Summary

This document summarizes three proposed architectures for organizing the main application loop and UI communication in the Agent Environment system.

## Context

We have a parallel agent execution system with Workers, Tasks, Jobs, and a Session orchestrator. We need a clean architecture that allows easy UI swapping (simple TUI → fancy TUI → web API) while maintaining clean separation between core logic and presentation.

---

## Idea 1: Layered Interface with UI Adapters

### Summary

The core application loop owns the Session and actively routes events to UI implementations through well-defined adapter traits. UI implementations provide concrete adapters that handle different concerns: output, input, interactions, and lifecycle. The core loop acts as an orchestrator, polling the Session for state changes and calling appropriate adapter methods.

This is a **push model** where the core loop drives all communication. The Session and Workers don't know about UI - the core loop translates their state changes into adapter calls. UI implementations are passive receivers that implement four main traits: `OutputAdapter` (streaming responses, state changes), `InputAdapter` (user input), `InteractionAdapter` (tool approvals, confirmations), and `LifecycleAdapter` (startup, shutdown, errors).

### Effort Estimation

**Medium-High (3-4 weeks)**

- Define 4 adapter traits with ~15-20 methods total: 3-4 days
- Implement core application loop with polling and routing: 4-5 days
- Refactor Session to expose state query methods: 2-3 days
- Implement TextUI adapters: 3-4 days
- Testing and integration: 4-5 days
- Documentation: 2 days

The main complexity is in designing the trait boundaries correctly and implementing the polling/routing logic in the core loop.

### Pros

- **Explicit contracts**: Clear trait boundaries define exactly what UI must implement
- **Type safety**: Compiler enforces interface compliance at compile time
- **Composable**: Can mix and match adapter implementations (TextOutput + WebInput)
- **Synchronous model**: Easier to reason about control flow, no async complexity
- **Testable**: Easy to create mock adapters for testing core loop
- **Familiar pattern**: Similar to traditional adapter/strategy patterns

### Cons

- **Core loop complexity**: Core loop has significant responsibility for routing and coordination
- **Tight coupling**: Core loop must know about all adapter types and when to call them
- **Polling overhead**: Core loop must actively poll Session for state changes
- **Trait proliferation**: Need to carefully design trait boundaries to avoid too many small traits
- **Less flexible**: Adding new event types requires modifying core loop routing logic
- **Synchronous bias**: Async UI patterns (websockets) require workarounds

---

## Idea 2: Hybrid Event Bus + Direct Interfaces

### Summary

This design combines two communication patterns: an event bus for asynchronous, fire-and-forget notifications (state changes, streaming output, progress updates) and direct trait-based interfaces for synchronous, request-response interactions (user prompts, tool approvals, error handling). The Session publishes events to a broadcast channel that UI implementations subscribe to, while also accepting trait objects for critical interactions.

The application loop is lightweight - it creates the Session with UI interface implementations, spawns event subscriber tasks, and coordinates shutdown. UI implementations both subscribe to the event stream for display updates and implement provider interfaces (`PromptProvider`, `ToolApprover`, `ErrorHandler`) for interactions. This gives loose coupling for display with clear contracts for critical operations.

### Effort Estimation

**Medium (2-3 weeks)**

- Define event enum hierarchy (~20-30 variants): 2-3 days
- Implement EventBus with broadcast channel: 1-2 days
- Define 3-4 provider interfaces: 2 days
- Modify Session to publish events and accept interfaces: 3-4 days
- Implement TextUI subscriber and providers: 3-4 days
- Testing and integration: 3-4 days
- Documentation: 2 days

This is the middle ground in complexity - event bus is straightforward, but coordinating two communication patterns requires care.

### Pros

- **Right tool for right job**: Events for notifications, interfaces for interactions
- **Loose coupling for display**: UI can subscribe/unsubscribe dynamically
- **Clear contracts for critical ops**: Tool approval, prompts have explicit interfaces
- **Multiple UIs simultaneously**: Multiple subscribers can receive same events
- **Flexible**: Can add new event types without changing interfaces
- **Async-friendly**: Event bus naturally supports async patterns
- **Separation of concerns**: Display logic separate from interaction logic

### Cons

- **Two patterns to understand**: Developers must learn both event bus and interfaces
- **State coordination**: UI must synchronize state from both events and interface calls
- **More complex mental model**: Need to decide which pattern for each communication
- **Potential race conditions**: Events and interface calls can interleave
- **Backpressure handling**: Need to handle slow subscribers on event bus
- **Testing complexity**: Must test both event flow and interface interactions

---

## Idea 3: Unified Event Stream Architecture

### Summary

Create a single, strongly-typed event stream that carries all events from Session, Workers, Jobs, and Tasks. The Session owns a broadcast channel and publishes a unified `AgentEnvEvent` enum covering all possible events. UI implementations subscribe to this stream and filter/handle events they care about. For synchronous operations (tool approval, user prompts), use separate oneshot channels embedded in request events.

This is a **pure event-driven model**. Everything is an event - state changes, streaming chunks, even requests for user input (which include a oneshot channel for the response). The core application loop is minimal - just creates Session, spawns UI subscriber tasks, and waits for shutdown. UI implementations are completely decoupled from core logic, only knowing about the event stream.

### Effort Estimation

**Low-Medium (2-3 weeks)**

- Define unified event enum (~25-35 variants): 3-4 days
- Implement EventBus: 1 day
- Define sync request types with oneshot channels: 1-2 days
- Modify Session/Worker/Job to publish events: 3-4 days
- Implement TextUI event subscriber: 3-4 days
- Testing and integration: 3-4 days
- Documentation: 2 days

This is the simplest conceptually - everything is an event. The main work is defining the event hierarchy and updating components to publish events.

### Pros

- **Clean separation**: Core completely independent of UI implementations
- **Easy to add UIs**: Just subscribe to event stream and filter
- **Multiple UIs simultaneously**: Natural support for multiple subscribers
- **Testable**: Can record/replay event streams for testing
- **Flexible**: Adding new events doesn't affect existing code
- **Async-native**: Event stream naturally supports async patterns
- **Minimal core loop**: Application loop is trivial
- **Observable**: Easy to add logging, debugging, telemetry

### Cons

- **Event ordering guarantees**: Must carefully handle event ordering
- **Backpressure handling**: Slow subscribers can cause memory buildup
- **Synchronous operations awkward**: Tool approval via events feels unnatural
- **Large event enum**: Single enum with 30+ variants can be unwieldy
- **Type safety trade-off**: Events are runtime values, not compile-time contracts
- **Debugging complexity**: Event flow can be harder to trace than direct calls
- **Potential event loss**: Broadcast channel can drop events if buffer full

---

## Comparison Table

| Aspect | Layered Interface | Hybrid Event + Interface | Unified Event Stream |
|--------|------------------|-------------------------|---------------------|
| **Core Loop Complexity** | High - active routing | Low - lightweight coordinator | Minimal - just spawns tasks |
| **UI Coupling** | Medium - via traits | Low - events + interfaces | Very Low - only events |
| **Type Safety** | Strong - compile-time | Strong for interfaces, runtime for events | Runtime - events are values |
| **Composability** | High - mix adapters | Medium - combine subscribers | High - filter events |
| **Async Support** | Requires workarounds | Native for events | Native throughout |
| **Multiple UIs** | Requires composition | Natural for events | Natural - multiple subscribers |
| **Synchronous Ops** | Natural - direct calls | Natural - interfaces | Awkward - events with channels |
| **Testing** | Easy - mock adapters | Medium - test both patterns | Easy - record/replay events |
| **Debugging** | Easy - direct call stack | Medium - two patterns | Harder - trace event flow |
| **Adding New Events** | Modify core loop | Add to event enum | Add to event enum |
| **Learning Curve** | Medium - familiar pattern | Higher - two patterns | Lower - single pattern |
| **Performance** | Good - direct calls | Good - broadcast channel | Good - broadcast channel |
| **Backpressure** | N/A - synchronous | Must handle for events | Must handle for events |
| **Event Ordering** | Guaranteed - sequential | Must coordinate | Must handle carefully |
| **Code Volume** | High - traits + loop | Medium - events + interfaces | Low - just events |
| **Migration Effort** | High - significant refactor | Medium - incremental | Medium - incremental |

---

## Recommendations

### For Simple TUI (Current Need)
**Hybrid Event + Interface** is the best fit. It provides loose coupling for display updates while maintaining clear contracts for user interactions. The two-pattern approach maps naturally to the problem: events for "show this" and interfaces for "ask the user".

### For Future Web API
**Unified Event Stream** becomes more attractive. Websockets naturally map to event streams, and REST endpoints can query state. The pure event-driven model is ideal for distributed systems.

### For Maximum Type Safety
**Layered Interface** provides the strongest compile-time guarantees. If preventing UI implementation errors is critical, explicit traits are the way to go.

### Hybrid Approach
Consider starting with **Hybrid Event + Interface** for the initial implementation, then evolving toward **Unified Event Stream** as requirements become clearer. The event bus portion is compatible between the two approaches.

---

## Next Steps

1. **Prototype**: Build a minimal proof-of-concept for the chosen approach
2. **Validate**: Test with current TextUI and a mock WebUI
3. **Refine**: Adjust based on prototype learnings
4. **Implement**: Full implementation with tests and documentation
5. **Migrate**: Gradually migrate from current demo implementation
