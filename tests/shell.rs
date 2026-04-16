use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn write_minimal_config(path: &Path) {
    fs::write(
        path,
        r#"{
  "active_profile": "work",
  "profiles": {
    "work": {
      "id": "profile-1"
    }
  }
}"#,
    )
    .expect("config should be written");
}

fn configure_command(command: &mut Command, config_path: &Path, server: &MockServer) {
    command
        .arg("--config-path")
        .arg(config_path)
        .env("CONFLUENCE_DOMAIN", server.address().to_string())
        .env("CONFLUENCE_PROTOCOL", "http")
        .env("CONFLUENCE_API_PATH", "/wiki/rest/api")
        .env("CONFLUENCE_AUTH_TYPE", "bearer")
        .env("CONFLUENCE_API_TOKEN", "token-123");
}

#[test]
fn one_liner_page_read_resolves_space_overview_url() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    write_minimal_config(&config_path);

    let space = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/spaces")
            .query_param("keys", "~7120202d4ccbf388e240f58d10c28a0d13083e")
            .query_param("limit", "1")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "space-1",
                "key": "~7120202d4ccbf388e240f58d10c28a0d13083e",
                "name": "Oscar Yang",
                "homepageId": "855670887"
            }]
        }));
    });

    let page = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/855670887")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "855670887",
            "status": "current",
            "title": "Space Home",
            "spaceId": "space-1",
            "version": { "number": 3 },
            "body": {
                "storage": {
                    "value": "<p>Overview</p>",
                    "representation": "storage"
                }
            }
        }));
    });

    let mut command = Command::new(env!("CARGO_BIN_EXE_confluence"));
    configure_command(&mut command, &config_path, &server);
    let output = command
        .arg("page")
        .arg("read")
        .arg(format!(
            "http://{}/wiki/spaces/~7120202d4ccbf388e240f58d10c28a0d13083e/overview",
            server.address()
        ))
        .output()
        .expect("command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("<p>Overview</p>"));
    space.assert();
    page.assert();
}

#[test]
fn shell_uses_page_context_for_page_info() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    write_minimal_config(&config_path);

    let page = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/855670887")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "855670887",
            "status": "current",
            "title": "ICU in Windows",
            "spaceId": "space-1",
            "version": { "number": 7 }
        }));
    });

    let mut command = Command::new(env!("CARGO_BIN_EXE_confluence"));
    configure_command(&mut command, &config_path, &server);
    let mut child = command
        .arg("shell")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("shell should start");

    child
        .stdin
        .as_mut()
        .expect("stdin should exist")
        .write_all(b"use page 855670887\npage info\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("page: 855670887"));
    assert!(stdout.contains("ICU in Windows [855670887]"));
    page.assert();
}

#[test]
fn shell_uses_space_context_for_page_create() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    write_minimal_config(&config_path);

    let spaces = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/spaces")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "100",
                "key": "ENG",
                "name": "Engineering"
            }]
        }));
    });

    let create = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v2/pages")
            .header("authorization", "Bearer token-123")
            .header("content-type", "application/json")
            .body_contains("\"spaceId\":\"100\"")
            .body_contains("\"title\":\"Draft\"")
            .body_contains("\"value\":\"<h1>Hello</h1>\"");
        then.status(200).json_body(json!({
            "id": "200",
            "status": "current",
            "title": "Draft",
            "spaceId": "100",
            "version": { "number": 1 }
        }));
    });

    let mut command = Command::new(env!("CARGO_BIN_EXE_confluence"));
    configure_command(&mut command, &config_path, &server);
    let mut child = command
        .arg("shell")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("shell should start");

    child
        .stdin
        .as_mut()
        .expect("stdin should exist")
        .write_all(b"use space-key ENG\npage create --title Draft --body '# Hello'\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("space-key: ENG"));
    assert!(stdout.contains("Created Draft [200]"));
    spaces.assert();
    create.assert();
}

#[test]
fn root_help_advertises_shell_and_drill_down() {
    let output = Command::new(env!("CARGO_BIN_EXE_confluence"))
        .arg("--help")
        .output()
        .expect("help command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("confluence shell"));
    assert!(stdout.contains("confluence page --help"));
    assert!(stdout.contains("confluence shell --help"));
}

#[test]
fn page_help_shows_common_one_liners() {
    let output = Command::new(env!("CARGO_BIN_EXE_confluence"))
        .arg("page")
        .arg("--help")
        .output()
        .expect("page help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("Common page flows:"));
    assert!(stdout.contains("confluence page search \"release notes\""));
    assert!(stdout.contains("confluence page read https://your-site.atlassian.net/wiki/spaces/ENG/pages/12345/Page+Title"));
    assert!(stdout.contains("confluence page create --space-key ENG"));
}

#[test]
fn shell_help_shows_context_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_confluence"))
        .arg("shell")
        .arg("--help")
        .output()
        .expect("shell help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("use page 12345"));
    assert!(stdout.contains("use space-key ENG"));
    assert!(stdout.contains("help page"));
}
