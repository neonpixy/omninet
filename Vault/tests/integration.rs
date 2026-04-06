use uuid::Uuid;
use chrono::Utc;
use vault::{
    Vault, VaultError, ManifestEntry, IdeaFilter,
    CollectiveRole,
};
use sentinal::SecureData;

const PASSWORD: &str = "integration-test-password-456";

fn temp_vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::new();
    (dir, vault)
}

fn make_entry(path: &str, creator: &str) -> ManifestEntry {
    ManifestEntry {
        id: Uuid::new_v4(),
        path: path.to_string(),
        title: Some("Test Idea".to_string()),
        extended_type: Some("text".to_string()),
        creator: creator.to_string(),
        created_at: Utc::now(),
        modified_at: Utc::now(),
        collective_id: None,
        header_cache: None,
    }
}

#[test]
fn full_vault_lifecycle() {
    let (dir, mut vault) = temp_vault();
    let root = dir.path().to_path_buf();

    // Unlock.
    vault.unlock(PASSWORD, root.clone()).unwrap();
    assert!(vault.is_unlocked());

    // Register an idea.
    let entry = make_entry("Personal/song.idea", "cpub1abc");
    let id = entry.id;
    vault.register_idea(entry).unwrap();

    // Query by ID.
    let found = vault.get_idea(&id).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().path, "Personal/song.idea");

    // Query by path.
    let found = vault.get_idea_by_path("Personal/song.idea").unwrap();
    assert!(found.is_some());

    // Lock.
    vault.lock().unwrap();
    assert!(!vault.is_unlocked());

    // All operations fail when locked.
    assert!(matches!(vault.get_idea(&id), Err(VaultError::Locked)));

    // Re-unlock with same password.
    vault.unlock(PASSWORD, root).unwrap();

    // Data persisted in SQLCipher.
    let found = vault.get_idea(&id).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().path, "Personal/song.idea");

    vault.lock().unwrap();
}

#[test]
fn wrong_password_after_relock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create vault with password A.
    {
        let mut vault = Vault::new();
        vault.unlock("password-A", root.clone()).unwrap();
        vault.register_idea(make_entry("test.idea", "cpub1")).unwrap();
        vault.lock().unwrap();
    }

    // Try to reopen with password B.
    {
        let mut vault = Vault::new();
        let result = vault.unlock("password-B", root);
        assert!(result.is_err());
        // Vault should remain locked after failed unlock.
        assert!(!vault.is_unlocked());
    }
}

#[test]
fn encrypt_decrypt_for_idea() {
    let (dir, mut vault) = temp_vault();
    vault.unlock(PASSWORD, dir.path().to_path_buf()).unwrap();

    let idea_id = Uuid::new_v4();
    let plaintext = b"This is sovereign data, protected by the Covenant.";

    let encrypted = vault.encrypt_for_idea(plaintext, &idea_id).unwrap();
    assert_ne!(&encrypted, plaintext);

    let decrypted = vault.decrypt_for_idea(&encrypted, &idea_id).unwrap();
    assert_eq!(decrypted, plaintext);

    vault.lock().unwrap();
}

#[test]
fn manifest_crud_operations() {
    let (dir, mut vault) = temp_vault();
    vault.unlock(PASSWORD, dir.path().to_path_buf()).unwrap();

    // Insert.
    let entry = make_entry("Personal/a.idea", "cpub1abc");
    let id = entry.id;
    vault.register_idea(entry).unwrap();
    assert_eq!(vault.idea_count().unwrap(), 1);

    // Update (same ID, different title).
    let mut updated = vault.get_idea(&id).unwrap().unwrap().clone();
    updated.title = Some("Updated Title".to_string());
    vault.register_idea(updated).unwrap();
    assert_eq!(vault.get_idea(&id).unwrap().unwrap().title.as_deref(), Some("Updated Title"));

    // Remove.
    vault.unregister_idea(&id).unwrap();
    assert_eq!(vault.idea_count().unwrap(), 0);

    vault.lock().unwrap();
}

#[test]
fn manifest_filter_queries() {
    let (dir, mut vault) = temp_vault();
    vault.unlock(PASSWORD, dir.path().to_path_buf()).unwrap();

    vault.register_idea(make_entry("Personal/a.idea", "cpub1abc")).unwrap();
    vault.register_idea(make_entry("Personal/b.idea", "cpub1xyz")).unwrap();

    let mut music = make_entry("Collectives/shared/c.idea", "cpub1abc");
    music.extended_type = Some("music".to_string());
    vault.register_idea(music).unwrap();

    // Filter by creator.
    let filter = IdeaFilter::new().creator("cpub1abc");
    let results = vault.list_ideas(&filter).unwrap();
    assert_eq!(results.len(), 2);

    // Filter by type.
    let filter = IdeaFilter::new().extended_type("music");
    let results = vault.list_ideas(&filter).unwrap();
    assert_eq!(results.len(), 1);

    // List in folder.
    let personal = vault.list_ideas_in_folder("Personal").unwrap();
    assert_eq!(personal.len(), 2);

    vault.lock().unwrap();
}

#[test]
fn module_state_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // First session: save state.
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root.clone()).unwrap();
        vault.save_module_state("crown", "profile", r#"{"name":"Sam"}"#).unwrap();
        vault.save_module_state("crown", "settings", r#"{"theme":"dark"}"#).unwrap();
        vault.lock().unwrap();
    }

    // Second session: verify persistence.
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root).unwrap();

        let profile = vault.load_module_state("crown", "profile").unwrap();
        assert_eq!(profile.as_deref(), Some(r#"{"name":"Sam"}"#));

        let keys = vault.list_module_state_keys("crown").unwrap();
        assert_eq!(keys, vec!["profile", "settings"]);

        // Delete.
        vault.delete_module_state("crown", "settings").unwrap();
        let keys = vault.list_module_state_keys("crown").unwrap();
        assert_eq!(keys, vec!["profile"]);

        vault.lock().unwrap();
    }
}

#[test]
fn collective_lifecycle() {
    let (dir, mut vault) = temp_vault();
    vault.unlock(PASSWORD, dir.path().to_path_buf()).unwrap();

    // Create collective.
    let coll = vault.create_collective("Test Band".to_string(), "cpub1owner".to_string()).unwrap();
    let coll_id = coll.id;
    assert_eq!(coll.name, "Test Band");

    // Verify key exists.
    let key = vault.collective_key(&coll_id).unwrap();
    assert_eq!(key.expose().len(), 32);

    // List collectives.
    let list = vault.list_collectives().unwrap();
    assert_eq!(list.len(), 1);

    // Leave collective.
    vault.leave_collective(&coll_id).unwrap();
    let list = vault.list_collectives().unwrap();
    assert_eq!(list.len(), 0);

    vault.lock().unwrap();
}

#[test]
fn collective_persists_across_lock_cycles() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let coll_id;

    // First session: create collective.
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root.clone()).unwrap();
        let coll = vault.create_collective("Persistent Group".to_string(), "cpub1owner".to_string()).unwrap();
        coll_id = coll.id;
        vault.lock().unwrap();
    }

    // Second session: collective should be restored.
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root).unwrap();

        let list = vault.list_collectives().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, coll_id);
        assert_eq!(list[0].name, "Persistent Group");

        vault.lock().unwrap();
    }
}

#[test]
fn vocabulary_seed_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let seed1;
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root.clone()).unwrap();
        seed1 = vault.vocabulary_seed().unwrap().expose().to_vec();
        vault.lock().unwrap();
    }

    let seed2;
    {
        let mut vault = Vault::new();
        vault.unlock(PASSWORD, root).unwrap();
        seed2 = vault.vocabulary_seed().unwrap().expose().to_vec();
        vault.lock().unwrap();
    }

    assert_eq!(seed1, seed2);
    assert_eq!(seed1.len(), 32);
}

#[test]
fn join_collective_with_external_key() {
    let (dir, mut vault) = temp_vault();
    vault.unlock(PASSWORD, dir.path().to_path_buf()).unwrap();

    let coll_id = Uuid::new_v4();
    let key = SecureData::random(32).unwrap();

    vault.join_collective(coll_id, "External Group".to_string(), key, CollectiveRole::Member).unwrap();

    let list = vault.list_collectives().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].our_role, CollectiveRole::Member);

    vault.lock().unwrap();
}
