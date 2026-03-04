# Architecture

**Analysis Date:** 2026-03-04

## Pattern Overview

**Overall:** Layered multi-crate architecture with asynchronous task scheduling, RESTful HTTP API, and pluggable search engine.

**Key Characteristics:**
- Monolithic HTTP service (Actix-web) with modular internal structure via Rust crates
- Async task scheduler with batching for index operations
- Separation between API layer (Actix routes) and search/indexing engine (milli)
- Authentication abstraction with JWT/API keys
- Support for distributed deployments via network features

## Layers

**HTTP Server (Web Layer):**
- Purpose: Handle incoming HTTP requests, authentication, validation, and response serialization
- Location: `crates/meilisearch/src/routes/`, `crates/meilisearch/src/extractors/`
- Contains: REST endpoint handlers, request extractors, OpenAPI schema definitions
- Depends on: Index Scheduler, Auth Controller, Search Queue
- Used by: External clients via HTTP

**Task Scheduler & Orchestration:**
- Purpose: Manage asynchronous indexing operations, batch tasks, and coordinate execution
- Location: `crates/index-scheduler/src/`
- Contains: Task queue, scheduler loop, batch processing, status tracking
- Depends on: Milli (search engine), Auth, Database (LMDB via heed)
- Used by: HTTP routes for index operations, document updates, settings changes

**Search & Indexing Engine:**
- Purpose: Low-level search execution, document indexing, filters, rankings, embeddings
- Location: `crates/milli/src/`
- Contains: Index data structures, search algorithms, document parsing, vector operations
- Depends on: Tokenization (charabia), embeddings (async-openai)
- Used by: Index Scheduler for task execution

**Authentication & Authorization:**
- Purpose: API key/JWT validation, role-based access control
- Location: `crates/meilisearch-auth/src/`
- Contains: Key generation, token validation, scope/permission checking
- Depends on: LMDB database for key storage
- Used by: Route extractors for guarding endpoints

**Request Extraction & Deserialization:**
- Purpose: Parse and validate incoming HTTP payloads
- Location: `crates/meilisearch/src/extractors/`
- Contains: Custom Actix extractors for JSON, query params, authentication headers
- Depends on: Deserr (validation library), Auth Controller
- Used by: All HTTP route handlers

**Search Queue:**
- Purpose: Limit concurrent search requests to prevent resource exhaustion
- Location: `crates/meilisearch/src/search_queue.rs`
- Contains: Semaphore-based concurrency limiter
- Depends on: None (fundamental utility)
- Used by: Search route handlers

**Type System & Constants:**
- Purpose: Shared definitions across crates (Task types, Settings, Error codes)
- Location: `crates/meilisearch-types/src/`
- Contains: Task definitions, API schemas, error enums, settings structures
- Depends on: Milli for shared enums
- Used by: All other crates

## Data Flow

**Search Request Flow:**

1. HTTP GET/POST arrives at `crates/meilisearch/src/routes/indexes/search.rs`
2. Request extracted via `crates/meilisearch/src/extractors/` (validates, parses query)
3. Auth guard checks API key/JWT via `crates/meilisearch-auth/`
4. Search queue semaphore acquired (`crates/meilisearch/src/search_queue.rs`)
5. Index retrieved from Index Scheduler
6. Search executed in `crates/milli/src/search/` with ranking/filtering
7. Results formatted and returned as JSON

**Document Indexing Flow:**

1. HTTP POST to `/indexes/{uid}/documents` arrives at `crates/meilisearch/src/routes/indexes/documents.rs`
2. Request deserialized and validated via extractors
3. Auth checked via `crates/meilisearch-auth/`
4. Task created (KindWithContent::DocumentsAdditionOrUpdate) via `crates/meilisearch-types/`
5. Task queued in Index Scheduler (`crates/index-scheduler/src/`)
6. Scheduler processes in batch with other tasks
7. Milli performs indexing: tokenization, vector embedding, LMDB writes
8. Task status updated, response returned with task_id

**Federated Search Flow:**

1. Request arrives at search endpoint with multiple indexes
2. Routed to `crates/meilisearch/src/search/federated/` module
3. For distributed: `proxy::` modules route to remote Meilisearch instances
4. Local indexes searched via milli, remote results fetched via HTTP
5. Results merged and ranked in `weighted_scores.rs`
6. Facets aggregated via `MergeFacets`

**State Management:**

- **Index State:** LMDB environment at `{db_path}/indexes/` per index (milli::Index)
- **Task Queue:** LMDB tables in `{db_path}/tasks/` storing pending/completed tasks
- **Auth State:** LMDB environment at `{db_path}/auth/` storing API keys
- **In-Memory Search Cache:** LRU cache of open indexes in Index Scheduler
- **Search Queue:** Semaphore limiting concurrent requests
- **Embeddings Cache:** Optional in-memory cache of embedding vectors (configurable size)

## Key Abstractions

**IndexScheduler:**
- Purpose: Central coordinator for all index operations and task execution
- Examples: `crates/index-scheduler/src/lib.rs`, `crates/index-scheduler/src/scheduler/`
- Pattern: Actor-like pattern—runs in dedicated thread with message passing via Arc<RwLock>

**Index (from milli):**
- Purpose: Represents a single searchable index with documents, settings, and indexes
- Examples: `crates/milli/src/index.rs`
- Pattern: RAII resource holding LMDB transaction + data structures

**Task/TaskView:**
- Purpose: Represents async operation (document update, settings change, backup)
- Examples: `crates/meilisearch-types/src/tasks.rs`
- Pattern: State machine with statuses (Enqueued → Succeeded/Failed)

**SearchContext:**
- Purpose: Encapsulates all state needed to execute a single search query
- Examples: `crates/milli/src/search/new/mod.rs`
- Pattern: Builder pattern for query parameters, then execute

**GuardedData<T>:**
- Purpose: HTTP extractor wrapper ensuring authenticated access with scope checking
- Examples: `crates/meilisearch/src/extractors/authentication/mod.rs`
- Pattern: Actix extractor implementing FromRequest trait

**Settings<Checked> vs Settings<Unchecked>:**
- Purpose: Type-level distinction between user input and validated settings
- Examples: `crates/meilisearch-types/src/settings.rs`
- Pattern: Phantom type parameters for compile-time validation guarantee

## Entry Points

**Binary Entry:**
- Location: `crates/meilisearch/src/main.rs`
- Triggers: Application startup via cargo run
- Responsibilities: Parse CLI args, initialize logging, call setup_meilisearch

**Library Setup:**
- Location: `crates/meilisearch/src/lib.rs`
- Triggers: Import by main.rs or integration tests
- Responsibilities: Setup IndexScheduler/AuthController, create Actix app, configure routes

**HTTP App Factory:**
- Location: `crates/meilisearch/src/lib.rs::create_app()`
- Triggers: Main thread after setup
- Responsibilities: Mount all middleware (CORS, compression, tracing, auth)

**Scheduler Loop:**
- Location: `crates/index-scheduler/src/scheduler/`
- Triggers: Spawned in separate thread from setup
- Responsibilities: Poll task queue, batch similar tasks, execute via milli

**Route Handlers:**
- Locations: `crates/meilisearch/src/routes/{tasks,indexes,api_key,etc}/`
- Triggers: HTTP requests matching route patterns
- Responsibilities: Extract request, call IndexScheduler/Auth methods, serialize response

## Error Handling

**Strategy:** Layered error conversion with context-specific error types.

**Patterns:**

- **HTTP Errors:** `crates/meilisearch/src/error.rs` defines `MeilisearchHttpError` implementing Actix error response
- **Type Errors:** `crates/meilisearch-types/src/error.rs` has `Code` enum (InvalidIndexUid, InvalidQuery, etc.)
- **Task Errors:** Tasks can fail with error messages stored in task.error
- **Scheduler Errors:** `crates/index-scheduler/src/error.rs` wraps LMDB/IO errors
- **Milli Errors:** Search/index errors bubble up through stack with context
- **Auth Errors:** `crates/meilisearch/src/extractors/authentication/error.rs` for auth failures

Error path: Route handler → Result type → Actix error response converter → 400/401/404/500 HTTP status

## Cross-Cutting Concerns

**Logging:**

- Framework: `tracing` crate with subscriber/layer architecture
- Configurable: Human or JSON format via `experimental_logs_mode`
- Spans: HTTP requests traced end-to-end with method/route/status
- Location: Enabled in `crates/meilisearch/src/main.rs::setup()`

**Validation:**

- Framework: `deserr` crate for custom error messages
- Applied: Request extractors check types/ranges before handlers execute
- Settings: Two-phase validation (unchecked input → checked settings)
- Queries: Numeric ranges, enum values, required field checks

**Authentication:**

- Methods: API key (basic auth) or JWT (bearer token)
- Scopes: Per-key actions (documents.read, indexes.manage, etc.)
- Check: Via `GuardedData<T, P>` extractor parametrized by permission policy
- Storage: LMDB with encrypted key material

**Tracing:**

- Instrumentation: `tracing` spans on request entry/exit
- Child spans: Automatically created for tasks, index operations
- Propagation: Trace IDs included in response headers for debugging

**Metrics:**

- Collection: Prometheus exporter at `/metrics` endpoint
- Tracked: Request counts, latencies, task queue depth
- Location: `crates/meilisearch/src/metrics.rs` and middleware

**Personalization:**

- Service: `crates/meilisearch/src/personalization/` module
- Mechanism: Optional user context embeddings affecting search ranking
- Integration: Plugs into search context before ranking

---

*Architecture analysis: 2026-03-04*
