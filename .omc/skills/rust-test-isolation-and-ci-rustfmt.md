---
id: rust-test-isolation-ci-rustfmt
name: Rust Test Isolation & CI rustfmt Version Mismatch
description: Cache integration tests share a single file causing parallel test failures; CI uses newer rustfmt than local
source: conversation
triggers:
  - "test FAILED"
  - "EOF while parsing a value"
  - "cache_integration_test"
  - "Format fail"
  - "cargo fmt --check"
  - "rustfmt diff"
  - "parallel test failure"
  - "unwrap on Err"
  - "No such file or directory"
quality: hard-won
---

# Rust Test Isolation & CI rustfmt Version Mismatch

## The Insight

Two independent problems that compound in CI:

1. **Shared mutable state in parallel tests**: Rust's `cargo test` runs tests in parallel by default. If multiple `#[tokio::test]` functions read/write the same file (e.g., a cache file in `tests/fixtures/`), they will intermittently corrupt each other's data. The failure is non-deterministic - passes locally sometimes, fails in CI.

2. **rustfmt version drift**: CI uses `dtolnay/rust-toolchain@stable` which auto-updates. If your local Rust is older, `cargo fmt` produces different output. The CI Format check will fail even though local `cargo fmt --check` passes.

## Why This Matters

- Test failures appear random: "EOF while parsing a value", "No such file or directory", "unwrap on Err" - all pointing to file I/O on the shared cache file. One test's `cleanup_cache()` deletes the file another test just wrote.
- Format failures are confusing because `cargo fmt --check` passes locally but fails in CI.

## Recognition Pattern

- `cache_integration_test` failures with JSON parse errors or file-not-found
- Tests that call `cleanup_cache()` or similar shared-file cleanup at the start
- CI Format job fails but local `cargo fmt --check` passes
- CI rustc version (check job logs) differs from `rustc --version` locally

## The Approach

**For test isolation:**
- Use `tempfile::tempdir()` per test (already in dev-dependencies)
- Copy fixture files into the temp dir
- Each test gets its own cache file path via the unique temp dir
- No `cleanup_cache()` needed - temp dir auto-cleans on drop

**For rustfmt mismatch:**
- Run `rustup update stable` before committing
- Check CI logs for rustc version: `grep "rustc" in CI logs`
- Match local version: `rustc --version` should equal CI's version
- This project's CI uses `dtolnay/rust-toolchain@stable` (currently rustc 1.93.0)

## Example

```rust
// BEFORE: Shared state, race conditions
fn cleanup_cache() {
    let cache_path = cache_file_path(); // tests/fixtures/.openapi-sync.cache.json
    if cache_path.exists() { std::fs::remove_file(&cache_path).ok(); }
}

#[tokio::test]
async fn test_foo() {
    cleanup_cache(); // Deletes file another parallel test might be reading!
    // ...
}

// AFTER: Isolated temp dirs
fn setup_test_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    std::fs::copy(test_spec_path(), dir.path().join("test-api.json"))
        .expect("Failed to copy test spec");
    dir
}

#[tokio::test]
async fn test_foo() {
    let dir = setup_test_dir(); // Unique dir, no conflicts
    let input = ParseInput {
        source: spec_path(&dir),
        project_dir: Some(project_dir(&dir)),
        // ...
    };
}
```
