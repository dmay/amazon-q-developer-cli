# Idea: Layered Interface with UI Adapters

## Core Concept

Define explicit interface layers between core application loop and UI implementations. Core loop calls into UI adapters through well-defined traits, UI implementations provide concrete adapters.

## Key Components

1. **Core Application Loop**: Owns Session, manages lifecycle
   - Polls Session for state changes
   - Routes events to UI adapter
   - Handles coordination between multiple workers

2. **UI Adapter Traits**: Multiple traits for different concerns
   - `OutputAdapter`: Handle streaming output, state changes
   - `InputAdapter`: Provide user input, handle prompts
   - `InteractionAdapter`: Tool approvals, confirmations
   - `LifecycleAdapter`: Startup, shutdown, error handling

3. **UI Implementation**: Provides concrete adapter implementations
   - TextUI implements all adapter traits
   - WebUI implements same traits differently
   - Can compose adapters (TextOutput + WebInput)

4. **Event Routing**: Core loop actively routes events
   - Session notifies core loop
   - Core loop calls appropriate adapter methods
   - Adapters update their UI state

## Benefits

- Explicit contracts: Clear what UI must implement
- Type safety: Compiler enforces interface compliance
- Composable: Mix and match adapter implementations
- Synchronous model: Easier to reason about control flow

## Challenges

- Core loop has more responsibility
- Tighter coupling between core and UI concepts
- Need to design trait boundaries carefully
