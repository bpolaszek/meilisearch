---
phase: 03-integration-tests
plan: 01
subsystem: testing
tags: [rust, actix-rt, integration-tests, jwt, tenant-token, indexRules, searchRules]

# Dependency graph
requires:
  - phase: 02-route-injection
    provides: "indexRules guard injected into document endpoints (GET/POST/single-doc)"
  - phase: 01-auth-foundation
    provides: "indexRules JWT claim parsing and TenantTokenOutcome::Valid extension"
provides:
  - "search_rules_filter_unaffected_by_index_rules: result-count assertion for Criterion 5"
  - "index_rules_filter_fused_with_query_filter: automated coverage for fuse_filters() AND behavior"
  - "index_rules_null_rule_returns_all_documents: automated coverage for null-rule whitelist behavior"
affects: [03-integration-tests]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "index.fetch_documents(payload) used for POST /documents/fetch with filter payload (vs dummy_request which sends empty body)"
    - "Run tests individually by name on macOS to avoid os error 22 tempfile parallelization conflict"

key-files:
  created: []
  modified:
    - crates/meilisearch/tests/auth/tenant_token.rs

key-decisions:
  - "Tests run individually for correctness verification — os error 22 on macOS is a pre-existing parallel execution issue, not a regression"
  - "index.fetch_documents() used (not dummy_request) for filter fusion test — dummy_request sends empty body and cannot pass a filter payload"
  - "filterableAttributes includes both tenant_id and id for filter fusion test — id must be filterable for the caller-supplied id > 1 filter"

patterns-established:
  - "Pattern: use index.search() with callback for search result-count assertions, dummy_request for document endpoint assertions"
  - "Pattern: fetch_documents(json!({filter})) for POST /documents/fetch with caller-supplied filter"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, DOCS-01, DOCS-02, DOCS-03, DOCS-04, DOCS-05]

# Metrics
duration: 8min
completed: 2026-03-04
---

# Phase 3 Plan 01: Integration Tests Summary

**3 new security edge-case integration tests for indexRules: searchRules filter independence (result-count), fuse_filters() AND behavior, and null-rule whitelist returning all documents**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-04T16:40:00Z
- **Completed:** 2026-03-04T16:48:38Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- `search_rules_filter_unaffected_by_index_rules`: asserts 1 hit (tenant_id=b) with searchRules=b + indexRules=a — proves searchRules filter governs search results, not indexRules
- `index_rules_filter_fused_with_query_filter`: asserts doc 2 only returned when indexRules allows tenant_id=a and caller filter is id>1 — proves fuse_filters() correctly ANDs both filters
- `index_rules_null_rule_returns_all_documents`: asserts all 3 docs returned with indexRules={"sales": null} — proves null-rule whitelist returns all docs, not 0 or 403

## Task Commits

1. **Task 1: Add 3 integration tests for security edge cases** - `e6b594a30` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/meilisearch/tests/auth/tenant_token.rs` - 3 new test functions appended after line 984

## Decisions Made
- Used `index.fetch_documents(json!({"filter": "id > 1"}))` for the filter fusion test — `dummy_request` sends an empty POST body and cannot pass a filter payload
- Added `"id"` to `filterableAttributes` in the filter fusion test — required since the caller-supplied filter uses `id > 1`
- Tests verified individually by name — the macOS `os error 22` parallel tempfile conflict is a pre-existing infrastructure issue documented in RESEARCH.md, not caused by the new tests

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

When running `cargo test -- auth::tenant_token::index_rules` in parallel, 4 tests (including the new `index_rules_null_rule_returns_all_documents`) fail with `os error 22 (Invalid argument)` on macOS — this is the documented Pitfall 4 in RESEARCH.md, a pre-existing tempfile parallelization issue. All 3 new tests pass when run individually, confirming correctness.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All 5 success criteria for `indexRules` are now covered by automated tests with result-count assertions
- The v1 requirement traceability is complete: AUTH-01 through DOCS-05 all have integration test coverage
- No blockers — project is ready for PR review and merge

---
*Phase: 03-integration-tests*
*Completed: 2026-03-04*
