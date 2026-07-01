use openapi_sync_mcp::types::OpenApiVersion;
use openapi_sync_mcp::{ParseFormat, ParseInput, parse_spec};
use std::path::PathBuf;

fn xquik_spec_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("xquik-api.json")
        .to_string_lossy()
        .to_string()
}

fn parse_input(format: ParseFormat) -> ParseInput {
    ParseInput {
        source: xquik_spec_path(),
        format,
        project_dir: None,
        use_cache: false,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    }
}

#[tokio::test]
async fn test_xquik_openapi31_summary() {
    let result = parse_spec(parse_input(ParseFormat::Summary)).await;

    assert!(result.success, "parse failed: {:?}", result.error);

    let metadata = result.metadata.expect("metadata should be present");
    assert_eq!(metadata.title, "Xquik API");
    assert_eq!(metadata.version, "1.0");
    assert_eq!(metadata.openapi_version, OpenApiVersion::OpenApi31);
    assert_eq!(metadata.endpoint_count, 2);
    assert_eq!(metadata.schema_count, 4);
}

#[tokio::test]
async fn test_xquik_openapi31_tagged_endpoint_filter() {
    let mut input = parse_input(ParseFormat::Endpoints);
    input.tag = Some("x".to_string());

    let result = parse_spec(input).await;

    assert!(result.success, "parse failed: {:?}", result.error);

    let endpoints = result.endpoints.expect("endpoints should be present");
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].path, "/api/v1/x/tweets/search");
    assert_eq!(endpoints[0].method, "GET");
    assert_eq!(endpoints[0].operation_id.as_deref(), Some("searchTweets"));
    assert_eq!(endpoints[0].schema_refs, ["TweetSearchResponse"]);
}
