# stateql Testing Guidelines

## Overview

This document defines testing strategies for `stateql`. All tests must be isolated, reproducible, and safe to run in any environment.

## Core Testing Principles

### 1. Isolation Requirements

- **No Host System Modification**: Tests must never modify the actual file system outside of designated temporary directories
- **No Real Database Modification**: Tests must never affect existing databases on the host machine
- **Process Isolation**: Background processes spawned during tests must be properly contained and terminated

### 2. Test Categories

#### Unit Tests (`src/*/mod.rs`, `#[cfg(test)]`)

- Test pure functions with no side effects
- Test data transformations and business logic
- Example: SQL parsing, IR construction, diff computation

#### Integration Tests (`tests/`)

- Test end-to-end functionality with real databases (via testcontainers or similar)
- Verify SQL generation, schema diffing, and database operations
- Container cleanup is automatic

#### Documentation Tests (`///` examples)

- Include usage examples for public APIs
- Keep doctests passing

### 3. Test Execution

```bash
# Run unit and integration tests
cargo nextest run

# Run doctests (not supported by nextest)
cargo test --doc

# Run only unit tests
cargo nextest run --lib --bins

# Run a specific test
cargo nextest run test_name
```

## Testing Philosophy: Real Over Mocked

### Why We Avoid Mocks

1. **Mocks test implementation, not behavior**: They verify your code calls the right methods, not that it actually works
2. **False confidence**: Mocks can pass even when real integration would fail
3. **Maintenance burden**: Mocks need updating whenever implementation changes

### When Pure Functions Don't Need External Dependencies

Only truly pure functions should be tested without external dependencies:

```rust
// Pure function - no side effects, no I/O
fn diff_schemas(desired: &Schema, current: &Schema) -> Vec<Migration> {
    // Diff logic only
}

#[test]
fn test_diff_schemas_add_column() {
    let desired = Schema { /* ... */ };
    let current = Schema { /* ... */ };
    let result = diff_schemas(&desired, &current);
    assert_eq!(result.len(), 1);
}
```

## Safety Checklist

Before adding tests, verify:

- [ ] All file operations use `tempfile` or `tempdir`
- [ ] No hardcoded paths to real directories
- [ ] Database operations target only test databases (containers)
- [ ] All resources cleaned up in test teardown
- [ ] PID files use test-specific locations

## Troubleshooting

### Common Issues

**Problem**: Tests fail with "Permission denied"
**Solution**: Ensure temp directories have proper permissions (0755)

**Problem**: Tests hang indefinitely
**Solution**: Add timeout annotations: `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` with appropriate timeout

**Problem**: Flaky tests in CI
**Required Actions**:

1. **Identify root cause**: Run test with `--nocapture` and logging enabled
2. **Document the issue**: Create detailed bug report
3. **Mark as flaky**: Use `#[ignore]` with comment explaining the issue

   ```rust
   #[test]
   #[ignore] // FLAKY: Race condition - see issue #XXX
   fn test_something_flaky() {
       // Test implementation
   }
   ```

### Timeout Policy

Timeouts should be deterministic and minimal:

```rust
// GOOD: Explicit, minimal timeout
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_quick_operation() {
    // Should complete in milliseconds
}

// BAD: Extended timeout to "fix" flaky test
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]  // ❌ with unreasonable timeout
async fn test_that_sometimes_hangs() {
    // Hiding a race condition
}
```

## Appendix: Recommended Test Dependencies

```bash
# Core testing
cargo add --dev tempfile       # Safe temporary file/directory creation
cargo add --dev assert_cmd     # Command execution assertions
cargo add --dev predicates     # Flexible assertions

# Property-based testing (for parsers)
cargo add --dev proptest

# Parameterized tests
cargo add --dev test-case
```
