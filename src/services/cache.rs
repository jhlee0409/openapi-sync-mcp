//! Cache management service

use crate::types::*;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Default TTL in seconds (24 hours)
/// API specs rarely change frequently, so a longer TTL is reasonable
pub const DEFAULT_TTL_SECONDS: u64 = 86400;

/// Cache manager for OpenAPI specs
pub struct CacheManager {
    project_dir: String,
}

impl CacheManager {
    pub fn new(project_dir: &str) -> Self {
        Self {
            project_dir: project_dir.to_string(),
        }
    }

    /// Get cache file path
    fn cache_path(&self) -> std::path::PathBuf {
        Path::new(&self.project_dir).join(".openapi-sync.cache.json")
    }

    /// Get state file path
    #[allow(dead_code)]
    fn state_path(&self) -> std::path::PathBuf {
        Path::new(&self.project_dir).join(".openapi-sync.state.json")
    }

    /// Load cache from file
    pub fn load_cache(&self) -> OasResult<OasCache> {
        let path = self.cache_path();
        let content = std::fs::read_to_string(&path).map_err(|_| OasError::CacheNotFound)?;

        serde_json::from_str(&content).map_err(|e| OasError::CacheCorrupted(e.to_string()))
    }

    /// Save cache to file
    pub fn save_cache(&self, cache: &OasCache) -> OasResult<()> {
        let path = self.cache_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(cache)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        // Atomic write using temp file
        let temp_path = path.with_extension("json.tmp");
        std::fs::write(&temp_path, &content)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        std::fs::rename(&temp_path, &path)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Load state from file
    #[allow(dead_code)]
    pub fn load_state(&self) -> OasResult<OasState> {
        let path = self.state_path();
        let content = std::fs::read_to_string(&path).map_err(|_| OasError::CacheNotFound)?;

        serde_json::from_str(&content).map_err(|e| OasError::CacheCorrupted(e.to_string()))
    }

    /// Save state to file
    #[allow(dead_code)]
    pub fn save_state(&self, state: &OasState) -> OasResult<()> {
        let path = self.state_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(state)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        // Atomic write
        let temp_path = path.with_extension("json.tmp");
        std::fs::write(&temp_path, &content)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        std::fs::rename(&temp_path, &path)
            .map_err(|e| OasError::CacheWriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Create cache from parsed spec with HTTP headers
    pub fn create_cache(
        &self,
        spec: &ParsedSpec,
        source: &str,
        ttl_seconds: Option<u64>,
        http_headers: Option<&super::parser::HttpHeaders>,
    ) -> OasCache {
        // For local files, extract mtime for cache validation
        let local_cache = if !source.starts_with("http") {
            if let Ok(metadata) = std::fs::metadata(source) {
                if let Ok(modified) = metadata.modified() {
                    LocalCacheInfo {
                        mtime: Some(DateTime::<Utc>::from(modified).to_rfc3339()),
                    }
                } else {
                    LocalCacheInfo::default()
                }
            } else {
                LocalCacheInfo::default()
            }
        } else {
            LocalCacheInfo::default()
        };

        OasCache {
            version: "1.0.0".to_string(),
            schema_version: crate::types::CACHE_SCHEMA_VERSION,
            last_fetch: Utc::now().to_rfc3339(),
            spec_hash: spec.spec_hash.clone(),
            source: source.to_string(),
            ttl_seconds: ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS),
            http_cache: HttpCacheInfo {
                etag: http_headers.and_then(|h| h.etag.clone()),
                last_modified: http_headers.and_then(|h| h.last_modified.clone()),
            },
            local_cache,
            meta: CachedMeta {
                title: Some(spec.metadata.title.clone()),
                version: Some(spec.metadata.version.clone()),
                openapi_version: Some(
                    serde_json::to_string(&spec.metadata.openapi_version)
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_string(),
                ),
                endpoint_count: spec.metadata.endpoint_count,
                schema_count: spec.metadata.schema_count,
            },
            parsed_spec: Some(spec.clone()),
        }
    }

    /// Parse spec with caching support - returns cached spec if valid, otherwise fetches fresh
    ///
    /// Cache validation order:
    /// 1. Schema version check (invalidate if ParsedSpec structure changed)
    /// 2. Source match check
    /// 3. TTL + mtime/ETag validation
    /// 4. Return parsed_spec if available (zero parsing!)
    /// 5. Graceful fallback: any failure → fresh fetch
    pub async fn parse_with_cache(
        &self,
        source: &str,
        ttl_seconds: Option<u64>,
    ) -> OasResult<ParsedSpec> {
        // Try to use cache with graceful fallback
        if let Ok(cache) = self.load_cache() {
            // Check schema version compatibility
            if cache.schema_version != crate::types::CACHE_SCHEMA_VERSION {
                // Schema changed - cache is incompatible, fetch fresh
            } else if cache.source == source {
                // Validate cache (TTL + mtime/ETag)
                let is_valid = if source.starts_with("http") {
                    self.check_remote_cache(source, &cache).await
                } else {
                    self.check_local_cache(source, &cache)
                };

                if is_valid {
                    // Use pre-parsed spec (zero parsing!)
                    if let Some(parsed_spec) = cache.parsed_spec {
                        // Verify hash matches for data integrity
                        if parsed_spec.spec_hash == cache.spec_hash {
                            return Ok(parsed_spec);
                        }
                        // Hash mismatch - cache corrupted, fetch fresh
                    }
                    // No parsed_spec or corrupted - fetch fresh
                }
            }
        }

        // Cache miss, invalid, or incompatible - fetch fresh
        let (spec, headers) = self.fetch_and_parse(source).await?;

        // Save to cache
        let cache = self.create_cache(&spec, source, ttl_seconds, Some(&headers));
        let _ = self.save_cache(&cache);

        Ok(spec)
    }

    /// Fetch content and parse spec (internal helper)
    async fn fetch_and_parse(
        &self,
        source: &str,
    ) -> OasResult<(ParsedSpec, super::parser::HttpHeaders)> {
        if source.starts_with("http://") || source.starts_with("https://") {
            // Remote fetch
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| OasError::ConnectionFailed(e.to_string()))?;

            let response = client
                .get(source)
                .send()
                .await
                .map_err(|e| OasError::ConnectionFailed(e.to_string()))?;

            if !response.status().is_success() {
                return Err(OasError::HttpError {
                    status: response.status().as_u16(),
                    message: response.status().to_string(),
                });
            }

            let headers = super::parser::HttpHeaders {
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

            let spec = super::parser::OpenApiParser::parse_content(&content, source)?;
            Ok((spec, headers))
        } else {
            // Local file read
            let content =
                std::fs::read_to_string(source).map_err(|e| OasError::ReadError(e.to_string()))?;
            let spec = super::parser::OpenApiParser::parse_content(&content, source)?;
            Ok((spec, super::parser::HttpHeaders::default()))
        }
    }

    /// Check if cache has expired based on TTL
    pub fn is_cache_expired(&self, cache: &OasCache) -> bool {
        let last_fetch = match DateTime::parse_from_rfc3339(&cache.last_fetch) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => return true, // If we can't parse, assume expired
        };

        let now = Utc::now();
        let elapsed = now.signed_duration_since(last_fetch);

        elapsed.num_seconds() > cache.ttl_seconds as i64
    }

    /// Check if cache is valid for a URL (using HEAD request + TTL)
    pub async fn check_remote_cache(&self, url: &str, cache: &OasCache) -> bool {
        // First check TTL - if expired, don't even bother with HTTP check
        if self.is_cache_expired(cache) {
            return false;
        }

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        let response = match client.head(url).send().await {
            Ok(r) => r,
            Err(_) => {
                // Network error - use cache if within TTL (already checked above)
                return true;
            }
        };

        // Check ETag
        if let Some(etag) = response.headers().get("etag")
            && let Ok(etag_str) = etag.to_str()
            && let Some(cached_etag) = &cache.http_cache.etag
        {
            return etag_str == cached_etag;
        }

        // Check Last-Modified
        if let Some(last_modified) = response.headers().get("last-modified")
            && let Ok(lm_str) = last_modified.to_str()
            && let Some(cached_lm) = &cache.http_cache.last_modified
        {
            return lm_str == cached_lm;
        }

        // No cache headers - fall back to TTL only (already passed TTL check above)
        true
    }

    /// Check if local file cache is valid (mtime + TTL)
    pub fn check_local_cache(&self, path: &str, cache: &OasCache) -> bool {
        // First check TTL
        if self.is_cache_expired(cache) {
            return false;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };

        if let Ok(modified) = metadata.modified() {
            let mtime = chrono::DateTime::<Utc>::from(modified).to_rfc3339();
            if let Some(cached_mtime) = &cache.local_cache.mtime {
                return &mtime == cached_mtime;
            }
        }

        false
    }
}
