#!/bin/sh
# cursor SessionStart hook.
# Forwards the event JSON to the ai-memory server (fire-and-forget).
# Cursor does not reliably deliver SessionStart additional_context to the
# agent. Do not consume the single-use handoff until that delivery is proven;
# it remains available through MCP memory_handoff_accept.
#
_lib_dir="$(dirname "$0")"
[ -f "$_lib_dir/_lib.sh" ] || _lib_dir="$_lib_dir/.."
. "$_lib_dir/_lib.sh"

SERVER="${AI_MEMORY_HOOK_URL:-http://127.0.0.1:49374}"
PAYLOAD=$(cat)
CWD=$(ai_memory_extract_cwd "$PAYLOAD")
QS=$(ai_memory_marker_qs "$CWD")

printf '%s' "$PAYLOAD" \
    | ai_memory_post_hook "$SERVER/hook?event=session-start&agent=cursor${QS}" >/dev/null 2>&1 || true
exit 0
