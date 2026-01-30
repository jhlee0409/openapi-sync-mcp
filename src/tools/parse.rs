//! oas_parse tool implementation

use crate::services::{CacheManager, GraphBuilder, OpenApiParser};
use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ParseInput {
    /// URL or file path to OpenAPI spec
    pub source: String,
    /// Output format
    #[serde(default)]
    pub format: ParseFormat,
    /// Project directory for caching
    pub project_dir: Option<String>,
    /// Whether to use cache
    #[serde(default)]
    pub use_cache: bool,
    /// Cache TTL in seconds (default: 86400 = 24 hours)
    pub ttl_seconds: Option<u64>,
    /// Limit number of results (for pagination)
    pub limit: Option<usize>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
    /// Filter by tag
    pub tag: Option<String>,
    /// Filter by path prefix
    pub path_prefix: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParseFormat {
    /// Just metadata and stats - minimal output
    #[default]
    Summary,
    /// List endpoint keys only (for discovery)
    EndpointsList,
    /// List schema names only (for discovery)
    SchemasList,
    /// Endpoint details (paginated)
    Endpoints,
    /// Schema details (paginated)
    Schemas,
    /// Full output (WARNING: can be large)
    Full,
}

#[derive(Debug, Serialize)]
pub struct ParseOutput {
    pub success: bool,
    pub metadata: Option<SpecMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Vec<EndpointSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schemas: Option<Vec<SchemaSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_stats: Option<GraphStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct EndpointSummary {
    pub key: String,
    pub path: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub schema_refs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SchemaSummary {
    pub name: String,
    pub refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parse an OpenAPI spec
pub async fn parse_spec(input: ParseInput) -> ParseOutput {
    // Parse spec (with caching if enabled)
    let spec = if let (true, Some(project_dir)) = (input.use_cache, input.project_dir.as_ref()) {
        let cache_manager = CacheManager::new(project_dir);
        match cache_manager
            .parse_with_cache(&input.source, input.ttl_seconds)
            .await
        {
            Ok(spec) => spec,
            Err(e) => {
                return ParseOutput {
                    success: false,
                    metadata: None,
                    endpoints: None,
                    endpoint_keys: None,
                    schemas: None,
                    schema_names: None,
                    graph_stats: None,
                    pagination: None,
                    error: Some(e.to_string()),
                };
            }
        }
    } else {
        // No caching, parse directly
        match OpenApiParser::parse(&input.source).await {
            Ok(spec) => spec,
            Err(e) => {
                return ParseOutput {
                    success: false,
                    metadata: None,
                    endpoints: None,
                    endpoint_keys: None,
                    schemas: None,
                    schema_names: None,
                    graph_stats: None,
                    pagination: None,
                    error: Some(e.to_string()),
                };
            }
        }
    };

    // Build dependency graph
    let graph = GraphBuilder::build(&spec);

    // Default limit for paginated outputs
    let limit = input.limit.unwrap_or(50);
    let offset = input.offset;

    // Filter endpoints by tag/path
    let filtered_endpoints: Vec<_> = spec
        .endpoints
        .values()
        .filter(|e| {
            if let Some(ref tag) = input.tag {
                if !e.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                    return false;
                }
            }
            if let Some(ref prefix) = input.path_prefix {
                if !e.path.starts_with(prefix) {
                    return false;
                }
            }
            true
        })
        .collect();

    // Format output based on requested format
    match input.format {
        ParseFormat::Summary => ParseOutput {
            success: true,
            metadata: Some(spec.metadata),
            endpoints: None,
            endpoint_keys: None,
            schemas: None,
            schema_names: None,
            graph_stats: Some(graph.stats()),
            pagination: None,
            error: None,
        },

        ParseFormat::EndpointsList => {
            let keys: Vec<String> = filtered_endpoints.iter().map(|e| e.key()).collect();
            ParseOutput {
                success: true,
                metadata: Some(spec.metadata),
                endpoints: None,
                endpoint_keys: Some(keys),
                schemas: None,
                schema_names: None,
                graph_stats: Some(graph.stats()),
                pagination: None,
                error: None,
            }
        }

        ParseFormat::SchemasList => {
            let names: Vec<String> = spec.schemas.keys().cloned().collect();
            ParseOutput {
                success: true,
                metadata: Some(spec.metadata),
                endpoints: None,
                endpoint_keys: None,
                schemas: None,
                schema_names: Some(names),
                graph_stats: Some(graph.stats()),
                pagination: None,
                error: None,
            }
        }

        ParseFormat::Endpoints => {
            let total = filtered_endpoints.len();
            let paginated: Vec<_> = filtered_endpoints
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|e| EndpointSummary {
                    key: e.key(),
                    path: e.path.clone(),
                    method: e.method.to_string(),
                    operation_id: e.operation_id.clone(),
                    tags: e.tags.clone(),
                    deprecated: e.deprecated,
                    schema_refs: e.schema_refs.clone(),
                })
                .collect();

            ParseOutput {
                success: true,
                metadata: Some(spec.metadata),
                endpoints: Some(paginated),
                endpoint_keys: None,
                schemas: None,
                schema_names: None,
                graph_stats: Some(graph.stats()),
                pagination: Some(PaginationInfo {
                    total,
                    offset,
                    limit,
                    has_more: offset + limit < total,
                }),
                error: None,
            }
        }

        ParseFormat::Schemas => {
            let all_schemas: Vec<_> = spec.schemas.values().collect();
            let total = all_schemas.len();
            let paginated: Vec<_> = all_schemas
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|s| SchemaSummary {
                    name: s.name.clone(),
                    refs: s.refs.clone(),
                    description: s.description.clone(),
                })
                .collect();

            ParseOutput {
                success: true,
                metadata: Some(spec.metadata),
                endpoints: None,
                endpoint_keys: None,
                schemas: Some(paginated),
                schema_names: None,
                graph_stats: Some(graph.stats()),
                pagination: Some(PaginationInfo {
                    total,
                    offset,
                    limit,
                    has_more: offset + limit < total,
                }),
                error: None,
            }
        }

        ParseFormat::Full => {
            // Warning: can be very large! Apply pagination anyway
            let total_endpoints = filtered_endpoints.len();
            let total_schemas = spec.schemas.len();

            let paginated_endpoints: Vec<_> = filtered_endpoints
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|e| EndpointSummary {
                    key: e.key(),
                    path: e.path.clone(),
                    method: e.method.to_string(),
                    operation_id: e.operation_id.clone(),
                    tags: e.tags.clone(),
                    deprecated: e.deprecated,
                    schema_refs: e.schema_refs.clone(),
                })
                .collect();

            let paginated_schemas: Vec<_> = spec
                .schemas
                .values()
                .skip(offset)
                .take(limit)
                .map(|s| SchemaSummary {
                    name: s.name.clone(),
                    refs: s.refs.clone(),
                    description: s.description.clone(),
                })
                .collect();

            ParseOutput {
                success: true,
                metadata: Some(spec.metadata),
                endpoints: Some(paginated_endpoints),
                endpoint_keys: None,
                schemas: Some(paginated_schemas),
                schema_names: None,
                graph_stats: Some(graph.stats()),
                pagination: Some(PaginationInfo {
                    total: total_endpoints.max(total_schemas),
                    offset,
                    limit,
                    has_more: offset + limit < total_endpoints || offset + limit < total_schemas,
                }),
                error: None,
            }
        }
    }
}
