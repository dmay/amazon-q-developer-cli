# MVP Tools Basic - Scope v2 - Outline

## Document Structure

### 1. Overview
- What problem we're solving
- High-level approach
- Key architectural changes from original scope

### 2. Architecture Components
- 2.1 ToolProvider
  - Purpose and responsibilities
  - Location in Worker
  - Tool registry and approval rules
  - execute_tool() method
- 2.2 ToolResult Enum
  - Success
  - Failure
  - ApprovalRequired
- 2.3 OpenToolsApprovalRequests
  - Storage in Worker
  - Structure and lifecycle
  - Approval/rejection flow

### 3. Conversation History Updates
- Current structure
- Required changes for tool support
- Multiple tool requests per assistant response
- Tool results storage
- Alignment with Bedrock and CodeWhisperer APIs

### 4. ContextBuilder
- Purpose and responsibilities
- Tool definitions in ModelRequest
- Tool requests and results in conversation history
- Integration with ModelProviders

### 5. ModelProvider Updates
- 5.1 ModelRequest Changes
  - Tool definitions
  - Conversation history with tools
- 5.2 ModelResponse Changes
  - Tool use requests
  - Streaming tool requests
- 5.3 Bedrock Implementation
  - Bedrock Converse API tool format
  - Request/response handling
- 5.4 CodeWhisperer Implementation
  - CodeWhisperer API tool format
  - Request/response handling
  - Compatibility considerations

### 6. AgentLoop Integration
- 6.1 Task Start Behavior
  - Check OpenToolsApprovalRequests
  - Execute approved tools
  - Stop if unapproved requests exist
- 6.2 Tool Execution Flow
  - Receive tool requests from LLM
  - Call ToolProvider.execute_tool()
  - Handle ToolResult variants
- 6.3 Success/Failure Handling
  - Add to Conversation History
  - Continue loop with new LLM request
- 6.4 ApprovalRequired Handling
  - Store in OpenToolsApprovalRequests
  - Publish EventBus event
  - Exit task

### 7. EventBus Integration
- New event types for tool approvals
- Event publishing points
- Event subscribers (UIs)

### 8. UI Integration
- 8.1 TextUi Approval Flow
  - Trigger on task completion
  - Approval prompt UI
  - Commands: y/n/t/text
  - Full trust flag setting
  - Relaunch AgentLoop when approved
- 8.2 StructuredIO Approval Flow
  - JSON approval format
  - Full trust flag setting
  - Relaunch AgentLoop when approved

### 9. Tool Implementations
- 9.1 Tool Trait for agent_env
- 9.2 fs_read Implementation
- 9.3 fs_write Implementation
- 9.4 Migration Strategy for Other Tools

### 10. Implementation Phases
- Phase 1: Research
  - Bedrock tool API
  - CodeWhisperer tool API
  - Conversation history requirements
- Phase 2: Design
  - Detailed component designs
  - Sequence diagrams
  - Data structures
- Phase 3: Core Infrastructure
  - ToolProvider
  - ToolResult
  - Conversation History updates
  - ContextBuilder
- Phase 4: ModelProvider Updates
  - ModelRequest/ModelResponse changes
  - Bedrock implementation
  - CodeWhisperer implementation
- Phase 5: AgentLoop Integration
  - Tool execution flow
  - Approval request handling
  - Task restart logic
- Phase 6: EventBus and UI
  - Event types
  - TextUi approval flow
  - StructuredIO approval flow
- Phase 7: Tool Implementations
  - fs_read
  - fs_write

### 11. Testing Strategy
- Unit tests
- Integration tests
- End-to-end scenarios
- Approval flow testing

### 12. Acceptance Criteria
- Per-phase criteria
- Overall success metrics

### 13. Dependencies and Risks
- Dependencies on existing components
- Risk mitigation strategies

### 14. Related Files and Documentation
- Implementation files
- Architecture documentation
- API documentation
