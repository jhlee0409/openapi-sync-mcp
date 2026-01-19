# OpenAPI Sync MCP

[![CI](https://github.com/jhlee0409/openapi-sync-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/jhlee0409/openapi-sync-mcp/actions/workflows/ci.yml)
[![MCP](https://img.shields.io/badge/MCP-2025--11--25-blue)](https://modelcontextprotocol.io)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![MCP Badge](https://lobehub.com/badge/mcp/jhlee0409-openapi-sync-mcp)](https://lobehub.com/mcp/jhlee0409-openapi-sync-mcp)

A high-performance MCP server for OpenAPI specifications. Parse, diff, track dependencies, and generate code - all from your AI assistant.

**Built with Rust** for speed and minimal resource usage.

## Features

- **Dependency Graph** - Know which endpoints break when you change a schema
- **Smart Diff** - Detect breaking changes before they hit production
- **Paginated Parsing** - Handle massive specs without overwhelming context
- **24h Cache** - Fast repeated queries with HTTP cache support
- **Multi-target Codegen** - TypeScript, Rust, Python from one spec

## Quick Start

```bash
npm install -g @jhlee0409/openapi-sync-mcp
```

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

## Tools

| Tool | Description |
|------|-------------|
| `oas_parse` | Parse and validate OpenAPI spec (with pagination) |
| `oas_deps` | Find affected endpoints when a schema changes |
| `oas_diff` | Compare two spec versions, detect breaking changes |
| `oas_status` | Check cache status |
| `oas_generate` | Generate TypeScript/Rust/Python code |

### Code Generation Targets

`typescript-types` · `typescript-fetch` · `typescript-axios` · `typescript-react-query` · `rust-serde` · `rust-reqwest` · `python-pydantic` · `python-httpx`

## Installation

```bash
# npm (recommended)
npm install -g @jhlee0409/openapi-sync-mcp

# Cargo
cargo install openapi-sync-mcp

# Or download from GitHub Releases
```

## Troubleshooting

**Server not responding?**
```bash
chmod +x openapi-sync-mcp
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | openapi-sync-mcp
```

**Cache issues?**
Delete `.openapi-sync.cache.json` or use `use_cache: false`.

## Development

```bash
cargo test          # Run tests
cargo fmt --check   # Check formatting
cargo clippy        # Lint
```

## License

MIT

## Related

- [claude-plugins](https://github.com/jhlee0409/claude-plugins) - Plugin commands (`/oas:*`) for enhanced workflow
