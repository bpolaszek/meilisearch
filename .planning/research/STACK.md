# Technology Stack — Document Multitenancy Extension

**Project:** Meilisearch `indexRules` JWT claim support
**Researched:** 2026-03-04
**Scope:** Incremental milestone — extending existing JWT multitenancy to document read endpoints

---

## Executive Summary

This is a **codebase extension, not a greenfield stack decision**. All required infrastructure already exists. No new crates are needed. The work is purely about extending data structures and wiring existing mechanisms to new code paths.

The core insight: `searchRules` in JWT already proves the full pattern end-to-end. `indexRules` is a parallel claim that routes through the same pipes — the only difference is which action gates the tenant token path and which route handlers consume the resulting filter.

---

## No New Dependencies Required

**Confidence: HIGH** — verified by direct inspection of the existing codebase.

Every building block is already present:

| Capability | Existing Crate | Where Used Today |
|------------|---------------|-----------------|
| JWT decode and validation | `jsonwebtoken 10.3.0` | `policies::authenticate_tenant_token` |
| JWT claim deserialization | `serde 1.0.228` + `serde_json 1.0.145` | `Claims` struct in `mod.rs:342` |
| Filter fusion (AND-combining filters) | `search::fuse_filters` | `add_search_rules` called in search routes |
| Index authorization check | `AuthFilter::is_index_authorized` | `ActionPolicy::authenticate` |
| Request deserialization/validation | `deserr 0.6.4` | `BrowseQuery`, `BrowseQueryGet` |
| Auth extractor pattern | `GuardedData<P, D>` | All document route handlers already use it |
| Error variants | `thiserror 1.0.69` | `AuthError` enum |

---

## Existing Stack Used by This Milestone

### Core Auth Infrastructure

| Component | Location | Version/Status | Role in This Milestone |
|-----------|----------|---------------|----------------------|
| `jsonwebtoken` | `Cargo.lock` | 10.3.0 (locked) | Decode the JWT that now carries `indexRules` claim |
| `AuthController` | `crates/meilisearch-auth/src/lib.rs` | Internal crate | Add `index_rules` field to `get_key_filters` return path |
| `AuthFilter` | `crates/meilisearch-auth/src/lib.rs:168` | Internal struct | Add `index_rules: Option<IndexRules>` field parallel to `search_rules` |
| `SearchRules` / `IndexSearchRules` | `crates/meilisearch-auth/src/lib.rs:272` | Internal enum/struct | `IndexRules` will mirror this exact structure |
| `ActionPolicy<A>` | `crates/meilisearch/src/extractors/authentication/mod.rs:228` | Internal | Extend `authenticate_tenant_token` to handle `DOCUMENTS_GET` action |
| `Claims` struct | `mod.rs:341` (private) | Internal | Add `index_rules: Option<IndexRules>` field |

### Relevant Route Infrastructure

| Component | Location | Role |
|-----------|----------|------|
| `get_documents` handler | `routes/indexes/documents.rs:651` | Inject `indexRules` filter before calling `documents_by_query` |
| `documents_by_query_post` handler | `routes/indexes/documents.rs:566` | Same injection point |
| `documents_by_query` helper | `routes/indexes/documents.rs:702` | Receives `filter: Option<Value>` — already accepts injected filters |
| `add_search_rules` / `fuse_filters` | `search/mod.rs:1254` | Reuse as-is for `indexRules` filter injection |
| `BrowseQuery.filter` field | `documents.rs:457` | The `Option<Value>` that gets fused with the tenant filter |

### Key Actions Enum

| Constant | Value | Current Tenant Token Support |
|----------|-------|------------------------------|
| `actions::SEARCH` | 1 | YES — gated in `authenticate_tenant_token` |
| `actions::CHAT_COMPLETIONS` | (non-zero) | YES — recently added alongside SEARCH |
| `actions::DOCUMENTS_GET` | 4 | **NO — explicitly returns `NotATenantToken`** |

This is the central gate to extend. Line `mod.rs:309`:
```rust
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS {
    return Ok(TenantTokenOutcome::NotATenantToken);
}
```
Must become:
```rust
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS && A != actions::DOCUMENTS_GET {
    return Ok(TenantTokenOutcome::NotATenantToken);
}
```
But the `TenantTokenOutcome::Valid` must carry both claims independently, not a merged value.

---

## Implementation Pattern — Mirror of `searchRules`

The `searchRules` mechanism is the exact template to follow. Below is the full pattern with the parallel `indexRules` extension mapped:

### Step 1 — Extend the `Claims` struct (private, in `policies` module)

```rust
// Before (mod.rs:341)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    search_rules: SearchRules,
    exp: Option<i64>,
    api_key_uid: Uuid,
}

// After
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    search_rules: SearchRules,
    index_rules: Option<IndexRules>,  // Optional — absent = no document endpoint access
    exp: Option<i64>,
    api_key_uid: Uuid,
}
```

`IndexRules` mirrors `SearchRules` in `meilisearch-auth/src/lib.rs`. The `Option<_>` wrapper encodes the fail-closed requirement: absent claim → 403, not fallback.

### Step 2 — Extend `TenantTokenOutcome` to carry `indexRules`

```rust
// Before (mod.rs:157)
enum TenantTokenOutcome {
    NotATenantToken,
    Valid(Uuid, SearchRules),
}

// After
enum TenantTokenOutcome {
    NotATenantToken,
    Valid(Uuid, SearchRules, Option<IndexRules>),
}
```

### Step 3 — Extend `AuthFilter` to hold `index_rules`

```rust
// In meilisearch-auth/src/lib.rs
pub struct AuthFilter {
    search_rules: Option<SearchRules>,
    index_rules: Option<IndexRules>,      // NEW
    key_authorized_indexes: SearchRules,
    allow_index_creation: bool,
}
```

Add a `get_index_index_rules(&self, index: &str) -> Option<IndexSearchRules>` method mirroring `get_index_search_rules`.

### Step 4 — Route handlers inject the filter

```rust
// In documents_by_query_post and get_documents (same pattern as search.rs:458-460)
if let Some(index_rules) = index_scheduler.filters().get_index_index_rules(&index_uid) {
    add_search_rules(&mut body.filter, index_rules);
} else if index_scheduler.filters().is_tenant_token() {
    // indexRules absent from a tenant token = 403
    return Err(ResponseError::from_msg(..., Code::InvalidApiKey));
}
```

`add_search_rules` and `fuse_filters` are reused as-is — they operate on `Option<Value>` and `IndexSearchRules.filter`, both of which are identical between `searchRules` and `indexRules`.

---

## What NOT to Use / Do

| Anti-Pattern | Why | What Instead |
|--------------|-----|--------------|
| New crate for JWT parsing | `jsonwebtoken 10.3.0` already present and handles HMAC-HS256/384/512 | Reuse existing `decode::<Claims>()` call |
| Middleware-level filter injection | Would require a new Actix middleware, adds complexity, breaks the existing extractor pattern | Inject in handler body, same as search routes |
| Merging `searchRules` and `indexRules` into one claim | Would break existing search multitenancy semantics; tenants expecting `searchRules` isolation would be affected | Keep fully independent as per project requirements |
| Fallback from `indexRules` to `searchRules` | Creates surprising cross-interaction; violates the explicit independence requirement | Fail-closed: absent `indexRules` on a tenant token → 403 |
| Adding `indexRules` check to `is_key_authorized` / stored API keys | Only JWT tenant tokens need this; regular API keys use the existing `key_authorized_indexes` mechanism | Scope change to `Claims` and `TenantTokenOutcome` only |
| New `GuardedData` policy type | `ActionPolicy<{ actions::DOCUMENTS_GET }>` already exists and is already used on document routes | Extend `authenticate_tenant_token` to accept `DOCUMENTS_GET`, not create a new policy |

---

## Affected Files — Precise Scope

**Confidence: HIGH** — determined by tracing the call chain through the codebase.

| File | Change Type | What Changes |
|------|-------------|--------------|
| `crates/meilisearch-auth/src/lib.rs` | Extend | Add `IndexRules` type (mirrors `SearchRules`), add `index_rules` field to `AuthFilter`, add `get_index_index_rules` method |
| `crates/meilisearch/src/extractors/authentication/mod.rs` | Extend | Add `DOCUMENTS_GET` to tenant token gate; extend `Claims` with `index_rules: Option<IndexRules>`; extend `TenantTokenOutcome` |
| `crates/meilisearch/src/routes/indexes/documents.rs` | Extend | Inject `indexRules`-derived filter in `get_documents` and `documents_by_query_post` before delegating to `documents_by_query` |
| `crates/meilisearch-types/src/` | Possibly | If `IndexRules` needs to be a shared type (likely yes, to mirror `SearchRules` which lives in `meilisearch-auth`) |

**Files deliberately NOT touched:**
- `search.rs`, `similar.rs`, `facet_search.rs` — `searchRules` injection unchanged
- `store.rs` — no persistence change needed
- `keys.rs` — no new action needed; `DOCUMENTS_GET` already exists

---

## Testing Infrastructure (Already Present)

| Capability | Where |
|------------|-------|
| Integration test harness | `crates/meilisearch/tests/` |
| Snapshot testing | `insta 1.39.0` — existing test snapshots for auth scenarios |
| HTTP mocking | `wiremock 0.6.5` — for external call tests |
| Tenant token test helpers | Existing tests in `crates/meilisearch/tests/auth/` |

New tests should follow the pattern in `crates/meilisearch/tests/auth/` — create a tenant token with specific `indexRules`, hit the document endpoints, assert filter-scoped responses and 403s for absent claims.

---

## Confidence Assessment

| Area | Confidence | Basis |
|------|------------|-------|
| No new crates needed | HIGH | All utilities verified present in codebase |
| `Claims` struct extension pattern | HIGH | `searchRules` precedent is exact structural mirror |
| `authenticate_tenant_token` gate change | HIGH | Line 309 in mod.rs is the single guard; pattern is unambiguous |
| `AuthFilter` extension | HIGH | Field addition with parallel method; no hidden dependencies |
| Route handler injection points | HIGH | `documents_by_query` takes `filter: Option<Value>` — same signature as search's injection target |
| `fuse_filters` reuse | HIGH | Function signature is generic over `Option<Value>` — works unchanged |
| 403 for absent `indexRules` on tenant token | HIGH | `is_tenant_token()` already exists on `AuthFilter` for exactly this kind of discriminant check |

---

## Sources

- Direct codebase inspection: `crates/meilisearch-auth/src/lib.rs` (AuthFilter, SearchRules, IndexSearchRules)
- Direct codebase inspection: `crates/meilisearch/src/extractors/authentication/mod.rs` (ActionPolicy, Claims, TenantTokenOutcome)
- Direct codebase inspection: `crates/meilisearch/src/routes/indexes/documents.rs` (BrowseQuery, retrieve_documents)
- Direct codebase inspection: `crates/meilisearch/src/routes/indexes/search.rs` (add_search_rules injection pattern)
- Direct codebase inspection: `crates/meilisearch/src/search/mod.rs` (add_search_rules, fuse_filters)
- `Cargo.lock` for exact crate versions
