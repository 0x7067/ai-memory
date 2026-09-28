//! Integration tests for `ai-memory backup-agents` and `ai-memory restore-agents`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

static CLI_TEST_LOCK: Mutex<()> = Mutex::new(());

fn cli_test_lock() -> MutexGuard<'static, ()> {
    CLI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ai-memory")
}

fn command_with_env(home: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(bin());
    let config_home = home.join(".config");
    let data_home = home.join(".local/share");
    for dir in [&config_home, &data_home] {
        fs::create_dir_all(dir).unwrap();
    }
    command
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", config_home)
        .env("XDG_DATA_HOME", data_home)
        .env("AI_MEMORY_HOME", home)
        .env("AI_MEMORY_DATA_DIR", home.join(".ai-memory-data"))
        .env("AI_MEMORY_EMBEDDING_PROVIDER", "none")
        .env_remove("AI_MEMORY_SERVER_URL")
        .env_remove("AI_MEMORY_AUTH_TOKEN")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("KIMI_CODE_HOME")
        .env_remove("KIRO_HOME");
    command
}

#[test]
fn backup_and_restore_agents_round_trip() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    // 1. Setup mock agent assets
    // Claude
    let claude_settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(claude_settings.parent().unwrap()).unwrap();
    fs::write(
        &claude_settings,
        r#"{"mcpServers":{"local":{"command":"test"}}}"#,
    )
    .unwrap();

    let claude_skill = home.path().join(".claude/skills/test-skill/SKILL.md");
    fs::create_dir_all(claude_skill.parent().unwrap()).unwrap();
    fs::write(&claude_skill, "# Claude Skill\nDetails here").unwrap();

    let claude_md = project.path().join("CLAUDE.md");
    fs::write(&claude_md, "# Project Instructions\nClaude rules").unwrap();

    // Codex
    let codex_config = home.path().join(".codex/config.toml");
    fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
    fs::write(
        &codex_config,
        "[mcp_servers.demo]\nurl = \"http://localhost\"\n",
    )
    .unwrap();

    let agents_md = project.path().join("AGENTS.md");
    fs::write(&agents_md, "# Project Rules\nCodex rules").unwrap();

    // Antigravity / Gemini
    let agy_mcp = home.path().join(".gemini/config/mcp_config.json");
    fs::create_dir_all(agy_mcp.parent().unwrap()).unwrap();
    fs::write(
        &agy_mcp,
        r#"{"mcpServers":{"agy":{"url":"http://127.0.0.1"}}}"#,
    )
    .unwrap();

    let backup_tar = project.path().join("backup.tar.gz");

    // 2. Run backup-agents
    let output = command_with_env(home.path(), project.path())
        .args([
            "backup-agents",
            "-o",
            backup_tar.to_str().unwrap(),
            "--include-secrets",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "backup-agents failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(backup_tar.exists(), "backup tarball was not created");

    // 3. Restore into clean sandbox
    let restore_home = tempfile::tempdir().unwrap();
    let restore_project = tempfile::tempdir().unwrap();

    // Dry-run first
    let dry_run = command_with_env(restore_home.path(), restore_project.path())
        .args(["restore-agents", "-i", backup_tar.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(dry_run.status.success());
    let dry_stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(dry_stdout.contains("[CREATE]"));
    assert!(!restore_home.path().join(".claude/settings.json").exists());

    // Apply restore
    let restore = command_with_env(restore_home.path(), restore_project.path())
        .args([
            "restore-agents",
            "-i",
            backup_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(
        restore.status.success(),
        "restore-agents failed: {}",
        String::from_utf8_lossy(&restore.stderr)
    );

    // Verify restored assets
    let restored_claude =
        fs::read_to_string(restore_home.path().join(".claude/settings.json")).unwrap();
    assert_eq!(
        restored_claude,
        r#"{"mcpServers":{"local":{"command":"test"}}}"#
    );

    let restored_skill = fs::read_to_string(
        restore_home
            .path()
            .join(".claude/skills/test-skill/SKILL.md"),
    )
    .unwrap();
    assert_eq!(restored_skill, "# Claude Skill\nDetails here");

    let restored_claude_md = fs::read_to_string(restore_project.path().join("CLAUDE.md")).unwrap();
    assert_eq!(restored_claude_md, "# Project Instructions\nClaude rules");

    let restored_codex =
        fs::read_to_string(restore_home.path().join(".codex/config.toml")).unwrap();
    assert_eq!(
        restored_codex,
        "[mcp_servers.demo]\nurl = \"http://localhost\"\n"
    );

    let restored_agents_md = fs::read_to_string(restore_project.path().join("AGENTS.md")).unwrap();
    assert_eq!(restored_agents_md, "# Project Rules\nCodex rules");

    let restored_agy =
        fs::read_to_string(restore_home.path().join(".gemini/config/mcp_config.json")).unwrap();
    assert_eq!(
        restored_agy,
        r#"{"mcpServers":{"agy":{"url":"http://127.0.0.1"}}}"#
    );
}

#[test]
fn backup_agents_sanitizes_secrets_by_default() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let claude_settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(claude_settings.parent().unwrap()).unwrap();
    fs::write(
        &claude_settings,
        r#"{"mcpServers":{"remote":{"env":{"GITHUB_TOKEN":"ghp_1234567890abcdef1234567890abcdef1234"}}}}"#,
    )
    .unwrap();

    let backup_tar = project.path().join("sanitized.tar.gz");

    let output = command_with_env(home.path(), project.path())
        .args(["backup-agents", "-o", backup_tar.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());

    // Restore to clean sandbox
    let restore_home = tempfile::tempdir().unwrap();
    let restore_project = tempfile::tempdir().unwrap();

    let restore = command_with_env(restore_home.path(), restore_project.path())
        .args([
            "restore-agents",
            "-i",
            backup_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(restore.status.success());

    let restored = fs::read_to_string(restore_home.path().join(".claude/settings.json")).unwrap();
    assert!(
        !restored.contains("ghp_1234567890abcdef1234567890abcdef1234"),
        "raw secret token must be redacted"
    );
    assert!(
        restored.contains("[REDACTED:"),
        "must contain redaction label, got: {restored}"
    );
}

#[test]
fn restore_agents_filters_by_agent_name() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let claude_settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(claude_settings.parent().unwrap()).unwrap();
    fs::write(&claude_settings, r#"{"mcpServers":{"claude":{}}}"#).unwrap();

    let codex_config = home.path().join(".codex/config.toml");
    fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
    fs::write(&codex_config, "[mcp_servers.codex]\n").unwrap();

    let backup_tar = project.path().join("multi_agent.tar.gz");

    let backup_res = command_with_env(home.path(), project.path())
        .args(["backup-agents", "-o", backup_tar.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(backup_res.status.success());

    // Restore ONLY claude
    let restore_home = tempfile::tempdir().unwrap();
    let restore_project = tempfile::tempdir().unwrap();

    let restore_res = command_with_env(restore_home.path(), restore_project.path())
        .args([
            "restore-agents",
            "-i",
            backup_tar.to_str().unwrap(),
            "--agents",
            "claude",
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(restore_res.status.success());

    // Claude restored, Codex NOT restored
    assert!(restore_home.path().join(".claude/settings.json").exists());
    assert!(!restore_home.path().join(".codex/config.toml").exists());
}

#[test]
fn restore_agents_rejects_path_traversal() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let evil_tar = project.path().join("evil.tar.gz");
    {
        let file = std::fs::File::create(&evil_tar).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let manifest = r#"{
            "version": 1,
            "created_at": "2026-09-28T00:00:00Z",
            "host": { "os": "macos", "arch": "aarch64" },
            "sanitized": true,
            "entries": [{
                "agent": "claude-code",
                "asset_kind": "mcp-config",
                "scope": "global",
                "archive_path": "evil.txt",
                "target_relative": "../../../etc/passwd"
            }]
        }"#;

        let mut h_m = tar::Header::new_gnu();
        h_m.set_size(manifest.len() as u64);
        h_m.set_cksum();
        tar.append_data(&mut h_m, "manifest.json", std::io::Cursor::new(manifest))
            .unwrap();

        let evil_body = "malicious payload";
        let mut h_b = tar::Header::new_gnu();
        h_b.set_size(evil_body.len() as u64);
        h_b.set_cksum();
        tar.append_data(&mut h_b, "evil.txt", std::io::Cursor::new(evil_body))
            .unwrap();

        tar.finish().unwrap();
    }

    let restore = command_with_env(home.path(), project.path())
        .args([
            "restore-agents",
            "-i",
            evil_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();

    assert!(
        !restore.status.success(),
        "must fail on path traversal payload"
    );
    let err = String::from_utf8_lossy(&restore.stderr);
    assert!(err.contains("dangerous") || err.contains("traversal") || err.contains("malformed"));
}

#[test]
fn restore_agents_rejects_unauthorized_destination_path() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let evil_tar = project.path().join("evil_target.tar.gz");
    {
        let file = std::fs::File::create(&evil_tar).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let manifest = r#"{
            "version": 1,
            "created_at": "2026-09-28T00:00:00Z",
            "host": { "os": "macos", "arch": "aarch64" },
            "sanitized": true,
            "entries": [{
                "agent": "claude-code",
                "asset_kind": "mcp-config",
                "scope": "project",
                "archive_path": "payload.txt",
                "target_relative": ".git/hooks/pre-commit"
            }]
        }"#;

        let mut h_m = tar::Header::new_gnu();
        h_m.set_size(manifest.len() as u64);
        h_m.set_cksum();
        tar.append_data(&mut h_m, "manifest.json", std::io::Cursor::new(manifest))
            .unwrap();

        let evil_body = "#!/bin/sh\necho malicious";
        let mut h_b = tar::Header::new_gnu();
        h_b.set_size(evil_body.len() as u64);
        h_b.set_cksum();
        tar.append_data(&mut h_b, "payload.txt", std::io::Cursor::new(evil_body))
            .unwrap();

        tar.finish().unwrap();
    }

    let restore = command_with_env(home.path(), project.path())
        .args([
            "restore-agents",
            "-i",
            evil_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();

    assert!(
        !restore.status.success(),
        "must reject unauthorized destination path"
    );
    let err = String::from_utf8_lossy(&restore.stderr);
    assert!(err.contains("unauthorized path"));
}

#[test]
fn restore_agents_binary_safe_preserves_raw_bytes() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let plugin_dir = home.path().join(".claude/plugins/test-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();
    let binary_bytes: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0xC0, 0xDF, 0x80];
    let binary_file = plugin_dir.join("asset.bin");
    fs::write(&binary_file, &binary_bytes).unwrap();

    let backup_tar = project.path().join("binary_test.tar.gz");

    let backup = command_with_env(home.path(), project.path())
        .args(["backup-agents", "-o", backup_tar.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(backup.status.success());

    let restore_home = tempfile::tempdir().unwrap();
    let restore_project = tempfile::tempdir().unwrap();

    let restore = command_with_env(restore_home.path(), restore_project.path())
        .args([
            "restore-agents",
            "-i",
            backup_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(
        restore.status.success(),
        "restore failed: {}",
        String::from_utf8_lossy(&restore.stderr)
    );

    let restored_bin = fs::read(
        restore_home
            .path()
            .join(".claude/plugins/test-plugin/asset.bin"),
    )
    .unwrap();
    assert_eq!(
        restored_bin, binary_bytes,
        "binary bytes must be preserved exactly"
    );
}

#[test]
fn restore_agents_verifies_sha256_integrity() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let tampered_tar = project.path().join("tampered.tar.gz");
    {
        let file = std::fs::File::create(&tampered_tar).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let manifest = r#"{
            "version": 1,
            "created_at": "2026-09-28T00:00:00Z",
            "host": { "os": "macos", "arch": "aarch64" },
            "sanitized": true,
            "entries": [{
                "agent": "claude-code",
                "asset_kind": "instruction",
                "scope": "project",
                "archive_path": "project/CLAUDE.md",
                "target_relative": "CLAUDE.md",
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
            }]
        }"#;

        let mut h_m = tar::Header::new_gnu();
        h_m.set_size(manifest.len() as u64);
        h_m.set_cksum();
        tar.append_data(&mut h_m, "manifest.json", std::io::Cursor::new(manifest))
            .unwrap();

        let body = "tampered instruction content";
        let mut h_b = tar::Header::new_gnu();
        h_b.set_size(body.len() as u64);
        h_b.set_cksum();
        tar.append_data(&mut h_b, "project/CLAUDE.md", std::io::Cursor::new(body))
            .unwrap();

        tar.finish().unwrap();
    }

    let restore = command_with_env(home.path(), project.path())
        .args([
            "restore-agents",
            "-i",
            tampered_tar.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();

    assert!(
        !restore.status.success(),
        "must fail on SHA-256 integrity mismatch"
    );
    let err = String::from_utf8_lossy(&restore.stderr);
    assert!(err.contains("integrity mismatch") || err.contains("SHA-256"));
}

#[test]
fn backup_and_restore_kebab_case_agent_filter() {
    let _guard = cli_test_lock();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let claude_settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(claude_settings.parent().unwrap()).unwrap();
    fs::write(&claude_settings, r#"{"mcpServers":{"claude":{}}}"#).unwrap();

    let codex_config = home.path().join(".codex/config.toml");
    fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
    fs::write(&codex_config, "[mcp_servers.codex]\n").unwrap();

    let backup_tar = project.path().join("kebab_filter.tar.gz");

    // Filter by kebab-case wire string "claude-code"
    let backup_res = command_with_env(home.path(), project.path())
        .args([
            "backup-agents",
            "-o",
            backup_tar.to_str().unwrap(),
            "--agents",
            "claude-code",
        ])
        .output()
        .unwrap();
    assert!(backup_res.status.success());

    let restore_home = tempfile::tempdir().unwrap();
    let restore_project = tempfile::tempdir().unwrap();

    let restore_res = command_with_env(restore_home.path(), restore_project.path())
        .args([
            "restore-agents",
            "-i",
            backup_tar.to_str().unwrap(),
            "--agents",
            "claude-code",
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(restore_res.status.success());

    assert!(restore_home.path().join(".claude/settings.json").exists());
    assert!(!restore_home.path().join(".codex/config.toml").exists());
}
