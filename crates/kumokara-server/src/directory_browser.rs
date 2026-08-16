use anyhow::{bail, Context, Result};
use kumokara_protocol::messages::DirectoryEntry;
use std::path::{Path, PathBuf};

pub struct DirectoryListing {
    pub home: String,
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<DirectoryEntry>,
}

pub async fn list(path: Option<String>, show_hidden: bool) -> Result<DirectoryListing> {
    tokio::task::spawn_blocking(move || list_blocking(path, show_hidden))
        .await
        .context("directory browser task failed")?
}

pub async fn create(parent: String, name: String) -> Result<String> {
    tokio::task::spawn_blocking(move || create_blocking(&parent, &name))
        .await
        .context("directory creation task failed")?
}

pub fn home() -> Result<PathBuf> {
    home_directory()?
        .canonicalize()
        .context("failed to resolve the server home directory")
}

fn list_blocking(path: Option<String>, show_hidden: bool) -> Result<DirectoryListing> {
    let home = home()?;
    let requested = path.map(PathBuf::from).unwrap_or_else(|| home.clone());
    let current = requested
        .canonicalize()
        .with_context(|| format!("directory does not exist: {}", requested.display()))?;
    if !current.is_dir() {
        bail!("path is not a directory: {}", current.display());
    }

    let mut entries = std::fs::read_dir(&current)
        .with_context(|| format!("cannot read directory: {}", current.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !show_hidden && name.starts_with('.') {
                return None;
            }
            if !entry.metadata().ok()?.is_dir() {
                return None;
            }
            let path = entry.path().canonicalize().ok()?;
            Some(DirectoryEntry {
                name,
                path: display_path(&path),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });

    let parent = current
        .parent()
        .filter(|parent| parent.is_dir())
        .map(display_path);
    Ok(DirectoryListing {
        home: display_path(&home),
        path: display_path(&current),
        parent,
        entries,
    })
}

fn create_blocking(parent: &str, name: &str) -> Result<String> {
    let name = name.trim();
    validate_directory_name(name)?;

    let parent = PathBuf::from(parent)
        .canonicalize()
        .with_context(|| format!("parent directory does not exist: {parent}"))?;
    if !parent.is_dir() {
        bail!("parent path is not a directory: {}", parent.display());
    }

    let target = parent.join(name);
    std::fs::create_dir(&target)
        .with_context(|| format!("cannot create directory: {}", target.display()))?;
    let target = target
        .canonicalize()
        .with_context(|| format!("cannot resolve new directory: {}", target.display()))?;
    Ok(display_path(&target))
}

fn validate_directory_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("folder name cannot be empty");
    }
    if name == "." || name == ".." {
        bail!("folder name cannot be . or ..");
    }
    if name.chars().any(|character| {
        character == '/' || character == '\\' || character == '\0' || character.is_control()
    }) {
        bail!("folder name cannot contain path separators or control characters");
    }
    Ok(())
}

fn home_directory() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
    {
        return Ok(path);
    }
    Ok(std::env::current_dir()?)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_contains_only_visible_directories_by_default() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("zeta")).unwrap();
        std::fs::create_dir(temp.path().join("Alpha")).unwrap();
        std::fs::create_dir(temp.path().join(".hidden")).unwrap();
        std::fs::write(temp.path().join("notes.txt"), "not a directory").unwrap();

        let listing = list_blocking(Some(display_path(temp.path())), false).unwrap();
        let names = listing
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Alpha", "zeta"]);

        let listing = list_blocking(Some(display_path(temp.path())), true).unwrap();
        assert!(listing.entries.iter().any(|entry| entry.name == ".hidden"));
    }

    #[test]
    fn creating_a_directory_rejects_traversal_and_returns_canonical_path() {
        let temp = tempfile::tempdir().unwrap();
        let parent = display_path(temp.path());

        assert!(create_blocking(&parent, "../escape").is_err());
        assert!(create_blocking(&parent, "nested/path").is_err());

        let created = create_blocking(&parent, "project").unwrap();
        assert_eq!(
            created,
            display_path(&temp.path().join("project").canonicalize().unwrap())
        );
        assert!(temp.path().join("project").is_dir());
    }
}
