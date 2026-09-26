use leanai_desktop_lib::keychain::KeychainStore;

#[test]
fn keychain_store_roundtrip_and_delete() {
    let store = KeychainStore::mock();

    // 1. Initially not configured
    assert!(!store.is_configured("openai").unwrap());
    assert_eq!(store.read("openai").unwrap(), None);

    // 2. Store credential
    store
        .store("openai", "api_key", "sk-test-secret-key-12345")
        .unwrap();
    assert!(store.is_configured("openai").unwrap());
    assert_eq!(
        store.read("openai").unwrap().as_deref(),
        Some("sk-test-secret-key-12345")
    );

    // 3. Update credential
    store
        .store("openai", "api_key", "sk-new-secret-key-67890")
        .unwrap();
    assert_eq!(
        store.read("openai").unwrap().as_deref(),
        Some("sk-new-secret-key-67890")
    );

    // 4. Delete credential (ADR 0007 item 4: disconnecting an account deletes keychain entry)
    let deleted = store.delete("openai").unwrap();
    assert!(deleted);
    assert!(!store.is_configured("openai").unwrap());
    assert_eq!(store.read("openai").unwrap(), None);

    // 5. Deleting nonexistent returns false
    let deleted_again = store.delete("openai").unwrap();
    assert!(!deleted_again);
}

#[test]
fn isolated_provider_namespaces() {
    let store = KeychainStore::mock();

    store.store("openai", "key1", "secret_openai").unwrap();
    store
        .store("anthropic", "key2", "secret_anthropic")
        .unwrap();

    assert_eq!(
        store.read("openai").unwrap().as_deref(),
        Some("secret_openai")
    );
    assert_eq!(
        store.read("anthropic").unwrap().as_deref(),
        Some("secret_anthropic")
    );

    store.delete("openai").unwrap();
    assert!(!store.is_configured("openai").unwrap());
    assert!(store.is_configured("anthropic").unwrap());
}

#[test]
fn github_token_keychain_storage() {
    let store = KeychainStore::mock();

    assert!(!store.is_configured("github").unwrap());

    store
        .store(
            "github",
            "octocat",
            "ghp_1234567890abcdefghijklmnopqrstuvwxyz",
        )
        .unwrap();
    assert!(store.is_configured("github").unwrap());
    assert_eq!(
        store.read("github").unwrap().as_deref(),
        Some("ghp_1234567890abcdefghijklmnopqrstuvwxyz")
    );

    let deleted = store.delete("github").unwrap();
    assert!(deleted);
    assert!(!store.is_configured("github").unwrap());
}
