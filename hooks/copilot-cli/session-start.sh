#!/bin/sh
# GitHub Copilot CLI SessionStart hook.
# Copilot CLI reads a top-level `additionalContext` from SessionStart stdout
# (not Claude Code's `hookSpecificOutput` envelope), and that injection is not
# demonstrated live, so this hook captures the event only. Do NOT fetch
# /handoff here: accepting a handoff is destructive and an undelivered handoff
# would be silently lost. Recover a prior session's handoff via the MCP
# `memory_handoff_accept` tool instead.
# At runtime (after `install-hooks --apply`) `_lib.sh` is staged
# alongside this script. From the source tree it lives one dir up.
_lib_dir="$(dirname "$0")"
[ -f "$_lib_dir/_lib.sh" ] || _lib_dir="$_lib_dir/.."
. "$_lib_dir/_lib.sh"

SERVER="${AI_MEMORY_HOOK_URL:-http://127.0.0.1:49374}"
PAYLOAD=$(cat)
CWD=$(ai_memory_extract_cwd "$PAYLOAD")
QS=$(ai_memory_marker_qs "$CWD")

printf '%s' "$PAYLOAD" \
    | ai_memory_post_hook "$SERVER/hook?event=session-start&agent=copilot-cli${QS}" >/dev/null 2>&1 || true
printf '{}\n'
exit 0
