//! Filesystem confinement for the namespaced wiki tree.

use std::path::{Component, Path, PathBuf};

use ai_memory_core::{ProjectId, WorkspaceId};

use crate::error::{WikiError, WikiResult};

#[derive(Clone, Copy)]
pub(crate) enum Prepare {
    Inspect,
    Parents,
    Directory,
}

pub(crate) fn initialize_root(root: &Path) -> WikiResult<()> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => require_directory(root, &metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(root)?;
            inspect_created_directory(root)
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn project_root(
    root: &Path,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
    prepare: Prepare,
) -> WikiResult<PathBuf> {
    tree_path(
        root,
        &PathBuf::from(workspace_id.to_string()).join(project_id.to_string()),
        prepare,
    )
}

pub(crate) fn project_path(
    root: &Path,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
    relative: &Path,
    prepare: Prepare,
) -> WikiResult<PathBuf> {
    tree_path(
        root,
        &PathBuf::from(workspace_id.to_string())
            .join(project_id.to_string())
            .join(relative),
        prepare,
    )
}

pub(crate) fn workspace_path(
    root: &Path,
    workspace_id: WorkspaceId,
    prepare: Prepare,
) -> WikiResult<PathBuf> {
    tree_path(root, Path::new(&workspace_id.to_string()), prepare)
}

pub(crate) fn tree_path(root: &Path, relative: &Path, prepare: Prepare) -> WikiResult<PathBuf> {
    validate_relative(relative)?;
    inspect_root(root)?;
    let components: Vec<_> = relative.components().collect();
    let ancestor_count = match prepare {
        Prepare::Inspect | Prepare::Parents => components.len().saturating_sub(1),
        Prepare::Directory => components.len(),
    };
    let create = !matches!(prepare, Prepare::Inspect);
    let mut current = root.to_path_buf();
    let mut missing = false;
    for component in components.iter().take(ancestor_count) {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => require_directory(&current, &metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing = true;
                if create {
                    std::fs::create_dir(&current)?;
                    inspect_created_directory(&current)?;
                }
            }
            Err(error) => return Err(error.into()),
        }
        if missing && !create {
            break;
        }
    }

    let target = root.join(relative);
    if !matches!(prepare, Prepare::Directory) && !missing {
        inspect_final(&target)?;
    }
    Ok(target)
}

/// Validate the complete wiki tree and its in-root repository metadata.
///
/// # Errors
/// Returns [`WikiError::Confinement`] for symbolic links, reparse points,
/// redirected repository metadata, or a working tree outside `root`.
pub fn validate_wiki_tree(root: &Path) -> WikiResult<()> {
    let git_dir = inspect_git_directory(root)?;
    inspect_tree_except(root, &[git_dir.as_path()])
}

pub(crate) fn inspect_tree(root: &Path) -> WikiResult<()> {
    inspect_tree_except(root, &[])
}

pub(crate) fn inspect_git_directory(root: &Path) -> WikiResult<PathBuf> {
    inspect_root(root)?;
    let git_dir = root.join(".git");
    let metadata = std::fs::symlink_metadata(&git_dir).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            WikiError::Confinement {
                path: git_dir.clone(),
                reason: "wiki repository metadata directory is missing",
            }
        } else {
            error.into()
        }
    })?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(WikiError::Confinement {
            path: git_dir,
            reason: "wiki repository metadata must be an in-root ordinary directory",
        });
    }
    for relative in [
        "commondir",
        "objects/info/alternates",
        "objects/info/http-alternates",
    ] {
        let redirect = git_dir.join(relative);
        match std::fs::symlink_metadata(&redirect) {
            Ok(_) => {
                return Err(WikiError::Confinement {
                    path: redirect,
                    reason: "wiki repository metadata must not redirect outside its in-root .git directory",
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    inspect_tree(&git_dir)?;
    inspect_configured_worktree(root, &git_dir)?;
    Ok(git_dir)
}

fn inspect_configured_worktree(root: &Path, git_dir: &Path) -> WikiResult<()> {
    for config_path in [git_dir.join("config"), git_dir.join("config.worktree")] {
        if !config_path.is_file() {
            continue;
        }
        let config = git2::Config::open(&config_path).map_err(|error| {
            WikiError::Io(std::io::Error::other(format!(
                "could not inspect wiki repository config {}: {error}",
                config_path.display()
            )))
        })?;
        let Ok(configured) = config.get_string("core.worktree") else {
            continue;
        };
        let configured = PathBuf::from(configured);
        let worktree = if configured.is_absolute() {
            configured
        } else {
            git_dir.join(configured)
        };
        if worktree.canonicalize()? != root.canonicalize()? {
            return Err(WikiError::Confinement {
                path: config_path,
                reason: "wiki repository working directory escapes the wiki root",
            });
        }
    }
    Ok(())
}

pub(crate) fn inspect_tree_if_present(root: &Path) -> WikiResult<()> {
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => inspect_tree(root),
    }
}

pub(crate) fn inspect_tree_except(root: &Path, excluded_roots: &[&Path]) -> WikiResult<()> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => require_directory(root, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if is_link_like(&metadata) {
                return Err(confined(&path));
            }
            if excluded_roots.iter().any(|excluded| path == *excluded) {
                continue;
            }
            if metadata.is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(())
}

/// Whether metadata identifies a symbolic link or Windows reparse point.
#[must_use]
pub(crate) fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn inspect_root(root: &Path) -> WikiResult<()> {
    let metadata = std::fs::symlink_metadata(root)?;
    require_directory(root, &metadata)
}

fn inspect_final(path: &Path) -> WikiResult<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if is_link_like(&metadata) => Err(confined(path)),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn inspect_created_directory(path: &Path) -> WikiResult<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    require_directory(path, &metadata)
}

fn require_directory(path: &Path, metadata: &std::fs::Metadata) -> WikiResult<()> {
    if is_link_like(metadata) {
        return Err(confined(path));
    }
    if !metadata.is_dir() {
        return Err(WikiError::Io(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("wiki path component is not a directory: {}", path.display()),
        )));
    }
    Ok(())
}

fn validate_relative(relative: &Path) -> WikiResult<()> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(WikiError::Confinement {
            path: relative.to_path_buf(),
            reason: "path is not a normalized relative wiki path",
        });
    }
    Ok(())
}

fn confined(path: &Path) -> WikiError {
    WikiError::Confinement {
        path: path.to_path_buf(),
        reason: "symbolic links and reparse points are not allowed in the wiki project tree",
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn link_metadata_helper_accepts_regular_entries() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        assert!(!is_link_like(&std::fs::symlink_metadata(file).unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn link_metadata_helper_rejects_dangling_links() {
        let temp = tempfile::tempdir().unwrap();
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(temp.path().join("missing"), &link).unwrap();
        assert!(is_link_like(&std::fs::symlink_metadata(link).unwrap()));
    }
}
