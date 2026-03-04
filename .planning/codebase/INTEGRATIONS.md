# External Integrations

**Analysis Date:** 2026-03-04

## APIs & External Services

**LLM/AI Services:**
- OpenAI - Chat completion API for AI-powered search features
  - SDK/Client: `async-openai` (custom fork in `external-crates/async-openai`)
  - Configuration: Configured via database settings in `crates/meilisearch/src/routes/chats/config.rs`
  - Auth: API key stored in database (no environment variable required)
  - Endpoint: Uses OpenAI-compatible API base URL (configurable)

- Azure OpenAI - Cloud-hosted OpenAI API
  - SDK/Client: `async-openai` (via `AzureConfig`)
  - Configuration: Via database settings
  - Auth: Azure API key
  - Endpoint: Azure deployment-specific URL

- Mistral AI - Alternative LLM provider (OpenAI-compatible)
  - SDK/Client: `async-openai` (uses OpenAI-compatible interface)
  - Configuration: Via database settings with custom base URL
  - Auth: API key in database

- vLLM - Self-hosted LLM server
  - SDK/Client: `async-openai` (OpenAI-compatible interface)
  - Configuration: Via database settings with custom base URL
  - Auth: API key if required

## Data Storage

**Embedded Databases:**
- Heed (LMDB wrapper) - Primary persistent storage
  - Location: `MEILI_DB_PATH` environment variable (default: `./data.ms`)
  - Client: Heed 0.22.1-nested-rtxns-6 (custom nested transaction support)
  - Contents: Search indexes, task database, metadata
  - Virtual address space: 2 TiB per index, 20 GiB for task database
  - Features: ACID transactions, nested transactions, memory-mapped access

**File Storage:**
- Local filesystem only (no cloud file storage integration)
  - Snapshot directory: `MEILI_SNAPSHOT_DIR` (default: `snapshots/`)
  - Dump directory: `MEILI_DUMP_DIR` (default: `dumps/`)
  - Formats: TAR archives for dumps, custom binary format for snapshots

**S3 Snapshot Storage (optional):**
- AWS S3 - Remote snapshot backup and restore
  - Client: `rusty-s3` 0.8.1
  - Authentication methods:
    - Static credentials: `MEILI_S3_ACCESS_KEY`, `MEILI_S3_SECRET_KEY`
    - IAM role (IRSA): `MEILI_EXPERIMENTAL_S3_ROLE_ARN`
    - Web identity tokens: `MEILI_EXPERIMENTAL_S3_WEB_IDENTITY_TOKEN_FILE`
  - Configuration:
    - `MEILI_S3_BUCKET_URL` - Endpoint URL (e.g., `https://s3.amazonaws.com`)
    - `MEILI_S3_BUCKET_REGION` - AWS region
    - `MEILI_S3_BUCKET_NAME` - Bucket name
    - `MEILI_S3_SNAPSHOT_PREFIX` - Path prefix for snapshots
  - Performance tuning:
    - `MEILI_EXPERIMENTAL_S3_MAX_IN_FLIGHT_PARTS` - Multipart upload concurrency (default: 10)
    - `MEILI_EXPERIMENTAL_S3_MULTIPART_PART_SIZE` - Part size (default: 375 MiB)
    - `MEILI_EXPERIMENTAL_S3_COMPRESSION_LEVEL` - Compression (0-9, default: 0)

**Caching:**
- None - No explicit cache backend (uses in-memory index caching)

## Authentication & Identity

**API Authentication:**
- Custom - Token-based API key system
  - Implementation: HMAC-SHA256 and JWT-based (see `crates/meilisearch-auth`)
  - Master key: `MEILI_MASTER_KEY` environment variable (required in production)
  - Key generation: `generate_master_key()` function (32+ character requirement)
  - Token types: Admin tokens, search-only tokens with index/action scoping
  - Features: Token expiration, API key revocation, multi-tenant support

**TLS/Client Authentication:**
- Rustls-based TLS server
  - Certificate: `MEILI_SSL_CERT_PATH`
  - Private key: `MEILI_SSL_KEY_PATH`
  - Client CA: `MEILI_SSL_AUTH_PATH` (optional client certificate validation)
  - OCSP: `MEILI_SSL_OCSP_PATH` (optional OCSP staple)
  - Session resumption: `MEILI_SSL_RESUMPTION` (enable/disable)
  - Session tickets: `MEILI_SSL_TICKETS` (enable/disable)
  - Require auth: `MEILI_SSL_REQUIRE_AUTH` (force client certs if enabled)

## Monitoring & Observability

**Metrics:**
- Prometheus 0.14.0 - Metrics exposition
  - Endpoint: Exposed via `/metrics` route (experimental, requires feature flag)
  - Metrics include: Process CPU, memory, request counts, latency
  - Enable: `MEILI_EXPERIMENTAL_ENABLE_METRICS`

**Logs:**
- Tracing subscriber + Tracing-actix-web
  - Format: Configurable via `MEILI_EXPERIMENTAL_LOGS_MODE` (HUMAN or JSON)
  - Level: Configurable via `MEILI_LOG_LEVEL` environment variable
  - Backends: Stderr/stdout logging, with optional route-based dynamic filtering
  - Structured logging: JSON output includes request IDs, spans, context

**Error Tracking:**
- None - No external error tracking service (use log analysis tools)

**Analytics (optional):**
- Segment (if enabled, see `crates/meilisearch/src/analytics/segment_analytics.rs`)
  - Disable: `MEILI_NO_ANALYTICS=true`
  - Data collected: Performance metrics, feature usage
  - Note: Disabled by default in some configurations

## CI/CD & Deployment

**Hosting:**
- Self-hosted only (no managed SaaS offering)
- Docker support: `Dockerfile` at repository root
- Mini-dashboard included: Web UI embedded in binary (optional feature `mini-dashboard`)

**CI Pipeline:**
- Not detected in analyzed code (see CI configuration files in repository root)

## Webhooks & Callbacks

**Outgoing Task Webhooks:**
- Task completion notifications
  - Configuration:
    - `MEILI_TASK_WEBHOOK_URL` - Webhook endpoint URL
    - `MEILI_TASK_WEBHOOK_AUTHORIZATION_HEADER` - Auth header value
  - Triggers: Document indexing completion, index deletion, settings update, etc.
  - Implementation: `crates/meilisearch/src/routes/webhooks.rs`
  - Retry logic: Exponential backoff via `backoff` crate

**Incoming Webhooks:**
- None detected

## HTTP Client Configuration

**Outbound HTTP Requests:**
- Reqwest 0.12.24 - Async HTTP client
  - TLS: Rustls with native root certificates
  - Features: Streaming, multipart uploads, connection pooling
  - Usage: OpenAI API calls, S3 operations, webhook delivery

- Ureq 3.1.4 - Sync HTTP client
  - Usage: Build-time downloads (mini-dashboard assets)
  - TLS: Rustls

## Environment Configuration

**Required Environment Variables (Production):**
- `MEILI_MASTER_KEY` - API authentication master key (minimum 32 characters)
- `MEILI_HTTP_ADDR` - Server bind address (default: `localhost:7700`)
- `MEILI_DB_PATH` - Database directory path (default: `./data.ms`)

**Critical Optional Variables:**
- `MEILI_TASK_WEBHOOK_URL` - For production task tracking
- S3 credentials if using S3 snapshots (4-6 variables depending on auth method)

**Secrets Location:**
- Environment variables only (no `.env` file support detected)
- At deployment time, pass via container environment, systemd, or orchestration platform
- Consider secret vaults (HashiCorp Vault, AWS Secrets Manager) for production

## LLM Integration Details

**Chat Completion Endpoints:**
- Location: `crates/meilisearch/src/routes/chats/`
- Configuration structure: `crates/meilisearch/src/routes/chats/config.rs`
- Error handling: `crates/meilisearch/src/routes/chats/errors.rs`
- Completion handler: `crates/meilisearch/src/routes/chats/chat_completions.rs`

**Supported Providers:**
- OpenAI (ChatGPT models)
- Azure OpenAI
- Mistral (via OpenAI-compatible endpoint)
- vLLM (self-hosted)
- Any OpenAI-compatible API server

**Configuration Format:**
- Provider settings stored in database
- Fields: API key, base URL, org ID, project ID, deployment ID, API version
- No hardcoded credentials - all credentials from database

## Integration Architecture

**HTTP Client Abstraction:**
- `crates/http-client/` - Custom HTTP client wrapper
  - Supports: Ureq (blocking) and Reqwest (async)
  - CIDR filtering: IP-based request filtering via `MEILI_EXPERIMENTAL_ALLOWED_IP_NETWORKS`
  - Used by: OpenAI client, S3 operations, webhook delivery

---

*Integration audit: 2026-03-04*
