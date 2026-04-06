use std::path::{Path, PathBuf};

use crate::error::VaultError;

/// Lock/unlock state and path resolution for the vault.
///
/// When locked: root_path is None, all path operations return `VaultError::Locked`.
/// When unlocked: root_path points to the vault root directory.
pub struct VaultState {
    root_path: Option<PathBuf>,
}

impl VaultState {
    /// Create a new locked vault state with no root path.
    pub fn new() -> Self {
        Self { root_path: None }
    }

    /// Returns true if the vault is currently unlocked and a root path is set.
    pub fn is_unlocked(&self) -> bool {
        self.root_path.is_some()
    }

    /// Mark vault as unlocked with the given root path.
    pub fn unlock(&mut self, root_path: PathBuf) {
        self.root_path = Some(root_path);
    }

    /// Mark vault as locked. Clears the root path.
    pub fn lock(&mut self) {
        self.root_path = None;
    }

    /// Get the vault root path (errors if locked).
    pub fn root_path(&self) -> Result<&Path, VaultError> {
        self.root_path.as_deref().ok_or(VaultError::Locked)
    }

    /// Path to .vault/ metadata directory.
    pub fn vault_dir(&self) -> Result<PathBuf, VaultError> {
        Ok(self.root_path()?.join(".vault"))
    }

    /// Path to config.json.
    pub fn config_path(&self) -> Result<PathBuf, VaultError> {
        Ok(self.vault_dir()?.join("config.json"))
    }

    /// Path to manifest.db (encrypted SQLite).
    pub fn manifest_path(&self) -> Result<PathBuf, VaultError> {
        Ok(self.vault_dir()?.join("manifest.db"))
    }

    /// Personal ideas directory: {root}/Personal/
    pub fn personal_path(&self) -> Result<PathBuf, VaultError> {
        Ok(self.root_path()?.join("Personal"))
    }

    /// Collectives directory: {root}/Collectives/
    pub fn collectives_path(&self) -> Result<PathBuf, VaultError> {
        Ok(self.root_path()?.join("Collectives"))
    }

    /// Resolve a relative path within the vault root.
    pub fn resolve_path(&self, relative: &str) -> Result<PathBuf, VaultError> {
        Ok(self.root_path()?.join(relative))
    }

    /// Get the relative path from vault root for an absolute path.
    /// Returns None if the path is not within the vault.
    pub fn relative_path(&self, absolute: &Path) -> Result<Option<String>, VaultError> {
        let root = self.root_path()?;
        match absolute.strip_prefix(root) {
            Ok(rel) => Ok(Some(rel.to_string_lossy().into_owned())),
            Err(_) => Ok(None),
        }
    }
}

impl Default for VaultState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_locked() {
        let state = VaultState::new();
        assert!(!state.is_unlocked());
        assert!(matches!(state.root_path(), Err(VaultError::Locked)));
    }

    #[test]
    fn unlock_sets_path() {
        let mut state = VaultState::new();
        state.unlock(PathBuf::from("/tmp/vault"));
        assert!(state.is_unlocked());
        assert_eq!(state.root_path().unwrap(), Path::new("/tmp/vault"));
    }

    #[test]
    fn lock_clears_state() {
        let mut state = VaultState::new();
        state.unlock(PathBuf::from("/tmp/vault"));
        state.lock();
        assert!(!state.is_unlocked());
        assert!(matches!(state.root_path(), Err(VaultError::Locked)));
    }

    #[test]
    fn vault_dir_path() {
        let mut state = VaultState::new();
        state.unlock(PathBuf::from("/tmp/vault"));
        assert_eq!(state.vault_dir().unwrap(), PathBuf::from("/tmp/vault/.vault"));
        assert_eq!(state.config_path().unwrap(), PathBuf::from("/tmp/vault/.vault/config.json"));
        assert_eq!(state.manifest_path().unwrap(), PathBuf::from("/tmp/vault/.vault/manifest.db"));
    }

    #[test]
    fn resolve_relative_path() {
        let mut state = VaultState::new();
        state.unlock(PathBuf::from("/tmp/vault"));
        assert_eq!(
            state.resolve_path("Personal/my-idea.idea").unwrap(),
            PathBuf::from("/tmp/vault/Personal/my-idea.idea")
        );
    }

    #[test]
    fn relative_path_extraction() {
        let mut state = VaultState::new();
        state.unlock(PathBuf::from("/tmp/vault"));

        let rel = state
            .relative_path(Path::new("/tmp/vault/Personal/my-idea.idea"))
            .unwrap();
        assert_eq!(rel.as_deref(), Some("Personal/my-idea.idea"));

        // Path outside vault returns None.
        let rel = state
            .relative_path(Path::new("/other/path"))
            .unwrap();
        assert!(rel.is_none());
    }
}
