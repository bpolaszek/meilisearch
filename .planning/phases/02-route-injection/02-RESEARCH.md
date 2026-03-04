# Phase 2: Route Injection - Research

**Researched:** 2026-03-04
**Domain:** Rust / Actix-web route handler filter injection, Meilisearch document browse endpoints
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DOCS-01 | `GET /indexes/{uid}/documents` applies `indexRules` filters to restrict visible documents | `documents_by_query()` is the shared implementation behind both GET and POST — inject filter there |
| DOCS-02 | `POST /indexes/{uid}/documents/fetch` applies `indexRules` filters to restrict visible documents | Same `documents_by_query()` call path as DOCS-01 |
| DOCS-03 | `GET /indexes/{uid}/documents/{id}` is protected — tenant cannot fetch documents outside their filter scope | Requires post-retrieval filter check returning 404 (not 403) to avoid confirming document existence |
| DOCS-04 | JWT without `indexRules` claim returns 403 on document read endpoints (fail-closed) | `AuthFilter::is_tenant_token()` + `get_index_browse_rules()` returning `None` → explicit 403 guard before any logic |
| DOCS-05 | Non-tenant tokens (API keys) continue to work without `indexRules` (no regression) | `is_tenant_token()` returns `false` for API keys → bypass all `indexRules` logic entirely |
</phase_requirements>

---

## Summary

Phase 2 is a pure route-layer change. All three document read handlers currently live in
`crates/meilisearch/src/routes/indexes/documents.rs`. The internal shared function
`documents_by_query()` handles both `GET /indexes/{uid}/documents` and
`POST /indexes/{uid}/documents/fetch`. The single-document handler `get_document()` calls
the internal `retrieve_document()` function.

Phase 1 already wired `index_rules` all the way through the auth stack — `AuthFilter` now
carries `get_index_browse_rules()`. Phase 2's only job is to call that method at the route
layer and act on the result, exactly mirroring how `get_index_search_rules()` is called in
`search.rs`.

The two cases require slightly different treatment:

**List endpoints (DOCS-01, DOCS-02):** The existing `filter` field in `BrowseQuery` is a
`Option<Value>` that is already passed to `retrieve_documents()`. The `indexRules` filter
must be fused into this field using the existing `fuse_filters()` utility, after a
fail-closed guard that 403s tenant tokens with no `indexRules` claim.

**Single-document endpoint (DOCS-03):** `retrieve_document()` does a direct lookup by
external document ID — it has no filter concept. The correct approach is: retrieve the
document normally, then evaluate the tenant's filter against the returned internal ID. If the
document does not pass the filter, return 404 (not 403 — 403 would confirm existence). This
requires a small new helper to evaluate an `IndexBrowseRules` filter against a specific
document ID.

**Admin API keys (DOCS-05):** `is_tenant_token()` returns `false` for API keys. The entire
`indexRules` code path is guarded behind `if index_scheduler.filters().is_tenant_token()`.
No change to admin key behavior.

**Primary recommendation:** In `documents_by_query()` and `get_document()`, add a
fail-closed guard followed by filter injection using `get_index_browse_rules()` and
`fuse_filters()`. For the single-doc endpoint, add a post-retrieval filter check that maps
out-of-scope results to `DocumentNotFound` (HTTP 404).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `meilisearch-auth` | workspace | `AuthFilter`, `IndexBrowseRules`, `get_index_browse_rules()` | Phase 1 delivered these — they are the direct input to Phase 2 |
| `serde_json::Value` | workspace | Filter expression type | Already used for `BrowseQuery.filter` and `IndexBrowseRules.filter` |
| `milli::Filter` | workspace | Filter evaluation against an LMDB index | Used inside `retrieve_documents()` via `parse_filter()` |

No new dependencies are required for this phase.

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `fuse_filters` | internal (`search/mod.rs:1258`) | AND-combines two `Option<Value>` filter expressions | Used to merge tenant filter with user-supplied filter in list endpoints |
| `parse_filter` | internal (`search/mod.rs`) | Parses a `Value` into a `milli::Filter` that can be `.evaluate()`d | Used in the single-doc post-retrieval check |
| `RoaringBitmap` | workspace | Document ID set from filter evaluation | Used to check if the retrieved internal doc ID is in the filter result set |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Post-retrieval filter check for `get_document` | Pre-filter then lookup | The `retrieve_documents` path does support filter-then-iterate, but `get_document` performs a direct external-ID → internal-ID lookup. Reusing that logic for the single-doc case would require significant refactor. Post-retrieval check is simpler and produces the correct 404 response. |
| `fuse_filters` for list endpoints | Prepend filter manually | `fuse_filters` already handles all edge cases (nil left, nil right, array vs scalar). Don't re-implement. |

**Installation:** No new packages needed.

---

## Architecture Patterns

### Key File Map

```
crates/
├── meilisearch/src/routes/indexes/
│   └── documents.rs                  # MODIFY: documents_by_query(), get_document()
│                                       # These are the only files touched in Phase 2
└── meilisearch/src/search/mod.rs     # READ-ONLY: fuse_filters, parse_filter (reused)
```

### Pattern 1: Fail-Closed Guard (DOCS-04)

**What:** Before any filter injection logic, check whether the token is a tenant token.
If it is a tenant token AND `get_index_browse_rules()` returns `None`, return 403.
**When to use:** At the start of both `documents_by_query()` and `get_document()`, before
any index access.

```rust
// In documents_by_query() and get_document():

// Fail-closed: tenant tokens without indexRules are forbidden on document endpoints.
if index_scheduler.filters().is_tenant_token() {
    if index_scheduler.filters().get_index_browse_rules(&index_uid).is_none() {
        return Err(ResponseError::from_msg(
            "The provided token does not have access to document browsing on this index."
                .to_string(),
            Code::InvalidApiKey,
        ));
    }
}
```

**Why `Code::InvalidApiKey`:** This is the same error code the existing tenant token
machinery uses for unauthorized access (HTTP 403, `"type": "auth"`). There is no dedicated
`MissingIndexRules` code in v1 (that is ISO-01, a v2 requirement). Using `InvalidApiKey`
is consistent with the rest of the auth layer's behavior.

### Pattern 2: Filter Injection for List Endpoints (DOCS-01, DOCS-02)

**What:** After the fail-closed guard, retrieve the `IndexBrowseRules` for the index and
fuse its `filter` field into the query's existing filter.
**When to use:** In `documents_by_query()`, after the guard and before calling
`retrieve_documents()`.

```rust
// Source: mirrors crates/meilisearch/src/routes/indexes/search.rs:458

// Inject indexRules filter (only if this is a tenant token — guard already passed above).
if let Some(browse_rules) = index_scheduler.filters().get_index_browse_rules(&index_uid) {
    filter = fuse_filters(filter, browse_rules.filter);
}
```

Note: `fuse_filters` is imported from `crate::search`. The variable `filter` is the
`Option<Value>` already extracted from `BrowseQuery`.

### Pattern 3: Post-Retrieval Filter Check for Single-Doc Endpoint (DOCS-03)

**What:** For `get_document()`, after the fail-closed guard and after retrieving the
document by external ID, evaluate the tenant's filter against the document's internal ID.
If the document is not in the filter result set, return `DocumentNotFound` (404).
**When to use:** Only when a tenant token with `indexRules` is present.
**Critical invariant:** Return 404, NOT 403. Returning 403 would confirm that the document
exists, enabling document existence probing.

```rust
// After retrieving internal_id and document in get_document():

if let Some(browse_rules) = index_scheduler.filters().get_index_browse_rules(&index_uid) {
    if let Some(filter_value) = browse_rules.filter {
        let txn = index.read_txn()?;
        let parsed = parse_filter(&filter_value, Code::InvalidDocumentFilter, features)?;
        if let Some(filter) = parsed {
            let allowed = filter.evaluate(&txn, &index).map_err(|e| ResponseError::from(e))?;
            if !allowed.contains(internal_id) {
                // Return 404, not 403 — do not confirm document existence to unauthorized tenant.
                return Err(MeilisearchHttpError::DocumentNotFound(doc_id.to_string()).into());
            }
        }
    }
}
```

This requires that `internal_id` is accessible before the document JSON is formatted.
In `get_document()`, the `internal_id` comes from `external_documents_ids().get(&txn, doc_id)`.
The current code discards it after obtaining the document. The handler must retain it for
the filter check.

### Pattern 4: Non-Tenant Bypass (DOCS-05)

**What:** The `is_tenant_token()` guard on the fail-closed check naturally bypasses all
`indexRules` logic for API keys.
**When to use:** Implicitly — `is_tenant_token()` returns `false` for master-key and
API-key auth flows, so the guard block is never entered.

```rust
// is_tenant_token() returns false for API keys -> block never entered -> no regression
if index_scheduler.filters().is_tenant_token() {
    // ... indexRules logic, invisible to admin keys
}
```

### Pattern 5: Accessing `filters()` from a Handler

**What:** The `GuardedData` extractor exposes `.filters()` which returns `&AuthFilter`.
In every document handler, `index_scheduler` is of type
`GuardedData<ActionPolicy<{...}>, Data<IndexScheduler>>`.

```rust
// Source: crates/meilisearch/src/extractors/authentication/mod.rs:25
// Already used in search.rs — mirrors exactly:
if let Some(search_rules) = index_scheduler.filters().get_index_search_rules(&index_uid) {
    add_search_rules(&mut query.filter, search_rules);
}
```

The same `.filters()` call works in document handlers without any additional plumbing.

### Architecture Impact on `documents_by_query()`

`documents_by_query()` is currently a free function that does NOT have access to `filters()`:

```rust
fn documents_by_query(
    index_scheduler: &IndexScheduler,    // <-- raw IndexScheduler, not GuardedData
    index_uid: web::Path<String>,
    query: BrowseQuery,
) -> Result<HttpResponse, ResponseError>
```

It receives `&IndexScheduler` (via `.as_ref()` or `.into_inner()` in the caller), not the
`GuardedData` wrapper. The `AuthFilter` is not accessible here. There are two options:

**Option A (recommended):** Move the guard + filter injection into both callers
(`get_documents()` and `documents_by_query_post()`), then pass the mutated `filter` into
`documents_by_query()`. This is minimal — `documents_by_query()` is not changed at all.

**Option B:** Pass `AuthFilter` as an additional parameter to `documents_by_query()`.
This is slightly more invasive but centralizes the logic.

**Recommendation: Option A.** The callers already destructure `BrowseQuery` and have
direct access to `index_scheduler` as a `GuardedData`. Adding 4-5 lines to each caller is
simpler than changing the `documents_by_query()` signature.

For `get_document()`, the `AuthFilter` check is performed directly inside the handler —
no propagation issue exists there.

### Anti-Patterns to Avoid

- **Returning 403 on out-of-scope single-doc access:** Returns 404. 403 confirms the document exists, enabling cross-tenant document ID enumeration.
- **Calling `get_index_browse_rules()` without the fail-closed guard:** A tenant token where `get_index_browse_rules()` returns `None` (Set format whitelist with no filter) must still be allowed — that's a valid whitelist with no filter restriction. Only `None` from `is_tenant_token() == true && get_index_browse_rules() == None` (no `indexRules` claim at all) triggers the 403.
- **Confusing `None` from `get_index_browse_rules()` (not a tenant token) with `None` from missing claim:** The guard must check `is_tenant_token()` first, THEN check `get_index_browse_rules()`.
- **Injecting filters into write endpoints:** Phase 2 only touches the three read endpoints. Write routes (add, update, delete documents) are out of scope.
- **Changing `documents_by_query()` signature unnecessarily:** Option A (mutate filter in callers) avoids signature change.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Filter AND-combination | Custom string concatenation | `fuse_filters()` in `search/mod.rs` | Handles array/scalar variants, nil cases, already tested |
| Filter parsing | Custom expression parser | `parse_filter()` in `search/mod.rs` | Handles full milli filter syntax, error mapping |
| Filter evaluation | Custom document bitmap | `Filter::evaluate(&txn, index)` (milli) | Handles all filter types, returns `RoaringBitmap` |
| 403 error construction | Custom error struct | `ResponseError::from_msg(msg, Code::InvalidApiKey)` | Consistent with rest of auth layer |
| 404 error construction | Custom error struct | `MeilisearchHttpError::DocumentNotFound(doc_id)` | Already used in `retrieve_document()` |

---

## Common Pitfalls

### Pitfall 1: Returning 403 Instead of 404 for Out-of-Scope Single Document

**What goes wrong:** `get_document()` returns 403 when the document ID exists but the tenant's filter excludes it.
**Why it happens:** 403 is the intuitive "not authorized" response, but it leaks information.
**How to avoid:** Always return `DocumentNotFound` (404) when a single-doc request is filtered out. The external behavior must be identical whether the document ID doesn't exist or is outside the tenant's scope.
**Warning signs:** Integration test for DOCS-03 fails with `assert_eq!(code, 404)` but receives 403.

### Pitfall 2: Fail-Open on Missing `indexRules` Claim

**What goes wrong:** Tenant token without `indexRules` claim can browse all documents because `get_index_browse_rules()` returns `None` and no filter is applied.
**Why it happens:** `None` from `get_index_browse_rules()` is also returned for API keys (where it means "no restriction"). Treating both `None` cases the same causes fail-open.
**How to avoid:** Gate the entire `indexRules` block behind `is_tenant_token()`. When `is_tenant_token()` is true AND `get_index_browse_rules()` is `None`, return 403.
**Warning signs:** DOCS-04 test passes with `code == 200` instead of `code == 403`.

### Pitfall 3: `documents_by_query()` Not Having Access to `AuthFilter`

**What goes wrong:** Filter injection is placed inside `documents_by_query()` but `&IndexScheduler` has no `.filters()` method — only `GuardedData<_, Data<IndexScheduler>>` has it.
**Why it happens:** `documents_by_query()` receives a raw `&IndexScheduler` reference, not the `GuardedData` wrapper.
**How to avoid:** Place filter injection in the two callers (`get_documents()` and `documents_by_query_post()`), before delegating to `documents_by_query()`. Pass the mutated `query` to the helper.
**Warning signs:** Compile error: no method named `filters` found for type `&IndexScheduler`.

### Pitfall 4: `IndexBrowseRules::Set` with No Filter Rejected by Fail-Closed Guard

**What goes wrong:** A tenant token with `"indexRules": ["sales"]` (Set format, no per-index filter) is incorrectly rejected with 403 because `browse_rules.filter` is `None`.
**Why it happens:** Confusing `IndexBrowseRules { filter: None }` (valid whitelist, no restriction beyond index scope) with "no `indexRules` claim at all".
**How to avoid:** The fail-closed guard checks `get_index_browse_rules().is_none()` — which only returns `None` if the index is not authorized OR there is no `index_rules` field at all. When `IndexRules::Set` contains the index, `get_index_browse_rules()` returns `Some(IndexBrowseRules::default())` (with `filter: None`). The guard passes. The filter injection step then finds `browse_rules.filter == None` and skips filter fusion (correct — no additional filter needed).
**Warning signs:** AUTH-03 Set-format test fails with 403 after Phase 2 changes.

### Pitfall 5: `internal_id` Not Retained in `get_document()` Handler

**What goes wrong:** Phase 2 needs to evaluate the tenant filter against `internal_id` after the document is retrieved, but `internal_id` is discarded before the filter check can occur.
**Why it happens:** The current `get_document()` handler calls `retrieve_document()` which does the external→internal ID mapping internally and discards `internal_id`.
**How to avoid:** The filter check must happen either (a) inside `retrieve_document()` with the filter passed as a parameter, or (b) by inlining the external→internal lookup in the handler to retain `internal_id`. Option (b) is cleaner — the handler already has access to the index. Alternatively, `retrieve_document()` could accept an optional `allowed_ids: Option<&RoaringBitmap>` parameter.
**Warning signs:** `internal_id` is not available at the point where the filter check needs to happen.

### Pitfall 6: `features` Not Available in `get_document()` for `parse_filter()`

**What goes wrong:** `parse_filter()` requires `RoFeatures`, but `get_document()` doesn't currently receive it.
**Why it happens:** The current handler signature doesn't include `features` because the existing path doesn't need it.
**How to avoid:** Add `index_scheduler.features()` to the filter evaluation call. `index_scheduler` is already available in the handler as `GuardedData<_, Data<IndexScheduler>>` — calling `.features()` on the inner `IndexScheduler` is straightforward. The deref impl on `GuardedData` allows calling `index_scheduler.features()` directly.
**Warning signs:** Compile error: `features` not in scope when calling `parse_filter()`.

---

## Code Examples

### How search routes currently inject `searchRules` (direct template)

```rust
// Source: crates/meilisearch/src/routes/indexes/search.rs:457-460
// Tenant token search_rules.
if let Some(search_rules) = index_scheduler.filters().get_index_search_rules(&index_uid) {
    add_search_rules(&mut query.filter, search_rules);
}
```

Phase 2 mirrors this exactly, replacing `get_index_search_rules` with `get_index_browse_rules`
and using `fuse_filters` directly (since there's no `add_browse_rules` helper yet — it's a
one-liner anyway).

### `fuse_filters` function (direct reuse)

```rust
// Source: crates/meilisearch/src/search/mod.rs:1258
pub fn fuse_filters(left: Option<Value>, right: Option<Value>) -> Option<Value> {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(left), Some(right)) => {
            let left = match left {
                Value::Array(filter) => filter,
                filter => vec![filter],
            };
            let right = match right {
                Value::Array(rules_filter) => rules_filter,
                rules_filter => vec![rules_filter],
            };
            Some(Value::Array([left, right].concat()))
        }
    }
}
```

### `get_index_browse_rules()` (from Phase 1)

```rust
// Source: crates/meilisearch-auth/src/lib.rs:279-285
pub fn get_index_browse_rules(&self, index: &str) -> Option<IndexBrowseRules> {
    if !self.is_index_authorized(index) {
        return None;
    }
    let index_rules = self.index_rules.as_ref()?;
    index_rules.get_index_browse_rules(index)
}
```

Returns `None` in two cases: (1) index not authorized by the API key, (2) no `index_rules`
field on the `AuthFilter`. The fail-closed guard distinguishes these via `is_tenant_token()`.

### Current `retrieve_document()` function (for reference)

```rust
// Source: crates/meilisearch/src/routes/indexes/documents.rs:1945-1974
fn retrieve_document<S: AsRef<str>>(
    index: &Index,
    doc_id: &str,
    attributes_to_retrieve: Option<Vec<S>>,
    retrieve_vectors: RetrieveVectors,
) -> Result<Document, ResponseError> {
    let txn = index.read_txn()?;

    let internal_id = index
        .external_documents_ids()
        .get(&txn, doc_id)?
        .ok_or_else(|| MeilisearchHttpError::DocumentNotFound(doc_id.to_string()))?;

    let document = some_documents(index, &txn, Some(internal_id), retrieve_vectors)?
        .next()
        .ok_or_else(|| MeilisearchHttpError::DocumentNotFound(doc_id.to_string()))??;
    // ...
}
```

For the single-doc filter check, either (a) add an `allowed_ids: Option<&RoaringBitmap>`
parameter to `retrieve_document()` and check `!allowed_ids.contains(internal_id)`, or (b)
perform the filter evaluation in the handler before calling `retrieve_document()`. Option
(a) is preferred — it keeps the `DocumentNotFound` return path inside `retrieve_document()`
which already owns that error path.

### Complete injection pattern for `get_documents()` caller

```rust
// In get_documents() / documents_by_query_post(), BEFORE calling documents_by_query():

let index_uid_str = index_uid.as_str();  // already validated as IndexUid at this point

// Fail-closed: tenant tokens without indexRules are forbidden on document endpoints.
if index_scheduler.filters().is_tenant_token() {
    if index_scheduler.filters().get_index_browse_rules(index_uid_str).is_none() {
        return Err(ResponseError::from_msg(
            format!("The provided token does not have access to document browsing on index `{index_uid_str}`."),
            Code::InvalidApiKey,
        ));
    }
}

// Inject indexRules filter for tenant tokens that DO have the claim.
if let Some(browse_rules) = index_scheduler.filters().get_index_browse_rules(index_uid_str) {
    query.filter = fuse_filters(query.filter.take(), browse_rules.filter);
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact for This Phase |
|---|---|---|
| Tenant document access not scoped | `indexRules` filter injected at route layer | Phase 2 implements the injection |
| `documents_by_query()` ignores auth filters | Callers inject filter before calling helper | Minimal change — no signature change needed |
| `get_document()` returns document or 404 | Returns 404 also for out-of-scope docs | Indistinguishable from "not found" to tenant |

**Not deprecated:** `retrieve_document()` internals, `BrowseQuery`, `retrieve_documents()`. All unchanged.

---

## Open Questions

1. **Where exactly to check `is_tenant_token()` in `get_document()`**
   - What we know: `get_document()` has direct access to `index_scheduler.filters()`.
   - What's unclear: The `index_uid` is a plain `String` at the start of the handler (`document_param.index_uid`). It must be validated with `IndexUid::try_from()` before being used in `get_index_browse_rules()`. The guard must come after that validation.
   - Recommendation: Place the fail-closed guard after `let index_uid = IndexUid::try_from(index_uid)?;` and before `let index = index_scheduler.index(&index_uid)?`.

2. **Whether to add `allowed_ids` parameter to `retrieve_document()` or inline the check in the handler**
   - What we know: `retrieve_document()` currently owns the external→internal ID mapping and the `DocumentNotFound` path. The internal_id is computed inside and not returned.
   - Recommendation: Add `allowed_ids: Option<&RoaringBitmap>` parameter to `retrieve_document()`. This keeps the 404 response in one place. The handler computes the filter bitmap before calling `retrieve_document()` and passes it in.

3. **Error message wording for DOCS-04 403 response**
   - What we know: The existing `InvalidApiKey` error code gives: `"type": "auth"`, `"code": "invalid_api_key"`, HTTP 403.
   - What's unclear: Should the message be specific ("missing indexRules claim") or generic ("invalid key").
   - Recommendation: Use a generic message consistent with existing auth errors — specific messages aid debugging but can leak information. Since this is v1 and ISO-01 (dedicated error code) is deferred to v2, use the generic path: `ResponseError::from_msg(...)` with `Code::InvalidApiKey`.

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

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOCS-01 | `GET /indexes/{uid}/documents` with tenant JWT returns only documents matching `indexRules` filter | integration | `cargo test -p meilisearch --test auth -- index_rules_list_filtered` | ❌ Wave 0 |
| DOCS-02 | `POST /indexes/{uid}/documents/fetch` with tenant JWT returns only documents matching `indexRules` filter | integration | `cargo test -p meilisearch --test auth -- index_rules_fetch_filtered` | ❌ Wave 0 |
| DOCS-03 | `GET /indexes/{uid}/documents/{id}` returns 404 when document is outside tenant's filter scope | integration | `cargo test -p meilisearch --test auth -- index_rules_single_doc_out_of_scope` | ❌ Wave 0 |
| DOCS-04 | Tenant JWT without `indexRules` claim returns 403 on all three endpoints | integration | `cargo test -p meilisearch --test auth -- index_rules_fail_closed` | ❌ Wave 0 |
| DOCS-05 | Admin API key returns unfiltered results on all three endpoints (no regression) | integration | `cargo test -p meilisearch --test auth -- index_rules_admin_key_unaffected` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p meilisearch --test auth -- tenant_token`
- **Per wave merge:** `cargo test -p meilisearch --test auth`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/meilisearch/tests/auth/tenant_token.rs` — add 5 test functions for DOCS-01 through DOCS-05 (extend existing file — test helper `generate_tenant_token` already present)
- [ ] No new test infrastructure needed — `Server`, `server.index()`, `wait_task()` all available

*(Existing Phase 1 test infrastructure covers all prerequisites.)*

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read: `crates/meilisearch/src/routes/indexes/documents.rs` — `get_document`, `get_documents`, `documents_by_query_post`, `documents_by_query`, `retrieve_document`, `retrieve_documents`, `BrowseQuery`
- Direct codebase read: `crates/meilisearch-auth/src/lib.rs` — `AuthFilter`, `IndexRules`, `IndexBrowseRules`, `get_index_browse_rules`, `is_tenant_token` (Phase 1 output)
- Direct codebase read: `crates/meilisearch/src/extractors/authentication/mod.rs` — `GuardedData::filters()`
- Direct codebase read: `crates/meilisearch/src/routes/indexes/search.rs:457-460` — direct template for filter injection
- Direct codebase read: `crates/meilisearch/src/search/mod.rs:1254-1275` — `add_search_rules`, `fuse_filters`
- Direct codebase read: `crates/meilisearch/tests/auth/tenant_token.rs` — existing integration test patterns, `generate_tenant_token` helper

### Secondary (MEDIUM confidence)
- `.planning/phases/01-auth-foundation/01-RESEARCH.md` — confirmed Phase 1 design decisions and completed state
- `.planning/ROADMAP.md` — Phase 2 success criteria (authoritative source for DOCS-01 through DOCS-05 expected behaviors)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, all required functions verified in codebase
- Architecture: HIGH — `documents_by_query()` structure read directly; filter injection pattern confirmed from `search.rs` template; `get_index_browse_rules()` confirmed in Phase 1 output
- Pitfalls: HIGH — identified from direct code inspection (404 vs 403, `documents_by_query` lacking `AuthFilter`, `internal_id` scoping, `features` availability)

**Research date:** 2026-03-04
**Valid until:** 2026-04-04 (stable codebase, no external fast-moving deps)
