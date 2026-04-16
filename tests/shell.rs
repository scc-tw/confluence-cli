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
            .query_param("keys", "~user-123")
            .query_param("limit", "1")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "space-1",
                "key": "~user-123",
                "name": "User Alpha",
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
            "http://{}/wiki/spaces/~user-123/overview",
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
fn shell_root_ls_lists_spaces() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
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
        .write_all(b"ls\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("- ALPHA/  Workspace Alpha [100]"));
    spaces.assert();
}

#[test]
fn shell_cd_space_then_ls_lists_homepage_children() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [
                {
                    "id": "2",
                    "status": "current",
                    "title": "Project Alpha",
                    "spaceId": "100",
                    "version": { "number": 2 }
                },
                {
                    "id": "3",
                    "status": "current",
                    "title": "Scratchpad",
                    "spaceId": "100",
                    "version": { "number": 1 }
                }
            ]
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
        .write_all(b"cd ALPHA\npwd\nls\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("/ALPHA"));
    assert!(stdout.contains("- Project Alpha [2]"));
    assert!(stdout.contains("- Scratchpad [3]"));
    spaces.assert();
    homepage_children.assert();
}

#[test]
fn shell_ls_accepts_page_id_target() {
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
                "id": "33023",
                "key": "TEAM",
                "name": "Team Hub",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "18548718",
                "status": "current",
                "title": "People Docs",
                "spaceId": "33023",
                "version": { "number": 1 }
            }]
        }));
    });

    let target_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/18548718/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "200",
                "status": "current",
                "title": "Policy Guide",
                "spaceId": "33023",
                "version": { "number": 1 }
            }]
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
        .write_all(b"cd TEAM\nls 18548718\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("- Policy Guide [200]"));
    spaces.assert();
    homepage_children.assert();
    target_children.assert();
}

#[test]
fn shell_ls_accepts_quoted_page_title_target() {
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
                "id": "33023",
                "key": "TEAM",
                "name": "Team Hub",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "18548718",
                "status": "current",
                "title": "People Docs",
                "spaceId": "33023",
                "version": { "number": 1 }
            }]
        }));
    });

    let target_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/18548718/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "200",
                "status": "current",
                "title": "Policy Guide",
                "spaceId": "33023",
                "version": { "number": 1 }
            }]
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
        .write_all("cd TEAM\nls \"People Docs\"\nexit\n".as_bytes())
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("- Policy Guide [200]"));
    spaces.assert();
    homepage_children.assert();
    target_children.assert();
}

#[test]
fn shell_cd_page_then_page_info_uses_current_page() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "2",
                "status": "current",
                    "title": "Platform Notes",
                "spaceId": "100",
                "version": { "number": 7 }
            }]
        }));
    });

    let page = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/2")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "2",
            "status": "current",
            "title": "Platform Notes",
            "spaceId": "100",
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
        .write_all(b"cd ALPHA\ncd 2\npwd\npage info\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("/ALPHA/Platform Notes"));
    assert!(stdout.contains("Platform Notes [2]"));
    spaces.assert();
    homepage_children.assert();
    page.assert();
}

#[test]
fn shell_cd_space_then_page_create_uses_space_context() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
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
        .write_all(b"cd ALPHA\npage create --title Draft --body '# Hello'\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("Created Draft [200]"));
    spaces.assert();
    create.assert();
}

#[test]
fn shell_cd_reports_ambiguous_page_titles() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [
                {
                    "id": "2",
                    "status": "current",
                    "title": "Notebook",
                    "spaceId": "100",
                    "version": { "number": 1 }
                },
                {
                    "id": "3",
                    "status": "current",
                    "title": "Notebook",
                    "spaceId": "100",
                    "version": { "number": 1 }
                }
            ]
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
        .write_all(b"cd ALPHA\ncd Notebook\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("is ambiguous under /ALPHA; use an id instead"));
    spaces.assert();
    homepage_children.assert();
}

#[test]
fn shell_ls_handles_space_without_homepage_id() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": null
            }]
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
        .write_all(b"cd ALPHA\nls\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("No child pages found."));
    spaces.assert();
}

#[test]
fn shell_cat_reads_current_page_text() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let homepage_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "2",
                "status": "current",
                "title": "People Docs",
                "spaceId": "100",
                "version": { "number": 1 }
            }]
        }));
    });

    let page = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/2")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "2",
            "status": "current",
            "title": "People Docs",
            "spaceId": "100",
            "version": { "number": 1 },
            "body": {
                "storage": {
                    "value": "<h1>People Docs</h1><p>Policy guide content</p>",
                    "representation": "storage"
                }
            }
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
        .write_all(b"cd ALPHA\ncd 2\ncat\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("Policy guide content"));
    spaces.assert();
    homepage_children.assert();
    page.assert();
}

#[test]
fn shell_find_walks_subtree() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let root_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "2",
                "status": "current",
                "title": "People Docs",
                "spaceId": "100",
                "version": { "number": 1 }
            }]
        }));
    });

    let nested_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/2/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "3",
                "status": "current",
                "title": "Policy Guide",
                "spaceId": "100",
                "version": { "number": 1 }
            }]
        }));
    });

    let leaf_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/3/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({ "results": [] }));
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
        .write_all(b"find ALPHA\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("/ALPHA [100]"));
    assert!(stdout.contains("/ALPHA/People Docs [2]"));
    assert!(stdout.contains("/ALPHA/People Docs/Policy Guide [3]"));
    spaces.assert();
    root_children.assert();
    nested_children.assert();
    leaf_children.assert();
}

#[test]
fn shell_grep_searches_subtree() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let root_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "2",
                "status": "current",
                "title": "People Docs",
                "spaceId": "100",
                "version": { "number": 1 }
            }]
        }));
    });

    let nested_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/2/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "3",
                "status": "current",
                "title": "Policy Guide",
                "spaceId": "100",
                "version": { "number": 1 }
            }]
        }));
    });

    let leaf_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/3/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({ "results": [] }));
    });

    let page_2 = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/2")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "2",
            "status": "current",
            "title": "People Docs",
            "spaceId": "100",
            "version": { "number": 1 },
            "body": {
                "storage": {
                    "value": "<p>Alpha policy note</p>",
                    "representation": "storage"
                }
            }
        }));
    });

    let page_3 = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/3")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "3",
            "status": "current",
            "title": "Policy Guide",
            "spaceId": "100",
            "version": { "number": 1 },
            "body": {
                "storage": {
                    "value": "<p>Policy keyword appears here</p>",
                    "representation": "storage"
                }
            }
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
        .write_all(b"grep Policy ALPHA\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("/ALPHA/People Docs/Policy Guide [3]:1:Policy keyword appears here"));
    spaces.assert();
    root_children.assert();
    nested_children.assert();
    leaf_children.assert();
    page_2.assert();
    page_3.assert();
}

#[test]
fn shell_pipeline_ls_into_grep_filters_listing() {
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
                "key": "ALPHA",
                "name": "Workspace Alpha",
                "homepageId": "1"
            }]
        }));
    });

    let root_children = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/1/children")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [
                {
                    "id": "2",
                    "status": "current",
                    "title": "People Docs",
                    "spaceId": "100",
                    "version": { "number": 1 }
                },
                {
                    "id": "3",
                    "status": "current",
                    "title": "Policy Guide",
                    "spaceId": "100",
                    "version": { "number": 1 }
                }
            ]
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
        .write_all(b"ls ALPHA | grep People\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("People Docs"));
    assert!(!stdout.contains("Policy Guide [3]"));
    spaces.assert();
    root_children.assert();
}

#[test]
fn shell_rejects_stateful_builtins_in_pipelines() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    write_minimal_config(&config_path);

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
        .write_all(b"cd ALPHA | grep ALPHA\nexit\n")
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("`cd` cannot be used in a pipeline"));
}

#[test]
fn shell_rejects_pipelines_longer_than_limit() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");
    write_minimal_config(&config_path);

    let pipeline = std::iter::repeat("pwd")
        .take(17)
        .collect::<Vec<_>>()
        .join(" | ");
    let input = format!("{pipeline}\nexit\n");

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
        .write_all(input.as_bytes())
        .expect("shell input should be written");

    let output = child
        .wait_with_output()
        .expect("shell output should be captured");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("pipeline depth exceeds maximum of 16 stages"));
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
    assert!(stdout.contains(
        "confluence page read https://your-site.atlassian.net/wiki/spaces/SPACE/pages/12345/Page+Title"
    ));
    assert!(stdout.contains("confluence page create --space-key SPACE"));
}

#[test]
fn shell_help_shows_filesystem_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_confluence"))
        .arg("shell")
        .arg("--help")
        .output()
        .expect("shell help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("pwd"));
    assert!(stdout.contains("ls"));
    assert!(stdout.contains("cat [target]"));
    assert!(stdout.contains("grep <pattern> [target]"));
    assert!(stdout.contains("find [target] [--name <pattern>]"));
    assert!(stdout.contains("cd SPACE"));
    assert!(stdout.contains("help page"));
}
