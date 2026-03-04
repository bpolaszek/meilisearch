---
phase: 02-route-injection
plan: "02"
subsystem: auth
tags: [rust, actix, jwt, tenant-token, meilisearch, documents, roaring-bitmap, filter-injection]

# Dependency graph
requires:
  - phase: 01-auth-foundation
    provides: AuthFilter with is_tenant_token(), get_index_browse_rules(), is_index_browse_authorized(), IndexBrowseRules type
  - phase: 02-route-injection
    plan: "01"
    provides: Fail-closed guard pattern and is_index_browse_authorized() for list endpoints

provides:
  - "Fail-closed guard in get_document(): tenant tokens without is_index_browse_authorized() get 403"
  - "RoaringBitmap allowed_ids computed from indexRules filter in get_document()"
  - "retrieve_document() accepts allowed_ids: Option<&RoaringBitmap> and returns 404 for out-of-scope documents"

affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Post-retrieval filter check: compute RoaringBitmap from filter, check internal_id membership, 404 on miss"
    - "Fail-closed single-doc: same guard pattern as list endpoints (is_tenant_token() && !is_index_browse_authorized())"
    - "404 not 403 for out-of-scope: avoids confirming document existence to unauthorized tenant"

key-files:
  created: []
  modified:
    - "crates/meilisearch/src/routes/indexes/documents.rs"

key-decisions:
  - "Return 404 (not 403) for out-of-scope documents — 403 would confirm document existence and enable ID enumeration"
  - "Guard reuses is_index_browse_authorized() from 02-01 — same null-rule vs missing-entry distinction applies to single-doc endpoint"
  - "allowed_ids computed in get_document() and passed to retrieve_document() — keeps inner function testable without auth state"

patterns-established:
  - "Single-doc guard: is_tenant_token() + !is_index_browse_authorized() → 403, same as list endpoints"
  - "RoaringBitmap filter membership check after external→internal ID resolution — minimal change to retrieve_document() signature"

requirements-completed: [DOCS-03, DOCS-04, DOCS-05]

# Metrics
duration: 8min
completed: 2026-03-04
---

# Phase 2 Plan 02: Single-Document Endpoint Guard Summary

**Fail-closed guard and RoaringBitmap post-retrieval filter check in GET /indexes/{uid}/documents/{id}: out-of-scope documents return 404, tokens without indexRules get 403, admin keys unaffected**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-04T16:35:00Z
- **Completed:** 2026-03-04T16:43:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- `get_document()` now enforces fail-closed guard using `is_index_browse_authorized()` (same pattern as list endpoints)
- `allowed_ids` RoaringBitmap computed from indexRules filter (or `None` for admin keys / null-rule whitelisted indexes)
- `retrieve_document()` extended with `allowed_ids: Option<&RoaringBitmap>` parameter — returns 404 when internal_id not in bitmap
- DOCS-03 (`index_rules_single_doc_out_of_scope`), DOCS-04 (`index_rules_fail_closed`), DOCS-05 (`index_rules_admin_key_unaffected`) all GREEN

## Task Commits

Each task was committed atomically:

1. **Task 1: Add fail-closed guard and post-retrieval filter check to single-doc endpoint** - `fcb4c87bd` (feat)

**Plan metadata:** TBD (docs)

## Files Created/Modified

- `crates/meilisearch/src/routes/indexes/documents.rs` — Added fail-closed guard in `get_document()`, RoaringBitmap `allowed_ids` computation, extended `retrieve_document()` with `allowed_ids: Option<&RoaringBitmap>` + membership check returning 404 for out-of-scope

## Decisions Made

- **404 not 403 for out-of-scope documents**: Returning 403 would confirm to an attacker that a document exists but is outside their scope. 404 prevents ID enumeration by making out-of-scope indistinguishable from non-existent.
- **Reused is_index_browse_authorized() for guard**: The same `is_tenant_token() && !is_index_browse_authorized()` pattern from 02-01 handles the null-rule (allow) vs missing-entry (403) distinction correctly.
- **allowed_ids computed in get_document(), passed down**: Keeps `retrieve_document()` free of auth scheduler state, making it testable in isolation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Used is_index_browse_authorized() instead of get_index_browse_rules().is_none() in guard**
- **Found during:** Task 1 (implementing guard)
- **Issue:** The plan specified `get_index_browse_rules().is_none()` for the guard, but this would incorrectly return 403 for null-rule entries (index whitelisted with no filter) — the same regression that occurred in 02-01.
- **Fix:** Applied the corrected guard pattern from 02-01: `is_tenant_token() && !is_index_browse_authorized()`. This correctly distinguishes null-rule (authorized, no filter) from missing entry (403 required).
- **Files modified:** `crates/meilisearch/src/routes/indexes/documents.rs`
- **Verification:** All 3 integration tests pass. Pre-existing `documents_get_tenant_token_gate` test unaffected.
- **Committed in:** `fcb4c87bd` (included in Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug — incorrect guard pattern from original plan specification)
**Impact on plan:** Essential correctness fix. Prevented regression on null-rule whitelisted indexes. No scope creep.

## Issues Encountered

- `index_uid.as_ref()` in format string caused E0283 ambiguity (multiple AsRef<T> impls on IndexUid). Fixed by binding to `let index_uid_str: &str = index_uid.as_ref();` — same pattern established by 02-01 at line 639.
- Parallel test suite shows intermittent "os error 22" failures on macOS (pre-existing tempfile parallelization noise, documented in 02-01). All tests pass when run individually.

## Next Phase Readiness

- All 5 DOCS requirements (DOCS-01 through DOCS-05) are GREEN
- Phase 02-route-injection is complete
- The `filterable_attributes` prerequisite concern remains: if `tenant_id` is not in `filterable_attributes`, the filter silently returns zero documents. Must be documented prominently in the PR description.

---
*Phase: 02-route-injection*
*Completed: 2026-03-04*
