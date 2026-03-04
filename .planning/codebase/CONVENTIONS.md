# Coding Conventions

**Analysis Date:** 2026-03-04

## Naming Patterns

**Files:**
- `lib.rs` for library crate entry points
- `main.rs` for binary entry points
- Test files suffixed with `_test.rs` (e.g., `autobatcher_test.rs`) or placed in `tests/` directories
- Configuration suffixed with `.toml` (e.g., `Cargo.toml`, `config.toml`)
- Snapshot test files placed in `snapshots/` subdirectories (e.g., `crates/index-scheduler/src/queue/snapshots/batches_test.rs`)

**Functions:**
- Snake case for all function names: `create_index()`, `add_documents()`, `update_settings()`
- Test functions prefixed with `test_` followed by descriptive name: `test_sort()`, `test_redacted()`, `fn create_index()`
- Helper functions in test modules: `collect_field_values()`, `create_index()` used to set up test state
- Private functions use leading underscore convention avoided; visibility controlled via `pub`/`pub(crate)` markers

**Variables:**
- Snake case for all variable and binding names: `new_fields_ids_map`, `primary_key`, `index_scheduler`
- Temporary/iterator variables use semantic names: `doc`, `txn`, `wtxn` (write transaction), `rtxn` (read transaction)
- Constants use SCREAMING_SNAKE_CASE: `DEFAULT_MIN_WORD_LEN_ONE_TYPO`, `CURRENT_DUMP_VERSION`, `VERSION_KEY`

**Types:**
- Pascal case for struct/enum names: `TempIndex`, `IndexScheduler`, `SearchResult`, `MeilisearchHttpError`
- Trait names in Pascal case: `Error`, `MatchingStrategy`
- Type aliases in Pascal case: `TaskId` (pub type), `Result<T, E>`
- Generic parameters use single uppercase letters: `T`, `E`, `S`

## Code Style

**Formatting:**
- Formatter: rustfmt with custom configuration
- Config location: `.rustfmt.toml` in project root
- Key settings:
  - `unstable_features = true` - enables unstable rustfmt features
  - `use_small_heuristics = "max"` - more aggressive line wrapping
  - `imports_granularity = "Module"` - groups imports at module level
  - `group_imports = "StdExternalCrate"` - standard imports → external crates → local modules

**Linting:**
- Linter: Clippy (Rust's official linter)
- Config location: `clippy.toml` in project root
- Disallowed patterns enforced:
  - `tar::Archive::unpack` - prefer `ArchiveExt::safe_unpack` function for security
- Common allow attributes:
  - `#![allow(clippy::type_complexity)]` - used when types legitimately complex
  - `#![allow(clippy::result_large_err)]` - used in crates with large error enums to improve match ergonomics
  - `#![allow(clippy::wrong_self_convention)]` - used where non-standard self patterns are intentional

## Import Organization

**Order:**
1. Standard library imports (`std::*`)
2. External crate imports (alphabetical)
3. Internal crate imports (relative paths)

**Path Aliases:**
- Wildcard imports avoided; all imports explicit
- Module re-exports used via `pub use`: `pub use self::error::Error;` in lib.rs
- Common aliases:
  - `use crate::index::tests::TempIndex;` - test utilities
  - `use heed::RwTxn;` - database transactions
  - `use serde_json::Value;` - JSON values

**Example from `crates/dump/src/lib.rs`:**
```rust
use std::collections::BTreeMap;

use meilisearch_types::batches::BatchId;
use meilisearch_types::byte_unit::Byte;
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

mod error;
mod reader;
mod writer;

pub use error::Error;
pub use reader::{DumpReader, UpdateFile};
pub use writer::DumpWriter;
```

## Error Handling

**Patterns:**
- Error enum at crate level using `thiserror` crate: `#[derive(Debug, thiserror::Error)]`
- Descriptive error messages with context where applicable
- Variants handle both internal and user-facing errors
- Example from `crates/meilisearch/src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum MeilisearchHttpError {
    #[error("A Content-Type header is missing. Accepted values for the Content-Type header are: {}",
            .0.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", "))]
    MissingContentType(Vec<String>),
    #[error("Document `{0}` not found.")]
    DocumentNotFound(String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}
```

- Result type alias defined per crate: `pub type Result<T, E = Error> = std::result::Result<T, E>;`
- Conversion implementations via `From` traits for error composition
- `anyhow::Result` used in binary (`main.rs`) for simplified error propagation

**Error propagation in main.rs:**
- Uses `inspect_err()` for logging errors and their causal chain
- Implements error source traversal for detailed context

## Logging

**Framework:** Tracing crate (structured logging)

**Patterns:**
- Setup in main entry point: `crates/meilisearch/src/main.rs` configures tracing subscribers
- Supports both human-readable and JSON output formats (configurable via `LogMode`)
- Log level configurable via environment: `MEILI_LOG_LEVEL`
- Two-layer setup: route logging (via HTTP) and stderr logging
- Usage: `tracing::error!()`, `tracing::info!()` macros with interpolation
- Example: `tracing::error!(%error);` - uses `Display` format

## Comments

**When to Comment:**
- Document non-obvious logic or design decisions
- Explain why a decision was made, not what the code does
- Use when code differs from first intuition or has surprising behavior
- Flag known issues with `// TODO:` or `// FIXME:` comments
- Example: `// TODO: when next forward breaking dumps, it would be a good idea to move this field inside of`

**Documentation Comments:**
- Module-level documentation: `///` doccomments before `pub mod` or in lib.rs module declarations
- Struct/enum field documentation: inline doccomments or in attribute `#[doc]`
- Example from `crates/index-scheduler/src/lib.rs`:
```rust
/*!
This crate defines the index scheduler, which is responsible for:
1. Keeping references to meilisearch's indexes...
2. Scheduling tasks given by the user...
*/
```

**Snapshot Test Documentation:**
- Test modules begin with documentation comment block explaining what is tested
- Example from `crates/milli/src/search/new/tests/sort.rs`:
```rust
/*!
This module tests the `sort` ranking rule:

1. an error is returned if the sort ranking rule exists but no fields-to-sort were given at search time
2. an error is returned if the fields-to-sort are not sortable
...
*/
```

## Function Design

**Size:**
- No strict line limits; functions are as long as needed for logical coherence
- Complex functions broken into smaller helpers with semantic names
- Test setup functions may be 50+ lines if needed

**Parameters:**
- Pass by reference when possible: `&Index`, `&mut RwTxn`
- Use builder pattern for complex initialization: `IndexDocumentsConfig`, `SearchParameters`
- Transaction objects commonly passed as parameters: `txn: &heed::RoTxn`, `wtxn: &mut heed::RwTxn`

**Return Values:**
- Explicit Result types with specific error handling
- `Result<T>` aliases defined per crate
- Builder methods return `Self` for chaining
- Void operations return `Result<()>` rather than `()`

## Module Design

**Exports:**
- Selective public exports via `pub use` in lib.rs
- Private by default; only public when intentional
- Public modules typically have their own public types exported
- Example from `crates/milli/src/lib.rs`:
```rust
pub use self::error::{
    Error, FieldIdMapMissingEntry, InternalError, SerializationError, UserError,
};
pub use self::index::{CreateOrOpen, Index};
pub use self::search::{
    FacetDistribution, Filter, FormatOptions, MatchBounds, MatcherBuilder,
    MatchingWords, OrderBy, Search, SearchResult, SemanticSearch,
};
```

**Barrel Files:**
- Heavy use of re-exports in lib.rs for ergonomic public API
- Internal module structure decoupled from public API
- Each major feature area (error, search, index, update) has dedicated module with re-exports

**Organization:**
- Database-related types grouped in index modules
- Search and filtering logic in `search/` subdirectory
- Update operations in `update/` subdirectory
- Error types colocated with modules they belong to

## Serde Serialization

**Attributes:**
- `#[serde(rename_all = "camelCase")]` used throughout for JSON API compatibility
- `#[serde(skip_serializing_if = "Option::is_none")]` to omit null fields
- `#[serde(transparent)]` for newtype wrappers
- `#[serde(with = "time::serde::rfc3339")]` for datetime serialization

**Derive Macros:**
- Combine multiple derives: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Order typically: Debug, Clone, Serialize, Deserialize, other traits
- Custom order attributes paired: `#[deserr(rename_all = camelCase)]` alongside serde

## Testing Attributes

- `#[cfg(test)]` to conditionally compile test modules
- `#[test]` marks test functions
- `#[cfg(feature = "...")]` for feature-gated tests
- Example from `crates/milli/src/lib.rs`:
```rust
#[cfg(test)]
#[macro_use]
pub mod snapshot_tests;
```

---

*Convention analysis: 2026-03-04*
