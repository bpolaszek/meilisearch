# Phase 3: Integration Tests - Research

**Researched:** 2026-03-04
**Domain:** Rust / Meilisearch integration test suite — tenant token security property verification
**Confidence:** HIGH

---

## Summary

Phase 3's goal is to ensure the security properties of `indexRules` are verified by automated
tests that would catch both the silent bypass and fail-open regressions. **The critical finding
is that 4 of the 5 success criteria are already satisfied by tests created in Phase 2.**

The five Phase 2 test stubs (`index_rules_list_filtered`, `index_rules_fetch_filtered`,
`index_rules_single_doc_out_of_scope`, `index_rules_fail_closed`, `index_rules_admin_key_unaffected`)
are all GREEN and live in `crates/meilisearch/tests/auth/tenant_token.rs`. Additionally, Phase 1
delivered `index_rules_independent_from_search_rules` which verifies that `searchRules` access
is not broken by `indexRules`. These together cover success criteria 1–5 with one gap.

**The gap:** Success criterion 5 requires confirming that `searchRules` *filter behavior* (not
just access) is unchanged. `index_rules_independent_from_search_rules` confirms that search
endpoints return HTTP 200 when both claims are present, but does not assert that search results
still respect `searchRules` filter values when `indexRules` is also set. This is the one new
test Phase 3 needs to add.

Additionally, the Phase 2 VERIFICATION.md flagged two "Human Verification Required" scenarios
that have no automated test coverage:
- Null-rule whitelisted index (`indexRules: {"sales": null}`) returning all documents (not just 200)
- Combined-filter case (caller-supplied filter AND indexRules filter both applied via `fuse_filters()`)

These were not required by Phase 2's success criteria but are legitimate security edge cases.
Phase 3 should decide whether to include them.

**Primary recommendation:** Phase 3 adds 1 required new test (searchRules filter independence)
plus 2 optional edge-case tests (null-rule result set, filter fusion). All 3 go in the existing
`tenant_token.rs` file. No new infrastructure needed.

---

## Gap Analysis: Phase 3 Success Criteria vs Existing Tests

| # | Phase 3 Success Criterion | Existing Test | Status | Gap |
|---|--------------------------|---------------|--------|-----|
| 1 | Cross-tenant access blocked — tenant A cannot see tenant B's documents | `index_rules_list_filtered` (2 of 3 docs returned) | COVERED | None |
| 2 | JWT without `indexRules` returns 403 on document list endpoints | `index_rules_fail_closed` (asserts 403 on GET /documents) | COVERED | None |
| 3 | Admin API key returns 200 with unfiltered results | `index_rules_admin_key_unaffected` (asserts 200 + 3 docs) | COVERED | None |
| 4 | Single-doc endpoint returns 404 for out-of-scope document ID | `index_rules_single_doc_out_of_scope` (asserts 404) | COVERED | None |
| 5 | `searchRules` behavior unchanged — search endpoints unaffected | `index_rules_independent_from_search_rules` (asserts 200 on search) | PARTIAL | Does not verify search result count respects `searchRules` filter |

### VERIFICATION.md Unautomated Gaps (Phase 2 flagged, not required by Phase 2 criteria)

| Scenario | What it tests | Priority |
|----------|--------------|----------|
| Null-rule whitelist (`indexRules: {"sales": null}`) returns all documents | `fuse_filters()` correctly skips injection when `browse_rules.filter == None` | MEDIUM — security edge case |
| Combined-filter fusion (`indexRules` filter AND caller-supplied filter) | `fuse_filters()` correctly ANDs both filters; both must apply | HIGH — filter bypass risk if broken |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `actix-rt` | workspace | Async test runtime | All existing tenant_token tests use `#[actix_rt::test]` |
| `jsonwebtoken` | workspace | JWT encoding for test token generation | Used by `generate_tenant_token()` already in the file |
| `maplit` | workspace | `hashmap!{}` macro for token body construction | Already imported, used by all Phase 1+2 tests |
| `time` | workspace | Token expiry timestamps | Already imported as `time::OffsetDateTime`, `time::Duration` |

No new dependencies.

### Test Helpers Available (no new infrastructure needed)
| Helper | Location | What it does |
|--------|----------|--------------|
| `generate_tenant_token(uid, key, hashmap)` | `tenant_token.rs:12` | Creates a signed JWT with arbitrary claims |
| `Server::new_auth().await` | `tests/common/server.rs` | Starts a Meilisearch instance with auth enabled |
| `server.use_admin_key("MASTER_KEY").await` | `tests/common/server.rs` | Authenticates as master |
| `server.use_api_key(&token)` | `tests/common/server.rs` | Switches auth to a specific key/token |
| `server.add_api_key(body)` | `tests/common/server.rs` | Creates a new API key via the admin API |
| `server.dummy_request(method, url)` | `tests/common/server.rs` | Makes a bare HTTP request with current auth |
| `index.search(json, callback)` | `tests/common/index.rs` | Makes a search request and asserts on the result |
| `server.wait_task(uid).await.succeeded()` | `tests/common/server.rs` | Waits for async task completion |

**Installation:** None needed.

---

## Architecture Patterns

### Key File Map

```
crates/meilisearch/tests/auth/
└── tenant_token.rs          # ONLY FILE TO MODIFY
                               # Append new tests after line 984 (end of file)
```

No other files are touched.

### Pattern 1: Standard Test Setup (reuse from Phase 2)

All Phase 2 tests share an identical boilerplate. Phase 3 tests follow the same pattern:

```rust
// Source: tenant_token.rs:781 (index_rules_list_filtered)
#[actix_rt::test]
async fn my_new_test() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(
        crate::json!({"filterableAttributes": ["tenant_id"]})
    ).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // ... generate token, use it, assert
}
```

### Pattern 2: Using `index.search()` for result-count assertions

The `index.search()` helper is used in pre-existing tests (lines 122–138) to verify filter
behavior. For Phase 3 criterion 5, this is the right tool — it asserts on both response
code AND result count within the same call.

```rust
// Source: tenant_token.rs:120–138 (compute_authorized_search macro)
let index = server.index("sales");
index
    .search(json!({ "filter": "..." }), |response, code| {
        assert_eq!(code, 200, "...");
        assert_eq!(
            response["hits"].as_array().unwrap().len(),
            EXPECTED_COUNT,
            "..."
        );
    })
    .await;
```

Note: `index.search()` uses the server's current auth token — call `server.use_api_key(&token)`
before calling `server.index("sales")` and `index.search()`.

### Pattern 3: `dummy_request` for document endpoints

For all document endpoints (GET /documents, POST /documents/fetch, GET /documents/{id}),
use `server.dummy_request(method, url)` which works with any auth token including tenant JWTs.

### Anti-Patterns to Avoid

- **Duplicating Phase 2 test logic:** Phase 3 does NOT rewrite the 5 existing tests.
  It adds only what is missing (the `searchRules` filter-result independence test).
- **Using `index.search()` for document endpoints:** `index.search()` hits
  `POST /indexes/{uid}/search`. For document browse, use `server.dummy_request()`.
- **Separate filterableAttributes per claim:** Both `searchRules` and `indexRules` filter
  tests require `filterableAttributes` configured for the filtered field. If testing
  `searchRules` filter on `tenant_id` AND `indexRules` filter on `tenant_id`, one
  `update_settings` call is sufficient.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JWT creation | Custom encoding | `generate_tenant_token()` in test file | Already handles `apiKeyUid` injection, already tested |
| HTTP assertions | Custom HTTP client | `server.dummy_request()` + `index.search()` | Already wired to the test server instance |
| Document setup | Custom seed scripts | `index.add_documents()` + `server.wait_task()` | Handles async task completion correctly |

---

## Common Pitfalls

### Pitfall 1: Phase 3 Duplicates Phase 2 Tests

**What goes wrong:** Phase 3 re-implements the 5 Phase 2 tests, creating redundancy and
confusing future maintainers about which tests are canonical.
**How to avoid:** Phase 3 adds only the gaps identified in the Gap Analysis. The 5 Phase 2
tests already satisfy criteria 1–4 and partially satisfy criterion 5.

### Pitfall 2: `searchRules` Filter Independence Test Missing a Result Count Assertion

**What goes wrong:** A test titled "searchRules unaffected" only checks HTTP 200, not that
search RESULTS are filtered by `searchRules` and NOT additionally filtered by `indexRules`.
**How to avoid:** The new test must assert on hit count, not just status code. With
`indexRules: {"sales": {"filter": "tenant_id = a"}}` and
`searchRules: {"sales": {"filter": "tenant_id = b"}}`, the search result must contain
only documents matching `tenant_id = b` (i.e., 1 hit out of 3) — not 0 (which would
indicate cross-contamination from `indexRules`).

### Pitfall 3: `filterable_attributes` Not Configured

**What goes wrong:** The filter silently returns zero documents if `tenant_id` is not in
`filterable_attributes`.
**How to avoid:** Always call `update_settings({"filterableAttributes": ["tenant_id"]})` and
wait for the task before executing filter-dependent tests. This is done in all Phase 2 tests.

### Pitfall 4: Parallel Test Execution "os error 22" on macOS

**What goes wrong:** Running the full test suite in parallel on macOS produces intermittent
"os error 22" (invalid argument) failures on unrelated tests due to tempfile parallelization.
**How to avoid:** This is a pre-existing environmental issue documented in Phase 2 summaries.
Run individual tests by name to verify correctness. The CI pipeline is unaffected.

### Pitfall 5: `index.search()` vs `server.dummy_request()` for Search

**What goes wrong:** Using `server.dummy_request("POST", "/indexes/sales/search")` for
search result assertions requires manually parsing the JSON body. `index.search()` provides
a cleaner callback pattern with direct assertion access.
**How to avoid:** Use `index.search()` for search assertions. Use `server.dummy_request()`
for document browse endpoints.

---

## Code Examples

### Required New Test: `searchRules` Filter Independence (Criterion 5)

```rust
// Source pattern: mirrors index_rules_independent_from_search_rules (tenant_token.rs:620)
// but adds a result count assertion, not just a status code check.

/// Phase 3 — Criterion 5: `searchRules` filter behavior is unchanged when `indexRules`
/// is also present. Search results must be scoped by `searchRules`, NOT by `indexRules`.
#[actix_rt::test]
async fn search_rules_filter_unaffected_by_index_rules() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(
        crate::json!({"filterableAttributes": ["tenant_id"]})
    ).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // searchRules scoped to tenant_id = b (1 document)
    // indexRules scoped to tenant_id = a (2 documents)
    // Search MUST return tenant_id = b results (1 hit), NOT tenant_id = a (cross-contamination).
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"sales": {"filter": "tenant_id = b"}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let index = server.index("sales");
    index
        .search(crate::json!({}), |response, code| {
            assert_eq!(
                code, 200,
                "Phase3-Crit5: search must return 200 with searchRules scoped to tenant_id = b, got {}: {:?}",
                code, response
            );
            assert_eq!(
                response["hits"].as_array().unwrap().len(),
                1,
                "Phase3-Crit5: search must return 1 hit (tenant_id = b), not 2 (tenant_id = a cross-contamination): {:?}",
                response
            );
        })
        .await;
}
```

### Optional Edge Case: Combined Filter Fusion (VERIFICATION.md gap)

```rust
/// Phase 3 — Optional: fuse_filters() applies both indexRules filter AND caller-supplied
/// filter. Both must apply — only the intersection is returned.
#[actix_rt::test]
async fn index_rules_filter_fused_with_query_filter() {
    // Setup: 3 docs — id:1 tenant_id:a, id:2 tenant_id:a, id:3 tenant_id:b
    // indexRules: tenant_id = a (allows doc 1 and 2)
    // Query filter: id > 1 (doc 2 only from the allowed set)
    // Expected: 1 result (doc 2 only — both filters must be ANDed)

    // Use POST /documents/fetch with body {"filter": "id > 1"} via dummy_request.
    // Then assert results.len() == 1 and results[0]["id"] == 2.
}
```

### Optional Edge Case: Null-Rule Whitelist Returns All Documents (VERIFICATION.md gap)

```rust
/// Phase 3 — Optional: indexRules with null rule (whitelisted index, no filter) must
/// return all documents, NOT an empty result set or 403.
#[actix_rt::test]
async fn index_rules_null_rule_returns_all_documents() {
    // Setup: 3 docs — id:1 tenant_id:a, id:2 tenant_id:a, id:3 tenant_id:b
    // indexRules: {"sales": null} (whitelist, no filter restriction)
    // Expected: 200 with 3 documents

    // Note: documents_get_tenant_token_gate (Phase 1) verifies HTTP 200.
    // This test additionally asserts result count == 3.
}
```

---

## State of the Art

| Old State | Phase 3 State | Impact |
|-----------|---------------|--------|
| 5 Phase 2 tests cover criteria 1–4, criterion 5 only partially | 1 new test fully covers criterion 5 with result-count assertion | Security regression guard is complete |
| 2 edge cases (null-rule result count, filter fusion) unautomated | Optionally automated | Better regression coverage for `fuse_filters()` behavior |

**Not needed in Phase 3:**
- New test infrastructure
- Modifications to any production code
- New test files (all tests go in existing `tenant_token.rs`)

---

## Open Questions

1. **Scope of Phase 3: required 1 test only, or also the 2 optional edge cases?**
   - What we know: The ROADMAP success criteria require only criterion 5 as new work. The
     2 optional cases (null-rule result count, filter fusion) are security edge cases that
     have no existing automated coverage.
   - Recommendation: Include all 3 tests. They are low-effort (same boilerplate), and the
     filter fusion test in particular is a plausible regression vector if `fuse_filters()`
     behavior changes.

2. **Should Phase 3 add DOCS-04 coverage for the POST and single-doc endpoints?**
   - What we know: `index_rules_fail_closed` only tests 403 on `GET /indexes/sales/documents`.
     Phase 2 VERIFICATION.md confirms the guard is in all three handlers but only one test
     exercises the 403 path.
   - Recommendation: Add one additional 403 test covering `GET /documents/{id}` to make the
     fail-closed contract explicit across all guarded endpoints. Low effort, high signal.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `actix-rt` + `cargo test` (integration tests via `Server` helper) |
| Config file | none — `#[actix_rt::test]` macro on each test function |
| Quick run command | `cargo test -p meilisearch --test auth -- index_rules` |
| Full suite command | `cargo test -p meilisearch --test auth` |

### Phase Requirements → Test Map

Phase 3 has no new requirements (validates all v1 requirements end-to-end). The test map
shows which success criteria are covered by existing vs new tests.

| Criterion | Behavior | Test Type | Existing Test | New Test Needed |
|-----------|----------|-----------|---------------|-----------------|
| Crit-1 | Cross-tenant access blocked (tenant A cannot see tenant B) | integration | `index_rules_list_filtered` | No |
| Crit-2 | JWT without `indexRules` returns 403 on document list | integration | `index_rules_fail_closed` | No (optional: extend to single-doc endpoint) |
| Crit-3 | Admin API key returns 200 unfiltered (regression guard) | integration | `index_rules_admin_key_unaffected` | No |
| Crit-4 | Single-doc 404 for out-of-scope document ID | integration | `index_rules_single_doc_out_of_scope` | No |
| Crit-5 | `searchRules` filter behavior unchanged (result count) | integration | none | **YES** — `search_rules_filter_unaffected_by_index_rules` |

### Sampling Rate
- **Per task commit:** `cargo test -p meilisearch --test auth -- index_rules`
- **Per wave merge:** `cargo test -p meilisearch --test auth`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

None — all existing test infrastructure is in place. Phase 3 only adds new tests to an
existing file. No new setup, no new config, no new fixtures needed.

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read: `crates/meilisearch/tests/auth/tenant_token.rs` — full test file,
  all 21 existing tests, helper functions, imports
- Direct read: `.planning/phases/02-route-injection/02-VERIFICATION.md` — Phase 2
  verification report, flagged human-verification gaps
- Direct read: `.planning/phases/02-route-injection/02-01-SUMMARY.md` and
  `02-02-SUMMARY.md` — confirmed which tests are GREEN and what was implemented
- Direct read: `.planning/ROADMAP.md` — Phase 3 success criteria (authoritative)
- Direct read: `.planning/REQUIREMENTS.md` — v1 requirement traceability

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` — accumulated decisions and known concerns
- `.planning/phases/02-route-injection/02-RESEARCH.md` — established patterns and pitfalls
  carried forward from Phase 2

---

## Metadata

**Confidence breakdown:**
- Gap analysis: HIGH — based on direct comparison of Phase 3 criteria vs existing test file
- Standard stack: HIGH — no new dependencies, same infrastructure as Phase 2
- Architecture: HIGH — single file, established patterns, no new machinery
- Pitfalls: HIGH — drawn from Phase 2 summaries and verification report

**Research date:** 2026-03-04
**Valid until:** 2026-04-04 (stable codebase, no external fast-moving deps)
