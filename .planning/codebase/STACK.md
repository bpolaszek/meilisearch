# Technology Stack

**Analysis Date:** 2026-03-04

## Languages

**Primary:**
- Rust 1.91.1 - Core search engine and HTTP server

**Secondary:**
- TOML - Configuration (build profiles, workspace setup)

## Runtime

**Environment:**
- Tokio 1.48.0 - Async runtime for HTTP server and task processing

**Package Manager:**
- Cargo - Rust package manager (workspace with 26 crates)
- Lockfile: `Cargo.lock` (present)

## Frameworks

**Core Web:**
- Actix-web 4.12.0 - HTTP server framework with CORS, compression (Brotli/Gzip), TLS support
- Actix-http 3.11.2 - Low-level HTTP protocol support

**Indexing/Search:**
- Milli - Custom search index implementation (internal crate `crates/milli`)
- Heed 0.22.1-nested-rtxns-6 - LMDB-based embedded database for index persistence
- Roaring 0.10.12 - Compressed bitmap library for document sets

**API Documentation:**
- Utoipa 5.4.0 - OpenAPI/Swagger documentation generation with Actix integration
- Utoipa-scalar 0.3.0 - Swagger UI rendering (optional feature)

**Testing:**
- Insta 1.39.0 - Snapshot testing framework (fixed version)
- Wiremock 0.6.5 - HTTP mocking for tests

**Build/Dev Tools:**
- Xtask pattern - Build automation via `crates/xtask`

## Key Dependencies

**Critical Infrastructure:**
- Serde 1.0.228 + Serde_json 1.0.145 - JSON serialization/deserialization with order preservation
- Tokio 1.48.0 - Async runtime with full feature set (networking, time, IO, sync primitives)
- Anyhow 1.0.100 - Error handling with backtrace support

**Search/Indexing:**
- Charabia 0.9.9 - Tokenization and stemming across multiple languages
- Candle-core/transformers/nn 0.9.1 - ML framework for embeddings
- Tokenizers 0.22.1 - Hugging Face tokenizers for embedding models
- Fst 0.4.7 - Finite state transducers for efficient prefix matching
- Levenshtein_automata 0.2.1 - Fuzzy search via Levenshtein distance
- Geojson 0.24.2 + Geoutils 0.5.1 - Geographic search (GeoJSON parsing and distance calculations)

**Security & Authentication:**
- Jsonwebtoken 10.3.0 - JWT token generation and validation
- Rustls 0.23.36 - TLS implementation (replaces OpenSSL)
- Rustls-pemfile 2.2.0 - PEM certificate parsing
- Sha2 0.10.9 - SHA-256 hashing for API key derivation
- Secrecy 0.10.3 - Memory-safe secret storage

**Persistence & Storage:**
- LMDB (via Heed) - Embedded transactional key-value store for indexes
- Memmap2 0.9.9 - Memory-mapped file access for large data structures
- Grenad 0.5.0 - Sorting and merging large datasets during indexing

**HTTP & Networking:**
- Reqwest 0.12.24 - HTTP client for outbound API calls
- Ureq 3.1.4 - Lightweight sync HTTP client alternative
- Hyper-util 0.1.19 - HTTP utilities (tower service compatible)
- Backoff 0.4.0 - Exponential backoff for retries

**Cloud/DevOps:**
- Rusty-s3 0.8.1 - AWS S3 client for snapshot uploads (optional, integrated via index-scheduler)
- Sysinfo 0.37.2 - System memory and CPU information

**Monitoring:**
- Prometheus 0.14.0 - Metrics collection with process metrics
- Tracing 0.1.41 - Structured logging framework
- Tracing-subscriber 0.3.20 - Logging layer with JSON support
- Tracing-actix-web 0.7.19 - Request/response tracing integration

**AI/LLM Integration:**
- async-openai (custom fork in `external-crates/async-openai`) - OpenAI/Azure OpenAI/Mistral API client
- async-openai-macros - Derive macros for async-openai

**Utilities:**
- Clap 4.5.52 - CLI argument parsing with environment variable support
- Uuid 1.18.1 - UUID generation (v4 and v7 variants)
- Time 0.3.47 - Date/time handling with formatting and parsing
- Regex 1.12.2 - Regular expression engine
- Itertools 0.14.0 - Iterator utilities
- Rayon 1.11.0 - Data parallelism
- Lazy_static 1.5.0 - Lazy initialization
- Once_cell 1.21.3 - Safe lazy initialization
- Byte-unit 5.1.6 - Byte size parsing and formatting
- Cidr 0.3.2 - CIDR notation parsing for IP filtering
- Humantime 2.3.0 - Human-friendly duration parsing

**Compression:**
- Flate2 1.1.5 - Gzip/Deflate compression
- Brotli 8.0.2 - Brotli compression (dev dependency)
- Tar 0.4.44 - TAR archive handling for dumps

**Allocator:**
- MiMalloc 0.1.48 - High-performance memory allocator (replaces system allocator on non-Windows)

## Configuration

**Environment Variables (core):**
- `MEILI_DB_PATH` - Database storage location (default: `./data.ms`)
- `MEILI_HTTP_ADDR` - Server bind address (default: `localhost:7700`)
- `MEILI_MASTER_KEY` - API authentication key (required in production)
- `MEILI_ENV` - Environment mode: `development` or `production`
- `MEILI_LOG_LEVEL` - Logging level (trace, debug, info, warn, error)
- `MEILI_EXPERIMENTAL_LOGS_MODE` - Log format: `HUMAN` or `JSON`

**Environment Variables (webhooks):**
- `MEILI_TASK_WEBHOOK_URL` - Webhook URL for task completion notifications
- `MEILI_TASK_WEBHOOK_AUTHORIZATION_HEADER` - Authorization header for webhook requests

**Environment Variables (TLS/SSL):**
- `MEILI_SSL_CERT_PATH` - SSL certificate file path
- `MEILI_SSL_KEY_PATH` - SSL private key file path
- `MEILI_SSL_AUTH_PATH` - Client certificate authority file
- `MEILI_SSL_OCSP_PATH` - OCSP staple file
- `MEILI_SSL_REQUIRE_AUTH` - Require client certificate authentication
- `MEILI_SSL_RESUMPTION` - Enable TLS session resumption
- `MEILI_SSL_TICKETS` - Enable TLS session tickets

**Environment Variables (snapshots):**
- `MEILI_SNAPSHOT_DIR` - Snapshot storage directory (default: `snapshots/`)
- `MEILI_SCHEDULE_SNAPSHOT` - Snapshot interval (default: `86400` seconds)
- `MEILI_IMPORT_SNAPSHOT` - Path to snapshot for import on startup

**Environment Variables (S3 snapshots):**
- `MEILI_S3_BUCKET_URL` - S3 bucket endpoint URL
- `MEILI_S3_BUCKET_REGION` - AWS region
- `MEILI_S3_BUCKET_NAME` - S3 bucket name
- `MEILI_S3_SNAPSHOT_PREFIX` - Prefix for snapshots in bucket
- `MEILI_S3_ACCESS_KEY` - AWS access key ID
- `MEILI_S3_SECRET_KEY` - AWS secret access key
- `MEILI_EXPERIMENTAL_S3_ROLE_ARN` - IAM role ARN for IRSA
- `MEILI_EXPERIMENTAL_S3_WEB_IDENTITY_TOKEN_FILE` - Web identity token file path
- `MEILI_EXPERIMENTAL_S3_MAX_IN_FLIGHT_PARTS` - S3 multipart upload concurrency (default: 10)
- `MEILI_EXPERIMENTAL_S3_COMPRESSION_LEVEL` - Snapshot compression level (default: 0)
- `MEILI_EXPERIMENTAL_S3_MULTIPART_PART_SIZE` - S3 part size (default: 375 MiB)

**Environment Variables (performance tuning):**
- `MEILI_MAX_INDEXING_MEMORY` - Memory limit for indexing operations
- `MEILI_MAX_INDEXING_THREADS` - Thread pool size for indexing
- `MEILI_HTTP_PAYLOAD_SIZE_LIMIT` - Max request body size (default: `100 MB`)
- `MEILI_EXPERIMENTAL_SEARCH_QUEUE_SIZE` - In-flight search request limit
- `MEILI_EXPERIMENTAL_DROP_SEARCH_AFTER` - Search timeout duration
- `MEILI_EXPERIMENTAL_NB_SEARCHES_PER_CORE` - Concurrent searches per CPU core

**Environment Variables (AI/LLM):**
- Chat completion settings configured through database, not env vars. See `crates/meilisearch/src/routes/chats/config.rs` for OpenAI/Azure/Mistral API configuration

**Config File:**
- `config.toml` - TOML configuration file at root (see `crates/meilisearch/src/option.rs` for available settings)
- Profiles: Dev profile uses optimized compilation for `flate2`, `grenad`, `roaring`, `gemm-f16`

**Build Configuration:**
- Single codegen unit for release builds (for better optimization)
- Heed compiled with debug assertions enabled even in release (for disk corruption detection)

## Platform Requirements

**Development:**
- Rust 1.91.1 (via `rust-toolchain.toml`)
- Clippy for linting
- Linux/macOS/Windows supported (Unix allocator uses MiMalloc, Windows uses system allocator)

**Production:**
- Linux (most common deployment target)
- Minimum 2 TiB virtual address space per index (see `INDEX_SIZE` constant in `crates/meilisearch/src/option.rs`)
- 20 GiB virtual address space for task database
- System with sufficient RAM for indexing operations (configurable via `MEILI_MAX_INDEXING_MEMORY`)

**Docker:**
- Dockerfile present: `Dockerfile` at repository root
- Docker image includes mini-dashboard v0.4.1 by default (optional feature)

---

*Stack analysis: 2026-03-04*
