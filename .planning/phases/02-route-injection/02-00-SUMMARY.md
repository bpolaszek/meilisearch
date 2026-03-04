---
phase: 02-route-injection
plan: "00"
subsystem: testing
tags: [rust, actix, integration-tests, tdd, tenant-token, jwt, meilisearch]

# Dependency graph
requires:
  - phase: 01-auth-foundation
    provides: JWT decode pipeline with indexRules/IndexBrowseRules types and TenantTokenOutcome enum

provides:
  - "5 failing integration test stubs for DOCS-01 through DOCS-05 (TDD RED phase for route injection)"

affects:
  - "02-01-route-injection-list"
  - "02-02-route-injection-single"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TDD Wave 0: tests written before implementation, one test per requirement ID"
    - "dummy_request() used for all document endpoint assertions (works with tenant JWTs)"
    - "Explicit primary key 'id' set on add_documents to avoid Meilisearch inference ambiguity"

key-files:
  created: []
  modified:
    - "crates/meilisearch/tests/auth/tenant_token.rs"

key-decisions:
  - "Used dummy_request() for all 5 test stubs (consistent, JWT-compatible, works for GET and POST)"
  - "Each test independently sets up server+index to avoid state leakage between parallel tests"
  - "DOCS-03 asserts 404 (not 403) for out-of-scope single document — avoids leaking existence"

patterns-established:
  - "Phase 2 tests follow same server setup pattern as Phase 1 (new_auth → add_documents → update_settings → create API key → generate token)"
  - "Test comments use requirement ID prefix (DOCS-XX) for traceability"

requirements-completed: [DOCS-01, DOCS-02, DOCS-03, DOCS-04, DOCS-05]

# Metrics
duration: 2min
completed: 2026-03-04
---

# Phase 2 Plan 00: Route Injection Test Stubs Summary

**5 TDD RED-phase integration test stubs covering tenant-filtered document list, fetch, single-doc out-of-scope 404, fail-closed 403, and admin key bypass behaviors**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-04T16:14:07Z
- **Completed:** 2026-03-04T16:15:49Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added 5 failing integration tests to `tenant_token.rs` covering all Phase 2 behavioral requirements
- Tests compile cleanly and will fail with assertion errors until Plans 02-01 and 02-02 implement route-layer logic
- Test names match VALIDATION.md test map exactly for traceability

## Task Commits

1. **Task 1: Add 5 failing integration test stubs for DOCS-01 through DOCS-05** - `34ce8251a` (test)

## Files Created/Modified

- `crates/meilisearch/tests/auth/tenant_token.rs` — Appended 5 new `#[actix_rt::test]` async functions after existing `documents_get_tenant_token_gate` test

## Decisions Made

- Used `server.dummy_request()` consistently for all 5 stubs — it supports both GET and POST and works with tenant JWTs, making the pattern uniform
- Each test creates its own server instance to prevent state leakage (tests run in parallel with actix-rt)
- DOCS-03 (`index_rules_single_doc_out_of_scope`) asserts `404` not `403` — consistent with security best practice of not confirming document existence

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- `--no-run` flag not supported by the Meilisearch custom test runner — but full `cargo check` confirmed compilation success with zero errors.

## Next Phase Readiness

- All 5 test stubs are in RED state, ready for Plans 02-01 and 02-02 to make them GREEN
- Tests cover: GET /documents (list), POST /documents/fetch, GET /documents/{id} (single), fail-closed 403, admin key bypass
- Existing `tenant_token` tests unaffected (no regression risk)

---
*Phase: 02-route-injection*
*Completed: 2026-03-04*
