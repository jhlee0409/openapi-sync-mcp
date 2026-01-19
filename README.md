# OpenAPI Sync MCP

[![CI](https://github.com/jhlee0409/openapi-sync-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/jhlee0409/openapi-sync-mcp/actions/workflows/ci.yml)
[![MCP](https://img.shields.io/badge/MCP-2025--11--25-blue)](https://modelcontextprotocol.io)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance MCP server for OpenAPI specifications. Parse, diff, track dependencies, and generate code - all from your AI assistant.

**Built with Rust** for speed and minimal resource usage.

## Why This MCP?

| Feature | Benefit |
|---------|---------|
| **Dependency Graph** | Know exactly which endpoints break when you change a schema |
| **Smart Diff** | Detect breaking changes before they hit production |
| **Paginated Parsing** | Handle massive specs without overwhelming context |
| **24h Cache** | Fast repeated queries with HTTP cache support (ETag/Last-Modified) |
| **Multi-target Codegen** | TypeScript, Rust, Python from one spec |

## Quick Start

### 1. Install

```bash
# npm (recommended)
npm install -g @jhlee0409/openapi-sync-mcp

# or Cargo
cargo install openapi-sync-mcp

# or download binary from GitHub Releases
```

### 2. Configure Claude Code

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "oas": {
      "command": "openapi-sync-mcp"
    }
  }
}
```

### 3. Use It

```
You: "Parse the OpenAPI spec at https://petstore3.swagger.io/api/v3/openapi.json"
You: "What endpoints would break if I change the Pet schema?"
You: "Generate TypeScript types for User and Order schemas"
```

## Tools

### `oas_parse` - Parse OpenAPI Spec

```json
{
  "source": "https://api.example.com/openapi.json",
  "format": "summary"
}
```

| Format | Description | Use Case |
|--------|-------------|----------|
| `summary` | Metadata only (default) | Quick overview |
| `endpoints-list` | Endpoint names | Discover available APIs |
| `schemas-list` | Schema names | Discover data models |
| `endpoints` | Paginated details | Deep dive into APIs |
| `schemas` | Paginated details | Deep dive into models |
| `full` | Everything | Complete analysis |

**Pagination:** Use `limit` and `offset` for large specs.

### `oas_deps` - Dependency Graph

Find what breaks when a schema changes:

```json
{
  "source": "./openapi.json",
  "schema": "User",
  "direction": "downstream"
}
```

**Directions:**
- `downstream` - What uses this schema?
- `upstream` - What does this schema depend on?
- `both` - Full dependency tree

### `oas_diff` - Compare Versions

```json
{
  "old_source": "./openapi-v1.json",
  "new_source": "./openapi-v2.json",
  "breaking_only": true
}
```

Detects:
- Added/removed endpoints
- Changed request/response schemas
- Breaking changes (removed fields, type changes)

### `oas_status` - Cache Status

```json
{
  "project_dir": ".",
  "check_remote": true
}
```

### `oas_generate` - Code Generation

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

| Target | Output |
|--------|--------|
| `typescript-types` | Type definitions |
| `typescript-fetch` | Fetch API client |
| `typescript-axios` | Axios client |
| `typescript-react-query` | React Query hooks |
| `rust-serde` | Serde structs |
| `rust-reqwest` | Reqwest client |
| `python-pydantic` | Pydantic models |
| `python-httpx` | HTTPX client |

## Installation Options

### npm (Recommended)

```bash
npm install -g @jhlee0409/openapi-sync-mcp
```

### Cargo

```bash
cargo install openapi-sync-mcp
```

### Binary Download

Download from [GitHub Releases](https://github.com/jhlee0409/openapi-sync-mcp/releases):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `openapi-sync-mcp-aarch64-apple-darwin` |
| macOS (Intel) | `openapi-sync-mcp-x86_64-apple-darwin` |
| Linux (x64) | `openapi-sync-mcp-x86_64-unknown-linux-gnu` |
| Linux (ARM64) | `openapi-sync-mcp-aarch64-unknown-linux-gnu` |
| Windows | `openapi-sync-mcp-x86_64-pc-windows-msvc.exe` |

### Build from Source

```bash
git clone https://github.com/jhlee0409/openapi-sync-mcp
cd openapi-sync-mcp
cargo build --release
# Binary: target/release/openapi-sync-mcp
```

## Configuration Examples

### Claude Code

```json
{
  "mcpServers": {
    "oas": {
      "command": "openapi-sync-mcp"
    }
  }
}
```

### Claude Code (npx)

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

### Cursor / Continue

```json
{
  "mcpServers": {
    "oas": {
      "command": "/path/to/openapi-sync-mcp",
      "args": []
    }
  }
}
```

## Troubleshooting

### "Server not responding"

1. Check if binary is executable: `chmod +x openapi-sync-mcp`
2. Test manually: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | openapi-sync-mcp`

### "Parse error" on remote specs

1. Check URL is accessible: `curl -I <url>`
2. Some APIs require auth headers - currently not supported for remote specs

### Cache issues

Cache files are stored in project directory:
- `.openapi-sync.cache.json` - Spec cache (24h TTL)

Delete to force refresh, or use `use_cache: false`.

## Protocol

- **MCP Version:** 2025-11-25
- **Transport:** stdio (JSON-RPC 2.0)
- **Capabilities:** tools, resources, prompts

## Error Codes

| Code | Category | Description |
|------|----------|-------------|
| E1xx | Network | Connection, timeout, HTTP errors |
| E2xx | Parse | JSON, YAML, OpenAPI validation |
| E3xx | File System | File not found, permission denied |
| E4xx | Code Generation | Template, pattern errors |
| E5xx | Configuration | Invalid config |
| E6xx | Cache | Cache corrupted |

## Development

```bash
cargo test          # Run tests
cargo fmt --check   # Check formatting
cargo clippy        # Lint
cargo run           # Run locally
```

## Contributing

Contributions welcome! Please:

1. Fork the repo
2. Create a feature branch
3. Run `cargo fmt && cargo clippy`
4. Open a PR

## License

MIT

## Related

- [claude-plugins](https://github.com/jhlee0409/claude-plugins) - Plugin commands (`/oas:*`) for enhanced workflow
