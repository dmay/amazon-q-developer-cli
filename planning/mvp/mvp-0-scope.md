# Scope for Minimal Valuable Product

To present the implementation of new backend and get traction with other engineers, I need to present
my prototype with some functions that would attract interest.

There are two categories - minimal functionality and fancy features.

**Minimal functionality**
- Support --no-interactive in both TextUi and StrucuredIO
- AgentLoop 
    - actually accumulating the history
    - Minimal tools: fs_read, fs_write (this is going to be the biggest change that need significant research and design)
- Default worker built with --agent context
    - requires research how the context files are stored and then sent to CodeWhispererAPI
- CodeWhispererAPI model provider
    - option to switch? --platform=CodeWhisperer|Bedrock
- StrucuredIO 
    - display events when workers being added/deleted
    - display events for the default worker when it's created == start listening to EventBus before Session start send stuff there
    - make sure it interrupts properly on `{"command":"quit"}`


**Fancy features**
- Web UI with minimal presentation of the workers
    - One-page app based on /Volumes/workplace/web-q/public + /Volumes/workplace/web-q/src design

**Questions people will ask**
- How MCP servers will be integrated into this infra? Specifically initialization procedure, and potential support for worker-specific MCP instances
    - first goal is to have MCP server instances started for the whole app and shared between workers
