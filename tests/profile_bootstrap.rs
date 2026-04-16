use confluence_cli::run_from;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
fn config_init_uses_explicit_config_path() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("custom-config.json");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "config",
        "init",
        "--name",
        "work",
        "--domain",
        "example.atlassian.net",
    ])
    .expect("config init should succeed");

    let raw = fs::read_to_string(&config_path).expect("custom config path should exist");
    let json: Value = serde_json::from_str(&raw).expect("config json should parse");
    assert_eq!(json["active_profile"], "work");
    assert_eq!(json["profiles"]["work"]["domain"], "example.atlassian.net");
}

#[test]
fn profile_add_preserves_existing_profile_id() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "config",
        "init",
        "--name",
        "work",
        "--domain",
        "example.atlassian.net",
    ])
    .expect("config init should succeed");

    let before: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should exist after init"),
    )
    .expect("config json should parse");
    let id_before = before["profiles"]["work"]["id"]
        .as_str()
        .expect("profile id should exist")
        .to_owned();

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "profile",
        "add",
        "work",
        "--domain",
        "staging.example.atlassian.net",
    ])
    .expect("profile add should update existing profile");

    let after: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should still exist after update"),
    )
    .expect("config json should parse");
    let id_after = after["profiles"]["work"]["id"]
        .as_str()
        .expect("profile id should still exist")
        .to_owned();

    assert_eq!(id_before, id_after);
    assert_eq!(
        after["profiles"]["work"]["domain"],
        "staging.example.atlassian.net"
    );
}

#[test]
fn profile_add_preserves_secret_backend_and_auth_type_without_new_secrets() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "config",
        "init",
        "--name",
        "work",
        "--domain",
        "example.atlassian.net",
        "--auth-type",
        "bearer",
        "--api-token",
        "token-123",
    ])
    .expect("config init should succeed");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "profile",
        "add",
        "work",
        "--domain",
        "staging.example.atlassian.net",
    ])
    .expect("profile add should preserve auth linkage");

    let after: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should still exist after update"),
    )
    .expect("config json should parse");

    assert_eq!(after["profiles"]["work"]["secret_backend"], "keyring");
    assert_eq!(after["profiles"]["work"]["auth_type"], "bearer");
    assert_eq!(
        after["profiles"]["work"]["domain"],
        "staging.example.atlassian.net"
    );
}

#[test]
fn profile_add_preserves_read_only_when_flag_is_omitted() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "config",
        "init",
        "--name",
        "work",
        "--domain",
        "example.atlassian.net",
        "--read-only",
        "true",
    ])
    .expect("config init should succeed");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "profile",
        "add",
        "work",
        "--domain",
        "staging.example.atlassian.net",
    ])
    .expect("profile add should preserve read_only");

    let after: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should still exist after update"),
    )
    .expect("config json should parse");

    assert_eq!(after["profiles"]["work"]["read_only"], true);
}

#[test]
fn profile_add_preserves_legacy_plaintext_secret_when_no_new_secret_is_provided() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    fs::write(
        &config_path,
        r#"{
          "active_profile": "work",
          "profiles": {
            "work": {
              "id": "profile-1",
              "domain": "example.atlassian.net",
              "auth_type": "bearer",
              "api_token": "legacy-token"
            }
          }
        }"#,
    )
    .expect("legacy config should be written");

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "profile",
        "add",
        "work",
        "--domain",
        "staging.example.atlassian.net",
    ])
    .expect("profile add should preserve legacy plaintext secret");

    let after: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should still exist after update"),
    )
    .expect("config json should parse");

    assert_eq!(after["profiles"]["work"]["api_token"], "legacy-token");
    assert_eq!(after["profiles"]["work"]["auth_type"], "bearer");
}
