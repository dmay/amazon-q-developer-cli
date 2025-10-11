# MVP Workflows Overview

## Summary

This document provides an overview of all MVP sub-task workflows. Each workflow has its own folder with a detailed scope document (`*-0-scope.md`) that will be expanded into technical designs and implementation plans.

## Workflow Structure

```
planning/
├── mvp-workflows-overview.md          # This file
├── mvp-small-wins/                    # Quick implementation tasks
│   └── mvp-small-wins-0-scope.md
├── mvp-agent-and-context/             # Context management
│   └── mvp-agent-and-context-0-scope.md
├── mvp-tools-basic/                   # Tool system integration
│   └── mvp-tools-basic-0-scope.md
├── mvp-tools-mcp/                     # MCP integration
│   └── mvp-tools-mcp-0-scope.md
├── mvp-codewhisperer/                 # CodeWhisperer provider
│   └── mvp-codewhisperer-0-scope.md
└── mvp-webui/                         # Web UI
    └── mvp-webui-0-scope.md
```

---

## Workflows

### 1. mvp-small-wins

**Tasks:**
- 1.1: --no-interactive support
- 1.6: StructuredIO enhancements

**Complexity:** Low  
**Effort:** 3-5 hours total  
**Dependencies:** None  
**Priority:** High (quick wins)

**Description:**
Quick implementation tasks that provide immediate value. Includes making both TextUi and StructuredIO work in non-interactive mode, and enhancing StructuredIO to display all worker lifecycle events and handle quit commands properly.

**Key Deliverables:**
- Non-interactive mode works in both UIs
- StructuredIO displays worker creation/deletion events
- Quit command interrupts immediately

---

### 2. mvp-agent-and-context

**Tasks:**
- 1.4: --agent context loading
- 1.2: History accumulation (passing history to LLM)

**Complexity:** Medium  
**Effort:** 7-10 hours total  
**Dependencies:** None (independent tasks)  
**Priority:** High (foundation for multi-turn conversations)

**Description:**
Context management and agent configuration integration. History accumulation enables multi-turn conversations by sending entire conversation history to LLM. Agent context loading reads resource files from agent configs and injects them into the conversation.

**Key Deliverables:**
- Multi-turn conversations work correctly
- Agent configs load with resource files
- Context is available to LLM
- /context command shows full history and agent context

---

### 3. mvp-tools-basic

**Tasks:**
- 1.3: Tool system integration (fs_read, fs_write)

**Complexity:** High  
**Effort:** 26-34 hours (includes research and design)  
**Dependencies:** None, but blocks MCP integration  
**Priority:** Critical (critical path for MVP)

**Description:**
Integration of the tool system into agent_env architecture. This is the most complex task requiring significant research and design. Implements two basic tools (fs_read and fs_write) and creates a strategy for migrating other tools.

**Phases:**
1. Research (6-8 hours): Study existing tools, Bedrock API, permission system
2. Design (8-10 hours): Design tool trait, registry, execution flow
3. Implementation (12-16 hours): Implement tool system and port fs_read/fs_write

**Key Deliverables:**
- Tool trait defined for agent_env
- ToolRegistry implemented
- ModelProvider supports tool use
- fs_read and fs_write work in agent_env
- Permission system functional
- Migration strategy for other tools

---

### 4. mvp-tools-mcp

**Tasks:**
- 3.1: MCP integration (shared instances)
- 3.2: Worker-specific MCP (future - out of MVP scope)

**Complexity:** High  
**Effort:** 20-28 hours (Task 3.1 only)  
**Dependencies:** mvp-tools-basic (Task 1.3) must be complete  
**Priority:** Medium (post-MVP core)

**Description:**
Model Context Protocol integration for external tool providers. Task 3.1 implements shared MCP instances available to all workers. Task 3.2 (worker-specific MCP) is documented but out of MVP scope.

**Phases:**
1. Research (4-6 hours): Study MCP protocol, existing integration
2. Design (6-8 hours): Design MCP server manager, tool wrappers
3. Implementation (10-14 hours): Implement MCP integration

**Key Deliverables:**
- MCP servers can be initialized from agent config
- Tools are discovered from MCP servers
- MCP tools work through ToolRegistry
- Multiple MCP servers supported
- Clean lifecycle management

---

### 5. mvp-codewhisperer

**Tasks:**
- 1.5: CodeWhisperer model provider

**Complexity:** High  
**Effort:** 16-24 hours (depends on API capabilities)  
**Dependencies:** None (independent)  
**Priority:** Medium (nice to have for MVP)

**Description:**
Integration of CodeWhisperer as an alternative model provider. Adds --platform flag to choose between Bedrock and CodeWhisperer. Effort depends heavily on CodeWhisperer API capabilities (streaming, tool use support).

**Phases:**
1. Research (4-6 hours): Study CodeWhisperer API, capabilities
2. Design (4-6 hours): Design provider implementation, request/response mapping
3. Implementation (8-12 hours): Implement CodeWhispererModelProvider

**Key Deliverables:**
- CodeWhispererModelProvider implemented
- --platform flag works
- Streaming works (native or simulated)
- Tool use works (if supported)
- Authentication works

**Risks:**
- CodeWhisperer may not support chat API
- May not support streaming
- May not support tool use
- Alternative approaches documented in scope

---

### 6. mvp-webui

**Tasks:**
- 2.1: Web UI with minimal worker presentation

**Complexity:** High  
**Effort:** 20-30 hours  
**Dependencies:** None (independent, but benefits from other features)  
**Priority:** Low (fancy feature, optional for MVP)

**Description:**
Web-based user interface for agent_env. Demonstrates flexibility of event-driven architecture. Provides visual interface for workers, streaming output, and command sending.

**Phases:**
1. Design (4-6 hours): Architecture, technology choices, security model
2. Backend (8-12 hours): Web server, WebSocket, event streaming
3. Frontend (8-12 hours): UI implementation, event handling

**Key Deliverables:**
- Web server with WebSocket support
- Worker list with real-time updates
- Streaming output display
- Prompt input and job cancellation
- Responsive design

---

## Dependencies Graph

```
┌─────────────────────┐
│  mvp-small-wins     │  (No dependencies)
│  - 1.1, 1.6        │
└─────────────────────┘

┌─────────────────────┐
│ mvp-agent-context   │  (No dependencies)
│  - 1.2, 1.4        │
└─────────────────────┘

┌─────────────────────┐
│  mvp-tools-basic    │  (No dependencies, but blocks MCP)
│  - 1.3             │
└──────────┬──────────┘
           │
           │ (blocks)
           ▼
┌─────────────────────┐
│  mvp-tools-mcp      │  (Depends on tools-basic)
│  - 3.1, 3.2        │
└─────────────────────┘

┌─────────────────────┐
│ mvp-codewhisperer   │  (No dependencies)
│  - 1.5             │
└─────────────────────┘

┌─────────────────────┐
│  mvp-webui          │  (No dependencies, benefits from others)
│  - 2.1             │
└─────────────────────┘
```

---

## Implementation Priority

### Phase 1: Quick Wins (Day 1)
**Goal:** Immediate improvements, build momentum

- ✅ mvp-small-wins (1.1, 1.6)
  - Effort: 3-5 hours
  - No dependencies
  - High value, low effort

### Phase 2: Foundation (Day 1-2)
**Goal:** Enable multi-turn conversations

- ✅ mvp-agent-and-context (1.2)
  - Effort: 3-4 hours
  - History accumulation
  - Foundation for conversations

### Phase 3: Critical Path (Day 2-5)
**Goal:** Tool system integration

- ⚠️ mvp-tools-basic (1.3)
  - Effort: 26-34 hours
  - Research → Design → Implementation
  - Blocks MCP integration
  - Most complex task

### Phase 4: Parallel Work (Day 5-7)
**Goal:** Context loading and model provider

- ✅ mvp-agent-and-context (1.4)
  - Effort: 4-6 hours
  - Agent context loading
  - Can be done in parallel with CodeWhisperer

- ✅ mvp-codewhisperer (1.5)
  - Effort: 16-24 hours
  - Research → Design → Implementation
  - Independent of other tasks

### Phase 5: Optional Features (Day 8+)
**Goal:** Polish and fancy features

- 🎨 mvp-webui (2.1)
  - Effort: 20-30 hours
  - Nice to have
  - Demonstrates architecture flexibility

- 🔮 mvp-tools-mcp (3.1)
  - Effort: 20-28 hours
  - Depends on tools-basic
  - Post-MVP core feature

---

## Effort Summary

### MVP Core (Must Have)
- mvp-small-wins: 3-5 hours
- mvp-agent-and-context: 7-10 hours
- mvp-tools-basic: 26-34 hours
- **Total: 36-49 hours (5-7 days)**

### MVP Extended (Nice to Have)
- mvp-codewhisperer: 16-24 hours
- **Total with CodeWhisperer: 52-73 hours (7-10 days)**

### MVP Complete (All Features)
- mvp-webui: 20-30 hours
- **Total with WebUI: 72-103 hours (10-14 days)**

### Post-MVP
- mvp-tools-mcp: 20-28 hours
- **Total with MCP: 92-131 hours (12-18 days)**

---

## Risk Assessment

### High Risk
- **mvp-tools-basic (1.3)**: Most complex, critical path, requires design
  - Mitigation: Thorough research phase, incremental implementation

### Medium Risk
- **mvp-codewhisperer (1.5)**: API capabilities unknown
  - Mitigation: Research phase, alternative approaches documented
  
- **mvp-tools-mcp (3.1)**: Process management complexity
  - Mitigation: Leverage existing code, thorough testing

### Low Risk
- **mvp-small-wins**: Simple, well-defined
- **mvp-agent-and-context**: Straightforward, existing patterns
- **mvp-webui**: Independent, optional

---

## Success Criteria

### MVP Core Success
- ✅ Non-interactive mode works
- ✅ Multi-turn conversations work
- ✅ Agent context loading works
- ✅ fs_read and fs_write tools work
- ✅ Tool permission system works
- ✅ StructuredIO displays all events

### MVP Extended Success
- ✅ CodeWhisperer provider works
- ✅ Platform switching works

### MVP Complete Success
- ✅ Web UI displays workers
- ✅ Web UI shows streaming output
- ✅ Web UI sends commands

### Post-MVP Success
- ✅ MCP servers initialize
- ✅ MCP tools work

---

## Next Steps

For each workflow:

1. **Review scope document** (`*-0-scope.md`)
2. **Conduct research phase** (if needed)
   - Create `*-1-research.md`
3. **Create design document** (if needed)
   - Create `*-2-design.md`
4. **Create implementation plan**
   - Create `*-3-implementation-plan.md`
5. **Execute implementation**
   - Track progress in `*-4-implementation-log.md`

---

## Related Documentation

- [MVP Scope](mvp/mvp-0-scope.md) - Original MVP requirements
- [MVP Detailed Scope](mvp/mvp-1-detailed-scope.md) - Task breakdown
- [MVP Processing Plan](mvp/mvp-2-scope-processing-plan.md) - Original task list
- [Agent Environment README](../codebase/agent-environment/README.md) - Architecture overview
- [EventBus Design](event-bus/event-bus-1-design.md) - EventBus architecture

---

## Notes

- Each workflow is designed to be independently understandable
- Scope documents contain all necessary context
- Dependencies are clearly documented
- Effort estimates include research and design time
- Risk mitigation strategies are included
- Success criteria are measurable
