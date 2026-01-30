//! High-performance OpenAPI parser service
//!
//! Optimizations:
//! - Parallel parsing with rayon
//! - Global HTTP client with connection pooling
//! - Single-pass reference extraction
//! - Zero-copy where possible

use crate::types::*;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// Global HTTP client for connection pooling
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to create HTTP client")
});

/// HTTP cache headers extracted from response
#[derive(Debug, Default)]
pub struct HttpHeaders {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Parse result with refs extracted in single pass
struct ParsedOperation {
    endpoint: Endpoint,
}

/// Schema parse result
struct ParsedSchema {
    name: String,
    schema: Schema,
}

/// OpenAPI parser service (high-performance)
pub struct OpenApiParser;

impl OpenApiParser {
    /// Parse OpenAPI spec from a source (URL or file path)
    pub async fn parse(source: &str) -> OasResult<ParsedSpec> {
        let (spec, _headers) = Self::parse_with_headers(source).await?;
        Ok(spec)
    }

    /// Parse OpenAPI spec and return HTTP headers (for caching)
    pub async fn parse_with_headers(source: &str) -> OasResult<(ParsedSpec, HttpHeaders)> {
        let (content, headers) = Self::fetch_content(source).await?;
        let spec = Self::parse_content(&content, source)?;
        Ok((spec, headers))
    }

    /// Fetch content from URL or file
    async fn fetch_content(source: &str) -> OasResult<(String, HttpHeaders)> {
        if source.starts_with("http://") || source.starts_with("https://") {
            Self::fetch_remote(source).await
        } else {
            let content = Self::read_local(source)?;
            Ok((content, HttpHeaders::default()))
        }
    }

    /// Fetch from remote URL using global client
    async fn fetch_remote(url: &str) -> OasResult<(String, HttpHeaders)> {
        let response = HTTP_CLIENT
            .get(url)
            .send()
            .await
            .map_err(|e| OasError::ConnectionFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(OasError::HttpError {
                status: response.status().as_u16(),
                message: response.status().to_string(),
            });
        }

        // Extract cache headers
        let headers = HttpHeaders {
            etag: response
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            last_modified: response
                .headers()
                .get("last-modified")
                .and_then(|v| v.to_str().ok())
                .map(String::from),
        };

        let content = response
            .text()
            .await
            .map_err(|e| OasError::ConnectionFailed(e.to_string()))?;

        Ok((content, headers))
    }

    /// Read from local file
    fn read_local(path: &str) -> OasResult<String> {
        let path = Path::new(path);

        // Security: prevent path traversal
        if path.to_string_lossy().contains("..") {
            return Err(OasError::PathTraversal(path.display().to_string()));
        }

        let canonical = path.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => OasError::FileNotFound(path.display().to_string()),
            _ => {
                OasError::PathTraversal(format!("Cannot resolve path: {} ({})", path.display(), e))
            }
        })?;

        let canonical_str = canonical.to_string_lossy();
        if canonical_str.contains("..") {
            return Err(OasError::PathTraversal(canonical.display().to_string()));
        }

        std::fs::read_to_string(&canonical).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => OasError::FileNotFound(path.display().to_string()),
            std::io::ErrorKind::PermissionDenied => {
                OasError::PermissionDenied(path.display().to_string())
            }
            _ => OasError::ReadError(e.to_string()),
        })
    }

    /// Parse content as JSON or YAML (public for cache reuse)
    pub fn parse_content(content: &str, source: &str) -> OasResult<ParsedSpec> {
        // Try JSON first (faster), then YAML
        let value: serde_json::Value = if content.trim().starts_with('{') {
            serde_json::from_str(content).map_err(|e| OasError::InvalidJson(e.to_string()))?
        } else {
            serde_yaml::from_str(content).map_err(|e| OasError::InvalidYaml(e.to_string()))?
        };

        // Detect OpenAPI version
        let version = Self::detect_version(&value)?;

        // Parse based on version
        match version {
            OpenApiVersion::Swagger2 => Self::parse_swagger2(value, source),
            OpenApiVersion::OpenApi30 | OpenApiVersion::OpenApi31 => {
                Self::parse_openapi3(value, source, version)
            }
        }
    }

    /// Detect OpenAPI version from spec
    fn detect_version(value: &serde_json::Value) -> OasResult<OpenApiVersion> {
        if let Some(swagger) = value.get("swagger").and_then(|v| v.as_str())
            && swagger.starts_with("2.")
        {
            return Ok(OpenApiVersion::Swagger2);
        }

        if let Some(openapi) = value.get("openapi").and_then(|v| v.as_str()) {
            if openapi.starts_with("3.0") {
                return Ok(OpenApiVersion::OpenApi30);
            }
            if openapi.starts_with("3.1") {
                return Ok(OpenApiVersion::OpenApi31);
            }
            return Err(OasError::UnsupportedVersion(openapi.to_string()));
        }

        Err(OasError::InvalidOpenApi(
            "Missing 'openapi' or 'swagger' field".to_string(),
        ))
    }

    /// Parse Swagger 2.0 spec
    fn parse_swagger2(value: serde_json::Value, source: &str) -> OasResult<ParsedSpec> {
        let info = value
            .get("info")
            .ok_or_else(|| OasError::InvalidOpenApi("Missing 'info' field".to_string()))?;

        let title = info
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown API")
            .to_string();

        let version = info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();

        let description = info
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse definitions (parallel)
        let schemas = Self::parse_swagger2_definitions_parallel(&value);

        // Parse paths (parallel)
        let endpoints = Self::parse_swagger2_paths_parallel(&value);

        // Collect tags
        let tags: Vec<String> = endpoints
            .values()
            .flat_map(|e| e.tags.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Compute spec hash
        let spec_hash = Self::compute_hash(&value);

        Ok(ParsedSpec {
            metadata: SpecMetadata {
                title,
                version,
                description,
                openapi_version: OpenApiVersion::Swagger2,
                endpoint_count: endpoints.len(),
                schema_count: schemas.len(),
                tag_count: tags.len(),
            },
            endpoints,
            schemas,
            tags,
            spec_hash,
            source: source.to_string(),
        })
    }

    /// Parse Swagger 2.0 definitions in parallel
    fn parse_swagger2_definitions_parallel(value: &serde_json::Value) -> HashMap<String, Schema> {
        if let Some(definitions) = value.get("definitions").and_then(|v| v.as_object()) {
            // Convert to Vec for parallel iteration (serde_json::Map doesn't implement par_iter)
            let items: Vec<_> = definitions.iter().collect();
            let parsed: Vec<ParsedSchema> = items
                .par_iter()
                .map(|(name, def)| {
                    let (refs, hash) = Self::extract_refs_and_hash(def);
                    ParsedSchema {
                        name: (*name).clone(),
                        schema: Schema {
                            name: (*name).clone(),
                            schema_type: Self::parse_schema_type(def),
                            description: def
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            refs,
                            hash,
                        },
                    }
                })
                .collect();

            parsed.into_iter().map(|p| (p.name, p.schema)).collect()
        } else {
            HashMap::new()
        }
    }

    /// Parse Swagger 2.0 paths in parallel
    fn parse_swagger2_paths_parallel(value: &serde_json::Value) -> HashMap<String, Endpoint> {
        if let Some(paths) = value.get("paths").and_then(|v| v.as_object()) {
            // Collect all operations first
            let operations: Vec<(&String, &str, &serde_json::Value)> = paths
                .iter()
                .flat_map(|(path, path_item)| {
                    path_item
                        .as_object()
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(method, op)| {
                                    Self::parse_http_method(method)
                                        .map(|_| (path, method.as_str(), op))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();

            // Parse in parallel
            let parsed: Vec<ParsedOperation> = operations
                .par_iter()
                .filter_map(|(path, method, operation)| {
                    Self::parse_http_method(method).map(|http_method| ParsedOperation {
                        endpoint: Self::parse_swagger2_operation_optimized(
                            path,
                            http_method,
                            operation,
                        ),
                    })
                })
                .collect();

            parsed
                .into_iter()
                .map(|p| (p.endpoint.key(), p.endpoint))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Parse a single Swagger 2.0 operation (optimized)
    fn parse_swagger2_operation_optimized(
        path: &str,
        method: HttpMethod,
        operation: &serde_json::Value,
    ) -> Endpoint {
        let operation_id = operation
            .get("operationId")
            .and_then(|v| v.as_str())
            .map(String::from);

        let summary = operation
            .get("summary")
            .and_then(|v| v.as_str())
            .map(String::from);

        let description = operation
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let tags: Vec<String> = operation
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let deprecated = operation
            .get("deprecated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let parameters = Self::parse_swagger2_parameters(operation);
        let request_body = Self::parse_swagger2_body(operation);
        let responses = Self::parse_swagger2_responses(operation);

        // Single pass: extract refs and compute hash together
        let (schema_refs, hash) = Self::extract_refs_and_hash(operation);

        Endpoint {
            path: path.to_string(),
            method,
            operation_id,
            summary,
            description,
            tags,
            parameters,
            request_body,
            responses,
            deprecated,
            hash,
            schema_refs,
        }
    }

    /// Parse Swagger 2.0 parameters
    fn parse_swagger2_parameters(operation: &serde_json::Value) -> Vec<Parameter> {
        let mut params = Vec::new();

        if let Some(parameters) = operation.get("parameters").and_then(|v| v.as_array()) {
            for param in parameters {
                let in_value = param.get("in").and_then(|v| v.as_str()).unwrap_or("");

                if in_value == "body" {
                    continue;
                }

                let location = match in_value {
                    "path" => ParameterLocation::Path,
                    "query" => ParameterLocation::Query,
                    "header" => ParameterLocation::Header,
                    _ => continue,
                };

                params.push(Parameter {
                    name: param
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    location,
                    required: param
                        .get("required")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(location == ParameterLocation::Path),
                    description: param
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    schema_ref: None,
                    schema_type: param.get("type").and_then(|v| v.as_str()).map(String::from),
                });
            }
        }

        params
    }

    /// Parse Swagger 2.0 body parameter as request body
    fn parse_swagger2_body(operation: &serde_json::Value) -> Option<RequestBody> {
        if let Some(parameters) = operation.get("parameters").and_then(|v| v.as_array()) {
            for param in parameters {
                if param.get("in").and_then(|v| v.as_str()) == Some("body") {
                    let schema_ref = param
                        .get("schema")
                        .and_then(|s| s.get("$ref"))
                        .and_then(|v| v.as_str())
                        .map(|r| r.replace("#/definitions/", ""));

                    return Some(RequestBody {
                        required: param
                            .get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        description: param
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        content_types: operation
                            .get("consumes")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_else(|| vec!["application/json".to_string()]),
                        schema_ref,
                    });
                }
            }
        }
        None
    }

    /// Parse Swagger 2.0 responses
    fn parse_swagger2_responses(operation: &serde_json::Value) -> HashMap<String, Response> {
        let mut responses = HashMap::new();

        if let Some(resp_obj) = operation.get("responses").and_then(|v| v.as_object()) {
            for (status, resp) in resp_obj {
                let schema_ref = resp
                    .get("schema")
                    .and_then(|s| s.get("$ref"))
                    .and_then(|v| v.as_str())
                    .map(|r| r.replace("#/definitions/", ""));

                responses.insert(
                    status.clone(),
                    Response {
                        status_code: status.clone(),
                        description: resp
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        content_types: operation
                            .get("produces")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_else(|| vec!["application/json".to_string()]),
                        schema_ref,
                    },
                );
            }
        }

        responses
    }

    /// Parse OpenAPI 3.x spec (optimized)
    fn parse_openapi3(
        value: serde_json::Value,
        source: &str,
        version: OpenApiVersion,
    ) -> OasResult<ParsedSpec> {
        let info = value
            .get("info")
            .ok_or_else(|| OasError::InvalidOpenApi("Missing 'info' field".to_string()))?;

        let title = info
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown API")
            .to_string();

        let spec_version = info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();

        let description = info
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse schemas in parallel
        let schemas = Self::parse_openapi3_schemas_parallel(&value);

        // Parse paths in parallel
        let endpoints = Self::parse_openapi3_paths_parallel(&value);

        // Collect tags
        let tags: Vec<String> = endpoints
            .values()
            .flat_map(|e| e.tags.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Compute spec hash
        let spec_hash = Self::compute_hash(&value);

        Ok(ParsedSpec {
            metadata: SpecMetadata {
                title,
                version: spec_version,
                description,
                openapi_version: version,
                endpoint_count: endpoints.len(),
                schema_count: schemas.len(),
                tag_count: tags.len(),
            },
            endpoints,
            schemas,
            tags,
            spec_hash,
            source: source.to_string(),
        })
    }

    /// Parse OpenAPI 3.x schemas in parallel
    fn parse_openapi3_schemas_parallel(value: &serde_json::Value) -> HashMap<String, Schema> {
        if let Some(components) = value.get("components")
            && let Some(schema_obj) = components.get("schemas").and_then(|v| v.as_object())
        {
            // Convert to Vec for parallel iteration
            let items: Vec<_> = schema_obj.iter().collect();
            let parsed: Vec<ParsedSchema> = items
                .par_iter()
                .map(|(name, def)| {
                    let (refs, hash) = Self::extract_refs_and_hash(def);
                    ParsedSchema {
                        name: (*name).clone(),
                        schema: Schema {
                            name: (*name).clone(),
                            schema_type: Self::parse_schema_type(def),
                            description: def
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            refs,
                            hash,
                        },
                    }
                })
                .collect();

            return parsed.into_iter().map(|p| (p.name, p.schema)).collect();
        }
        HashMap::new()
    }

    /// Parse OpenAPI 3.x paths in parallel
    fn parse_openapi3_paths_parallel(value: &serde_json::Value) -> HashMap<String, Endpoint> {
        if let Some(paths) = value.get("paths").and_then(|v| v.as_object()) {
            // Collect all operations first
            let operations: Vec<(&String, &str, &serde_json::Value)> = paths
                .iter()
                .flat_map(|(path, path_item)| {
                    path_item
                        .as_object()
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(method, op)| {
                                    Self::parse_http_method(method)
                                        .map(|_| (path, method.as_str(), op))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();

            // Parse in parallel
            let parsed: Vec<ParsedOperation> = operations
                .par_iter()
                .filter_map(|(path, method, operation)| {
                    Self::parse_http_method(method).map(|http_method| ParsedOperation {
                        endpoint: Self::parse_openapi3_operation_optimized(
                            path,
                            http_method,
                            operation,
                        ),
                    })
                })
                .collect();

            parsed
                .into_iter()
                .map(|p| (p.endpoint.key(), p.endpoint))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Parse a single OpenAPI 3.x operation (optimized)
    fn parse_openapi3_operation_optimized(
        path: &str,
        method: HttpMethod,
        operation: &serde_json::Value,
    ) -> Endpoint {
        let operation_id = operation
            .get("operationId")
            .and_then(|v| v.as_str())
            .map(String::from);

        let summary = operation
            .get("summary")
            .and_then(|v| v.as_str())
            .map(String::from);

        let description = operation
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let tags: Vec<String> = operation
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let deprecated = operation
            .get("deprecated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let parameters = Self::parse_openapi3_parameters(operation);
        let request_body = Self::parse_openapi3_body(operation);
        let responses = Self::parse_openapi3_responses(operation);

        // Single pass: extract refs and compute hash together
        let (schema_refs, hash) = Self::extract_refs_and_hash(operation);

        Endpoint {
            path: path.to_string(),
            method,
            operation_id,
            summary,
            description,
            tags,
            parameters,
            request_body,
            responses,
            deprecated,
            hash,
            schema_refs,
        }
    }

    /// Parse OpenAPI 3.x parameters
    fn parse_openapi3_parameters(operation: &serde_json::Value) -> Vec<Parameter> {
        let mut params = Vec::new();

        if let Some(parameters) = operation.get("parameters").and_then(|v| v.as_array()) {
            for param in parameters {
                let in_value = param.get("in").and_then(|v| v.as_str()).unwrap_or("");

                let location = match in_value {
                    "path" => ParameterLocation::Path,
                    "query" => ParameterLocation::Query,
                    "header" => ParameterLocation::Header,
                    "cookie" => ParameterLocation::Cookie,
                    _ => continue,
                };

                let schema_ref = param
                    .get("schema")
                    .and_then(|s| s.get("$ref"))
                    .and_then(|v| v.as_str())
                    .map(|r| r.replace("#/components/schemas/", ""));

                let schema_type = param
                    .get("schema")
                    .and_then(|s| s.get("type"))
                    .and_then(|v| v.as_str())
                    .map(String::from);

                params.push(Parameter {
                    name: param
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    location,
                    required: param
                        .get("required")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(location == ParameterLocation::Path),
                    description: param
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    schema_ref,
                    schema_type,
                });
            }
        }

        params
    }

    /// Parse OpenAPI 3.x request body
    fn parse_openapi3_body(operation: &serde_json::Value) -> Option<RequestBody> {
        let body = operation.get("requestBody")?;
        let content = body.get("content").and_then(|v| v.as_object())?;

        let content_types: Vec<String> = content.keys().cloned().collect();

        let schema_ref = content
            .values()
            .next()
            .and_then(|c| c.get("schema"))
            .and_then(|s| s.get("$ref"))
            .and_then(|v| v.as_str())
            .map(|r| r.replace("#/components/schemas/", ""));

        Some(RequestBody {
            required: body
                .get("required")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            description: body
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            content_types,
            schema_ref,
        })
    }

    /// Parse OpenAPI 3.x responses
    fn parse_openapi3_responses(operation: &serde_json::Value) -> HashMap<String, Response> {
        let mut responses = HashMap::new();

        if let Some(resp_obj) = operation.get("responses").and_then(|v| v.as_object()) {
            for (status, resp) in resp_obj {
                let (content_types, schema_ref) =
                    if let Some(content) = resp.get("content").and_then(|v| v.as_object()) {
                        let types: Vec<String> = content.keys().cloned().collect();
                        let schema = content
                            .values()
                            .next()
                            .and_then(|c| c.get("schema"))
                            .and_then(|s| s.get("$ref"))
                            .and_then(|v| v.as_str())
                            .map(|r| r.replace("#/components/schemas/", ""));
                        (types, schema)
                    } else {
                        (vec![], None)
                    };

                responses.insert(
                    status.clone(),
                    Response {
                        status_code: status.clone(),
                        description: resp
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        content_types,
                        schema_ref,
                    },
                );
            }
        }

        responses
    }

    /// Parse HTTP method string
    fn parse_http_method(method: &str) -> Option<HttpMethod> {
        match method.to_lowercase().as_str() {
            "get" => Some(HttpMethod::Get),
            "post" => Some(HttpMethod::Post),
            "put" => Some(HttpMethod::Put),
            "patch" => Some(HttpMethod::Patch),
            "delete" => Some(HttpMethod::Delete),
            "head" => Some(HttpMethod::Head),
            "options" => Some(HttpMethod::Options),
            "trace" => Some(HttpMethod::Trace),
            _ => None,
        }
    }

    /// Parse schema type
    fn parse_schema_type(schema: &serde_json::Value) -> SchemaType {
        if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
            return SchemaType::Ref {
                reference: ref_str
                    .replace("#/definitions/", "")
                    .replace("#/components/schemas/", ""),
            };
        }

        if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
            return SchemaType::OneOf {
                variants: one_of.iter().map(Self::parse_schema_type).collect(),
            };
        }

        if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
            return SchemaType::AnyOf {
                variants: any_of.iter().map(Self::parse_schema_type).collect(),
            };
        }

        if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
            return SchemaType::AllOf {
                variants: all_of.iter().map(Self::parse_schema_type).collect(),
            };
        }

        match schema.get("type").and_then(|v| v.as_str()) {
            Some("string") => SchemaType::String {
                format: schema
                    .get("format")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                enum_values: schema.get("enum").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            },
            Some("number") => SchemaType::Number {
                format: schema
                    .get("format")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
            Some("integer") => SchemaType::Integer {
                format: schema
                    .get("format")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
            Some("boolean") => SchemaType::Boolean,
            Some("array") => {
                let items = schema
                    .get("items")
                    .map(Self::parse_schema_type)
                    .unwrap_or(SchemaType::Unknown);
                SchemaType::Array {
                    items: Box::new(items),
                }
            }
            Some("object") | None if schema.get("properties").is_some() => {
                let properties = schema
                    .get("properties")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), Self::parse_schema_type(v)))
                            .collect()
                    })
                    .unwrap_or_default();

                let required = schema
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                SchemaType::Object {
                    properties,
                    required,
                }
            }
            _ => SchemaType::Unknown,
        }
    }

    /// Extract all $ref references AND compute hash in a single pass
    /// This is the key optimization - avoid walking the tree twice
    fn extract_refs_and_hash(value: &serde_json::Value) -> (Vec<String>, String) {
        let mut refs = Vec::new();
        let mut hasher = Sha256::new();

        Self::collect_refs_and_hash(value, &mut refs, &mut hasher);

        refs.sort();
        refs.dedup();

        let result = hasher.finalize();
        let hash = hex::encode(&result[..8]);

        (refs, hash)
    }

    /// Recursive helper: collect refs and update hash in single traversal
    fn collect_refs_and_hash(
        value: &serde_json::Value,
        refs: &mut Vec<String>,
        hasher: &mut Sha256,
    ) {
        match value {
            serde_json::Value::Object(obj) => {
                // Update hash with object structure
                hasher.update(b"{");

                // Sort keys for deterministic hash
                let mut keys: Vec<_> = obj.keys().collect();
                keys.sort();

                for key in keys {
                    hasher.update(key.as_bytes());
                    hasher.update(b":");

                    let val = &obj[key];

                    // Check for $ref
                    if key == "$ref"
                        && let Some(ref_str) = val.as_str()
                    {
                        let clean_ref = ref_str
                            .replace("#/definitions/", "")
                            .replace("#/components/schemas/", "");
                        refs.push(clean_ref);
                    }

                    Self::collect_refs_and_hash(val, refs, hasher);
                }

                hasher.update(b"}");
            }
            serde_json::Value::Array(arr) => {
                hasher.update(b"[");
                for v in arr {
                    Self::collect_refs_and_hash(v, refs, hasher);
                }
                hasher.update(b"]");
            }
            serde_json::Value::String(s) => {
                hasher.update(b"\"");
                hasher.update(s.as_bytes());
                hasher.update(b"\"");
            }
            serde_json::Value::Number(n) => {
                hasher.update(n.to_string().as_bytes());
            }
            serde_json::Value::Bool(b) => {
                hasher.update(if *b {
                    "true".as_bytes()
                } else {
                    "false".as_bytes()
                });
            }
            serde_json::Value::Null => {
                hasher.update(b"null");
            }
        }
    }

    /// Compute SHA256 hash of JSON value
    pub fn compute_hash(value: &serde_json::Value) -> String {
        let normalized = serde_json::to_string(value).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_refs() {
        let json = serde_json::json!({
            "schema": {
                "$ref": "#/components/schemas/User"
            },
            "items": {
                "$ref": "#/components/schemas/Post"
            }
        });

        let (refs, _hash) = OpenApiParser::extract_refs_and_hash(&json);
        assert!(refs.contains(&"User".to_string()));
        assert!(refs.contains(&"Post".to_string()));
    }

    #[test]
    fn test_extract_refs_and_hash() {
        let json = serde_json::json!({
            "schema": {
                "$ref": "#/components/schemas/User"
            }
        });

        let (refs, hash) = OpenApiParser::extract_refs_and_hash(&json);
        assert!(refs.contains(&"User".to_string()));
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_parallel_parsing() {
        // Verify rayon is working
        let items: Vec<i32> = (0..100).collect();
        let sum: i32 = items.par_iter().map(|x| x * 2).sum();
        assert_eq!(sum, 9900);
    }
}
