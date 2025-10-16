# ToolManager Analysis - What would we need in re-implementation?

## MCP servers loading

ToolManager initialization for an agent is async - maning building a worker is async in nature

Special case for the main worker build - TextUi has to show loading indicator, that can be broken with Ctrl+C.
StructuredIO can also show signals about loading status on startup.

## Modifield model

- Worker owns ToolsProvider
- ToolProvider owns a list of available tools, `Map<String, dyn ToolInstance>`
- ToolProvider is subscribed to EventBus to listen for 'McpOrchestrator::NewToolAvailable'
- ToolInstance has
    - ToolId
    - Trust flag
    - Tool config
    - Tool name and description for LLM
- WorkerBuilder configures ToolsProvider using Agent config data
    - creates ToolInstances for built-in tools on the spot
    - identifies MCP servers to launch (and to share in future)
    - Talks to McpOrchestrator to launch the servers
        - `McpOrchestrator.launch(mcpConfig, exclusiveId ('shared' or workerId)) -> Available(mcpServerInstanceId) | Loading(mcpServerInstanceId)`
    - if requested server is available, will add tools to the ToolProvider immediately
    - if requested server is loading - will monitor EventBus to listen for 'McpOrchestrator::NewToolAvailable'


- McpOrchestrator maintains connected MCP Servers
    - `.launch(mcpConfig, exclusiveId ('shared' or workerId), serverConfigName) -> Available(mcpServerInstanceId) | Loading(mcpServerInstanceId)`
    - emits to EventBus `McpOrchestrator::ServerLoaded(mcpServerInstanceId)`
    - emits to EventBus `McpOrchestrator::NewToolAvailable(mcpServerInstanceId, toolNameAtTheServer)`
    - listens EventBus for `WorkerDeleted(worker_id)` to cleanup related server instances
    - maintains the list of active and loading MCP server instances
        - each instance is associated with one or more workerIDs
        - each instance has state Loading|Failed|Ready