# Improvement for Web UI

The overall scope of this workflow is to make the implemented basic web-ui into something more useful.

- We need to mimic the UI defined in `web-q` prototype - `/Volumes/workplace/web-q/public`
    - List of workers on the left, active chat session for the selected worker (Conversation History) on the right (no web teminal tab, only chat)
        - Note that the chat sesssion tab must display whole Conversation History, not just the last response
    - Indicate a busy worker like in current implementation
    - Received response stream must be joined into the same response container, to appear as single block of text
        - web-q prototype uses 'answer bubble' styling for it. We only need bubble style for the user's prompts. Assistant Answers must be presented as text blocks between prompts.
        - bonus: make it et applied markdown styling as receive the pieces
    - 'New worker` button - also asking for a path, but not using it _yet_; asking for agent name though, to pass to worker builder