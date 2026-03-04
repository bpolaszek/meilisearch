# Requirements: Meilisearch Document Multitenancy

**Defined:** 2026-03-04
**Core Value:** Tenant-isolated document reads via JWT `indexRules` claim

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### JWT Authentication

- [x] **AUTH-01**: JWT tenant tokens can carry an `indexRules` claim using the same structure as `searchRules`
- [x] **AUTH-02**: `indexRules` and `searchRules` are parsed independently — no cross-interaction
- [x] **AUTH-03**: `indexRules` supports both index whitelisting and per-index filter expressions

### Document Access Control

- [x] **DOCS-01**: `GET /indexes/{uid}/documents` applies `indexRules` filters to restrict visible documents
- [x] **DOCS-02**: `POST /indexes/{uid}/documents/fetch` applies `indexRules` filters to restrict visible documents
- [x] **DOCS-03**: `GET /indexes/{uid}/documents/{id}` is protected — tenant cannot fetch documents outside their filter scope
- [x] **DOCS-04**: JWT without `indexRules` claim returns 403 on document read endpoints (fail-closed)
- [x] **DOCS-05**: Non-tenant tokens (API keys) continue to work without `indexRules` (no regression)

## v2 Requirements

### Enhanced Isolation

- **ISO-01**: Dedicated error code for missing `indexRules` (better DX than generic 403)
- **ISO-02**: `total` document count respects tenant filter (no metadata leakage)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Write operation multitenancy | Read-only scope — write isolation is a different concern |
| `indexRules` on search endpoints | Search uses `searchRules` — independent by design |
| `searchRules` fallback for documents | Explicit independence decision — no cross-contamination |
| Admin API key changes | Only JWT tenant tokens affected |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUTH-01 | Phase 1 | Complete |
| AUTH-02 | Phase 1 | Complete |
| AUTH-03 | Phase 1 | Complete |
| DOCS-01 | Phase 2 | Complete |
| DOCS-02 | Phase 2 | Complete |
| DOCS-03 | Phase 2 | Complete |
| DOCS-04 | Phase 2 | Complete |
| DOCS-05 | Phase 2 | Complete |

**Coverage:**
- v1 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-04*
*Last updated: 2026-03-04 after roadmap creation*
