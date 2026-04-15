use confluence_cli::api::{
    CommentCreateRequest, ConfluenceApi, HttpConfluenceApi, MovePageRequest,
};
use confluence_cli::app::{self, PageExportResult};
use confluence_cli::config::{AuthKind, ResolvedProfile};
use confluence_cli::domain::{CommentLocation, MoveTarget, PageId, PageRef};
use confluence_cli::run_from;
use confluence_cli::support::ConfluenceCliError;
use httpmock::Method::{DELETE, GET, POST, PUT};
use httpmock::{Mock, MockServer};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn test_profile(server: &MockServer) -> ResolvedProfile {
    ResolvedProfile {
        id: "profile-1".to_owned(),
        name: Some("test".to_owned()),
        domain: server.address().to_string(),
        protocol: "http".to_owned(),
        api_path: "/wiki/rest/api".to_owned(),
        auth_type: AuthKind::Bearer,
        email: None,
        username: None,
        api_token: Some("token-123".to_owned()),
        password: None,
        read_only: false,
        secret_backend: Some(confluence_cli::config::SecretBackend::Keyring),
    }
}

fn page_v1(
    id: u64,
    title: &str,
    status: &str,
    space_id: &str,
    ancestors: &[u64],
) -> serde_json::Value {
    json!({
        "id": id.to_string(),
        "title": title,
        "status": status,
        "version": { "number": 3 },
        "space": { "id": space_id, "key": "ENG" },
        "body": {
            "storage": {
                "value": "<p>Hello</p>",
                "representation": "storage"
            }
        },
        "ancestors": ancestors.iter().map(|id| json!({"id": id.to_string()})).collect::<Vec<_>>()
    })
}

fn page_v1_with_key(
    id: u64,
    title: &str,
    status: &str,
    space_id: &str,
    space_key: &str,
    ancestors: &[u64],
) -> serde_json::Value {
    json!({
        "id": id.to_string(),
        "title": title,
        "status": status,
        "version": { "number": 3 },
        "space": { "id": space_id, "key": space_key },
        "body": {
            "storage": {
                "value": "<p>Hello</p>",
                "representation": "storage"
            }
        },
        "ancestors": ancestors.iter().map(|id| json!({"id": id.to_string()})).collect::<Vec<_>>()
    })
}

fn attachment_list_response(next: Option<&str>) -> serde_json::Value {
    json!({
        "results": [{
            "id": "att-1",
            "title": "spec.md",
            "metadata": { "mediaType": "text/markdown" },
            "extensions": { "fileSize": 128 },
            "version": { "number": 2 },
            "_links": { "download": "/wiki/download/attachments/123/spec.md" }
        }],
        "_links": next.map(|value| json!({ "next": value })).unwrap_or_else(|| json!({}))
    })
}

fn property_response(key: &str, value: serde_json::Value, version: u32) -> serde_json::Value {
    json!({
        "key": key,
        "value": value,
        "version": { "number": version }
    })
}

#[test]
fn search_pages_cql_follows_next_links() {
    let server = MockServer::start();

    let first = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/search")
            .query_param("cql", "type=page")
            .query_param("limit", "25")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{ "id": "1", "title": "Page One" }],
            "_links": { "next": "/wiki/rest/api/content/search/next-page?cursor=page-2" }
        }));
    });

    let second = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/search/next-page")
            .query_param("cursor", "page-2")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{ "id": "2", "title": "Page Two" }],
            "_links": {}
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let pages = api
        .search_pages_cql("type=page")
        .expect("search should paginate successfully");

    first.assert();
    second.assert();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].title, "Page One");
    assert_eq!(pages[1].title, "Page Two");
}

#[test]
fn move_before_rejects_top_level_targets_before_put() {
    let server = MockServer::start();

    let source = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/10")
            .query_param("expand", "version,space,body.storage")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(page_v1(10, "Source", "current", "100", &[1, 2]));
    });

    let target = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/20")
            .query_param("expand", "space,ancestors")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(page_v1(20, "Target", "current", "100", &[]));
    });

    let put_move: Mock<'_> = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/10/move/before/20");
        then.status(200);
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let error = api
        .move_page(MovePageRequest {
            page: PageRef::Id(PageId::new(10)),
            target: MoveTarget::Before(PageRef::Id(PageId::new(20))),
            title: None,
        })
        .expect_err("top-level target should be rejected");

    source.assert();
    target.assert();
    put_move.assert_hits(0);
    assert!(matches!(error, ConfluenceCliError::Config(_)));
}

#[test]
fn comment_reply_posts_parent_id_and_inline_properties() {
    let server = MockServer::start();

    let comment = server.mock(|when, then| {
        when.method(POST)
            .path("/wiki/rest/api/content")
            .header("authorization", "Bearer token-123")
            .header("content-type", "application/json")
            .body_contains("\"container\":{\"id\":\"123\",\"type\":\"page\"}")
            .body_contains("\"ancestors\":[{\"id\":\"c-1\"}]")
            .body_contains("\"location\":\"inline\"")
            .body_contains("\"inlineProperties\":{\"markerRef\":\"m1\"}");
        then.status(200).json_body(json!({
            "id": "c-2",
            "status": "current",
            "body": { "storage": { "value": "<p>Reply</p>" } },
            "version": { "number": 1 },
            "ancestors": [{ "id": "c-1", "type": "comment" }],
            "extensions": { "location": "inline" }
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let summary = api
        .create_comment(CommentCreateRequest {
            page: PageRef::Id(PageId::new(123)),
            body_storage: "<p>Reply</p>".to_owned(),
            parent_id: Some("c-1".to_owned()),
            location: CommentLocation::Inline,
            inline_properties: Some(json!({ "markerRef": "m1" })),
        })
        .expect("comment reply should succeed");

    comment.assert();
    assert_eq!(summary.parent_id.as_deref(), Some("c-1"));
    assert_eq!(summary.location, Some(CommentLocation::Inline));
}

#[test]
fn attachment_list_follows_pagination_and_maps_summary_fields() {
    let server = MockServer::start();

    let first = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/attachment")
            .query_param("start", "0")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(attachment_list_response(Some(
            "/wiki/rest/api/content/123/child/attachment?start=100&limit=100",
        )));
    });

    let second = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/attachment")
            .query_param("start", "100")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(json!({ "results": [], "_links": {} }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let attachments = api
        .list_attachments(&PageRef::Id(PageId::new(123)))
        .expect("attachment listing should succeed");

    first.assert();
    second.assert();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].title, "spec.md");
    assert_eq!(attachments[0].media_type, "text/markdown");
    assert_eq!(attachments[0].file_size, 128);
    assert_eq!(attachments[0].version, Some(2));
    assert_eq!(
        attachments[0].download_link.as_deref(),
        Some(
            format!(
                "{}/wiki/download/attachments/123/spec.md",
                server.base_url()
            )
            .as_str(),
        )
    );
}

#[test]
fn download_attachment_fetches_metadata_then_binary() {
    let server = MockServer::start();

    let metadata = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/attachment/att-1")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "att-1",
            "title": "spec.md",
            "_links": { "download": "/wiki/download/attachments/123/spec.md" }
        }));
    });

    let download = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/download/attachments/123/spec.md")
            .header("authorization", "Bearer token-123");
        then.status(200).body("hello world");
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let bytes = api
        .download_attachment(&PageRef::Id(PageId::new(123)), "att-1")
        .expect("attachment download should succeed");

    metadata.assert();
    download.assert();
    assert_eq!(bytes, b"hello world");
}

#[test]
fn upload_attachment_uses_post_put_and_fields() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let file_path = dir.path().join("spec.md");
    fs::write(&file_path, "hello").expect("fixture should be written");

    let post = server.mock(|when, then| {
        when.method(POST)
            .path("/wiki/rest/api/content/123/child/attachment")
            .header("authorization", "Bearer token-123")
            .header("x-atlassian-token", "nocheck")
            .header_exists("content-type");
        then.status(200).json_body(attachment_list_response(None));
    });

    let put = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/123/child/attachment")
            .header("authorization", "Bearer token-123")
            .header("x-atlassian-token", "nocheck")
            .header_exists("content-type");
        then.status(200).json_body(attachment_list_response(None));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let _ = api.upload_attachment(confluence_cli::api::AttachmentUploadRequest {
        page: PageRef::Id(PageId::new(123)),
        file_path: file_path.clone(),
        comment: Some("upload".to_owned()),
        minor_edit: true,
        replace: false,
    });
    let _ = api.upload_attachment(confluence_cli::api::AttachmentUploadRequest {
        page: PageRef::Id(PageId::new(123)),
        file_path,
        comment: None,
        minor_edit: false,
        replace: true,
    });

    post.assert();
    put.assert();
}

#[test]
fn delete_attachment_hits_delete_route() {
    let server = MockServer::start();
    let delete = server.mock(|when, then| {
        when.method(DELETE)
            .path("/wiki/rest/api/content/123/child/attachment/att-1")
            .header("authorization", "Bearer token-123");
        then.status(204);
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    api.delete_attachment(&PageRef::Id(PageId::new(123)), "att-1")
        .expect("attachment delete should succeed");
    delete.assert();
}

#[test]
fn property_round_trip_covers_list_get_set_and_delete() {
    let server = MockServer::start();

    let list = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/property")
            .query_param("start", "0")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [property_response("release notes", json!({ "value": 1 }), 2)],
            "_links": {}
        }));
    });

    let get_before_set = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/property/release%20notes")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(property_response("release notes", json!({ "value": 1 }), 2));
    });

    let set = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/123/property/release%20notes")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":3")
            .body_contains("\"release notes\"");
        then.status(200)
            .json_body(property_response("release notes", json!({ "value": 2 }), 3));
    });

    let delete = server.mock(|when, then| {
        when.method(DELETE)
            .path("/wiki/rest/api/content/123/property/release%20notes")
            .header("authorization", "Bearer token-123");
        then.status(204);
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let list_result = api
        .list_properties(&PageRef::Id(PageId::new(123)))
        .expect("property list should succeed");
    let property = api
        .get_property(&PageRef::Id(PageId::new(123)), "release notes")
        .expect("property get should succeed");
    let updated = api
        .set_property(
            &PageRef::Id(PageId::new(123)),
            "release notes",
            json!({ "value": 2 }),
        )
        .expect("property set should succeed");
    api.delete_property(&PageRef::Id(PageId::new(123)), "release notes")
        .expect("property delete should succeed");

    list.assert();
    get_before_set.assert_hits(2);
    set.assert();
    delete.assert();
    assert_eq!(list_result.len(), 1);
    assert_eq!(property.key, "release notes");
    assert_eq!(updated.version, 3);
}

#[test]
fn set_property_starts_at_version_one_after_not_found() {
    let server = MockServer::start();

    let get = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/property/new%20prop")
            .header("authorization", "Bearer token-123");
        then.status(404);
    });

    let put = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/123/property/new%20prop")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":1");
        then.status(200)
            .json_body(property_response("new prop", json!({ "ok": true }), 1));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let property = api
        .set_property(
            &PageRef::Id(PageId::new(123)),
            "new prop",
            json!({ "ok": true }),
        )
        .expect("property set should succeed");

    get.assert();
    put.assert();
    assert_eq!(property.version, 1);
}

#[test]
fn page_export_uses_page_info_body_and_attachment_requests() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");

    let page_info = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/123")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "123",
            "title": "Design Doc",
            "status": "current",
            "spaceId": "100",
            "version": { "number": 5 }
        }));
    });

    let page_body = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123")
            .query_param("expand", "body.storage,version,space")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "123",
            "title": "Design Doc",
            "status": "current",
            "version": { "number": 5 },
            "space": { "id": "100", "key": "ENG" },
            "body": { "storage": { "value": "<h1>Title</h1><p>Hello</p>" } }
        }));
    });

    let list_attachments = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/attachment")
            .query_param("start", "0")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(attachment_list_response(None));
    });

    let attachment_metadata = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/attachment/att-1")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "att-1",
            "title": "spec.md",
            "_links": { "download": "/wiki/download/attachments/123/spec.md" }
        }));
    });

    let attachment_download = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/download/attachments/123/spec.md")
            .header("authorization", "Bearer token-123");
        then.status(200).body("attachment-body");
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let result: PageExportResult = app::page_export(
        &api,
        &PageRef::Id(PageId::new(123)),
        dir.path(),
        confluence_cli::domain::BodyFormat::Markdown,
        true,
    )
    .expect("page export should succeed");

    page_info.assert();
    page_body.assert();
    list_attachments.assert();
    attachment_metadata.assert();
    attachment_download.assert();
    assert_eq!(result.attachment_count, 1);
    assert!(result.content_path.exists());
    assert!(dir.path().join("attachments").join("spec.md").exists());
}

#[test]
fn move_to_parent_puts_full_payload_with_incremented_version() {
    let server = MockServer::start();

    let current = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/10")
            .query_param("expand", "version,space,body.storage")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(page_v1(10, "Source", "current", "100", &[1, 2]));
    });

    let parent = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/20")
            .query_param("expand", "space")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(page_v1(20, "Parent", "current", "100", &[1]));
    });

    let put = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/10")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":4")
            .body_contains("\"ancestors\":[{\"id\":\"20\"}]")
            .body_contains("\"value\":\"<p>Hello</p>\"");
        then.status(200)
            .json_body(page_v1(10, "Source", "current", "100", &[20]));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let summary = api
        .move_page(MovePageRequest {
            page: PageRef::Id(PageId::new(10)),
            target: MoveTarget::Parent(PageRef::Id(PageId::new(20))),
            title: None,
        })
        .expect("parent move should succeed");

    current.assert();
    parent.assert();
    put.assert();
    assert_eq!(summary.id, 10);
}

#[test]
fn move_before_rejects_cross_space_targets_before_put() {
    let server = MockServer::start();

    let current = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/10")
            .query_param("expand", "version,space,body.storage")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(page_v1(10, "Source", "current", "100", &[1, 2]));
    });

    let target = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/20")
            .query_param("expand", "space,ancestors")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(page_v1_with_key(
            20,
            "Target",
            "current",
            "200",
            "OPS",
            &[1],
        ));
    });

    let put_move = server.mock(|when, then| {
        when.method(PUT)
            .path("/wiki/rest/api/content/10/move/before/20");
        then.status(200);
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let error = api
        .move_page(MovePageRequest {
            page: PageRef::Id(PageId::new(10)),
            target: MoveTarget::Before(PageRef::Id(PageId::new(20))),
            title: None,
        })
        .expect_err("cross-space target should be rejected");

    current.assert();
    target.assert();
    put_move.assert_hits(0);
    assert!(matches!(error, ConfluenceCliError::Config(_)));
}

#[test]
fn comment_list_uses_location_query_and_follows_pagination() {
    let server = MockServer::start();

    let first = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/comment")
            .query_param("location", "inline")
            .query_param("start", "0")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "results": [{
                "id": "c-1",
                "status": "current",
                "body": { "storage": { "value": "<p>First</p>" } },
                "extensions": { "location": "inline", "resolution": { "status": "open" } }
            }],
            "_links": { "next": "/wiki/rest/api/content/123/child/comment?location=inline&start=100&limit=100" }
        }));
    });

    let second = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/123/child/comment")
            .query_param("location", "inline")
            .query_param("start", "100")
            .query_param("limit", "100")
            .header("authorization", "Bearer token-123");
        then.status(200)
            .json_body(json!({ "results": [], "_links": {} }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let comments = api
        .list_comments(
            &PageRef::Id(PageId::new(123)),
            Some(CommentLocation::Inline),
        )
        .expect("comment list should succeed");

    first.assert();
    second.assert();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].location, Some(CommentLocation::Inline));
    assert_eq!(comments[0].resolution.as_deref(), Some("open"));
}

#[test]
fn comment_delete_hits_delete_route() {
    let server = MockServer::start();
    let delete = server.mock(|when, then| {
        when.method(DELETE)
            .path("/wiki/rest/api/content/c-1")
            .header("authorization", "Bearer token-123");
        then.status(204);
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    api.delete_comment("c-1")
        .expect("comment delete should succeed");
    delete.assert();
}

#[test]
fn comment_info_reads_full_comment_document() {
    let server = MockServer::start();
    let get = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/c-1")
            .query_param("expand", "body.storage,history,version,ancestors")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "body": { "storage": { "value": "<p>Inline comment</p>" } },
            "history": { "createdBy": { "displayName": "Ada" }, "createdDate": "2025-01-01" },
            "version": { "number": 2 },
            "extensions": {
                "location": "inline",
                "resolution": { "status": "open" },
                "inlineProperties": { "markerRef": "m-1", "selection": "selected text" }
            }
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let comment = api.get_comment("c-1").expect("comment info should succeed");

    get.assert();
    assert_eq!(comment.location, Some(CommentLocation::Inline));
    assert_eq!(comment.resolution.as_deref(), Some("open"));
    assert_eq!(
        comment.inline_properties,
        Some(json!({ "markerRef": "m-1", "selection": "selected text" }))
    );
}

#[test]
fn comment_resolve_and_reopen_use_inline_comment_v2_transport() {
    let server = MockServer::start();

    let get_resolve = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/inline-comments/c-1")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "version": { "number": 2 },
            "resolutionStatus": "open",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-1", "inlineOriginalSelection": "selected text" }
        }));
    });

    let put_resolve = server.mock(|when, then| {
        when.method(PUT)
            .path("/api/v2/inline-comments/c-1")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":3")
            .body_contains("\"resolved\":true");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "version": { "number": 3 },
            "resolutionStatus": "resolved",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-1", "inlineOriginalSelection": "selected text" }
        }));
    });

    let get_reopen = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/inline-comments/c-2")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-2",
            "status": "current",
            "version": { "number": 5 },
            "resolutionStatus": "resolved",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-2", "inlineOriginalSelection": "selected text" }
        }));
    });

    let put_reopen = server.mock(|when, then| {
        when.method(PUT)
            .path("/api/v2/inline-comments/c-2")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":6")
            .body_contains("\"resolved\":false");
        then.status(200).json_body(json!({
            "id": "c-2",
            "status": "current",
            "version": { "number": 6 },
            "resolutionStatus": "open",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-2", "inlineOriginalSelection": "selected text" }
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let resolved = api
        .set_inline_comment_resolution("c-1", true)
        .expect("resolve should succeed");
    let reopened = api
        .set_inline_comment_resolution("c-2", false)
        .expect("reopen should succeed");

    get_resolve.assert();
    put_resolve.assert();
    get_reopen.assert();
    put_reopen.assert();
    assert_eq!(resolved.resolution.as_deref(), Some("resolved"));
    assert_eq!(resolved.inline_marker_ref.as_deref(), Some("m-1"));
    assert_eq!(reopened.resolution.as_deref(), Some("open"));
    assert_eq!(
        reopened.inline_original_selection.as_deref(),
        Some("selected text")
    );
}

#[test]
fn comment_info_reads_richer_inline_metadata() {
    let server = MockServer::start();
    let get = server.mock(|when, then| {
        when.method(GET)
            .path("/wiki/rest/api/content/c-1")
            .query_param("expand", "body.storage,history,version,ancestors")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "body": { "storage": { "value": "<p>Inline comment</p>" } },
            "history": { "createdBy": { "displayName": "Ada" }, "createdDate": "2025-01-01" },
            "version": { "number": 2 },
            "extensions": {
                "location": "inline",
                "resolution": { "status": "open" },
                "inlineProperties": { "markerRef": "m-1", "selection": "selected text" }
            }
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let comment = api.get_comment("c-1").expect("comment info should succeed");

    get.assert();
    assert_eq!(comment.location, Some(CommentLocation::Inline));
    assert_eq!(comment.resolution.as_deref(), Some("open"));
    assert_eq!(
        comment.inline_properties,
        Some(json!({ "markerRef": "m-1", "selection": "selected text" }))
    );
}

#[test]
fn comment_resolve_and_reopen_use_inline_comment_endpoint() {
    let server = MockServer::start();

    let get_resolve = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/inline-comments/c-1")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "version": { "number": 2 },
            "resolutionStatus": "open",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-1", "inlineOriginalSelection": "selected text" }
        }));
    });

    let put_resolve = server.mock(|when, then| {
        when.method(PUT)
            .path("/api/v2/inline-comments/c-1")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":3")
            .body_contains("\"resolved\":true");
        then.status(200).json_body(json!({
            "id": "c-1",
            "status": "current",
            "version": { "number": 3 },
            "resolutionStatus": "resolved",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-1", "inlineOriginalSelection": "selected text" }
        }));
    });

    let get_reopen = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/inline-comments/c-2")
            .query_param("body-format", "storage")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "c-2",
            "status": "current",
            "version": { "number": 5 },
            "resolutionStatus": "resolved",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-2", "inlineOriginalSelection": "selected text" }
        }));
    });

    let put_reopen = server.mock(|when, then| {
        when.method(PUT)
            .path("/api/v2/inline-comments/c-2")
            .header("authorization", "Bearer token-123")
            .body_contains("\"number\":6")
            .body_contains("\"resolved\":false");
        then.status(200).json_body(json!({
            "id": "c-2",
            "status": "current",
            "version": { "number": 6 },
            "resolutionStatus": "open",
            "body": { "storage": { "value": "<p>Comment</p>" } },
            "properties": { "inlineMarkerRef": "m-2", "inlineOriginalSelection": "selected text" }
        }));
    });

    let api = HttpConfluenceApi::new(test_profile(&server)).expect("api should initialize");
    let resolved = api
        .set_inline_comment_resolution("c-1", true)
        .expect("resolve should succeed");
    let reopened = api
        .set_inline_comment_resolution("c-2", false)
        .expect("reopen should succeed");

    get_resolve.assert();
    put_resolve.assert();
    get_reopen.assert();
    put_reopen.assert();
    assert_eq!(resolved.resolution.as_deref(), Some("resolved"));
    assert_eq!(resolved.inline_marker_ref.as_deref(), Some("m-1"));
    assert_eq!(reopened.resolution.as_deref(), Some("open"));
    assert_eq!(
        reopened.inline_original_selection.as_deref(),
        Some("selected text")
    );
}

#[test]
fn cli_profile_flag_selects_profile_for_http_requests() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    fs::write(
        &config_path,
        serde_json::to_string_pretty(&json!({
            "active_profile": "default",
            "profiles": {
                "default": {
                    "domain": "inactive.example.invalid",
                    "auth_type": "bearer",
                    "api_token": "inactive"
                },
                "work": {
                    "domain": server.address().to_string(),
                    "protocol": "http",
                    "api_path": "/wiki/rest/api",
                    "auth_type": "bearer",
                    "api_token": "token-123"
                }
            }
        }))
        .expect("config json should serialize"),
    )
    .expect("config should be written");

    let page_info = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v2/pages/123")
            .header("authorization", "Bearer token-123");
        then.status(200).json_body(json!({
            "id": "123",
            "title": "Design Doc",
            "status": "current",
            "spaceId": "100",
            "version": { "number": 5 }
        }));
    });

    run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "--profile",
        "work",
        "page",
        "info",
        "123",
    ])
    .expect("CLI should succeed with explicit profile");

    page_info.assert();
}

#[test]
fn cli_read_only_profile_blocks_mutation_before_http() {
    let server = MockServer::start();
    let dir = tempdir().expect("tempdir should be created");
    let config_path = dir.path().join("config.json");

    fs::write(
        &config_path,
        serde_json::to_string_pretty(&json!({
            "active_profile": "work",
            "profiles": {
                "work": {
                    "domain": server.address().to_string(),
                    "protocol": "http",
                    "api_path": "/wiki/rest/api",
                    "auth_type": "bearer",
                    "api_token": "token-123",
                    "read_only": true
                }
            }
        }))
        .expect("config json should serialize"),
    )
    .expect("config should be written");

    let create = server.mock(|when, then| {
        when.method(POST).path("/wiki/api/v2/pages");
        then.status(200);
    });

    let error = run_from([
        "confluence",
        "--config-path",
        config_path
            .to_str()
            .expect("config path should be valid utf-8"),
        "page",
        "create",
        "--title",
        "Design",
        "--body",
        "# Hello",
        "--space-id",
        "100",
    ])
    .expect_err("read-only profile should block mutating commands");

    create.assert_hits(0);
    assert!(matches!(error, ConfluenceCliError::Config(_)));
}
