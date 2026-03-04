# Testing Patterns

**Analysis Date:** 2026-03-04

## Test Framework

**Runner:**
- Built-in Rust test framework via `cargo test`
- Executes tests using standard test harness

**Assertion/Snapshot Library:**
- `insta` crate (version 1.39.0) for snapshot testing - fixed version to prevent format breakages
- `meili-snap` wrapper crate provides snapshot utilities and insta integration
- Standard Rust assertions via `assert!()`, `assert_eq!()` macros (used less frequently than snapshots)

**Run Commands:**
```bash
cargo test                          # Run all tests
cargo test -- --test-threads=1     # Run tests serially (for tests with shared state)
cargo test --lib                   # Run library tests only
cargo test --all                   # Run all workspace tests
```

## Test File Organization

**Location:**
- **Inline tests:** Most tests live alongside implementation code in `mod tests { }` blocks
- **Separate test files:** Located in `tests/` directory (integration tests) or as `_test.rs` files
- **Snapshot tests:** Live inline in test modules; snapshots stored in `snapshots/` subdirectories

**Naming:**
- Test files: `*_test.rs` or `test_*.rs` convention (e.g., `autobatcher_test.rs`, `sort.rs`)
- Test functions: `fn test_<feature_name>()` (e.g., `test_sort()`, `test_redacted()`)
- Test modules: `mod tests { }`

**Structure:**
```
crates/
├── milli/
│   ├── src/
│   │   ├── search/
│   │   │   ├── new/
│   │   │   │   ├── mod.rs
│   │   │   │   └── tests/
│   │   │   │       ├── mod.rs         # Test module listing
│   │   │   │       ├── sort.rs        # Test file for sort feature
│   │   │   │       ├── distinct.rs    # Other feature tests
│   │   │   │       └── snapshots/     # Snapshot data
│   │   │   │           ├── sort_test  # Snapshots for sort.rs tests
│   │   │   │           └── distinct_test
│   │   └── test_index.rs              # Shared test utilities
│   └── tests/                         # Integration tests
│       └── search/
│           ├── mod.rs
│           └── sort.rs
```

## Test Structure

**Suite Organization:**

```rust
/*!
This module tests the `sort` ranking rule:

1. an error is returned if the sort ranking rule exists but no fields-to-sort were given at search time
2. an error is returned if the fields-to-sort are not sortable
...
*/

use crate::index::tests::TempIndex;

// Helper function to set up test state
fn create_index() -> TempIndex {
    let index = TempIndex::new();

    index.update_settings(|s| {
        s.set_primary_key("id".to_owned());
        s.set_searchable_fields(vec!["text".to_owned()]);
        s.set_criteria(vec![Criterion::Sort]);
    }).unwrap();

    index.add_documents(documents!([
        { "id": 0, "letter": "A", "rank": 0 },
        { "id": 1, "letter": "B", "rank": 1 },
    ])).unwrap();

    index
}

#[test]
fn test_sort() {
    let index = create_index();
    let txn = index.read_txn().unwrap();

    // Test logic here
}

#[test]
fn test_redacted() {
    let index = create_index();
    // Different test variant
}
```

**Setup/Teardown Patterns:**
- Setup via helper functions (e.g., `create_index()`) that return initialized test state
- `TempIndex` handles cleanup automatically (implements `Deref` to `Index`, drops `TempDir`)
- Database transactions explicitly created: `index.read_txn().unwrap()` or `index.write_txn().unwrap()`
- No explicit teardown needed; relies on scope-based cleanup

**Assertion Patterns:**
- Snapshot assertions: `insta::assert_snapshot!(format!("{documents_ids:?}"), @"[21, 22, 23, ...]");`
- Debug snapshots: `insta::assert_debug_snapshot!(letter_values, @r###"..."###);`
- Inline snapshots (with `@`) store expected data in test code itself

## Mocking

**Framework:** No dedicated mocking framework; uses builder patterns and test doubles

**Patterns:**
- Test utilities module: `crates/milli/src/test_index.rs` provides `TempIndex` for isolated testing
- `TempIndex` structure:
```rust
pub(crate) struct TempIndex {
    pub inner: Index,
    pub indexer_config: IndexerConfig,
    pub index_documents_config: IndexDocumentsConfig,
    pub progress: Progress,
    _tempdir: TempDir,
}

impl Deref for TempIndex {
    type Target = Index;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
```

- Builder methods for configuration:
```rust
impl TempIndex {
    pub fn new() -> Self { /* creates temp directory with default config */ }
    pub fn new_with_map_size(size: usize) -> Self { /* custom size */ }
}
```

- Test doubles for external dependencies:
  - `IpPolicy::danger_always_allow()` in test code (marked as "NO DANGER: test" in comments)
  - Stub implementations used where external services unavailable

**What to Mock:**
- Database operations - use `TempIndex` instead of real index
- File I/O - use `TempDir` from tempfile crate
- Configuration - pass via builder methods to test double

**What NOT to Mock:**
- Core business logic - test with real implementations to catch integration issues
- Serialization/deserialization - test actual serde behavior
- Database transactions - use real heed transactions on temp database

## Fixtures and Factories

**Test Data:**
- Document fixtures use `documents!()` macro for inline JSON:
```rust
index.add_documents(documents!([
    { "id": 0, "letter": "A", "rank": 0, "vague": 0 },
    { "id": 1, "letter": "A", "rank": 1, "vague": "0" },
    { "id": 2, "letter": "B", "rank": 0, "vague": 1 },
])).unwrap();
```

- Query fixtures constructed via builder pattern:
```rust
let mut s = index.search(&txn);
s.terms_matching_strategy(TermsMatchingStrategy::Last);
s.sort_criteria(vec![AscDesc::Desc(Member::Field(S("letter")))]);
let SearchResult { documents_ids, document_scores, .. } = s.execute().unwrap();
```

- Helper utility `collect_field_values()` extracts values from search results:
```rust
fn collect_field_values(
    index: &crate::Index,
    txn: &heed::RoTxn<'_>,
    fid: &str,
    docids: &[u32],
) -> Vec<String> {
    // Collects field values for assertion in tests
}
```

**Location:**
- `crates/milli/src/test_index.rs` - shared test utilities (TempIndex, document helpers)
- `crates/milli/src/search/new/tests/mod.rs` - common test helpers like `collect_field_values()`
- Inline in test files for test-specific fixtures

## Coverage

**Requirements:**
- No enforced coverage percentage
- Coverage decisions made locally by crate maintainers
- Integration tests and snapshot tests provide most coverage

**View Coverage:**
```bash
# Using tarpaulin (if installed)
cargo tarpaulin --out Html --output-dir coverage

# Using llvm-cov (if installed)
cargo llvm-cov
```

## Test Types

**Unit Tests:**
- Scope: Individual functions/modules
- Approach: Use snapshot testing to verify behavior against baseline
- Organization: Inline in modules via `#[cfg(test)] mod tests { }`
- Example: `crates/milli/src/search/new/tests/sort.rs` tests sort ranking rule behavior

**Integration Tests:**
- Scope: Multiple crates working together
- Approach: Use `TempIndex` to set up realistic database state, execute operations, assert results
- Organization: `tests/` directories in crate root
- Example: Search tests that validate full pipeline from document ingestion through query execution

**E2E Tests:**
- Framework: Not detected in codebase; integration tests serve this role
- Approach: Full HTTP stack tested via `crates/meilisearch/tests/` or similar
- Note: Search pipeline tested extensively via integration tests, not separate E2E suite

## Common Patterns

**Async Testing:**
- Most tests are synchronous using database transactions directly
- Where async needed: Use `#[tokio::test]` attribute (not detected in survey but available)
- Async operations wrapped in sync wrapper for testing (e.g., within transaction scope)

**Error Testing:**
- Test error cases via expected panics or Result validation:
```rust
#[test]
fn test_missing_content_type() {
    let result = /* operation that should fail */;
    assert!(result.is_err());
    // Or with panics:
    let _ = result.unwrap(); // Will panic with descriptive message
}
```

- Error snapshots captured:
```rust
insta::assert_snapshot!(format!("{error:?}"));
```

**Search Query Testing:**
- Construct search via builder pattern
- Execute and collect `SearchResult`
- Assert on `documents_ids` via snapshot
- Validate scoring with `document_scores` snapshots
- Extract and verify field values via `collect_field_values()`

**Database Transaction Testing:**
```rust
let index = TempIndex::new();
// Read transaction for queries
let txn = index.read_txn().unwrap();
let result = index.search(&txn).execute().unwrap();

// Write transaction for mutations
let mut wtxn = index.write_txn().unwrap();
index.add_documents_using_wtxn(&mut wtxn, documents).unwrap();
wtxn.commit().unwrap();
```

## Snapshot Testing Details

**Tool:** `insta` snapshot testing library

**Inline Snapshot Syntax:**
```rust
insta::assert_snapshot!(format!("{value:?}"), @"expected_output");
```
- The `@` marker indicates inline snapshot
- Expected value embedded directly in test code
- Snapshots auto-updated with `cargo insta review` on test run

**External Snapshot Syntax:**
```rust
insta::assert_snapshot!("snapshot_name", format!("{value:?}"));
```
- Snapshots stored in `snapshots/` directory
- Filename matches test name with `.snap` suffix

**Debug Snapshots:**
```rust
insta::assert_debug_snapshot!(data);
```
- Captures `{:#?}` formatted output (pretty-printed)
- Used for complex structures like field value lists

**Snapshot File Organization:**
- Location: `crates/[crate]/src/[module]/snapshots/`
- Files named: `[module_name]_test/[test_name].snap` or similar
- Multiple snapshots per test supported via suffixes

**Redaction in Snapshots:**
- Some tests use "redacted" mode to handle non-deterministic output (e.g., IDs)
- Configuration in `meili-snap` crate via insta features: `["json", "redactions"]`

## Test Macros

**`documents!()` macro:**
- Defined in `crates/milli/src/documents.rs` (macro_use import in lib.rs)
- Creates test document sets inline with JSON syntax
- Converts to memmap for efficient document ingestion

**`db_snap!()` macro:**
- Defined in `crates/milli/src/snapshot_tests.rs`
- Captures database state as snapshot
- Used for verifying index structure after operations

---

*Testing analysis: 2026-03-04*
