# Architecture Patterns

**Domain:** Document-level multitenancy via `indexRules` JWT claim
**Researched:** 2026-03-04
**Confidence:** HIGH — traced from source, all claims verified against actual code

---

## Overview

This document maps the full code path for existing `searchRules` multitenancy and identifies the exact integration points for `indexRules`. Every integration point is anchored to a specific file and line range.

---

## Existing `searchRules` Architecture (Source of Truth)

### Component Boundaries

```
HTTP Request (Bearer JWT)
        │
        ▼
┌─────────────────────────────────────────┐
│  GuardedData<ActionPolicy<A>, D>        │  crates/meilisearch/src/extractors/
│  FromRequest impl                       │  authentication/mod.rs:88
│  - Extracts token from Authorization    │
│  - Calls ActionPolicy<A>::authenticate  │
└─────────────────┬───────────────────────┘
                  │ Ok(AuthFilter)
                  ▼
┌─────────────────────────────────────────┐
│  ActionPolicy<A>::authenticate()        │  mod.rs:237–300
│  - Checks if master key                 │
│  - Calls authenticate_tenant_token()   │
│  - Calls auth.get_key_filters()         │
│  - Returns AuthFilter                   │
└─────────────────┬───────────────────────┘
                  │
        ┌─────────┴──────────┐
        │                    │
  JWT path              API key path
        │                    │
        ▼                    ▼
authenticate_tenant_token()  get_optional_uid_from_encoded_key()
mod.rs:304–337              lib.rs:79–86
- Gate: A == SEARCH         (no search_rules, None passed to
  OR A == CHAT_COMPLETIONS   get_key_filters)
- Decodes JWT HMAC HS256
- Returns (Uuid, SearchRules)
        │
        ▼
┌─────────────────────────────────────────┐
│  AuthController::get_key_filters()      │  crates/meilisearch-auth/src/lib.rs:93
│  - Fetches key from LMDB store          │
│  - Builds AuthFilter {                  │
│      search_rules: Option<SearchRules>, │  ← Some(x) if JWT, None if API key
│      key_authorized_indexes,            │
│      allow_index_creation,              │
│    }                                    │
└─────────────────┬───────────────────────┘
                  │ AuthFilter stored in GuardedData
                  ▼
┌─────────────────────────────────────────┐
│  Route handler (e.g. search_with_post)  │  routes/indexes/search.rs:671
│  Accesses: index_scheduler.filters()   │  (GuardedData::filters())
│                                         │
│  // Tenant token search_rules.          │
│  if let Some(rules) =                   │
│    filters.get_index_search_rules(uid) {│
│      add_search_rules(&mut query.filter,│
│        rules);                          │
│  }                                      │
└─────────────────┬───────────────────────┘
                  │ query.filter now ANDed with tenant rules
                  ▼
┌─────────────────────────────────────────┐
│  add_search_rules / fuse_filters        │  crates/meilisearch/src/search/mod.rs:1254
│  - Merges tenant filter with user filter│
│  - Array concat (AND semantics in milli)│
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│  perform_search → milli filter eval     │  crates/milli/src/
│  - Filter evaluated against LMDB bitmaps│
│  - Documents matching both filters only │
└─────────────────────────────────────────┘
```

### JWT Claims Struct (current)

File: `crates/meilisearch/src/extractors/authentication/mod.rs:340–346`

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    search_rules: SearchRules,
    exp: Option<i64>,
    api_key_uid: Uuid,
}
```

`SearchRules` is either a `Set(HashSet<IndexUidPattern>)` (index whitelist) or a
`Map(HashMap<IndexUidPattern, Option<IndexSearchRules>>)` (per-index filter rules).

`IndexSearchRules` (`crates/meilisearch-auth/src/lib.rs:347–350`) holds a single
`filter: Option<serde_json::Value>` — the raw filter to inject.

---

## Data Flow: `searchRules` Filter Injection

```
JWT claim: { "searchRules": { "movies": { "filter": "tenant_id = 42" } } }
    │
    ▼ decoded in authenticate_tenant_token() → Claims.search_rules
    │
    ▼ stored in AuthFilter.search_rules (Option<SearchRules>)
    │
    ▼ route handler calls filters().get_index_search_rules("movies")
      → returns Some(IndexSearchRules { filter: Some("tenant_id = 42") })
    │
    ▼ add_search_rules(&mut query.filter, rules)
      → fuse_filters(user_filter, tenant_filter)
      → [user_filter, tenant_filter] (array = AND in milli)
    │
    ▼ passed to perform_search → milli filter evaluation
```

---

## Critical Constraint: The Action Gate

File: `crates/meilisearch/src/extractors/authentication/mod.rs:308–310`

```rust
// Only search and chat actions can be accessed by a tenant token.
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS {
    return Ok(TenantTokenOutcome::NotATenantToken);
}
```

This gate is the **primary blocker** for document endpoint multitenancy. When
`A == DOCUMENTS_GET`, `authenticate_tenant_token` immediately returns
`NotATenantToken`, so `search_rules` is never parsed from the JWT. The request
falls through to the plain API key path with `search_rules: None`.

For `indexRules` to work, either:
1. The gate is extended (`A != SEARCH && A != CHAT_COMPLETIONS && A != DOCUMENTS_GET`)
   and `indexRules` is parsed alongside `searchRules` in the same `Claims`, or
2. A separate decode path is added specifically for `DOCUMENTS_GET` that reads
   `indexRules` from the JWT without touching the `searchRules` flow.

Option 2 is cleaner and preserves the independence guarantee from the project spec.

---

## Gap: Document Endpoints Have No Filter Injection Today

File: `crates/meilisearch/src/routes/indexes/documents.rs:702–755`

```rust
fn documents_by_query(
    index_scheduler: &IndexScheduler,   // ← deref of GuardedData, filters() gone
    index_uid: web::Path<String>,
    query: BrowseQuery,
) -> Result<HttpResponse, ResponseError> {
    ...
    let index = index_scheduler.index(&index_uid)?;
    let (total, documents) = retrieve_documents(&index, offset, limit, ids, filter, ...)?;
    //                                                               ^^^^^^
    //   filter comes straight from the request, no tenant injection
}
```

The outer handlers (`get_documents` at line 651, `documents_by_query_post` at
line 566) receive `GuardedData<ActionPolicy<DOCUMENTS_GET>, Data<IndexScheduler>>`
and have access to `index_scheduler.filters()`. The injection site is the same
pattern used by search — before the inner helper is called.

---

## Integration Points for `indexRules`

### Point 1: JWT `Claims` struct — add `index_rules` field

File: `crates/meilisearch/src/extractors/authentication/mod.rs:340–346`

```rust
struct Claims {
    search_rules: SearchRules,
    index_rules: Option<SearchRules>,  // NEW — optional for backward compat
    exp: Option<i64>,
    api_key_uid: Uuid,
}
```

`Option<SearchRules>` reuses the existing type (same `Map` / `Set` variants,
same `IndexSearchRules { filter }` payload).

### Point 2: `TenantTokenOutcome` — carry `index_rules` out of decode

File: `crates/meilisearch/src/extractors/authentication/mod.rs:157–160`

```rust
enum TenantTokenOutcome {
    NotATenantToken,
    Valid(Uuid, SearchRules),               // existing: search_rules
    ValidWithIndexRules(Uuid, SearchRules), // NEW: index_rules path
}
```

Alternatively, extend the existing variant to carry both:
`Valid(Uuid, SearchRules, Option<SearchRules>)` where the second `SearchRules`
is `index_rules`. This avoids a new enum arm but may be noisier at call sites.

### Point 3: Action gate — extend to allow `DOCUMENTS_GET`

File: `crates/meilisearch/src/extractors/authentication/mod.rs:308–310`

```rust
// Current
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS {
    return Ok(TenantTokenOutcome::NotATenantToken);
}

// After change
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS && A != actions::DOCUMENTS_GET {
    return Ok(TenantTokenOutcome::NotATenantToken);
}
```

When `A == DOCUMENTS_GET`, the decode proceeds but **only `index_rules`** is
returned via `TenantTokenOutcome`. `search_rules` is not populated in this path.

### Point 4: `AuthFilter` — add `index_rules` field

File: `crates/meilisearch-auth/src/lib.rs:168–183`

```rust
pub struct AuthFilter {
    search_rules: Option<SearchRules>,
    index_rules: Option<SearchRules>,       // NEW
    key_authorized_indexes: SearchRules,
    allow_index_creation: bool,
}
```

Add a `get_index_index_rules()` method (name TBD, e.g. `get_index_browse_rules`)
mirroring `get_index_search_rules()`:

```rust
pub fn get_index_browse_rules(&self, index: &str) -> Option<IndexSearchRules> {
    if !self.is_index_authorized(index) {
        return None;
    }
    self.index_rules.as_ref()?.get_index_search_rules(index)
}
```

Note: for `indexRules`, access must be fail-closed. If `index_rules` is `None`
(no `indexRules` in JWT), the document endpoint must return 403, not proceed
unfiltered. This is different from the `searchRules` model where `None` means
"no extra filter, all indexes allowed" (search is opt-in).

### Point 5: Enforcement / 403 at the document handler level

Files: `crates/meilisearch/src/routes/indexes/documents.rs:651` and `:566`

```rust
pub async fn get_documents(
    index_scheduler: GuardedData<...>,
    ...
) -> Result<HttpResponse, ResponseError> {
    ...
    // NEW: reject JWT tenant tokens without indexRules
    if index_scheduler.filters().is_tenant_token() {
        match index_scheduler.filters().get_index_browse_rules(&index_uid) {
            Some(rules) => {
                // inject filter (same pattern as add_search_rules)
                add_search_rules(&mut query.filter, rules);
            }
            None => {
                // tenant token present but no indexRules for this index
                return Err(ResponseError::from_msg(
                    "JWT tenant token must include `indexRules` to access document endpoints.",
                    Code::InvalidApiKey,  // or a new Code variant
                ));
            }
        }
    }
    documents_by_query(&index_scheduler, index_uid, query)
}
```

The `is_tenant_token()` check (`lib.rs:193`) returns `true` whenever
`search_rules` is `Some`. For `indexRules` this will need to also return `true`
when `index_rules` is `Some`. Either extend that method or use a separate
`has_index_rules()` predicate.

### Point 6: `get_key_filters` — thread `index_rules` through

File: `crates/meilisearch-auth/src/lib.rs:93–105`

The method currently accepts `search_rules: Option<SearchRules>` and stores it.
A parallel `index_rules: Option<SearchRules>` parameter needs to be threaded
through from the decode in `authenticate_tenant_token`.

---

## Recommended Build Order

Dependencies between components dictate this sequence:

1. **`Claims` + `IndexRules` type** (`meilisearch-auth` crate)
   - Add `index_rules: Option<SearchRules>` to `Claims`
   - Add `index_rules` field to `AuthFilter`
   - Add `get_key_filters` overload or extend signature
   - Add `get_index_browse_rules()` to `AuthFilter`
   - This is the foundation — all other changes depend on it

2. **Action gate + decode path** (`extractors/authentication/mod.rs`)
   - Extend `TenantTokenOutcome` to carry `index_rules`
   - Add `DOCUMENTS_GET` to the action gate condition
   - Wire `index_rules` from `Claims` into `TenantTokenOutcome`
   - Wire through `authenticate()` into `AuthFilter`

3. **Filter injection at document handlers** (`routes/indexes/documents.rs`)
   - `get_documents`: add `is_tenant_token` + `get_index_browse_rules` + `add_search_rules` call
   - `documents_by_query_post`: identical injection
   - 403 path for tenant token without `indexRules`
   - Pass `GuardedData` filters down — the inner `documents_by_query` receives
     only `&IndexScheduler` (deref of GuardedData), so injection must happen in
     the outer async handlers before that call

4. **Error codes** (if new `Code` variant chosen for the 403 case)
   - `crates/meilisearch-types/src/error.rs` — add `MissingIndexRules` or similar
   - Keep minimal: reusing `Code::InvalidApiKey` is acceptable and consistent
     with how `searchRules` unauthorized index access is reported

5. **Tests**
   - Unit: `AuthFilter::get_index_browse_rules` isolation tests
   - Integration: `crates/meilisearch/src/routes/indexes/documents.rs` test module
     or new `documents_multitenancy_test.rs`
   - Snapshot: follow `meili-snap` pattern used elsewhere

---

## Key Files and Their Roles

| File | Role | What Changes |
|------|------|-------------|
| `crates/meilisearch/src/extractors/authentication/mod.rs` | JWT decode, action gate, Claims struct | Add `index_rules` to `Claims`, extend gate, extend `TenantTokenOutcome` |
| `crates/meilisearch-auth/src/lib.rs` | `AuthFilter`, `SearchRules`, `IndexSearchRules` | Add `index_rules` field to `AuthFilter`, add `get_index_browse_rules()`, extend `get_key_filters()` |
| `crates/meilisearch/src/routes/indexes/documents.rs` | GET/POST document handlers | Inject `indexRules` filter in `get_documents` and `documents_by_query_post` |
| `crates/meilisearch/src/search/mod.rs` | `add_search_rules`, `fuse_filters` | No changes — reused as-is |
| `crates/meilisearch-types/src/error.rs` | Error codes | Optional: new `Code` variant for 403 case |

No changes needed in:
- `crates/milli/` — filter evaluation already works; `retrieve_documents` just
  passes `filter: Option<Value>` to milli unchanged
- `crates/index-scheduler/` — no task scheduling involved; document reads are
  synchronous LMDB reads, not async tasks
- Search routes — `searchRules` and `indexRules` are fully independent; search
  handlers already inject `searchRules`, nothing to change there

---

## Scalability Considerations

| Concern | Notes |
|---------|-------|
| Filter injection overhead | Negligible. `add_search_rules` / `fuse_filters` is a pure in-memory JSON value merge. No I/O, no LMDB reads at this stage. |
| Filter evaluation cost | Identical to existing `searchRules` path — milli evaluates filters against RoaringBitmap document sets. Already proven pattern. |
| JWT decode for DOCUMENTS_GET | Adds one HMAC verification on the path currently skipped (`NotATenantToken`). Symmetric HMAC is microseconds; acceptable. |

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Reusing `search_rules` for document filtering
**What:** Checking `search_rules` in document handlers instead of adding `index_rules`
**Why bad:** Creates implicit coupling; `searchRules` absence would 403 document
endpoints even for valid API keys; breaks the independence guarantee
**Instead:** Introduce `index_rules` as a fully independent field in both
`AuthFilter` and `Claims`

### Anti-Pattern 2: Injecting at `documents_by_query` level
**What:** Passing `AuthFilter` into the `documents_by_query` helper and
injecting there
**Why bad:** `documents_by_query` currently receives `&IndexScheduler` (bare deref
after `GuardedData` is consumed). Changing its signature couples the inner helper
to auth concerns, making it harder to reuse and test.
**Instead:** Inject in the outer async handlers (`get_documents`,
`documents_by_query_post`) exactly as search does it — mutate `query.filter`
before calling the inner helper

### Anti-Pattern 3: Using a new filter mechanism instead of `fuse_filters`
**What:** Building a custom filter merge for `indexRules`
**Why bad:** `fuse_filters` already handles all edge cases (None+None, None+Some,
Some+Some, array vs. scalar). Duplicating it creates divergence.
**Instead:** Call `add_search_rules(&mut query.filter, rules)` unchanged —
it is type-compatible with `IndexSearchRules` which `indexRules` also uses

---

## Sources

All findings are from direct source code inspection — no external references needed.

- `crates/meilisearch/src/extractors/authentication/mod.rs` — JWT decode, Claims, action gate
- `crates/meilisearch-auth/src/lib.rs` — AuthFilter, SearchRules, IndexSearchRules, get_key_filters
- `crates/meilisearch/src/routes/indexes/documents.rs` — document handlers, documents_by_query, retrieve_documents
- `crates/meilisearch/src/routes/indexes/search.rs` — filter injection reference pattern (lines 458–459, 671–672)
- `crates/meilisearch/src/search/mod.rs` — add_search_rules, fuse_filters (lines 1253–1275)
- `crates/meilisearch-types/src/keys.rs` — Action enum, DOCUMENTS_GET constant (value: 4)
