#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/ai-memory-hooks")
        .to_path_buf()
}

#[test]
fn powershell_marker_aliases_are_bounded_and_forwarded_with_remote_identity() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("Repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "git@git.example.test:Acme/API.git"
            ])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        repo.join(".ai-memory.toml"),
        "project = \"acme-api\"\naliases = [\" Former-Name \", \"legacy_name\", \"Former-Name\"]\n",
    )
    .unwrap();
    let nested = repo.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join(".ai-memory.toml"),
        "[capture]\nignore_paths = [\"secrets/**\"]\n",
    )
    .unwrap();
    let helper = repo_root()
        .join("hooks")
        .join("lib")
        .join("ai-memory-hook.ps1")
        .to_string_lossy()
        .replace('\'', "''");
    let cwd = nested.to_string_lossy().replace('\'', "''");
    let program =
        format!(". '{helper}'; [Console]::Out.Write((Get-AiMemoryMarkerQuery -Cwd '{cwd}'))");
    let output = Command::new(ai_memory_test_support::powershell_exe())
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &program,
        ])
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let query = String::from_utf8(output.stdout).unwrap();
    assert!(
        query.contains("&project=acme-api&project_src=marker"),
        "{query}"
    );
    assert!(
        query.contains("&identity=git.example.test%2Facme%2Fapi&identity_src=git_remote"),
        "{query}"
    );
    assert!(
        query.contains("&aliases=%5B%22Former-Name%22%2C%22legacy_name%22%5D"),
        "{query}"
    );
}
