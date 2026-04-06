use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::VectorClock;

use crate::error::YokeError;

/// A named point in an idea's history.
///
/// Where Ideas' CRDT tracks individual operations (insert digit, update field),
/// a VersionTag marks a meaningful moment: "v2.0", "approved-final", "pre-rebrand".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTag {
    pub id: Uuid,
    pub idea_id: Uuid,
    pub name: String,
    pub message: Option<String>,
    pub snapshot_clock: VectorClock,
    pub branch: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

impl VersionTag {
    pub fn new(
        idea_id: Uuid,
        name: impl Into<String>,
        snapshot_clock: VectorClock,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            idea_id,
            name: name.into(),
            message: None,
            snapshot_clock,
            branch: "main".into(),
            author: author.into(),
            created_at: Utc::now(),
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn on_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = branch.into();
        self
    }
}

/// A branch in an idea's version timeline.
///
/// Branches let designers explore alternatives without losing the main line.
/// A branch can be merged back or abandoned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub created_from: Uuid,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub merged_into: Option<MergeRecord>,
}

/// Record of a branch merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRecord {
    pub target_branch: String,
    pub merge_version: Uuid,
    pub author: String,
    pub merged_at: DateTime<Utc>,
    pub message: Option<String>,
}

/// Tracks the complete version history of an idea.
///
/// Built on top of Ideas' CRDT infrastructure. Where X's OperationLog tracks
/// individual operations, VersionChain tracks named snapshots, branches, and
/// merges — the meaningful history a person navigates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChain {
    pub idea_id: Uuid,
    pub versions: Vec<VersionTag>,
    pub branches: Vec<Branch>,
}

impl VersionChain {
    pub fn new(idea_id: Uuid) -> Self {
        Self {
            idea_id,
            versions: Vec::new(),
            branches: Vec::new(),
        }
    }

    /// Add a version tag to the chain.
    pub fn tag_version(&mut self, tag: VersionTag) -> Result<(), YokeError> {
        if tag.idea_id != self.idea_id {
            return Err(YokeError::Validation(format!(
                "version idea_id {} doesn't match chain idea_id {}",
                tag.idea_id, self.idea_id
            )));
        }
        if self
            .versions
            .iter()
            .any(|v| v.name == tag.name && v.branch == tag.branch)
        {
            return Err(YokeError::Validation(format!(
                "version '{}' already exists on branch '{}'",
                tag.name, tag.branch
            )));
        }
        // Non-main branches must exist
        if tag.branch != "main" && !self.branches.iter().any(|b| b.name == tag.branch) {
            return Err(YokeError::BranchNotFound(tag.branch.clone()));
        }
        self.versions.push(tag);
        Ok(())
    }

    /// Create a new branch from a version.
    pub fn create_branch(
        &mut self,
        name: impl Into<String>,
        from_version: Uuid,
        author: impl Into<String>,
    ) -> Result<&Branch, YokeError> {
        let name = name.into();
        if name == "main" {
            return Err(YokeError::Validation("cannot create branch named 'main'".into()));
        }
        if self.branches.iter().any(|b| b.name == name) {
            return Err(YokeError::DuplicateBranch(name));
        }
        if !self.versions.iter().any(|v| v.id == from_version) {
            return Err(YokeError::VersionNotFound(from_version.to_string()));
        }
        self.branches.push(Branch {
            name,
            created_from: from_version,
            author: author.into(),
            created_at: Utc::now(),
            merged_into: None,
        });
        Ok(self.branches.last().expect("branch was just pushed"))
    }

    /// Merge a branch back.
    pub fn merge_branch(
        &mut self,
        source_branch: &str,
        target_branch: &str,
        merge_version: Uuid,
        author: impl Into<String>,
    ) -> Result<(), YokeError> {
        let branch = self
            .branches
            .iter_mut()
            .find(|b| b.name == source_branch)
            .ok_or_else(|| YokeError::BranchNotFound(source_branch.into()))?;
        if branch.merged_into.is_some() {
            return Err(YokeError::BranchAlreadyMerged(source_branch.into()));
        }
        branch.merged_into = Some(MergeRecord {
            target_branch: target_branch.into(),
            merge_version,
            author: author.into(),
            merged_at: Utc::now(),
            message: None,
        });
        Ok(())
    }

    /// Get all versions on a specific branch, chronologically.
    pub fn versions_on_branch(&self, branch: &str) -> Vec<&VersionTag> {
        let mut versions: Vec<&VersionTag> =
            self.versions.iter().filter(|v| v.branch == branch).collect();
        versions.sort_by_key(|v| v.created_at);
        versions
    }

    /// Get the latest version on a branch.
    pub fn latest_version(&self, branch: &str) -> Option<&VersionTag> {
        self.versions_on_branch(branch).into_iter().last()
    }

    /// Get a version by name (first match across all branches).
    pub fn version_by_name(&self, name: &str) -> Option<&VersionTag> {
        self.versions.iter().find(|v| v.name == name)
    }

    /// Get a version by ID.
    pub fn version_by_id(&self, id: Uuid) -> Option<&VersionTag> {
        self.versions.iter().find(|v| v.id == id)
    }

    /// List all branch names (including "main").
    pub fn branch_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = vec!["main"];
        names.extend(self.branches.iter().map(|b| b.name.as_str()));
        names
    }

    /// Check if a branch has been merged.
    pub fn is_branch_merged(&self, name: &str) -> bool {
        self.branches
            .iter()
            .any(|b| b.name == name && b.merged_into.is_some())
    }

    /// Get a branch by name.
    pub fn branch(&self, name: &str) -> Option<&Branch> {
        self.branches.iter().find(|b| b.name == name)
    }

    pub fn version_count(&self) -> usize {
        self.versions.len()
    }

    pub fn branch_count(&self) -> usize {
        self.branches.len() + 1 // +1 for main
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(author: &str, count: u64) -> VectorClock {
        let mut vc = VectorClock::new();
        for _ in 0..count {
            vc.increment(author);
        }
        vc
    }

    #[test]
    fn new_chain() {
        let id = Uuid::new_v4();
        let chain = VersionChain::new(id);
        assert_eq!(chain.idea_id, id);
        assert_eq!(chain.version_count(), 0);
        assert_eq!(chain.branch_count(), 1); // main
        assert_eq!(chain.branch_names(), vec!["main"]);
    }

    #[test]
    fn tag_version() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let tag = VersionTag::new(id, "v1.0", clock("alice", 5), "cpub1alice")
            .with_message("initial release");
        chain.tag_version(tag).unwrap();

        assert_eq!(chain.version_count(), 1);
        let v = chain.version_by_name("v1.0").unwrap();
        assert_eq!(v.message.as_deref(), Some("initial release"));
        assert_eq!(v.branch, "main");
    }

    #[test]
    fn reject_duplicate_version_name() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        chain
            .tag_version(VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice"))
            .unwrap();
        let result =
            chain.tag_version(VersionTag::new(id, "v1.0", clock("alice", 2), "cpub1alice"));
        assert!(result.is_err());
    }

    #[test]
    fn reject_wrong_idea_id() {
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let result =
            chain.tag_version(VersionTag::new(other, "v1.0", clock("alice", 1), "cpub1alice"));
        assert!(result.is_err());
    }

    #[test]
    fn same_name_different_branches() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        chain.create_branch("experimental", v1_id, "cpub1alice").unwrap();

        let v_exp = VersionTag::new(id, "v1.0", clock("alice", 2), "cpub1alice")
            .on_branch("experimental");
        chain.tag_version(v_exp).unwrap();

        assert_eq!(chain.version_count(), 2);
    }

    #[test]
    fn create_and_list_branches() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        chain.create_branch("dark-mode", v1_id, "cpub1bob").unwrap();
        chain.create_branch("rebrand", v1_id, "cpub1carol").unwrap();

        let names = chain.branch_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"main"));
        assert!(names.contains(&"dark-mode"));
        assert!(names.contains(&"rebrand"));
        assert_eq!(chain.branch_count(), 3);
    }

    #[test]
    fn reject_duplicate_branch() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        chain.create_branch("exp", v1_id, "cpub1alice").unwrap();
        let result = chain.create_branch("exp", v1_id, "cpub1alice");
        assert!(result.is_err());
    }

    #[test]
    fn reject_branch_named_main() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        let result = chain.create_branch("main", v1_id, "cpub1alice");
        assert!(result.is_err());
    }

    #[test]
    fn reject_branch_from_nonexistent_version() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);
        let result = chain.create_branch("exp", Uuid::new_v4(), "cpub1alice");
        assert!(result.is_err());
    }

    #[test]
    fn reject_version_on_nonexistent_branch() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);
        let tag = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice")
            .on_branch("nonexistent");
        let result = chain.tag_version(tag);
        assert!(result.is_err());
    }

    #[test]
    fn merge_branch() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        chain.create_branch("exp", v1_id, "cpub1bob").unwrap();
        assert!(!chain.is_branch_merged("exp"));

        let merge_v = Uuid::new_v4();
        chain.merge_branch("exp", "main", merge_v, "cpub1bob").unwrap();
        assert!(chain.is_branch_merged("exp"));
    }

    #[test]
    fn reject_double_merge() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        chain.create_branch("exp", v1_id, "cpub1bob").unwrap();
        chain
            .merge_branch("exp", "main", Uuid::new_v4(), "cpub1bob")
            .unwrap();

        let result = chain.merge_branch("exp", "main", Uuid::new_v4(), "cpub1bob");
        assert!(result.is_err());
    }

    #[test]
    fn versions_on_branch_sorted() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        // Tag in reverse order
        let mut v2 = VersionTag::new(id, "v2.0", clock("alice", 2), "cpub1alice");
        v2.created_at = Utc::now() + chrono::Duration::seconds(10);
        chain.tag_version(v2).unwrap();

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        chain.tag_version(v1).unwrap();

        let versions = chain.versions_on_branch("main");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].name, "v1.0");
        assert_eq!(versions[1].name, "v2.0");
    }

    #[test]
    fn latest_version() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        chain
            .tag_version(VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice"))
            .unwrap();
        let mut v2 = VersionTag::new(id, "v2.0", clock("alice", 2), "cpub1alice");
        v2.created_at = Utc::now() + chrono::Duration::seconds(1);
        chain.tag_version(v2).unwrap();

        let latest = chain.latest_version("main").unwrap();
        assert_eq!(latest.name, "v2.0");
    }

    #[test]
    fn version_by_id() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let tag = VersionTag::new(id, "v1.0", clock("alice", 1), "cpub1alice");
        let tag_id = tag.id;
        chain.tag_version(tag).unwrap();

        assert!(chain.version_by_id(tag_id).is_some());
        assert!(chain.version_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn serde_round_trip() {
        let id = Uuid::new_v4();
        let mut chain = VersionChain::new(id);

        let v1 = VersionTag::new(id, "v1.0", clock("alice", 3), "cpub1alice")
            .with_message("first release");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();
        chain.create_branch("exp", v1_id, "cpub1bob").unwrap();

        let json = serde_json::to_string(&chain).unwrap();
        let restored: VersionChain = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.idea_id, id);
        assert_eq!(restored.version_count(), 1);
        assert_eq!(restored.branch_count(), 2);
    }
}
