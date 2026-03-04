---
phase: 02-route-injection
plan: "01"
subsystem: auth
tags: [rust, actix, jwt, tenant-token, meilisearch, documents, filter-injection]

# Dependency graph
requires:
  - phase: 01-auth-foundation
    provides: AuthFilter with is_tenant_token(), get_index_browse_rules(), IndexBrowseRules type
  - phase: 02-route-injection
    plan: "00"
    provides: 5 failing TDD RED stubs for DOCS-01 through DOCS-05

provides:
  - "Fail-closed guard in get_documents() and documents_by_query_post(): tenant tokens without indexRules access get 403"
  - "Filter injection via fuse_filters() for tenant tokens with indexRules claim"
  - "is_index_browse_authorized() on AuthFilter to distinguish null-rule (allow, no filter) from missing entry (403)"

affects:
  - "02-02-route-injection-single"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fail-closed guard pattern: is_tenant_token() && !is_index_browse_authorized() → 403"
    - "Filter injection pattern: get_index_browse_rules() returns Some with filter → fuse with query filter"
    - "null vs missing distinction: is_index_browse_authorized() separates 'allowed no-filter' from 'not in claim'"

key-files:
  created: []
  modified:
    - "crates/meilisearch-auth/src/lib.rs"
    - "crates/meilisearch/src/routes/indexes/documents.rs"

key-decisions:
  - "Added is_index_browse_authorized() on AuthFilter — get_index_browse_rules() cannot distinguish 'null rule (allow)' from 'missing entry (403)' because both return None"
  - "Guard checks is_tenant_token() first, then !is_index_browse_authorized() — avoids false 403 for admin API keys"
  - "documents_by_query() signature unchanged — callers handle guard and mutation before delegating"

patterns-established:
  - "Two-step guard: is_tenant_token() gate + is_index_browse_authorized() membership check"
  - "Filter injection is separate from guard — fuse_filters() called only when get_index_browse_rules() returns Some with a filter"

requirements-completed: [DOCS-01, DOCS-02, DOCS-04, DOCS-05]

# Metrics
duration: 15min
completed: 2026-03-04
---

# Phase 2 Plan 01: Route Injection List Endpoints Summary

**Fail-closed indexRules guard and fuse_filters() injection in GET /documents and POST /documents/fetch, with is_index_browse_authorized() added to AuthFilter to correctly handle null-rule (allow without filter) vs missing entry (403)**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-04T16:17:00Z
- **Completed:** 2026-03-04T16:32:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Both `get_documents()` and `documents_by_query_post()` now enforce fail-closed guard for tenant tokens
- `fuse_filters()` injected to merge indexRules filter with any caller-supplied filter (handles all edge cases)
- Added `is_index_browse_authorized()` on `AuthFilter` to correctly distinguish null-rule access from missing-entry access
- DOCS-01 (GET list filtered), DOCS-02 (POST fetch filtered), DOCS-04 (fail-closed 403), DOCS-05 (admin key bypass) all GREEN
- `documents_get_tenant_token_gate` pre-existing test remains GREEN (null-rule = authorized without filter)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add fail-closed guard and filter injection to list endpoint callers** - `5a3d125d5` (feat)

**Plan metadata:** TBD (docs)

## Files Created/Modified

- `crates/meilisearch-auth/src/lib.rs` — Added `is_index_browse_authorized()` on `AuthFilter` to distinguish null-rule (access granted, no filter) from missing map entry (access denied)
- `crates/meilisearch/src/routes/indexes/documents.rs` — Added `fuse_filters` import; fail-closed guard and filter injection in `get_documents()` and `documents_by_query_post()`

## Decisions Made

- **is_index_browse_authorized() added to AuthFilter**: `get_index_browse_rules()` returns `None` both when an index has a `null` rule (authorized, no filter) and when it's absent from the claim (403 required). A separate boolean method is needed to distinguish the two cases. This is the minimal fix — no new type, no breaking change.
- **Guard shape**: `is_tenant_token() && !is_index_browse_authorized()` rather than nested ifs — flatter, easier to read.
- **documents_by_query() signature unchanged**: Callers mutate query/body and delegate; the inner function stays pure.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added is_index_browse_authorized() to AuthFilter**
- **Found during:** Task 1 (after initial implementation, test `documents_get_tenant_token_gate` failed with 403)
- **Issue:** `get_index_browse_rules()` returns `None` for both "index listed with null rule" (should be 200) and "index not listed in claim" (should be 403). The original guard `get_index_browse_rules().is_none()` could not distinguish these cases, causing a regression on the pre-existing gate test.
- **Fix:** Added `is_index_browse_authorized()` on `AuthFilter` (delegates to `IndexRules::is_index_authorized()`) which returns `true` when the index is in the `indexRules` claim regardless of filter value. Guard updated to `is_tenant_token() && !is_index_browse_authorized()`.
- **Files modified:** `crates/meilisearch-auth/src/lib.rs`
- **Verification:** `documents_get_tenant_token_gate` GREEN (null-rule = access granted), `index_rules_fail_closed` GREEN (missing entry = 403)
- **Committed in:** `5a3d125d5` (included in Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical — security correctness)
**Impact on plan:** Essential fix. Without it the null-rule case would incorrectly return 403, breaking the pre-existing gate test. No scope creep.

## Issues Encountered

- Parallel test run showed "os error 22" failures on unrelated tests — confirmed to be macOS tempfile parallelization noise by running tests individually. All tenant_token tests pass individually. Pre-existing issue, not caused by this plan.

## Next Phase Readiness

- DOCS-01, DOCS-02, DOCS-04, DOCS-05 are GREEN
- DOCS-03 (`index_rules_single_doc_out_of_scope`) and DOCS-05 single-doc variant remain RED — Plan 02-02 covers the single-document endpoint
- `is_index_browse_authorized()` is available for Plan 02-02 to reuse the same guard pattern

---
*Phase: 02-route-injection*
*Completed: 2026-03-04*
