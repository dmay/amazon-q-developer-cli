# Idea: Hybrid Event Bus + Direct Interfaces

## Core Concept

Combine event bus for asynchronous notifications with direct interface calls for synchronous interactions. Best of both worlds: loose coupling for events, clear contracts for interactions.

## Key Components

1. **Event Bus**: For asynchronous, fire-and-forget notifications
   - State changes (Worker state, Job state)
   - Streaming output (response chunks)
   - Progress updates
   - Broadcast channel, multiple subscribers

2. **Direct Interfaces**: For synchronous, request-response interactions
   - `PromptProvider`: UI provides user input when requested
   - `ToolApprover`: UI approves/denies tool execution
   - `ErrorHandler`: UI handles errors, decides recovery
   - Passed to Session/Workers as trait objects

3. **Application Loop**: Lightweight coordinator
   - Creates Session with UI interfaces
   - Spawns event bus subscriber task
   - Handles shutdown coordination

4. **UI Implementation**: Implements both patterns
   - Provides interface implementations (PromptProvider, etc.)
   - Subscribes to event bus for display updates
   - Internal state synchronized from both sources

## Benefits

- Right tool for right job: Events for notifications, interfaces for interactions
- Loose coupling for display, tight coupling for critical interactions
- Clear separation of concerns
- Flexible: Can add more events or interfaces independently

## Challenges

- Two communication patterns to understand
- Need to coordinate state between events and interface calls
- More complex mental model
