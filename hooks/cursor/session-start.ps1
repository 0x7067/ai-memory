. "$PSScriptRoot\..\lib\ai-memory-hook.ps1"
# Cursor does not reliably deliver SessionStart additional_context to the
# agent. Keep the single-use handoff pending for MCP memory_handoff_accept.
Invoke-AiMemoryHook -Event "session-start" -Agent "cursor"
exit 0
