# OpenAPI Sync MCP

A high-performance MCP (Model Context Protocol) server for parsing, validating, and generating code from OpenAPI specifications.

Built with Rust for speed and reliability.

## Features

- **Parse OpenAPI specs** - Support for OpenAPI 3.x and Swagger 2.0
- **Dependency tracking** - Find affected paths when schemas change
- **Diff detection** - Compare spec versions with breaking change detection
- **Code generation** - Generate TypeScript, Rust, or Python code
- **Smart caching** - Efficient caching with ETag/Last-Modified support

## Installation

### npm (Recommended)

```bash
npm install -g @jhlee0409/openapi-sync-mcp
```

### Cargo (Rust users)

```bash
cargo install openapi-sync-mcp
```

### Manual download

Download the appropriate binary from [GitHub Releases](https://github.com/jhlee0409/openapi-sync-mcp/releases).

## Usage with Claude Code

Add to your Claude Code MCP settings (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "oas": {
      "command": "openapi-sync-mcp",
      "args": []
    }
  }
}
```

Or if installed via npm:

```json
{
  "mcpServers": {
    "oas": {
      "command": "npx",
      "args": ["@jhlee0409/openapi-sync-mcp"]
    }
  }
}
```

## MCP Tools

### `oas_parse`

Parse OpenAPI spec with pagination support.

```json
{
  "source": "https://api.example.com/openapi.json",
  "format": "summary",
  "use_cache": true
}
```

**Formats:**
- `summary` - Metadata only (default)
- `endpoints-list` - Endpoint names only
- `schemas-list` - Schema names only
- `endpoints` - Paginated endpoint details
- `schemas` - Paginated schema details
- `full` - Everything (paginated)

### `oas_deps`

Query dependency graph to find affected paths when schema changes.

```json
{
  "source": "./openapi.json",
  "schema": "User",
  "direction": "downstream"
}
```

### `oas_diff`

Compare two OpenAPI spec versions.

```json
{
  "old_source": "./openapi-v1.json",
  "new_source": "./openapi-v2.json",
  "breaking_only": true
}
```

### `oas_status`

Get cached spec status without fetching.

```json
{
  "project_dir": "/path/to/project",
  "check_remote": true
}
```

### `oas_generate`

Generate code from OpenAPI spec.

```json
{
  "source": "./openapi.json",
  "target": "typescript-react-query",
  "style": {
    "type_naming": "PascalCase",
    "generate_docs": true
  }
}
```

**Targets:**
- `typescript-types` - TypeScript type definitions
- `typescript-fetch` - Fetch-based API client
- `typescript-axios` - Axios-based API client
- `typescript-react-query` - React Query hooks
- `rust-serde` - Rust types with serde
- `rust-reqwest` - Rust reqwest client
- `python-pydantic` - Python Pydantic models
- `python-httpx` - Python httpx client

## Building from Source

```bash
# Clone the repository
git clone https://github.com/jhlee0409/openapi-sync-mcp
cd openapi-sync-mcp

# Build release binary
cargo build --release

# Binary will be at target/release/openapi-sync-mcp
```

## Development

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy

# Run in development mode
cargo run
```

## Error Codes

| Code | Category | Description |
|------|----------|-------------|
| E1xx | Network | Connection, timeout, HTTP errors |
| E2xx | Parse | JSON, YAML, OpenAPI validation errors |
| E3xx | File System | File not found, permission denied |
| E4xx | Code Generation | Template, pattern detection errors |
| E5xx | Configuration | Config not found, invalid config |
| E6xx | Cache | Cache not found, corrupted |

## License

MIT

## Related

- [claude-plugins](https://github.com/jhlee0409/claude-plugins) - Plugin commands (`/oas:*`) for OpenAPI Sync MCP
