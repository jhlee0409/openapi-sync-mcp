# Claude Instructions for OpenAPI Sync MCP

This MCP server provides tools for working with OpenAPI specifications.

## Available Tools

| Tool | Description |
|------|-------------|
| `oas_parse` | Parse and validate OpenAPI spec with pagination |
| `oas_deps` | Find schema dependencies and impact |
| `oas_diff` | Compare two spec versions |
| `oas_status` | Check cached spec status |
| `oas_generate` | Generate code from spec |

## Common Workflows

### 1. Parse a spec

```json
{
  "name": "oas_parse",
  "arguments": {
    "source": "https://api.example.com/openapi.json",
    "format": "summary"
  }
}
```

### 2. Find what uses a schema

```json
{
  "name": "oas_deps",
  "arguments": {
    "source": "./openapi.json",
    "schema": "User",
    "direction": "downstream"
  }
}
```

### 3. Check for breaking changes

```json
{
  "name": "oas_diff",
  "arguments": {
    "old_source": "./openapi-v1.json",
    "new_source": "./openapi-v2.json",
    "breaking_only": true
  }
}
```

## Plugin Commands

This MCP is used with the OAS plugin which provides `/oas:*` commands:

- `/oas:init` - Initialize project
- `/oas:sync` - Sync code with spec
- `/oas:status` - Check status
- `/oas:diff` - Compare specs
- `/oas:check` - Validate code

## Development

```bash
# Build
cargo build --release

# Test
cargo test

# Run locally
./target/release/openapi-sync-mcp
```
