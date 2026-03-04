# Meilisearch — Document Multitenancy

## What This Is

Extension of Meilisearch's existing JWT-based multitenancy to cover document read endpoints (`GET /documents`, `POST /documents/fetch`). Currently, multitenancy only works for search via `searchRules` in JWT claims. This adds an `indexRules` claim that applies the same filtering mechanism to document retrieval.

## Core Value

Tenant-isolated document reads — a JWT with `indexRules` restricts which documents a tenant can see via document endpoints, using the same filterable attributes mechanism as `searchRules`.

## Requirements

### Validated

<!-- Existing capabilities inferred from codebase -->

- ✓ JWT authentication with `searchRules` claim for search multitenancy — existing
- ✓ Filterable attributes on indexes for tenant isolation — existing
- ✓ API key / JWT-based access control with scoped permissions — existing
- ✓ Document retrieval via `GET /indexes/{uid}/documents` and `POST /indexes/{uid}/documents/fetch` — existing
- ✓ Actix-web HTTP layer with `GuardedData<T>` extractors for auth — existing

### Active

- [ ] JWT `indexRules` claim parsed and validated alongside existing claims
- [ ] `indexRules` applies filterable attribute filters to `GET /indexes/{uid}/documents`
- [ ] `indexRules` applies filterable attribute filters to `POST /indexes/{uid}/documents/fetch`
- [ ] `indexRules` controls which indexes are accessible for document endpoints
- [ ] JWT without `indexRules` returns 403 on document read endpoints
- [ ] `searchRules` and `indexRules` are fully independent (no cross-interaction)

### Out of Scope

- Write operations (document add/update/delete) — this is read-only multitenancy
- `indexRules` applying to search endpoints — search uses `searchRules` only
- Fallback from `indexRules` to `searchRules` — explicitly independent
- Admin API key behavior changes — only JWT tenant tokens affected

## Context

- Meilisearch is a Rust search engine using Actix-web, LMDB (via heed), and a custom milli engine
- Auth lives in `crates/meilisearch-auth/` with JWT validation and key storage
- Route handlers in `crates/meilisearch/src/routes/indexes/documents.rs`
- `searchRules` mechanism already proves the pattern: JWT claim → filter injection at query time
- The same `filterable_attributes` infrastructure can be reused for `indexRules`

## Constraints

- **Compatibility**: Must not break existing `searchRules` behavior
- **Performance**: Filter injection on document reads must not add significant overhead
- **Security**: Absence of `indexRules` in JWT = 403 on document endpoints (fail-closed)
- **Codebase**: Follow existing patterns (Rust crates, Actix extractors, deserr validation)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Same filter mechanism as searchRules | Consistency, reuse existing filterable_attributes infra | — Pending |
| searchRules and indexRules independent | Simpler mental model, no surprising cross-effects | — Pending |
| No indexRules = 403 on document endpoints | Fail-closed security — explicit opt-in required | — Pending |

---
*Last updated: 2026-03-04 after initialization*
