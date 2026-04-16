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
