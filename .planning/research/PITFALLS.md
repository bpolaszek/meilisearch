# Domain Pitfalls: JWT-based Document Multitenancy

**Domain:** Extending JWT tenant token filtering to Meilisearch document retrieval endpoints
**Researched:** 2026-03-04
**Confidence:** HIGH — All findings derived directly from codebase inspection, not training data

---

## Critical Pitfalls

Mistakes that cause rewrites, security holes, or complete feature failure.

---

### Pitfall 1: Tenant Token Gate Hardcoded to SEARCH and CHAT_COMPLETIONS Only

**What goes wrong:** The JWT tenant token validation path in `authenticate_tenant_token` explicitly
returns `NotATenantToken` for any action that is not `actions::SEARCH` or `actions::CHAT_COMPLETIONS`.
This means a JWT with any `indexRules` claim is silently treated as a plain API key when used on
`DOCUMENTS_GET` endpoints. No filter is injected. No 403 is returned. The tenant token works —
but as if it carried no rules at all.

**Root cause:**
```rust
// crates/meilisearch/src/extractors/authentication/mod.rs:309
fn authenticate_tenant_token(...) -> Result<TenantTokenOutcome, AuthError> {
    // Only search and chat actions can be accessed by a tenant token.
    if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS {
        return Ok(TenantTokenOutcome::NotATenantToken);
    }
    // ...
}
```

**Consequences:**
- A tenant token carrying `indexRules` is accepted on `GET /indexes/{uid}/documents` but the filter
  is NEVER applied — tenants see all documents regardless of their claims.
- The silently-bypassed security constraint is the worst kind: no error, no panic, just wrong data.

**Prevention:**
- Extend the `if A != ...` guard to also allow `actions::DOCUMENTS_GET` when the `indexRules`
  claim is present. The cleanest approach is to add `DOCUMENTS_GET` to the allowed action set and
  introduce a parallel `IndexRules` type modelled after `SearchRules` / `IndexSearchRules`.
- Add an integration test: JWT with `indexRules` on a document endpoint must inject the filter and
  must NOT expose documents that do not match it.

**Detection:** A test that fetches `/documents` with a tenant token restricted to `tenant = "A"`
and receives documents for tenant `"B"` reveals this immediately.

**Phase:** Must be fixed before any other work. This is the load-bearing architectural change.

---

### Pitfall 2: `documents_by_query` Never Reads `AuthFilter` — Filter Injection Missing

**What goes wrong:** Even after fixing Pitfall 1, there is a second independent gap. The
`documents_by_query` function (which handles both `GET /indexes/{uid}/documents` and
`POST /indexes/{uid}/documents/fetch`) accepts the `GuardedData<ActionPolicy<DOCUMENTS_GET>, ...>`
wrapper but never calls `index_scheduler.filters().get_index_search_rules(...)`. The `AuthFilter`
is not consulted at all for filter injection.

**Root cause:**
```rust
// crates/meilisearch/src/routes/indexes/documents.rs:702
fn documents_by_query(
    index_scheduler: &IndexScheduler,
    index_uid: web::Path<String>,
    query: BrowseQuery,
) -> Result<HttpResponse, ResponseError> {
    // ...
    let (total, documents) = retrieve_documents(
        &index,
        offset,
        limit,
        ids,
        filter,   // <-- user-supplied filter only, no tenant filter merged in
        // ...
    )?;
```

Compare with the existing pattern in `search.rs:458`:
```rust
// Tenant token search_rules.
if let Some(search_rules) = index_scheduler.filters().get_index_search_rules(&index_uid) {
    add_search_rules(&mut query.filter, search_rules);
}
```

**Consequences:**
- Even with Pitfall 1 fixed, document endpoints bypass all tenant filtering. This is a
  complete security bypass for the feature.

**Prevention:**
- Before calling `retrieve_documents`, call `index_scheduler.filters().get_index_rules(...)` and
  fuse the resulting filter into the query filter using `fuse_filters` (already exists in
  `crates/meilisearch/src/search/mod.rs:1258`). Mirror the search endpoint pattern exactly.
- Apply the same pattern to BOTH callsites: `get_documents` (GET variant) and
  `documents_by_query_post` (POST variant).

**Detection:** Test: execute `GET /indexes/{uid}/documents` with a tenant JWT whose `indexRules`
restrict `tenant_id = "A"`. Response must not contain documents with `tenant_id = "B"`.

**Phase:** Core implementation phase — the first real code change after architecture decisions.

---

### Pitfall 3: `get_document` (Single Document by ID) Bypasses Tenant Filter Entirely

**What goes wrong:** `GET /indexes/{uid}/documents/{document_id}` calls `retrieve_document`, which
does a raw LMDB lookup by external document ID with zero filter evaluation. If a tenant token
restricts documents to `tenant_id = "A"`, a client can still fetch any document by guessing its
ID — even documents belonging to other tenants.

**Root cause:**
```rust
// crates/meilisearch/src/routes/indexes/documents.rs:1945
fn retrieve_document<S: AsRef<str>>(
    index: &Index,
    doc_id: &str,
    // ...
) -> Result<Document, ResponseError> {
    let internal_id = index
        .external_documents_ids()
        .get(&txn, doc_id)?
        .ok_or_else(|| MeilisearchHttpError::DocumentNotFound(doc_id.to_string()))?;
    // direct read, no filter evaluation
```

There is no mechanism in `retrieve_document` to evaluate a filter against a single known document ID.

**Consequences:**
- A tenant with token scoped to their own data can enumerate cross-tenant documents by ID.
  This is a data exfiltration vector if document IDs are guessable (sequential integers, UUIDs
  derived from known tenant data, etc.).

**Prevention:**
Option A (recommended): After fetching the document, evaluate the tenant filter against it by
checking whether the document's internal ID is in the filter's RoaringBitmap result set. The
milli `Filter::evaluate` API already returns a `RoaringBitmap` — check membership before returning.
Option B: Rewrite the single-document path to call `retrieve_documents` with `ids = [doc_id]`
and the tenant filter applied, then return 404 if the result set is empty. This reuses existing
filter infrastructure at the cost of slightly more overhead.

Do NOT return a 403 that reveals the document exists. Return 404 to avoid confirming existence.

**Detection:** Test: create doc with `tenant_id = "B"`, fetch it with JWT scoped to `tenant_id = "A"`.
Expected: 404. Failure: 200 with document body.

**Phase:** Core implementation phase — must be addressed alongside batch document filtering.

---

### Pitfall 4: `fuse_filters` Semantics — AND Logic, Not OR

**What goes wrong:** When both a user-supplied filter and a tenant `indexRules` filter are
present, `fuse_filters` merges them as a JSON array `[userFilter, tenantFilter]`. In milli's
filter parser, an array of conditions is evaluated as a logical AND.

This is correct semantics — you want `(user filter) AND (tenant restriction)` — but it is easy to
accidentally invert the logic when writing filter values in the JWT claim.

**Root cause:**
```rust
// crates/meilisearch/src/search/mod.rs:1262
(Some(left), Some(right)) => {
    // ...
    Some(Value::Array([left, right].concat()))
    // Array = AND in milli filter evaluation
}
```

**Consequences:**
- If the JWT claim contains `filter: "tenant_id = 'A' OR tenant_id = 'B'"`, the OR is preserved
  correctly. But if someone writes `filter: ["tenant_id = 'A'", "tenant_id = 'B'"]` (an array),
  this means AND between the two conditions — which will match nothing unless a document belongs
  to BOTH tenants simultaneously.
- The filter value in `IndexSearchRules` / future `IndexRules` is an opaque `serde_json::Value`.
  No structural validation is done on it at JWT decode time.

**Prevention:**
- Document clearly that the `filter` field in `indexRules` must use `OR` at the top level if
  multiple values are permitted per tenant. Do not use arrays for multi-value tenant filters.
- Add a validation step when decoding the `indexRules` claim: parse the filter value through
  `parse_filter` at auth time and reject the JWT if the filter is syntactically invalid. This
  also prevents filter injection via malformed JWT claims.

**Detection:** Test: claim with `filter: ["tenant_id = 'A'", "tenant_id = 'A'"]` (AND with itself)
must return the same documents as `filter: "tenant_id = 'A'"`. If it returns empty, the AND
fusion is being applied wrongly.

**Phase:** Implementation and testing phases.

---

### Pitfall 5: Missing `indexRules` Claim Must Return 403 — Fail-Closed Requires Explicit Guard

**What goes wrong:** The project spec states that a JWT without `indexRules` must return 403 on
document endpoints. But the existing auth infrastructure does not distinguish "claim absent" from
"claim present with wildcard filter". By default, `AuthFilter.get_index_search_rules()` returns
a permissive default when `search_rules` is None (no tenant token). The same will apply to
`indexRules` unless an explicit check is added.

**Root cause:**
```rust
// crates/meilisearch-auth/src/lib.rs:262
pub fn get_index_search_rules(&self, index: &str) -> Option<IndexSearchRules> {
    if !self.is_index_authorized(index) {
        return None;
    }
    let search_rules = self.search_rules.as_ref().unwrap_or(&self.key_authorized_indexes);
    search_rules.get_index_search_rules(index)
    // Falls back to key_authorized_indexes when search_rules is None
    // → no tenant token = no filter restriction = all documents visible
}
```

For `indexRules`, a symmetric new field will be introduced. If the absence check is not
explicit, a plain API key (not a JWT) will bypass the 403 gate entirely.

**Consequences:**
- A non-tenant API key with `documents.get` permission can call document endpoints freely.
  This is likely intentional for admin scenarios, BUT the design decision must be explicit.
  If the intent is "tenant tokens MUST carry indexRules", the guard must check
  `is_tenant_token()` AND `index_rules.is_some()` — not just `index_rules.is_some()`.

**Prevention:**
- Add an explicit guard at the start of `documents_by_query`:
  1. If the caller is a tenant token (`auth_filter.is_tenant_token() == true`):
     check that `index_rules` is present; return 403 if absent.
  2. If the caller is a plain API key: let through unconditionally (admin scenario).
- Write a test for each case: tenant JWT without `indexRules` → 403; admin API key → 200.

**Detection:** Test: JWT signed with a valid API key UID but no `indexRules` field → must return 403.

**Phase:** Implementation phase — the fail-closed default is a security constraint, not an
optimization.

---

## Moderate Pitfalls

---

### Pitfall 6: `IndexRules` vs `SearchRules` Type Reuse — Confusion Risk

**What goes wrong:** The temptation to reuse `SearchRules` / `IndexSearchRules` types for
`indexRules` is strong because the structures are identical (`filter: Option<serde_json::Value>`).
However, aliasing or reusing them creates two problems:
1. `AuthFilter` already stores `search_rules: Option<SearchRules>` and `key_authorized_indexes: SearchRules`.
   Adding `index_rules` as another `Option<SearchRules>` creates a confusing field where the
   name carries all the semantic meaning.
2. Future divergence (e.g., adding `attributes_to_retrieve` to `indexRules` but not `searchRules`)
   becomes a breaking refactor.

**Prevention:**
- Define a distinct `IndexRules` enum and `IndexDocumentRules` struct (parallel to `SearchRules`
  and `IndexSearchRules`) even if they start identical. The type distinction in `AuthFilter` makes
  intent explicit at the type level, matching the existing `Settings<Checked>` vs
  `Settings<Unchecked>` pattern in the codebase.

**Phase:** Architecture / initial implementation phase.

---

### Pitfall 7: `Claims` Struct Deserialization — Unknown Fields Silently Ignored

**What goes wrong:** The `Claims` struct decoded from the JWT is:
```rust
struct Claims {
    search_rules: SearchRules,
    exp: Option<i64>,
    api_key_uid: Uuid,
}
```
`serde` with `#[serde(rename_all = "camelCase")]` will deserialize successfully even if the JWT
contains an `indexRules` field — it will simply be ignored. This means a client sending a JWT
with `indexRules` against the current codebase gets silently no-op behavior instead of a clear
error.

**Consequences:**
- During rollout, clients with the new JWT format calling the old server version will not get
  an error — they will get unfiltered results. This is a silent security regression during
  version transitions.

**Prevention:**
- The new `Claims` struct must include `index_rules: Option<IndexRules>` as an explicit field.
  The claim is absent → `None` → triggers the fail-closed 403 (Pitfall 5 guard).
  The claim is present → parsed and applied.
- Consider adding `#[serde(deny_unknown_fields)]` to `Claims` if backward compatibility with
  pre-existing token issuers is not required.

**Phase:** Implementation phase — tied to the `Claims` struct modification.

---

### Pitfall 8: `total` Count in Pagination Reflects Filtered Set — But Leaks Tenant Shape

**What goes wrong:** `retrieve_documents` returns `(total, documents)` where `total` is the
cardinality of the `candidates` RoaringBitmap AFTER filter application. This is correct for
pagination, but it means the total count of documents visible to a tenant is exposed in the
response's `total` field.

If the tenant filter is `tenant_id = "A"`, the `total` field reveals how many documents tenant A
has in the index. This may be acceptable depending on the data model, but it can also leak
information (e.g., a user with token `tenant_id = "A"` can infer the total document count for
other tenants by probing with different filter values in the user-supplied filter field).

**Prevention:**
- Ensure the tenant filter is AND-fused BEFORE counting — `fuse_filters` already ensures this,
  so the count is always bounded to the tenant's visible set.
- Document this behavior explicitly in the feature spec. If cross-tenant enumeration via total
  count is a concern, consider rounding or omitting `total` for tenant token requests.

**Phase:** Implementation review phase.

---

### Pitfall 9: `IndexUidPattern` Wildcard Matching on Document Endpoints

**What goes wrong:** `SearchRules` supports wildcard index patterns (`*`, prefix patterns like
`products-*`). The `is_index_authorized` check in `AuthFilter` evaluates wildcards correctly.
However, the new `indexRules` must apply the same wildcard semantics when resolving which
filter to apply to a given index.

If `indexRules` is:
```json
{
  "products-*": { "filter": "tenant_id = 'A'" },
  "orders": { "filter": "owner = 'A'" }
}
```
The correct index-specific filter must be resolved, not a single catch-all filter. Forgetting
to use `get_index_search_rules` (or the equivalent `get_index_document_rules`) and instead
using a single flat filter will apply the wrong rule to pattern-matched indexes.

**Prevention:**
- Mirror the `get_index_search_rules` resolution logic exactly for `indexRules`. The
  `max_by_key(|(pattern, _)| (pattern.is_exact(), pattern.len()))` heuristic in
  `SearchRules::get_index_search_rules` picks the most specific matching pattern. Reuse or
  refactor this logic rather than reimplementing it.

**Phase:** Implementation phase.

---

## Minor Pitfalls

---

### Pitfall 10: `parse_filter` Error Code Mismatch

**What goes wrong:** `retrieve_documents` calls `parse_filter(filter, Code::InvalidDocumentFilter, features)`.
When the injected tenant filter is syntactically invalid (e.g., the JWT claim contained a
malformed filter expression), this will return `Code::InvalidDocumentFilter` — which implies
the *user* submitted a bad filter. This misleads both the client and any observability tooling.

**Prevention:**
- Validate the tenant filter value at JWT decode time (see Pitfall 4 prevention), not at
  query execution time. A bad tenant filter in the JWT should cause a 401/403 at auth time,
  not a 400 InvalidDocumentFilter at execution time.

**Phase:** Implementation phase.

---

### Pitfall 11: `filterable_attributes` Must Be Configured on Each Index

**What goes wrong:** Milli filter evaluation silently returns an empty bitmap (no matching
documents) if the filtered attribute is not in `filterable_attributes` for the index. If an
operator configures `indexRules` with `filter: "tenant_id = 'A'"` but forgets to add
`tenant_id` to the index's `filterable_attributes` settings, the endpoint returns zero
documents instead of an error.

**Consequences:**
- Silent data loss from the tenant's perspective: they authenticate correctly, their filter is
  applied, but the filter returns nothing because the attribute is not filterable. This is
  indistinguishable from "you have no documents" at the API level.

**Prevention:**
- Document the prerequisite: `filterable_attributes` must include the tenant discriminator field.
- At filter injection time, consider checking whether the attributes referenced in the
  `indexRules` filter are in the index's `filterable_attributes` and returning a clear error
  if not. (This check is optional — it adds a database read per request — but aids operators.)

**Phase:** Documentation and testing phases. Operational concern, not a code defect.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Auth layer changes (`authenticate_tenant_token`) | Pitfall 1: silent bypass of tenant token gate | Extend allowed action set to include `DOCUMENTS_GET`; add integration test before merging |
| Filter injection in `documents_by_query` | Pitfall 2: filter never applied | Mirror search.rs pattern exactly; test with data that crosses tenant boundaries |
| Single-document endpoint (`get_document`) | Pitfall 3: ID-based bypass | Post-fetch membership check or redirect through filtered path; return 404 not 403 |
| `Claims` struct modification | Pitfall 7: silent deserialization of new field | Add `index_rules` field explicitly; test old JWT format against new server |
| Fail-closed 403 logic | Pitfall 5: missing claim defaults to permissive | Explicit `is_tenant_token()` check; test tenant JWT without `indexRules` → 403 |
| `IndexRules` type definition | Pitfall 6: reuse of `SearchRules` type | Define distinct types from the start; refactoring later is costly |
| Pagination `total` field | Pitfall 8: count exposes tenant dataset size | Intentional decision; document it explicitly in spec |
| Index wildcard patterns | Pitfall 9: wrong per-index filter resolution | Reuse `get_index_search_rules` logic; test with wildcard-scoped `indexRules` |

---

## Sources

All findings are sourced directly from the Meilisearch codebase (confidence: HIGH):

- `crates/meilisearch/src/extractors/authentication/mod.rs` — JWT decode, `authenticate_tenant_token`, `Claims` struct, action gate
- `crates/meilisearch-auth/src/lib.rs` — `AuthFilter`, `SearchRules`, `IndexSearchRules`, `get_index_search_rules`, `fuse_filters`
- `crates/meilisearch/src/routes/indexes/documents.rs` — `get_document`, `get_documents`, `documents_by_query_post`, `documents_by_query`, `retrieve_documents`, `retrieve_document`
- `crates/meilisearch/src/routes/indexes/search.rs:458` — Reference implementation of filter injection via `add_search_rules`
- `crates/meilisearch/src/search/mod.rs:1254` — `add_search_rules` and `fuse_filters` implementations
- `crates/meilisearch-types/src/keys.rs` — `actions::DOCUMENTS_GET`, `actions::SEARCH`, `actions::CHAT_COMPLETIONS` constants
