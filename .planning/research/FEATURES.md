# Feature Landscape

**Domain:** Document-level multitenancy for Meilisearch — JWT `indexRules` claim
**Researched:** 2026-03-04
**Confidence:** HIGH (based on direct codebase analysis, no external sources needed)

---

## Context: What Already Exists

The `searchRules` mechanism is the canonical pattern this feature mirrors. Understanding
it precisely is the prerequisite for understanding what `indexRules` must do.

**Current `searchRules` flow (HIGH confidence — read from source):**

1. JWT `Claims` struct carries `search_rules: SearchRules` + `exp: Option<i64>` + `api_key_uid: Uuid`
2. `authenticate_tenant_token()` only activates for `actions::SEARCH` and `actions::CHAT_COMPLETIONS`
3. Decoded `search_rules` are passed into `AuthFilter` via `get_key_filters(key_uuid, Some(search_rules))`
4. `AuthFilter.get_index_search_rules(index)` returns an `IndexSearchRules { filter: Option<Value> }`
5. Route handler calls `add_search_rules(&mut query.filter, search_rules)` which fuses filters with AND
6. The fused filter is passed down to milli for execution — tenant is completely transparent to the engine

**The gap:** `documents_by_query()` and `get_document()` handlers never call `filters()` on the
`GuardedData` extractor. Auth passes, but no tenant filter is applied to results.

---

## Table Stakes

Features users expect from a `indexRules`-based document multitenancy system. Missing
any of these makes the feature unsafe or non-functional.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| JWT `indexRules` claim parsed at authentication | Without this, tenant tokens cannot express document-read scope. It's the entry point for the entire feature. | Low | Parallel to existing `search_rules` in `Claims` struct. Add field, deserialize, wire through. |
| Index-level access control via `indexRules` | Tenant should not be able to browse documents from an index not listed in their `indexRules`, even if the index exists. | Low | Already handled by `AuthFilter.is_index_authorized()` once `indexRules` are wired into it — but a second, parallel scope check path is needed. |
| Filter injection on `GET /indexes/{uid}/documents` | The primary list-all endpoint. Without filter injection, a tenant with a scoped JWT gets all documents, defeating the purpose. | Low | Mirror the 3-line pattern from `search.rs`: call `filters().get_index_index_rules()` → `add_index_rules()` before passing `filter` to `documents_by_query`. |
| Filter injection on `POST /indexes/{uid}/documents/fetch` | Body-based equivalent of the GET endpoint. Must be consistent. | Low | Same 3-line pattern in `documents_by_query_post()`. |
| Fail-closed: no `indexRules` in JWT = 403 on document endpoints | Security default. If a tenant JWT was issued without `indexRules`, it should not silently get unrestricted document access. This is the "explicit opt-in" requirement from PROJECT.md. | Medium | Requires a policy-level check. `authenticate_tenant_token()` currently returns `NotATenantToken` for non-SEARCH actions. New path: if action == DOCUMENTS_GET and token is a tenant token but has no `indexRules` → 403. |
| `indexRules` and `searchRules` are fully independent | Operators may want to restrict search but not document reads, or vice versa. Cross-coupling would be a footgun. | Low | Structural: use a separate field in `Claims`, separate storage in `AuthFilter`, separate lookup method. No shared state. |
| `GET /indexes/{uid}/documents/{documentId}` also filtered | Single-document retrieval must respect tenant scope. A tenant should not retrieve a document by ID if that document doesn't match their filter. | Medium | This is harder than list endpoints: `retrieve_document()` fetches by primary key directly. Must either: (a) post-fetch check the tenant filter, or (b) run a filtered lookup. Option (b) (filter-based re-fetch or validation) is safer. |
| Filter syntax parity with `searchRules` | `indexRules` filter syntax must accept the same filter expressions as `searchRules`. Operators already know this syntax. | Low | Uses same `filterable_attributes` infrastructure. `fuse_filters()` already handles `Value` regardless of origin. |

---

## Differentiators

Features that go beyond the minimum viable implementation. Valuable but not required
for correctness or security.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `indexRules` as both Set and Map (matching `searchRules` shape) | Operators can either whitelist indexes by name/pattern (`Set` variant) or attach per-index filter expressions (`Map` variant). Using only the Map form would work but is less ergonomic for pure index whitelisting. | Low | `SearchRules` is already a `#[serde(untagged)]` enum. Reuse the same enum type for `indexRules` for free. |
| Wildcard index patterns in `indexRules` (e.g., `"tenant-*"`) | Operators with many per-tenant indexes don't need to enumerate each one. Pattern matching via `IndexUidPattern` already exists. | Low | Free if `SearchRules` is reused as the `IndexRules` type — `IndexUidPattern.matches_str()` already handles wildcards. |
| Meaningful error messages distinguishing `indexRules` vs `searchRules` failures | Operators debugging multitenancy issues need to know which claim caused a 403, not just "invalid token". | Low | Add new `AuthError` variants parallel to `TenantTokenAccessingnUnauthorizedIndex` for the `indexRules` path. |
| `is_tenant_token()` semantics extended to cover `indexRules` presence | Downstream code using `is_tenant_token()` to decide routing or logging should remain correct when a token has `indexRules` but no `searchRules`. | Low | Consider making `is_tenant_token()` return true if either `search_rules` or `index_rules` is present in `AuthFilter`. |

---

## Anti-Features

Things to explicitly NOT build in this milestone. Each has a clear reason.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| `indexRules` fallback to `searchRules` for document reads | Implicit coupling between two claims creates unpredictable behavior. If an operator sets `searchRules` with a filter expecting it to apply everywhere, document reads would silently inherit it — or not, depending on which token was issued when. | Treat them as fully independent. Document this explicitly. No fallback, no inheritance. |
| `indexRules` applied to search endpoints | The symmetrical mistake: if `indexRules` were injected into search queries, operators could not issue a JWT with both claims without double-filtering one endpoint. | `searchRules` owns search. `indexRules` owns document reads. Hard boundary. |
| Write-operation scope via `indexRules` | The feature is explicitly read-only multitenancy per PROJECT.md. Adding write-side filtering would massively increase scope, touch the task system, and raise entirely different security questions. | Out of scope. Document endpoints for writes (`POST`, `PUT`, `DELETE`) are unaffected. |
| Admin API key behavior changes | Admin keys bypass tenant token logic by design (`authenticate()` returns `AuthFilter::default()` when `master_key == token`). Changing this would break existing admin workflows. | Leave admin path unchanged. The feature only affects JWT tenant tokens. |
| `indexRules` on single-document GET by ID returning 404 instead of 403 | Returning 404 when a document exists but doesn't match the tenant filter would leak information (document ID enumeration is possible via 403 vs 404 distinction). | Return 403 or make the filtered lookup return "not found" uniformly — but do NOT return 403 based on document existence, and do NOT return 404 only when the filter doesn't match (that leaks ID existence). Best: run the lookup with the filter injected so the engine itself returns nothing, and surface as document_not_found consistently. |
| Token introspection endpoint for `indexRules` | Nice to have, but not needed for the feature to work correctly. Adds surface area and complexity. | Not in scope for this milestone. |

---

## Feature Dependencies

```
JWT Claims struct with indexRules field
  └── IndexRules deserialization (reuse SearchRules enum type)
        └── AuthFilter carrying index_rules: Option<IndexRules>
              ├── get_key_filters() wires indexRules into AuthFilter
              │     └── is_index_authorized() consults both search_rules AND index_rules for document endpoints
              │           └── [Differentiator] Extended is_tenant_token() semantics
              │
              ├── get_index_index_rules(index) → Option<IndexSearchRules>
              │     ├── Filter injection in GET /indexes/{uid}/documents
              │     ├── Filter injection in POST /indexes/{uid}/documents/fetch
              │     └── Filter injection / post-fetch validation in GET /indexes/{uid}/documents/{documentId}
              │
              └── authenticate_tenant_token() extended to handle DOCUMENTS_GET action
                    └── Fail-closed: no indexRules + is_tenant_token + DOCUMENTS_GET action → 403
```

**Key dependency order:**
1. `Claims` struct change (auth crate) — everything depends on this
2. `AuthFilter` extension (auth crate) — carry the new field
3. `authenticate_tenant_token()` extended (auth extractor) — new action handling
4. `get_key_filters()` plumbing (auth crate) — wire `index_rules` through
5. Route handler changes (documents.rs) — consume the filter, inject it

---

## MVP Recommendation

Build exactly what PROJECT.md specifies. No more, no less.

**Prioritize:**
1. JWT `Claims` struct gains `index_rules: Option<IndexRules>` field (reuse `SearchRules` type)
2. `AuthFilter` carries `index_rules: Option<IndexRules>` alongside existing `search_rules`
3. `get_key_filters()` accepts and stores `index_rules`
4. `authenticate_tenant_token()` activates for `DOCUMENTS_GET` action, extracts `index_rules`
5. Fail-closed check: tenant token + DOCUMENTS_GET + no `index_rules` → 403
6. `get_index_index_rules()` method on `AuthFilter` (mirror of `get_index_search_rules()`)
7. Filter injection in `get_documents()` and `documents_by_query_post()` — 3 lines each
8. Single-document `get_document()` filtered lookup — slightly more involved, see anti-features note on 404 vs 403

**Defer:**
- Extended `is_tenant_token()` semantics: can be addressed in a follow-up, low risk
- New `AuthError` variants for `indexRules`-specific messages: nice-to-have, not blocking correctness
- Any write-side changes: explicitly out of scope per PROJECT.md

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Table stakes features | HIGH | Derived from direct codebase read of `auth/src/lib.rs`, `extractors/authentication/mod.rs`, `routes/indexes/documents.rs`, and `search/mod.rs` |
| Differentiators | HIGH | Structural — based on existing `SearchRules` enum reuse potential, confirmed from type definitions |
| Anti-features | HIGH | Derived from explicit PROJECT.md out-of-scope decisions + security reasoning from auth code |
| Feature dependencies | HIGH | Traced through actual call graph in source |

---

## Sources

All findings are HIGH confidence — derived from direct source analysis, no external research required.

- `crates/meilisearch-auth/src/lib.rs` — `SearchRules`, `IndexSearchRules`, `AuthFilter`, `get_key_filters()`
- `crates/meilisearch/src/extractors/authentication/mod.rs` — `Claims`, `ActionPolicy`, `authenticate_tenant_token()`, `TenantTokenOutcome`
- `crates/meilisearch/src/routes/indexes/documents.rs` — `get_documents()`, `documents_by_query_post()`, `get_document()`, `documents_by_query()`
- `crates/meilisearch/src/search/mod.rs` — `add_search_rules()`, `fuse_filters()`
- `crates/meilisearch/src/routes/indexes/search.rs` — canonical 3-line filter injection pattern (lines 457-459, 670-672)
- `.planning/PROJECT.md` — requirements, constraints, out-of-scope decisions
