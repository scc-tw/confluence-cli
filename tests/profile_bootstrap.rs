use confluence_cli::test_support::CliHarness;
use confluence_cli::{OutputFormat, run_from};
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

#[test]
fn profile_show_uses_selected_profile_precedence_and_hides_secrets_in_json() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    let harness = CliHarness::new();

    let mut work_input =
        std::io::Cursor::new(b"work\nexample.atlassian.net\nbearer\nsecret-token\nno\n".to_vec());
    let mut work_output = Vec::new();
    harness
        .login(
            &config_path,
            &mut work_input,
            &mut work_output,
            OutputFormat::Human,
        )
        .expect("work login should succeed");

    let mut stage_input = std::io::Cursor::new(
        b"stage\nstage.example.internal\nbasic\nstage@example.com\n\nstage-password\nyes\n"
            .to_vec(),
    );
    let mut stage_output = Vec::new();
    harness
        .login(
            &config_path,
            &mut stage_input,
            &mut stage_output,
            OutputFormat::Human,
        )
        .expect("stage login should succeed");

    unsafe { std::env::set_var("CONFLUENCE_PROFILE", "work") };
    let mut show_output = Vec::new();
    harness
        .profile_show(
            &config_path,
            Some("stage"),
            OutputFormat::Json,
            &mut show_output,
        )
        .expect("profile show should succeed");
    unsafe { std::env::remove_var("CONFLUENCE_PROFILE") };

    let rendered = String::from_utf8(show_output).expect("json output should be utf-8");
    let json: Value = serde_json::from_str(&rendered).expect("profile show json should parse");

    assert_eq!(json["name"], "stage");
    assert_eq!(json["account"], "stage@example.com");
    assert_eq!(json["domain"], "stage.example.internal");
    assert_eq!(json["auth"], "basic");
    assert_eq!(json["api_path"], "/rest/api");
    assert_eq!(json["read_only"], true);
    assert!(json.get("api_token").is_none());
    assert!(json.get("password").is_none());
    assert!(!rendered.contains("secret-token"));
    assert!(!rendered.contains("stage-password"));
}

#[test]
fn login_persists_keyring_backed_profile_and_activates_it() {
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    let harness = CliHarness::new();

    let mut input = std::io::Cursor::new(
        b"work\nexample.atlassian.net\nbasic\nuser@example.com\n\nsecret-password\nno\n".to_vec(),
    );
    let mut output = Vec::new();
    harness
        .login(&config_path, &mut input, &mut output, OutputFormat::Human)
        .expect("login should succeed");

    let rendered = String::from_utf8(output).expect("login output should be utf-8");
    let config: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("config should exist after login"),
    )
    .expect("config json should parse");
    let runtime = harness
        .load_runtime_context(&config_path, None)
        .expect("runtime should load after login");
    let resolved = runtime
        .runtime_config
        .resolved_profile
        .expect("resolved profile should exist");

    assert_eq!(config["active_profile"], "work");
    assert_eq!(
        config["profiles"]["work"]["domain"],
        "example.atlassian.net"
    );
    assert_eq!(config["profiles"]["work"]["auth_type"], "basic");
    assert_eq!(config["profiles"]["work"]["secret_backend"], "keyring");
    assert!(config["profiles"]["work"]["password"].is_null());
    assert!(config["profiles"]["work"]["api_token"].is_null());
    assert_eq!(resolved.name.as_deref(), Some("work"));
    assert_eq!(resolved.email.as_deref(), Some("user@example.com"));
    assert!(resolved.password.is_some());
    assert!(rendered.contains("* work"));
    assert!(!rendered.contains("secret-password"));
}
