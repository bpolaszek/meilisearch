# Codebase Structure

**Analysis Date:** 2026-03-04

## Directory Layout

```
meilisearch/
├── crates/                          # Rust workspace crates (core application)
│   ├── meilisearch/                # Main HTTP server binary and routes
│   ├── index-scheduler/            # Task scheduling and orchestration
│   ├── milli/                       # Search engine core (indexing, queries)
│   ├── meilisearch-types/          # Shared type definitions and schemas
│   ├── meilisearch-auth/           # Authentication and authorization
│   ├── dump/                        # Database export/import functionality
│   ├── file-store/                 # File storage abstraction
│   ├── filter-parser/              # Filter query DSL parser
│   ├── meilisearch-types/          # Types and error definitions
│   ├── permissive-json-pointer/    # JSON path parsing
│   ├── flatten-serde-json/         # JSON flattening utility
│   ├── json-depth-checker/         # JSON nesting depth validation
│   ├── http-client/                # HTTP client wrapper (ureq)
│   ├── meili-snap/                 # Testing snapshot utilities
│   ├── meilitool/                  # CLI tool for database operations
│   ├── benchmarks/                 # Performance benchmarks
│   ├── fuzzers/                    # Fuzzing tests
│   ├── tracing-trace/              # Custom tracing utilities
│   ├── xtask/                       # Build automation tasks
│   ├── build-info/                 # Build metadata
│   └── openapi-generator/          # OpenAPI schema generation
├── external-crates/                 # Vendored external crates
│   ├── async-openai/               # OpenAI API client (vendored)
│   ├── async-openai-macros/        # Macros for async-openai
│   └── reqwest-eventsource/        # Server-sent events client
├── workloads/                       # Benchmarking and test workloads
│   ├── search/                      # Search performance tests
│   └── tests/                       # Integration test workloads
├── documentation/                   # API documentation and guides
├── assets/                          # Static assets (dashboard, icons)
├── Cargo.toml                       # Workspace manifest
├── Cargo.lock                       # Dependency lock file
├── config.toml                      # Default configuration
├── TESTING.md                       # Test running instructions
└── README.md                        # Project overview
```

## Directory Purposes

**crates/meilisearch:**
- Purpose: HTTP API server, request handling, and route definitions
- Contains: Actix-web routes, extractors, middleware, analytics
- Key files: `src/main.rs` (entry point), `src/lib.rs` (app setup), `src/routes/` (endpoints)

**crates/meilisearch/src/routes/:**
- Purpose: RESTful API endpoint implementations
- Contains: `indexes/`, `tasks.rs`, `batches.rs`, `api_key.rs`, `chats/`, `webhooks.rs`, etc.
- Pattern: Each route file implements handlers and OpenAPI schema markers

**crates/meilisearch/src/routes/indexes/:**
- Purpose: Index-specific operations (CRUD, search, documents, settings)
- Contains: `mod.rs` (index CRUD), `search.rs`, `documents.rs`, `settings.rs`, `similar.rs`, `facet_search.rs`
- Key files: `search.rs` (search handler), `documents.rs` (document operations)

**crates/meilisearch/src/extractors/:**
- Purpose: Actix request extraction and validation
- Contains: `mod.rs` (payload types), `authentication/` (auth guards), `payload.rs` (size limits), `sequential_extractor.rs`
- Key pattern: GuardedData<T, Policy> for authenticated routes

**crates/meilisearch/src/search/:**
- Purpose: Search result building and result formatting
- Contains: `mod.rs` (SearchQuery struct, result building), `federated/` (multi-index search), `ranking_rules.rs`
- Key files: `mod.rs` (1000+ lines, core search logic)

**crates/index-scheduler/src/:**
- Purpose: Asynchronous task management and batch processing
- Contains: `lib.rs` (public API), `scheduler/` (main loop), `queue/` (task queue), `lru.rs` (index cache)
- Key files: `lib.rs` (IndexScheduler struct), `scheduler/mod.rs` (task execution loop)

**crates/milli/src/:**
- Purpose: Core search and indexing engine
- Contains: `search/` (query execution), `update/` (document indexing), `index.rs` (Index struct), `facet/`, `vector/`
- Key files: `lib.rs` (Index public API), `search/new/mod.rs` (SearchContext), `update/` (indexing pipeline)

**crates/milli/src/search/:**
- Purpose: Query execution, filtering, ranking, result formatting
- Contains: `new/mod.rs` (main search executor), `ranking_rules.rs`, `filtered_universe.rs`
- Key files: `new/mod.rs` (SearchContext, execute_search function)

**crates/milli/src/update/:**
- Purpose: Document indexing pipeline
- Contains: `new/` (modular indexer), `index_documents.rs` (main indexing logic)
- Key files: `index_documents.rs` (IndexDocuments operation)

**crates/meilisearch-types/src/:**
- Purpose: Shared type definitions and API contracts
- Contains: `tasks.rs` (Task/KindWithContent), `error.rs` (error codes), `settings.rs` (Settings struct)
- Key files: `tasks.rs` (task types), `error.rs` (Code enum), `milli/` (re-export from milli)

**crates/meilisearch-auth/src/:**
- Purpose: Authentication and authorization
- Contains: `lib.rs` (AuthController), API key/JWT handling
- Key files: `lib.rs` (AuthController struct with HMAC/JWT logic)

**crates/dump/src/:**
- Purpose: Database export/import for migration and backup
- Contains: `lib.rs` (Dump reader/writer), versioning logic
- Key files: `lib.rs` (Dump struct)

**external-crates/async-openai/:**
- Purpose: OpenAI API client for embeddings and chat
- Contains: Full OpenAI SDK (vendored for control)
- Used by: Vector embeddings, chat completions

## Key File Locations

**Entry Points:**

- `crates/meilisearch/src/main.rs`: Binary entry point, parses CLI args, calls setup_meilisearch()
- `crates/meilisearch/src/lib.rs`: Library exports (setup_meilisearch, create_app, ServicesData)

**Configuration:**

- `crates/meilisearch/src/option.rs`: Opt struct with all CLI/env options (master_key, db_path, etc.)
- `config.toml`: Default configuration template
- `Cargo.toml`: Workspace manifest and feature flags

**Core Logic:**

- `crates/index-scheduler/src/lib.rs`: IndexScheduler public API (register, process, etc.)
- `crates/index-scheduler/src/scheduler/mod.rs`: Main event loop thread
- `crates/milli/src/lib.rs`: Index struct and search entry points
- `crates/milli/src/search/new/mod.rs`: SearchContext and execute_search()
- `crates/meilisearch/src/routes/mod.rs`: Route configuration and OpenAPI setup
- `crates/meilisearch/src/routes/indexes/search.rs`: Search endpoint handler

**Testing:**

- `crates/meilisearch/src/main.rs` → integration tests scattered throughout
- `crates/meili-snap/src/`: Snapshot testing utilities (insta integration)
- `crates/meilisearch/src/routes/indexes/search_test.rs`: Search handler tests
- `workloads/tests/`: Workload/integration test definitions

**Authentication:**

- `crates/meilisearch/src/extractors/authentication/mod.rs`: Auth extractor and policies
- `crates/meilisearch-auth/src/lib.rs`: AuthController implementation

## Naming Conventions

**Files:**

- Route files: Lowercase, plural nouns (indexes.rs, documents.rs, api_keys.rs)
- Modules: Lowercase, descriptive (search_queue.rs, error.rs)
- Tests: Appended with `_test.rs` (search_test.rs, option_test.rs)
- Re-exports: `mod.rs` for module namespace

**Directories:**

- Feature areas: Lowercase, plural (routes/, extractors/, crates/)
- Route categories: Plural with focus (indexes/, chats/, webhooks/)
- Modules with substructure: Lowercase single name (facet/, vector/, update/)

**Functions & Types:**

- Functions: snake_case (execute_search, register_task)
- Structs: PascalCase (SearchQuery, IndexScheduler, GuardedData)
- Enums: PascalCase (Task, Status, Kind)
- Constants: UPPER_SNAKE_CASE (DEFAULT_SEARCH_OFFSET, PAGINATION_DEFAULT_LIMIT)
- Error types: PascalCase (MeilisearchHttpError, DeserrJsonError)

**Traits:**

- Generic bounds with policies: Uppercase (ActionPolicy, ReadPolicy)
- Extractor traits: Standard Actix names (FromRequest, FromRequestHead)

## Where to Add New Code

**New HTTP Endpoint:**

1. Create handler in `crates/meilisearch/src/routes/{feature}/` file (e.g., `routes/my_feature.rs`)
2. Add OpenAPI schema with `#[openapi(...)]` derive macro
3. Add `pub fn configure(cfg: &mut web::ServiceConfig)` function
4. Include route and submodule in `routes/mod.rs::MeilisearchApi` nest path
5. Register in `routes/mod.rs::configure()` function

Example location: `crates/meilisearch/src/routes/my_feature.rs`

**New Task Type:**

1. Add variant to `KindWithContent` enum in `crates/meilisearch-types/src/tasks.rs`
2. Implement handler in `crates/index-scheduler/src/scheduler/` module
3. Add error codes to `crates/meilisearch-types/src/error.rs` if new failures possible
4. Tests: Add cases in `crates/index-scheduler/src/` test utilities

**New Search Feature:**

1. Add parameter to `SearchQuery` struct in `crates/meilisearch/src/search/mod.rs`
2. Implement logic in `crates/milli/src/search/new/mod.rs` SearchContext
3. Add tests in `crates/meilisearch/src/routes/indexes/search_test.rs`

**New Setting Type:**

1. Add field to `Settings<Unchecked>` in `crates/meilisearch-types/src/settings.rs`
2. Add validation/checked version in same file
3. Implement serialization/deserialization if custom logic needed
4. Add handler in index scheduler update processing

**Shared Utilities:**

- Small helpers: `crates/meilisearch-types/src/utils.rs` or create new util crate
- JSON helpers: `crates/flatten-serde-json/` or `crates/permissive-json-pointer/`
- Type definitions: Always in `crates/meilisearch-types/`

**Benchmarks:**

- Performance tests: `crates/benchmarks/src/`
- Workload definitions: `workloads/` directory

## Special Directories

**crates/ (Core Workspace):**
- Purpose: Modular Rust crates for different system components
- Generated: No (hand-written)
- Committed: Yes
- Structure: Each crate has own Cargo.toml, src/, tests/

**external-crates/ (Vendored External):**
- Purpose: Third-party crates under our control (async-openai, reqwest-eventsource)
- Generated: No (manually vendored for specific features/versions)
- Committed: Yes
- Reason: Custom patches or specific version requirements

**documentation/:**
- Purpose: API guides, architecture docs
- Generated: No
- Committed: Yes

**workloads/:**
- Purpose: Integration test datasets and performance benchmarks
- Generated: No (curated test data)
- Committed: Yes

**/target/ (Build artifacts):**
- Purpose: Compiled binaries and dependencies
- Generated: Yes (cargo build)
- Committed: No (.gitignore'd)

**data.ms/ (Runtime Database):**
- Purpose: LMDB data files (indexes, tasks, auth)
- Generated: Yes (runtime)
- Committed: No (.gitignore'd)
- Recreatable: Via imports or startup

**.planning/codebase/ (GSD Documents):**
- Purpose: Architecture and structure analysis
- Generated: Yes (via /gsd:map-codebase)
- Committed: Yes

---

*Structure analysis: 2026-03-04*
