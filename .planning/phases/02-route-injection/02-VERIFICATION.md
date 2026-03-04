---
phase: 02-route-injection
verified: 2026-03-04T17:00:00Z
status: passed
score: 5/5 must-haves verified
---

# Phase 2: Route Injection — Verification Report

**Phase Goal:** All three document read endpoints apply `indexRules` filters correctly — tenant data is isolated, non-tenant tokens are unaffected, and missing `indexRules` returns 403
**Verified:** 2026-03-04T17:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `GET /indexes/{uid}/documents` with a tenant JWT returns only documents matching the `indexRules` filter | VERIFIED | Lines 763–778 of `documents.rs`: fail-closed guard + `fuse_filters()` injected into `get_documents()`. Integration test `index_rules_list_filtered` asserts 200 + 2 documents. |
| 2 | `POST /indexes/{uid}/documents/fetch` with a tenant JWT returns only documents matching the `indexRules` filter | VERIFIED | Lines 639–658 of `documents.rs`: same guard + filter injection pattern in `documents_by_query_post()`. Integration test `index_rules_fetch_filtered` asserts 200 + 2 documents. |
| 3 | `GET /indexes/{uid}/documents/{id}` returns 404 (not 403) when the document ID is outside the tenant's filter scope | VERIFIED | `get_document()` lines 274–321: computes `allowed_ids` RoaringBitmap from indexRules filter, passes to `retrieve_document()`. `retrieve_document()` lines 2041–2046 returns `DocumentNotFound` (404) on bitmap miss. Test `index_rules_single_doc_out_of_scope` asserts 404. |
| 4 | A tenant JWT without an `indexRules` claim receives 403 on all three document endpoints (fail-closed) | VERIFIED | All three handlers check `is_tenant_token() && !is_index_browse_authorized()` before any data access. Test `index_rules_fail_closed` asserts 403. |
| 5 | An admin API key continues to return unfiltered documents on all three endpoints (no regression) | VERIFIED | Guard short-circuits via `is_tenant_token() == false` for API keys. `is_index_browse_authorized()` correctly returns `false` only when `index_rules` field is `None`, which only happens for tenant tokens — admin keys are never tenant tokens. Test `index_rules_admin_key_unaffected` asserts 200 + 3 documents. |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/meilisearch/tests/auth/tenant_token.rs` | 5 failing test stubs for DOCS-01 through DOCS-05 (Plan 02-00) | VERIFIED | All 5 `#[actix_rt::test]` functions exist at lines 781, 824, 867, 910, 951. Each is fully substantive: sets up auth server, creates index, adds documents, configures filterableAttributes, creates API key, generates tenant token, makes request, asserts status code and body. |
| `crates/meilisearch/src/routes/indexes/documents.rs` | Fail-closed guard and filter injection in `get_documents()` and `documents_by_query_post()` (Plan 02-01) | VERIFIED | Both functions contain `is_tenant_token() && !is_index_browse_authorized()` guard + `fuse_filters()` injection. `let mut query` / `let mut body` rebinds confirmed. `documents_by_query()` signature unchanged. |
| `crates/meilisearch/src/routes/indexes/documents.rs` | Fail-closed guard in `get_document()` and `allowed_ids` parameter on `retrieve_document()` (Plan 02-02) | VERIFIED | `get_document()` has guard at lines 276–286, `allowed_ids` computation at lines 291–313, and passes `allowed_ids.as_ref()` to `retrieve_document()`. `retrieve_document()` signature extended with `allowed_ids: Option<&RoaringBitmap>` at line 2032; membership check at lines 2043–2046. |
| `crates/meilisearch-auth/src/lib.rs` | `is_index_browse_authorized()` method on `AuthFilter` (Plan 02-01 deviation) | VERIFIED | Method exists at lines 295–303. Correctly returns `false` when `index_rules` is `None` (no claim) and delegates to `index_rules.is_index_authorized(index)` otherwise. Resolves null-rule vs missing-entry ambiguity that `get_index_browse_rules().is_none()` could not distinguish. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `documents.rs` | `crates/meilisearch-auth/src/lib.rs` | `index_scheduler.filters().get_index_browse_rules()` | VERIFIED | Pattern found at lines 291–292, 654, 777 in `documents.rs`. Method defined at line 279 in `lib.rs`. |
| `documents.rs` | `crates/meilisearch-auth/src/lib.rs` | `index_scheduler.filters().is_index_browse_authorized()` | VERIFIED | Pattern found at lines 278, 642, 766 in `documents.rs`. Method defined at line 295 in `lib.rs`. |
| `documents.rs` | `crates/meilisearch/src/search/mod.rs` | `fuse_filters` import and usage | VERIFIED | Import at line 53: `use crate::search::{fuse_filters, parse_filter, ...}`. Used at lines 655, 778 for filter injection. `fuse_filters` defined at line 1258 of `search/mod.rs`. |
| `get_document()` | `retrieve_document()` | `allowed_ids: Option<&RoaringBitmap>` parameter | VERIFIED | Call at lines 315–321 passes `allowed_ids.as_ref()`. `retrieve_document()` signature at line 2032 accepts `Option<&RoaringBitmap>`. Membership check at lines 2043–2046. |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DOCS-01 | 02-00, 02-01 | `GET /indexes/{uid}/documents` applies `indexRules` filters | SATISFIED | Guard + `fuse_filters()` in `get_documents()`. Test `index_rules_list_filtered` asserts 2/3 docs returned. |
| DOCS-02 | 02-00, 02-01 | `POST /indexes/{uid}/documents/fetch` applies `indexRules` filters | SATISFIED | Guard + `fuse_filters()` in `documents_by_query_post()`. Test `index_rules_fetch_filtered` asserts 2/3 docs returned. |
| DOCS-03 | 02-00, 02-02 | `GET /indexes/{uid}/documents/{id}` returns 404 for out-of-scope document | SATISFIED | `get_document()` computes bitmap from filter; `retrieve_document()` returns `DocumentNotFound` on miss. Test `index_rules_single_doc_out_of_scope` asserts 404. |
| DOCS-04 | 02-00, 02-01, 02-02 | JWT without `indexRules` returns 403 (fail-closed) | SATISFIED | All three handlers: `is_tenant_token() && !is_index_browse_authorized()` → 403. Test `index_rules_fail_closed` asserts 403 on list endpoint. |
| DOCS-05 | 02-00, 02-01, 02-02 | Non-tenant tokens (API keys) work without `indexRules` (no regression) | SATISFIED | Guard skipped via `is_tenant_token() == false` for API keys. Test `index_rules_admin_key_unaffected` asserts 200 + 3 documents. |

**All 5 DOCS requirements satisfied. No orphaned requirements.**

---

### Anti-Patterns Found

No blockers or warnings found.

| File | Pattern | Severity | Notes |
|------|---------|----------|-------|
| `documents.rs` | No TODOs, FIXMEs, or placeholder returns in modified sections | — | Clean |
| `lib.rs` | No TODOs or stubs in new `is_index_browse_authorized()` | — | Clean |
| `tenant_token.rs` | No empty assertions or `todo!()` in 5 new tests | — | Tests were in RED state at creation (02-00) but are now GREEN per plan summaries |

---

### Human Verification Required

**1. Null-rule whitelisted index (no filter) — integration test not present**

**Test:** Create a tenant JWT with `indexRules: {"sales": null}` (or `{}` — an entry with no filter expression), then `GET /indexes/sales/documents`.
**Expected:** 200 with all 3 documents (index is authorized, no filter restriction applies).
**Why human:** The `is_index_browse_authorized()` method is verified to return `true` for null-rule entries (code path confirmed), and `get_index_browse_rules()` returns `Some(IndexBrowseRules { filter: None })` which skips filter injection. However, no integration test exercises this specific token shape in Phase 2 tests (it was covered by the pre-existing `documents_get_tenant_token_gate` test from Phase 1 — worth confirming that test still passes after Phase 2 changes).

**2. Filter fusion with existing caller-supplied filter**

**Test:** Set a tenant JWT with `indexRules: {"sales": {"filter": "tenant_id = a"}}`, call `POST /indexes/sales/documents/fetch` with body `{"filter": "id > 1"}`.
**Expected:** 200 with only document id=2 (both filters applied: tenant_id=a AND id>1).
**Why human:** `fuse_filters()` is used but no integration test exercises the combined-filter case in Phase 2.

---

### Gaps Summary

No gaps. All phase goal truths are verified. The implementation is complete, substantive, and correctly wired end-to-end.

One notable design decision made during execution (not a gap): the original plan specified `get_index_browse_rules().is_none()` as the fail-closed guard condition, but this would have caused a regression on null-rule (whitelisted) indexes. The executor correctly added `is_index_browse_authorized()` to `AuthFilter` to distinguish "null rule = authorized, no filter" from "missing entry = 403". This is fully implemented and wired.

---

_Verified: 2026-03-04T17:00:00Z_
_Verifier: Claude (gsd-verifier)_
