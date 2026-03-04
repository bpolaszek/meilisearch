---
phase: 01-auth-foundation
plan: "02"
subsystem: testing
tags: [jwt, rust, actix-rt, tenant-token, integration-tests, meilisearch-auth]

requires:
  - phase: 01-auth-foundation
    plan: "01"
    provides: "IndexRules/IndexBrowseRules types, DOCUMENTS_GET gate, get_index_browse_rules() accessor"

provides:
  - "5 integration tests covering AUTH-01 (indexRules decoded), AUTH-02 (independence), AUTH-03 Set/Map formats, and DOCUMENTS_GET gate"
  - "Test coverage proving tenant token auth layer accepts indexRules claim end-to-end"

affects:
  - "02-document-routes"

tech-stack:
  added: []
  patterns:
    - "Integration test pattern: explicit primaryKey on add_documents when doc has multiple *id fields"
    - "TDD pattern for pre-implemented features: write tests against existing implementation to lock behavior"

key-files:
  created: []
  modified:
    - "crates/meilisearch/tests/auth/tenant_token.rs"

key-decisions:
  - "Explicit primaryKey required in tests to avoid inference ambiguity when documents have fields ending in 'id' (e.g., 'id' and 'tenant_id')"
  - "Tests assert HTTP 200 (gate acceptance) not filtered results — filter injection is Phase 2 work"
  - "Test 5 (documents_get_tenant_token_gate) does not match 'index_rules' filter string — named separately, run via 'documents_get' filter"

patterns-established:
  - "Pattern: Always specify primaryKey explicitly in test add_documents calls when the document schema has multiple fields ending with 'id'"
  - "Pattern: Auth gate tests assert status code only (not response body) — filter application is tested separately in Phase 2"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03]

duration: 3min
completed: 2026-03-04
---

# Phase 1 Plan 2: Auth Foundation Integration Tests Summary

**5 integration tests verifying indexRules JWT claim parsing, searchRules/indexRules independence, Set/Map formats, and DOCUMENTS_GET gate acceptance**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-04T15:14:45Z
- **Completed:** 2026-03-04T15:18:44Z
- **Tasks:** 1 (TDD task)
- **Files modified:** 1

## Accomplishments

- `index_rules_claim_decoded` — AUTH-01: JWT with indexRules Map format (with filter) accepted on GET /indexes/{uid}/documents
- `index_rules_independent_from_search_rules` — AUTH-02: Both `searchRules` and `indexRules` in same JWT — search path uses searchRules, document browse path uses indexRules, fully independent
- `index_rules_set_format` — AUTH-03 Set: array format `["sales"]` grants document browse access
- `index_rules_map_format` — AUTH-03 Map: object format `{"sales": {"filter": "..."}}` accepted
- `documents_get_tenant_token_gate` — Gate: tenant JWT with `indexRules: {"sales": null}` reaches GET /indexes/sales/documents with HTTP 200

## Task Commits

Each task was committed atomically:

1. **Task 1: Integration tests for indexRules parsing, independence, and formats** - `75ebe074e` (test)

## Files Created/Modified

- `crates/meilisearch/tests/auth/tenant_token.rs` — Added 5 test functions at the end of the file (200 lines added)

## Decisions Made

- **Explicit primaryKey required:** Documents with both `id` and `tenant_id` fields triggered Meilisearch's primary key inference error ("2 fields ending with `id`"). Fixed by passing `Some("id")` to `add_documents`. This applies to all tests using `tenant_id` as a field name alongside `id`.
- **Tests validate acceptance, not filtering:** All tests assert HTTP 200 (the auth gate accepts the request). Actual filter injection on document results is Phase 2 work — documented in test comments.
- **TDD cycle with pre-existing implementation:** Since Plan 01 already implemented the auth layer, the RED phase revealed a setup bug (missing primaryKey) rather than a missing implementation. Tests moved directly to GREEN after fixing the test setup.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing primaryKey in test document setup**
- **Found during:** Task 1 (RED phase — tests failed on document indexing, not auth)
- **Issue:** Test documents had both `id` and `tenant_id` fields. Meilisearch's primary key inference failed with "2 fields ending with `id`" error, causing the `add_documents` task to fail before auth could be tested.
- **Fix:** Added `Some("id")` as explicit primaryKey parameter in `add_documents` calls for `index_rules_claim_decoded`, `index_rules_independent_from_search_rules`, and `index_rules_map_format`.
- **Files modified:** `crates/meilisearch/tests/auth/tenant_token.rs`
- **Verification:** All 5 tests pass after fix
- **Committed in:** `75ebe074e` (Task 1 commit — fix was within the same test-writing iteration)

---

**Total deviations:** 1 auto-fixed (Rule 1 — test setup bug)
**Impact on plan:** Auto-fix necessary for test correctness. No scope creep — only affected test setup code, not the feature being tested.

## Issues Encountered

- Pre-existing flaky tests in the full `tenant_token` suite (e.g., `search_authorized_simple_token`, `error_search_token_forbidden_parent_key`) still fail intermittently when run as a full suite due to concurrent TempDir usage. Confirmed pre-existing by running tests individually — all pass. Same issue documented in Plan 01-01 SUMMARY.md.
- `documents_get_tenant_token_gate` does not match the `index_rules` test filter string (its name starts with `documents_get_`). Run it separately with `-- "tenant_token::documents_get"` or include it in the full suite run.

## Next Phase Readiness

- AUTH-01, AUTH-02, AUTH-03 requirements are verified by passing tests
- Phase 2 (document routes) can consume `AuthFilter.get_index_browse_rules(index)` — its behavior is now tested
- Filter injection (applying the filter from `IndexBrowseRules` to actual document results) is the remaining Phase 2 work

---
*Phase: 01-auth-foundation*
*Completed: 2026-03-04*
