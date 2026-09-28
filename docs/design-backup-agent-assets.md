# Design Proposal: Backup and Restore of AI Agent Assets (Skills, MCP, Plugins)

## 1. Context and Motivation

`ai-memory` coordinates memory and context across AI coding agents (Claude Code, OpenAI Codex, Antigravity CLI, Gemini CLI, Cursor, OpenCode, Devin, Grok, Kiro, etc.).

Currently:
- **Server Backup:** `ai-memory backup` takes a snapshot of the server's SQLite database (`memory.sqlite`), git wiki (`wiki/`), and `config.toml` via `POST /admin/backup`.
- **Client Configuration Knowledge:** `ai-memory` installers (`install_mcp.rs`, `install_skills.rs`, `install_hooks.rs`, `uninstall.rs`) already have detailed definitions of where each supported AI harness stores MCP servers, skills, plugins, and project instructions.
- **Gap:** There is no command to snapshot, export, backup, or restore the host agent configurations (MCP servers, agent skills, plugins, instructions). When moving to a new machine or backing up a development environment, developers must reconfigure every agent manually.

---

## 2. Target Matrix by Agent Harness

| Harness | MCP Configuration | Skills (Global / Project) | Plugins / Extensions | Instructions / Rules |
| :--- | :--- | :--- | :--- | :--- |
| **Claude Code / Desktop** | `~/.claude.json`<br>`~/.claude/settings.json`<br>`claude_desktop_config.json` | `~/.claude/skills/`<br>`.claude/skills/` | `~/.claude/plugins/`<br>`Claude Extensions/` | `CLAUDE.md` |
| **OpenAI Codex CLI** | `~/.codex/config.toml`<br>`~/.codex/mcp.json` | `~/.agents/skills/`<br>`.agents/skills/`<br>`~/.codex/skills/` | `~/.codex/plugins/` | `AGENTS.md` |
| **Antigravity CLI (`agy`) / Gemini** | `~/.gemini/settings.json`<br>`~/.gemini/config/mcp_config.json`<br>`antigravity-cli/mcp_config.json` | `~/.gemini/antigravity-cli/skills/`<br>`~/.gemini/skills/`<br>`.gemini/skills/` | `antigravity-cli/plugins/`<br>`~/.gemini/plugins/` | `GEMINI.md`<br>`AGENTS.md`<br>`rules/` |
| **Cursor IDE** | `~/.cursor/mcp.json`<br>`.cursor/mcp.json` | — | VS Code / Cursor extensions | `.cursorrules`<br>`.cursor/rules/` |
| **OpenCode (v1 / v2)** | `opencode.json`<br>`opencode.jsonc`<br>`~/.config/opencode/opencode.json` | `~/.config/opencode/skills/` | `~/.config/opencode/plugins/` | `AGENTS.md` |
| **Devin CLI** | `~/.devin/config.json` | `~/.devin/skills/` | — | Devin instructions |
| **Grok Build CLI** | `~/.grok/config.toml` | `~/.grok/skills/` | — | Grok instructions |
| **Kiro CLI (AWS)** | `~/.kiro/settings/mcp.json` | `~/.kiro/agents/*.json` | — | — |
| **OpenClaw** | `~/.openclaw/config.json` | — | `~/.openclaw/extensions/` | — |
| **VS Code Copilot** | `.vscode/mcp.json` | — | Extensions | Copilot instructions |

---

## 3. Architecture & CLI Design

### Dedicated Host CLI Subcommands: `backup-agents` and `restore-agents`

Host configurations live on the client filesystem, independent of whether the `ai-memory serve` daemon is running. Therefore, backup and restore of agent assets execute as client-side commands.

#### CLI Command Specification

```bash
# Backup all detected agents into an archive
ai-memory backup-agents -o agent-assets.tar.gz

# Filter specific agents or scopes
ai-memory backup-agents --agents claude,codex,antigravity --scope both -o backup.tar.gz

# Include raw secrets (warns loudly, sets 0600 file mode)
ai-memory backup-agents -o backup.tar.gz --include-secrets

# Dry-run inspection of an archive before restoring
ai-memory restore-agents -i backup.tar.gz --dry-run

# Apply restoration atomically
ai-memory restore-agents -i backup.tar.gz --apply
```

---

## 4. Archive Manifest Schema (`manifest.json`)

Stored in the root of the generated tarball:

```json
{
  "$schema": "https://ai-memory.dev/schemas/agent-backup-v1.json",
  "version": 1,
  "created_at": "2026-09-28T14:30:00Z",
  "host": {
    "os": "macos",
    "arch": "aarch64",
    "home_dir": "/Users/developer"
  },
  "sanitized": true,
  "entries": [
    {
      "agent": "claude-code",
      "asset_kind": "mcp-config",
      "scope": "global",
      "archive_path": "agents/claude-code/settings.json",
      "target_relative": ".claude/settings.json"
    },
    {
      "agent": "claude-code",
      "asset_kind": "skill",
      "scope": "global",
      "archive_path": "agents/claude-code/skills/custom-skill/SKILL.md",
      "target_relative": ".claude/skills/custom-skill/SKILL.md"
    },
    {
      "agent": "codex",
      "asset_kind": "mcp-config",
      "scope": "global",
      "archive_path": "agents/codex/config.toml",
      "target_relative": ".codex/config.toml"
    }
  ]
}
```

---

## 5. Security & Invariants

1. **Secret Redaction:**
   - By default, known tokens, bearer headers, and environment keys (`*_TOKEN`, `*_KEY`, `*_SECRET`) in MCP configs are sanitized using `ai-memory-hooks::sanitizer`.
   - Passing `--include-secrets` bypasses redaction; the resulting `.tar.gz` is created with `0600` permissions and a warning is logged.
2. **Symlink Safety:**
   - Archives do not traverse or include external symlinks (`follow_symlinks(false)`).
3. **Path Traversal Protection:**
   - Restoring validates that every target path resolves strictly inside the intended destination directory (`home_dir()` or project cwd).
4. **Atomic Restores:**
   - Target files are written using temporary files + rename + sync (`apply_atomic`), avoiding partial or corrupted configurations.
   - Non-managed custom files are never overwritten without explicit `--force`.

---

## 6. Implementation Stages

1. **`crates/ai-memory-core`**:
   - Add `agent_backup` module containing `AgentAssetKind`, `AgentAssetScope`, `AgentBackupEntry`, and `AgentBackupManifest`.
2. **`crates/ai-memory-cli`**:
   - `src/cli.rs`: Register `BackupAgents(BackupAgentsArgs)` and `RestoreAgents(RestoreAgentsArgs)`.
   - `src/commands/backup_agents.rs`: Scan host configs, package archive.
   - `src/commands/restore_agents.rs`: Validate manifest, present diffs, write files.
3. **Tests**:
   - Roundtrip tests: archive -> verify manifest -> extract into temporary directory -> assert equality.
   - Sanitization tests: ensure keys are redacted unless requested otherwise.
   - Security tests: reject path traversal payloads (`../../etc/passwd`).
