# Roadmap: Meilisearch Document Multitenancy

## Overview

A surgical extension of Meilisearch's existing JWT multitenancy. The `searchRules` pattern
already proves the full data flow — this project wires an identical `indexRules` claim through
the same pipes (JWT decode → AuthFilter → route handler → filter injection) for document read
endpoints. Three phases: build the auth foundation, inject filters at the route layer, then
lock down the security properties with integration tests.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Auth Foundation** - Define `IndexRules` type, extend `AuthFilter` and JWT decode/action gate so `indexRules` flows end-to-end through the auth layer (completed 2026-03-04)
- [x] **Phase 2: Route Injection** - Inject `indexRules` filters into document read handlers, enforce fail-closed 403, protect single-doc endpoint against ID-based exfiltration (completed 2026-03-04)
- [x] **Phase 3: Integration Tests** - Verify all security properties with cross-tenant integration tests covering every endpoint and edge case (completed 2026-03-04)

## Phase Details

### Phase 1: Auth Foundation
**Goal**: The `indexRules` claim is parsed from JWT tokens, stored independently in `AuthFilter`, and routable to document endpoints without breaking `searchRules`
**Depends on**: Nothing (first phase)
**Requirements**: AUTH-01, AUTH-02, AUTH-03
**Success Criteria** (what must be TRUE):
  1. A JWT carrying `indexRules` is decoded and the claim is accessible via `AuthFilter.get_index_browse_rules()`
  2. `indexRules` and `searchRules` are stored in separate fields — setting one does not affect the other
  3. `indexRules` supports both index whitelisting and per-index filter expressions (same structure as `searchRules`)
  4. `DOCUMENTS_GET` action is accepted by `authenticate_tenant_token` for tenant JWTs (action gate extended)
  5. The codebase compiles with no regressions on the existing `searchRules` path
**Plans:** 2/2 plans complete

Plans:
- [x] 01-01-PLAN.md — Define IndexRules/IndexBrowseRules types, extend AuthFilter and JWT decode pipeline
- [x] 01-02-PLAN.md — Integration tests for indexRules parsing, independence, formats, and DOCUMENTS_GET gate

### Phase 2: Route Injection
**Goal**: All three document read endpoints apply `indexRules` filters correctly — tenant data is isolated, non-tenant tokens are unaffected, and missing `indexRules` returns 403
**Depends on**: Phase 1
**Requirements**: DOCS-01, DOCS-02, DOCS-03, DOCS-04, DOCS-05
**Success Criteria** (what must be TRUE):
  1. `GET /indexes/{uid}/documents` with a tenant JWT returns only documents matching the `indexRules` filter
  2. `POST /indexes/{uid}/documents/fetch` with a tenant JWT returns only documents matching the `indexRules` filter
  3. `GET /indexes/{uid}/documents/{id}` returns 404 (not 403) when the document ID is outside the tenant's filter scope
  4. A tenant JWT without an `indexRules` claim receives 403 on all three document endpoints (fail-closed)
  5. An admin API key continues to return unfiltered documents on all three endpoints (no regression)
**Plans:** 3/3 plans complete

Plans:
- [ ] 02-00-PLAN.md — Wave 0: Integration test stubs for DOCS-01 through DOCS-05 (Nyquist compliance)
- [ ] 02-01-PLAN.md — Fail-closed guard and filter injection for list endpoints (GET + POST)
- [ ] 02-02-PLAN.md — Post-retrieval filter check for single-document endpoint

### Phase 3: Integration Tests
**Goal**: The security properties of `indexRules` are verified by automated tests that would catch both the silent bypass and fail-open regressions
**Depends on**: Phase 2
**Requirements**: (no new requirements — validates all v1 requirements end-to-end)
**Success Criteria** (what must be TRUE):
  1. A test confirms cross-tenant document access is blocked — tenant A cannot see tenant B's documents
  2. A test confirms a JWT without `indexRules` returns 403 on document list endpoints
  3. A test confirms an admin API key returns 200 with unfiltered results (regression guard)
  4. A test confirms the single-doc endpoint returns 404 for an out-of-scope document ID
  5. A test confirms `searchRules` behavior is unchanged — search endpoints are unaffected by this change
**Plans:** 1/1 plans complete

Plans:
- [ ] 03-01-PLAN.md — Add 3 integration tests: searchRules filter independence, filter fusion, null-rule whitelist

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Auth Foundation | 2/2 | Complete   | 2026-03-04 |
| 2. Route Injection | 3/3 | Complete   | 2026-03-04 |
| 3. Integration Tests | 1/1 | Complete   | 2026-03-04 |
