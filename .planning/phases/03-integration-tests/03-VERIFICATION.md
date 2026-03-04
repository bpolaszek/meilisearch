---
phase: 03-integration-tests
verified: 2026-03-04T17:55:00Z
status: passed
score: 3/3 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Run all 3 new tests in isolation"
    expected: "All pass — individual test execution required due to pre-existing macOS os error 22 tempfile parallelization conflict"
    why_human: "SUMMARY documents that parallel execution triggers a pre-existing infrastructure failure on macOS; correctness was verified individually. CI on Linux should be green."
---

# Phase 3: Integration Tests Verification Report

**Phase Goal:** The security properties of `indexRules` are verified by automated tests that would catch both the silent bypass and fail-open regressions
**Verified:** 2026-03-04T17:55:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `searchRules` filter behavior is unchanged when `indexRules` is also present — search results are scoped by `searchRules`, NOT by `indexRules` | VERIFIED | `search_rules_filter_unaffected_by_index_rules` at line 991 asserts `hits.len() == 1` (tenant_id=b) with searchRules=b + indexRules=a — cross-contamination impossible |
| 2 | `indexRules` null-rule (whitelisted index, no filter) returns all documents, not an empty set or 403 | VERIFIED | `index_rules_null_rule_returns_all_documents` at line 1106 asserts `results.len() == 3` with `indexRules={"sales": null}` |
| 3 | `indexRules` filter and caller-supplied filter are ANDed together — only the intersection is returned | VERIFIED | `index_rules_filter_fused_with_query_filter` at line 1050 asserts `results.len() == 1` and `results[0]["id"] == 2` — intersection of indexRules(tenant_id=a) and caller filter(id>1) |

**Score:** 3/3 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/meilisearch/tests/auth/tenant_token.rs` | 3 new integration tests appended after line 984 | VERIFIED | File is 1150 lines; tests begin at lines 991, 1050, 1106 respectively. Commit `e6b594a30` adds 166 lines. No stubs, no TODOs. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `search_rules_filter_unaffected_by_index_rules` | `index.search()` with searchRules + indexRules token | `response["hits"].as_array().unwrap().len()` assertion at line 1037 | WIRED | Pattern `hits.*len.*1` confirmed at line 1037–1041 |
| `index_rules_filter_fused_with_query_filter` | `index.fetch_documents(json!({"filter": "id > 1"}))` | `results.len() == 1` at line 1092 + `results[0]["id"] == 2` at line 1096 | WIRED | `fetch_documents` call at line 1088 uses POST /documents/fetch with filter payload; result count and specific doc verified |
| `index_rules_null_rule_returns_all_documents` | `GET /indexes/sales/documents` with null-rule token | `results.len() == 3` at line 1145–1149 | WIRED | `dummy_request("GET", "/indexes/sales/documents")` at line 1142; 3-result assertion present |

---

### Requirements Coverage

Phase 3 declares no new requirements — it validates all v1 requirements end-to-end.

The PLAN frontmatter lists AUTH-01 through DOCS-05 as the requirements being validated. Each is traced below.

| Requirement | Description | Covered By (Phase) | Test Function | Status |
|-------------|-------------|-------------------|---------------|--------|
| AUTH-01 | JWT tenant tokens can carry `indexRules` claim using same structure as `searchRules` | Phase 1 (01-02) | `index_rules_claim_decoded` (line 579) | SATISFIED |
| AUTH-02 | `indexRules` and `searchRules` parsed independently — no cross-interaction | Phase 1 (01-02) | `index_rules_independent_from_search_rules` (line 620) + **strengthened** by Phase 3 `search_rules_filter_unaffected_by_index_rules` (line 991) with result-count assertion | SATISFIED |
| AUTH-03 | `indexRules` supports both index whitelisting and per-index filter expressions | Phase 1 (01-02) | `index_rules_set_format` (line 668), `index_rules_map_format` (line 703) | SATISFIED |
| DOCS-01 | `GET /documents` applies `indexRules` filters to restrict visible documents | Phase 2 (02-00 stubs / 02-01 impl) | `index_rules_list_filtered` (line 781) asserts `results.len() == 2` (tenant_id=a only) | SATISFIED |
| DOCS-02 | `POST /documents/fetch` applies `indexRules` filters to restrict visible documents | Phase 2 | `index_rules_fetch_filtered` (line 824) asserts `results.len() == 2` | SATISFIED — **additionally covered** by Phase 3 `index_rules_filter_fused_with_query_filter` (filter AND behavior) |
| DOCS-03 | `GET /documents/{id}` is protected — 404 for out-of-scope document ID | Phase 2 (02-02) | `index_rules_single_doc_out_of_scope` (line 867) asserts `code == 404` | SATISFIED |
| DOCS-04 | JWT without `indexRules` claim returns 403 on document read endpoints (fail-closed) | Phase 2 (02-01) | `index_rules_fail_closed` (line 910) asserts `code == 403` | SATISFIED |
| DOCS-05 | Non-tenant tokens (API keys) continue to work without `indexRules` (no regression) | Phase 2 (02-01) | `index_rules_admin_key_unaffected` (line 951) asserts `code == 200` and `results.len() == 3` | SATISFIED |

**Orphaned requirements:** None. All 8 v1 requirements are accounted for.

---

### Anti-Patterns Found

None. Grep on `tenant_token.rs` for TODO/FIXME/XXX/HACK/PLACEHOLDER/unimplemented!/todo!() returned no results.

---

### Phase 3 Success Criteria vs ROADMAP

The ROADMAP defines 5 success criteria for Phase 3. Cross-checking against Phase 3 + prior phase tests:

| SC | Criterion | Test | Status |
|----|-----------|------|--------|
| 1 | A test confirms cross-tenant access is blocked — tenant A cannot see tenant B's documents | `index_rules_list_filtered` (DOCS-01): asserts only 2 tenant_id=a docs returned out of 3 | SATISFIED |
| 2 | A test confirms JWT without `indexRules` returns 403 on document list endpoints | `index_rules_fail_closed` (DOCS-04): asserts 403 | SATISFIED |
| 3 | A test confirms admin API key returns 200 with unfiltered results (regression guard) | `index_rules_admin_key_unaffected` (DOCS-05): asserts 200 + 3 results | SATISFIED |
| 4 | A test confirms single-doc endpoint returns 404 for out-of-scope document ID | `index_rules_single_doc_out_of_scope` (DOCS-03): asserts 404 | SATISFIED |
| 5 | A test confirms `searchRules` behavior unchanged — search endpoints unaffected | `search_rules_filter_unaffected_by_index_rules` (Phase 3): asserts 1 hit (searchRules scope), not 2 (indexRules scope) | SATISFIED — **upgraded from HTTP-200-only to result-count assertion** |

---

### Human Verification Required

#### 1. Parallel test suite execution on CI (Linux)

**Test:** Run `cargo test -p meilisearch --test auth -- index_rules` (all index_rules tests in parallel)
**Expected:** All tests pass without `os error 22`
**Why human:** On macOS, parallel execution triggers a pre-existing tempfile infrastructure conflict (documented in RESEARCH.md Pitfall 4, SUMMARY key-decisions). The SUMMARY confirms all 3 new tests pass individually. Linux CI is expected to be clean — this is a macOS-only known issue — but human confirmation on CI is prudent before merging.

---

### Gaps Summary

No gaps. All 3 must-have truths are VERIFIED, all artifacts are substantive and wired, all 8 v1 requirements have test coverage, and no anti-patterns were found.

The one note for human follow-up is the macOS parallel test conflict — this is a pre-existing infrastructure issue documented in RESEARCH.md, not introduced by Phase 3. Tests pass individually and CI (Linux) is expected green.

---

_Verified: 2026-03-04T17:55:00Z_
_Verifier: Claude (gsd-verifier)_
