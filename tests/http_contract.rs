use confluence_cli::api::{
    CommentCreateRequest, ConfluenceApi, HttpConfluenceApi, MovePageRequest,
};
use confluence_cli::config::{AuthKind, ResolvedProfile};
use confluence_cli::domain::{CommentLocation, MoveTarget, PageId, PageRef};
use confluence_cli::support::ConfluenceCliError;
use httpmock::Method::{GET, POST, PUT};
use httpmock::{Mock, MockServer};
use serde_json::json;

fn test_profile(server: &MockServer) -> ResolvedProfile {
    ResolvedProfile {
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
