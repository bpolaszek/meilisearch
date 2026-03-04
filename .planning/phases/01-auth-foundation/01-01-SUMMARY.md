---
phase: 01-auth-foundation
plan: "01"
subsystem: auth
tags: [jwt, rust, serde, tenant-token, meilisearch-auth]

requires: []

provides:
  - "IndexRules enum (Set/Map untagged, same shape as SearchRules) in meilisearch-auth"
  - "IndexBrowseRules struct with optional filter field in meilisearch-auth"
  - "AuthFilter.index_rules: Option<IndexRules> independent field"
  - "AuthFilter.get_index_browse_rules() accessor"
  - "get_key_filters() extended with index_rules parameter"
  - "JWT Claims.index_rules field with #[serde(default)] for backward compat"
  - "TenantTokenOutcome::Valid carries Option<IndexRules> as third tuple element"
  - "DOCUMENTS_GET action accepted by authenticate_tenant_token action gate"

affects:
  - "02-document-routes"

tech-stack:
  added: []
  patterns:
    - "IndexRules mirrors SearchRules pattern: #[serde(untagged)] enum with Set/Map variants"
    - "Backward-compatible JWT claim extension via #[serde(default)] on Option<T>"
    - "Distinct type (not alias) for each rules domain to prevent cross-wiring"

key-files:
  created: []
  modified:
    - "crates/meilisearch-auth/src/lib.rs"
    - "crates/meilisearch/src/extractors/authentication/mod.rs"
    - "crates/meilisearch/tests/auth/errors.rs"
    - "crates/meilisearch/tests/auth/tenant_token.rs"

key-decisions:
  - "IndexRules is a distinct type (not alias of SearchRules) to prevent type confusion — follows STATE.md locked decision"
  - "is_tenant_token() extended to OR both search_rules and index_rules — a JWT with only indexRules is a tenant token"
  - "TenantTokenOutcome::Valid extended as 3-tuple (not a new variant) for simpler call sites — resolves STATE.md open question"
  - "Tests for error_access_forbidden_routes and invalid_tenant_token updated to reflect DOCUMENTS_GET gate expansion"

patterns-established:
  - "Pattern: Mirror SearchRules type for new rule domains — Set for whitelist, Map for per-index filters, #[serde(untagged)]"
  - "Pattern: Thread new JWT claim through TenantTokenOutcome -> ActionPolicy::authenticate -> get_key_filters -> AuthFilter"
  - "Pattern: get_index_browse_rules() accessor checks is_index_authorized() before delegating to IndexRules"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03]

duration: 18min
completed: 2026-03-04
---

# Phase 1 Plan 1: Auth Foundation Summary

**IndexRules/IndexBrowseRules types wired end-to-end through JWT decode pipeline with DOCUMENTS_GET gate and independent AuthFilter field**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-04T14:53:44Z
- **Completed:** 2026-03-04T15:11:24Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- New `IndexRules`/`IndexBrowseRules` types in `meilisearch-auth` (distinct from `SearchRules`, identical shape)
- `AuthFilter` extended with independent `index_rules: Option<IndexRules>` field and `get_index_browse_rules()` accessor
- JWT `Claims` struct deserialization extended for `indexRules` claim with backward compatibility via `#[serde(default)]`
- `DOCUMENTS_GET` action added to tenant token gate — tenant tokens can now reach document browse endpoints
- All regression tests pass; 2 test files updated to reflect intentional behavior changes

## Task Commits

Each task was committed atomically:

1. **Task 1: Define IndexRules/IndexBrowseRules types and extend AuthFilter** - `d80301218` (feat)
2. **Task 2: Extend JWT decode pipeline and action gate** - `2d971fe46` (feat)

## Files Created/Modified

- `crates/meilisearch-auth/src/lib.rs` — Added `IndexRules` enum, `IndexBrowseRules` struct, extended `AuthFilter` with `index_rules` field and `get_index_browse_rules()` accessor, extended `get_key_filters()` signature
- `crates/meilisearch/src/extractors/authentication/mod.rs` — Extended `Claims`, `TenantTokenOutcome::Valid`, action gate, and `get_key_filters` call
- `crates/meilisearch/tests/auth/errors.rs` — Updated `invalid_tenant_token` snapshot to reflect new decode attempt on documents route
- `crates/meilisearch/tests/auth/tenant_token.rs` — Updated `error_access_forbidden_routes` to exclude `documents.get` routes

## Decisions Made

- **TenantTokenOutcome variant strategy:** Extended existing `Valid` variant as 3-tuple rather than adding a new variant — resolves the open question from STATE.md. Simpler call sites, no pattern-match duplication.
- **is_tenant_token() semantics:** Returns `true` if either `search_rules` OR `index_rules` is set — a JWT carrying only `indexRules` is a tenant token for authorization purposes.
- **Test updates:** `error_access_forbidden_routes` now excludes `documents.get` routes (these are intentionally accepted for tenant tokens). `invalid_tenant_token` snapshot updated — a malformed JWT on `GET /indexes/:uid/documents` now triggers a decode attempt rather than an immediate 403.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated regression tests to match intentional behavior changes**
- **Found during:** Task 2 (JWT decode pipeline)
- **Issue:** Two existing tests (`error_access_forbidden_routes`, `invalid_tenant_token`) encoded the old behavior where `DOCUMENTS_GET` was not in the tenant token gate. After adding `DOCUMENTS_GET` to the gate, these tests correctly detected the behavior change but needed updating to reflect the new intended behavior.
- **Fix:** `error_access_forbidden_routes` now skips `documents.get` routes in its forbidden-route assertion. `invalid_tenant_token` snapshot updated with new error message (decode attempt instead of immediate API key rejection).
- **Files modified:** `tests/auth/tenant_token.rs`, `tests/auth/errors.rs`
- **Verification:** Both tests pass individually and in combination
- **Committed in:** `2d971fe46` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — test behavior update)
**Impact on plan:** The test change is a direct consequence of the intentional gate expansion. No scope creep.

## Issues Encountered

- Some `tenant_token` integration tests showed intermittent failures when run as a full suite (different tests failing on each run). These are pre-existing flaky tests due to concurrency in the test server setup — confirmed by running the same tests against the unmodified codebase and observing the same pattern. All tests pass when run individually.

## Next Phase Readiness

- `AuthFilter.get_index_browse_rules(index)` is available for Phase 2 to call from document routes
- The same pattern as `get_index_search_rules` used in search routes can be mirrored for document browse routes
- Operational concern (from STATE.md blockers): `tenant_id` must be in `filterable_attributes` for an index, otherwise the filter silently returns zero documents — document prominently in PR description

---
*Phase: 01-auth-foundation*
*Completed: 2026-03-04*
