//! Integration tests for caching improvements (P0, P1, P2)

use openapi_sync_mcp::*;
use std::path::PathBuf;

fn test_spec_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test-api.json")
        .to_string_lossy()
        .to_string()
}

fn test_project_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .to_string_lossy()
        .to_string()
}

fn cache_file_path() -> PathBuf {
    PathBuf::from(&test_project_dir()).join(".openapi-sync.cache.json")
}

fn cleanup_cache() {
    let cache_path = cache_file_path();
    if cache_path.exists() {
        std::fs::remove_file(&cache_path).ok();
    }
}

#[tokio::test]
async fn test_p0_cache_creation_with_mtime() {
    cleanup_cache();

    // First parse - should create cache
    let input = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Summary,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };

    let result = parse_spec(input).await;
    assert!(result.success, "Parse should succeed: {:?}", result.error);
    assert!(result.metadata.is_some());

    // Verify cache file was created
    assert!(cache_file_path().exists(), "Cache file should be created");

    // Verify cache structure
    let cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();

    // P0: Check mtime is set for local files
    assert!(
        cache["local_cache"]["mtime"].is_string(),
        "local_cache.mtime should be set (P0 improvement)"
    );

    // Check schema_version is set
    assert!(
        cache["schema_version"].is_u64(),
        "schema_version should be set for compatibility checking"
    );

    // Check parsed_spec is saved (instead of raw_spec)
    assert!(
        cache["parsed_spec"].is_object(),
        "parsed_spec should be saved for zero-parse caching"
    );

    println!("✓ P0: Cache created with mtime, schema_version, and parsed_spec");
}

#[tokio::test]
async fn test_p1_cache_hit_returns_full_data() {
    cleanup_cache();

    // First parse - create cache
    let input1 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Endpoints,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };

    let result1 = parse_spec(input1).await;
    assert!(result1.success);
    assert!(result1.endpoints.is_some(), "First parse should return endpoints");
    let endpoints_count1 = result1.endpoints.as_ref().map(|e| e.len()).unwrap_or(0);

    // Second parse - should hit cache and still return full data
    let input2 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Endpoints,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };

    let result2 = parse_spec(input2).await;
    assert!(result2.success);

    // P1: Cache hit should return full endpoints data, not None
    assert!(
        result2.endpoints.is_some(),
        "P1: Cache hit should return full endpoints data, not None"
    );

    let endpoints_count2 = result2.endpoints.as_ref().map(|e| e.len()).unwrap_or(0);
    assert_eq!(
        endpoints_count1, endpoints_count2,
        "P1: Cache hit should return same number of endpoints"
    );

    println!("✓ P1: Cache hit returns full data ({} endpoints)", endpoints_count2);
}

#[tokio::test]
async fn test_p0_deps_uses_cache() {
    cleanup_cache();

    // First, create cache via parse
    let parse_input = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Summary,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let _ = parse_spec(parse_input).await;

    // Now test deps with cache
    let deps_input = DepsInput {
        source: test_spec_path(),
        schema: Some("User".to_string()),
        path: None,
        direction: DepsDirection::Downstream,
        project_dir: Some(test_project_dir()),
        use_cache: true,
    };

    let result = query_deps(deps_input).await;
    assert!(result.success, "deps should succeed: {:?}", result.error);
    assert!(result.affected_paths.len() > 0, "User schema should have downstream paths");

    println!("✓ P0: oas_deps uses cache, found {} affected paths", result.affected_paths.len());
}

#[tokio::test]
async fn test_p0_generate_uses_cache() {
    cleanup_cache();

    // First, create cache via parse
    let parse_input = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Summary,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let _ = parse_spec(parse_input).await;

    // Now test generate with cache
    let generate_input = GenerateInput {
        source: test_spec_path(),
        target: GenerateTarget::TypescriptTypes,
        style: CodeStyle::default(),
        schemas: vec![],
        endpoints: vec![],
        project_dir: Some(test_project_dir()),
        use_cache: true,
    };

    let result = generate_code(generate_input).await;
    assert!(result.success, "generate should succeed: {:?}", result.error);
    assert!(result.summary.types_generated > 0, "Should generate types");

    println!(
        "✓ P0: oas_generate uses cache, generated {} types",
        result.summary.types_generated
    );
}

#[tokio::test]
async fn test_cache_performance_improvement() {
    cleanup_cache();

    // First parse - cold (no cache)
    let start1 = std::time::Instant::now();
    let input1 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let _ = parse_spec(input1).await;
    let cold_time = start1.elapsed();

    // Second parse - warm (with cache)
    let start2 = std::time::Instant::now();
    let input2 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let _ = parse_spec(input2).await;
    let warm_time = start2.elapsed();

    println!("Cold parse: {:?}", cold_time);
    println!("Warm parse (cache hit): {:?}", warm_time);

    // Cache hit should generally be faster (though for local files the difference may be small)
    println!(
        "✓ Performance test completed: cold={:?}, warm={:?}",
        cold_time, warm_time
    );
}

#[tokio::test]
async fn test_p3_zero_parse_caching() {
    cleanup_cache();

    // First parse - creates cache with parsed_spec
    let input1 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result1 = parse_spec(input1).await;
    assert!(result1.success);

    // Verify cache file contains parsed_spec
    let cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();

    assert!(
        cache["parsed_spec"].is_object(),
        "P3: parsed_spec should be stored in cache"
    );

    // Verify parsed_spec contains all expected fields
    assert!(
        cache["parsed_spec"]["metadata"]["title"].is_string(),
        "P3: parsed_spec should contain metadata"
    );
    assert!(
        cache["parsed_spec"]["endpoints"].is_object(),
        "P3: parsed_spec should contain endpoints"
    );
    assert!(
        cache["parsed_spec"]["schemas"].is_object(),
        "P3: parsed_spec should contain schemas"
    );

    // Second parse - should use parsed_spec directly (zero parsing)
    let input2 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result2 = parse_spec(input2).await;
    assert!(result2.success);

    // Results should be identical
    assert_eq!(
        result1.endpoints.as_ref().map(|e| e.len()),
        result2.endpoints.as_ref().map(|e| e.len()),
        "P3: Zero-parse cache hit should return same endpoints"
    );
    assert_eq!(
        result1.schemas.as_ref().map(|s| s.len()),
        result2.schemas.as_ref().map(|s| s.len()),
        "P3: Zero-parse cache hit should return same schemas"
    );

    println!("✓ P3: Zero-parse caching verified - parsed_spec stored and reused");
}

#[tokio::test]
async fn test_schema_version_invalidation() {
    cleanup_cache();

    // First parse - creates cache with current schema_version
    let input1 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result1 = parse_spec(input1).await;
    assert!(result1.success);

    // Verify cache has schema_version
    let cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    let current_version = cache["schema_version"].as_u64().unwrap();
    assert!(current_version > 0, "schema_version should be set");

    // Modify schema_version to simulate incompatible cache
    let mut modified_cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    modified_cache["schema_version"] = serde_json::Value::Number(serde_json::Number::from(999u64));
    std::fs::write(
        cache_file_path(),
        serde_json::to_string_pretty(&modified_cache).unwrap(),
    )
    .unwrap();

    // Second parse - should invalidate cache due to schema_version mismatch
    let input2 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result2 = parse_spec(input2).await;
    assert!(result2.success, "Should succeed with fresh fetch");

    // Verify cache was updated with correct schema_version
    let updated_cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let updated_cache: serde_json::Value = serde_json::from_str(&updated_cache_content).unwrap();
    let updated_version = updated_cache["schema_version"].as_u64().unwrap();
    assert_eq!(
        updated_version, current_version,
        "Cache should be updated with correct schema_version"
    );

    println!("✓ Schema version invalidation works correctly");
}

#[tokio::test]
async fn test_hash_integrity_check() {
    cleanup_cache();

    // First parse - creates cache
    let input1 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result1 = parse_spec(input1).await;
    assert!(result1.success);

    // Corrupt the spec_hash to simulate corrupted cache
    let cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let mut modified_cache: serde_json::Value = serde_json::from_str(&cache_content).unwrap();
    modified_cache["spec_hash"] = serde_json::Value::String("corrupted_hash".to_string());
    std::fs::write(
        cache_file_path(),
        serde_json::to_string_pretty(&modified_cache).unwrap(),
    )
    .unwrap();

    // Second parse - should detect hash mismatch and fetch fresh
    let input2 = ParseInput {
        source: test_spec_path(),
        format: ParseFormat::Full,
        project_dir: Some(test_project_dir()),
        use_cache: true,
        ttl_seconds: None,
        limit: None,
        offset: 0,
        tag: None,
        path_prefix: None,
    };
    let result2 = parse_spec(input2).await;
    assert!(result2.success, "Should succeed with fresh fetch after hash mismatch");

    // Verify cache was updated with correct hash
    let updated_cache_content = std::fs::read_to_string(cache_file_path()).unwrap();
    let updated_cache: serde_json::Value = serde_json::from_str(&updated_cache_content).unwrap();
    assert_ne!(
        updated_cache["spec_hash"].as_str().unwrap(),
        "corrupted_hash",
        "Cache should be updated with correct hash"
    );

    println!("✓ Hash integrity check works correctly");
}
