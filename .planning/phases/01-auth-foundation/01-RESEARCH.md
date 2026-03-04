# Phase 1: Auth Foundation - Research

**Researched:** 2026-03-04
**Domain:** Rust / Meilisearch JWT tenant token authentication layer
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AUTH-01 | JWT tenant tokens can carry an `indexRules` claim using the same structure as `searchRules` | `Claims` struct in `policies` module must gain an `index_rules` field; `TenantTokenOutcome::Valid` must carry it |
| AUTH-02 | `indexRules` and `searchRules` are parsed independently — no cross-interaction | `AuthFilter` gets a separate `index_rules: Option<IndexRules>` field; never merged with `search_rules` |
| AUTH-03 | `indexRules` supports both index whitelisting and per-index filter expressions | `IndexRules` is a new type with identical variants to `SearchRules` (`Set` / `Map`) — reused deserialization pattern |
</phase_requirements>

---

## Summary

Phase 1 is a pure auth-layer change. The entire work lives in two files:
`crates/meilisearch-auth/src/lib.rs` and
`crates/meilisearch/src/extractors/authentication/mod.rs`.
No routes, no document logic, no engine changes are touched in this phase.

The existing `searchRules` machinery is the exact template. The new `indexRules`
claim follows the same deserialization (`serde` untagged enum `Set`/`Map`) and
lives in its own distinct field on `AuthFilter` and `Claims`. The only gate change
is expanding `authenticate_tenant_token` to accept `DOCUMENTS_GET` alongside
`SEARCH` and `CHAT_COMPLETIONS`.

**Primary recommendation:** Mirror `SearchRules`/`IndexSearchRules` into a new
`IndexRules`/`IndexBrowseRules` pair; add `index_rules: Option<IndexRules>` to
both `Claims` and `AuthFilter`; add `get_index_browse_rules()` to `AuthFilter`;
extend the action gate to include `DOCUMENTS_GET`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `jsonwebtoken` | workspace | JWT decode/validate | Already used in `policies` module |
| `serde` / `serde_json` | workspace | JWT claim deserialization | Existing pattern for `SearchRules` |
| `uuid` | workspace | `api_key_uid` claim type | Already in `Claims` struct |

No new dependencies are required for this phase.

---

## Architecture Patterns

### Key File Map

```
crates/
├── meilisearch-auth/src/lib.rs          # AuthFilter, SearchRules, IndexSearchRules
│                                         # ADD: IndexRules, IndexBrowseRules
│                                         # MODIFY: AuthFilter, AuthController::get_key_filters()
└── meilisearch/src/extractors/
    └── authentication/mod.rs            # TenantTokenOutcome, Claims, authenticate_tenant_token
                                          # MODIFY: all three
```

### Pattern 1: Adding `IndexRules` type (mirrors `SearchRules`)

**What:** New public enum in `meilisearch-auth/src/lib.rs`, structurally identical to `SearchRules`.
**When to use:** Anywhere `indexRules` claim needs to be held — `Claims`, `AuthFilter`.

```rust
// Source: crates/meilisearch-auth/src/lib.rs (existing SearchRules as template)

/// Transparent wrapper for index browse rules.
/// Same shape as SearchRules — Set for whitelist, Map for per-index filters.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum IndexRules {
    Set(HashSet<IndexUidPattern>),
    Map(HashMap<IndexUidPattern, Option<IndexBrowseRules>>),
}

impl Default for IndexRules {
    fn default() -> Self {
        Self::Set(hashset! { IndexUidPattern::all() })
    }
}

/// Per-index filter expression for document browsing.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct IndexBrowseRules {
    pub filter: Option<serde_json::Value>,
}
```

### Pattern 2: `AuthFilter` extension

**What:** Add `index_rules: Option<IndexRules>` as a separate, independent field.
**Critical invariant:** Setting `index_rules` never touches `search_rules` and vice versa.

```rust
// Source: crates/meilisearch-auth/src/lib.rs (AuthFilter struct)

#[derive(Debug)]
pub struct AuthFilter {
    search_rules: Option<SearchRules>,           // unchanged
    index_rules: Option<IndexRules>,             // NEW — independent field
    key_authorized_indexes: SearchRules,         // unchanged
    allow_index_creation: bool,                  // unchanged
}

impl AuthFilter {
    // New accessor — used by document routes in Phase 2
    pub fn get_index_browse_rules(&self, index: &str) -> Option<IndexBrowseRules> {
        if !self.is_index_authorized(index) {
            return None;
        }
        let index_rules = self.index_rules.as_ref()?;
        index_rules.get_index_browse_rules(index)
    }
}
```

### Pattern 3: `Claims` struct extension

**What:** Add optional `index_rules` field to the JWT claims struct in `policies` module.
`#[serde(default)]` ensures backward compat — existing JWTs without the claim deserialize cleanly.

```rust
// Source: crates/meilisearch/src/extractors/authentication/mod.rs (Claims struct)

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    search_rules: SearchRules,
    #[serde(default)]              // backward compat: absent => None
    index_rules: Option<IndexRules>,
    exp: Option<i64>,
    api_key_uid: Uuid,
}
```

### Pattern 4: `TenantTokenOutcome::Valid` extension

**What:** Carry `index_rules` alongside `search_rules` in the valid outcome variant.
**Decision from STATE.md:** Extend existing variant (simpler call sites) rather than adding a new variant.

```rust
// Source: crates/meilisearch/src/extractors/authentication/mod.rs

enum TenantTokenOutcome {
    NotATenantToken,
    Valid(Uuid, SearchRules, Option<IndexRules>),   // extended
}
```

Downstream, in `authenticate_tenant_token`:
```rust
Ok(TenantTokenOutcome::Valid(uid, data.claims.search_rules, data.claims.index_rules))
```

And in `ActionPolicy::authenticate`:
```rust
Ok(TenantTokenOutcome::Valid(key_uuid, search_rules, index_rules)) => {
    (key_uuid, Some(search_rules), index_rules)
}
```

### Pattern 5: Action gate extension

**What:** `authenticate_tenant_token` currently only accepts `SEARCH` and `CHAT_COMPLETIONS`.
Extend to accept `DOCUMENTS_GET` so tenant tokens can reach document endpoints.

```rust
// Source: crates/meilisearch/src/extractors/authentication/mod.rs (line 309)

// BEFORE:
if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS {
    return Ok(TenantTokenOutcome::NotATenantToken);
}

// AFTER:
if A != actions::SEARCH
    && A != actions::CHAT_COMPLETIONS
    && A != actions::DOCUMENTS_GET
{
    return Ok(TenantTokenOutcome::NotATenantToken);
}
```

### Pattern 6: `get_key_filters` propagation

**What:** `AuthController::get_key_filters()` must thread `index_rules` into `AuthFilter`.

```rust
// Source: crates/meilisearch-auth/src/lib.rs

pub fn get_key_filters(
    &self,
    uid: Uuid,
    search_rules: Option<SearchRules>,
    index_rules: Option<IndexRules>,           // NEW param
) -> Result<AuthFilter> {
    let key = self.get_key(uid)?;
    let key_authorized_indexes = SearchRules::Set(key.indexes.into_iter().collect());
    let allow_index_creation = self.is_key_authorized(uid, Action::IndexesAdd, None)?;
    Ok(AuthFilter {
        search_rules,
        index_rules,                           // NEW field
        key_authorized_indexes,
        allow_index_creation,
    })
}
```

### Anti-Patterns to Avoid

- **Reusing `SearchRules` as `IndexRules`:** Even though the type structure is identical, using a type alias or the same type would enable accidental cross-wiring. Use a distinct type.
- **Missing `#[serde(default)]` on `index_rules` in `Claims`:** Without it, any existing JWT without `indexRules` would fail to deserialize, breaking all existing tenant tokens.
- **Putting `index_rules` logic inside `is_index_authorized`:** That method is search-path logic. Document browse authorization will be checked separately in Phase 2.
- **Adding `DOCUMENTS_GET` to the gate without threading `index_rules` through:** The gate must be extended in the same commit as the `TenantTokenOutcome` extension.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JWT decode | Custom HMAC validation | `jsonwebtoken::decode` (already present) | Handles algorithm selection, claim extraction |
| Index pattern matching | Custom glob logic | `IndexUidPattern::matches_str` (already used by `SearchRules`) | Handles `*`, prefix patterns, exact match |
| Filter fusion | Custom AND-combinator | `fuse_filters` in `search/mod.rs` | Already tested, handles array/scalar variants |
| Serde untagged enum | Manual JSON dispatch | `#[serde(untagged)]` on `IndexRules` | Exactly how `SearchRules` works |

---

## Common Pitfalls

### Pitfall 1: Missing `#[serde(default)]` on optional claim
**What goes wrong:** Existing JWTs (no `indexRules` field) fail to deserialize → all tenant tokens break.
**Why it happens:** `serde` requires the field to be present unless `default` is specified.
**How to avoid:** Add `#[serde(default)]` to the `index_rules` field in `Claims`.
**Warning signs:** Integration tests for existing `searchRules` flows returning 403.

### Pitfall 2: Breaking `get_key_filters` call sites
**What goes wrong:** Adding `index_rules` param to `get_key_filters` breaks every call site that only passes `search_rules`.
**Why it happens:** There are multiple call sites — at minimum in `ActionPolicy::authenticate`.
**How to avoid:** Grep all callers before changing the signature. Only one call site exists in `authentication/mod.rs` line 265-267.
**Warning signs:** Compile error at `auth.get_key_filters(key_uuid, search_rules)`.

### Pitfall 3: `TenantTokenOutcome::Valid` arm pattern mismatch
**What goes wrong:** Adding a third element to the tuple breaks the destructuring match arm.
**Why it happens:** Rust enum variants with tuples require exact destructuring.
**How to avoid:** Update the match arm in `ActionPolicy::authenticate` atomically with the enum change.
**Warning signs:** `error[E0023]: this pattern has 2 fields, but the corresponding tuple variant has 3 fields`.

### Pitfall 4: Action gate only partially extended
**What goes wrong:** `DOCUMENTS_GET` added to gate but `index_rules` not plumbed through → tenant tokens can reach document endpoints but `AuthFilter.index_rules` is always `None`.
**Why it happens:** Gate and outcome are modified in separate steps without verification.
**How to avoid:** Test that a JWT with `indexRules` produces a non-None `get_index_browse_rules()` result.

### Pitfall 5: `AuthFilter::default()` doesn't initialize `index_rules`
**What goes wrong:** Master key / no-auth paths panic or have unexpected `index_rules` state.
**Why it happens:** `Default` impl hardcodes fields.
**How to avoid:** Set `index_rules: None` in `AuthFilter::default()` — same as `search_rules`.

---

## Code Examples

### How search routes currently use `get_index_search_rules`
```rust
// Source: crates/meilisearch/src/routes/indexes/search.rs:458
// Tenant token search_rules.
if let Some(search_rules) = index_scheduler.filters().get_index_search_rules(&index_uid) {
    add_search_rules(&mut query.filter, search_rules);
}
```
Phase 2 will mirror this pattern using `get_index_browse_rules` on document routes.
Phase 1 only needs to make `get_index_browse_rules` available and correct.

### How `fuse_filters` works (reusable in Phase 2)
```rust
// Source: crates/meilisearch/src/search/mod.rs:1258
pub fn fuse_filters(left: Option<Value>, right: Option<Value>) -> Option<Value> {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(left), Some(right)) => {
            // Both are ANDed together as array elements
            Some(Value::Array([as_array(left), as_array(right)].concat()))
        }
    }
}
```

### Existing tenant token test helper (integration test pattern)
```rust
// Source: crates/meilisearch/tests/auth/tenant_token.rs:12
fn generate_tenant_token(
    parent_uid: impl AsRef<str>,
    parent_key: impl AsRef<str>,
    mut body: HashMap<&str, Value>,
) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let parent_uid = parent_uid.as_ref();
    body.insert("apiKeyUid", json!(parent_uid));
    encode(&Header::default(), &body, &EncodingKey::from_secret(parent_key.as_ref().as_bytes()))
        .unwrap()
}
// Usage: body includes "searchRules" and optionally "indexRules"
```

---

## State of the Art

| Old Approach | Current Approach | Impact for This Phase |
|---|---|---|
| Single `SearchRules` claim | Two independent claims (`searchRules`, `indexRules`) | Phase 1 adds the second claim slot |
| Tenant tokens only for `SEARCH` + `CHAT_COMPLETIONS` | Add `DOCUMENTS_GET` to gate | Gate expansion is atomic with claim parsing |

**Not deprecated:** The `SearchRules` / `IndexSearchRules` types and all existing search flow are untouched.

---

## Open Questions

1. **`IndexRules` method names — `get_index_browse_rules` vs `get_index_search_rules` naming**
   - What we know: `IndexSearchRules` is the existing result type for search. The parallel type for documents is new.
   - What's unclear: Whether to name the new type `IndexBrowseRules` (matches "browse" endpoint name) or `IndexDocumentRules`.
   - Recommendation: Use `IndexBrowseRules` — it maps to the `BrowseQuery` type already present in documents.rs.

2. **`is_tenant_token()` behavior when only `index_rules` is set (no `search_rules`)**
   - What we know: `is_tenant_token()` currently returns `self.search_rules.is_some()`.
   - What's unclear: Should a JWT with only `indexRules` and no `searchRules` be considered a tenant token for the `allow_index_creation` check?
   - Recommendation: Extend `is_tenant_token()` to `self.search_rules.is_some() || self.index_rules.is_some()`. A JWT is a tenant token if it carries ANY tenant claim.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `actix-rt` + `cargo test` (integration tests) |
| Config file | none — `#[actix_rt::test]` macro on each test function |
| Quick run command | `cargo test -p meilisearch --test auth -- tenant_token` |
| Full suite command | `cargo test -p meilisearch --test auth` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | JWT with `indexRules` claim is decoded and accessible via `get_index_browse_rules()` | integration | `cargo test -p meilisearch --test auth -- index_rules` | ❌ Wave 0 |
| AUTH-02 | Setting `indexRules` in JWT does not affect `searchRules` behavior; setting `searchRules` does not affect `indexRules` | integration | `cargo test -p meilisearch --test auth -- index_rules_independent` | ❌ Wave 0 |
| AUTH-03 | `IndexRules::Set` (whitelist) and `IndexRules::Map` (per-index filter) both deserialize correctly | integration | `cargo test -p meilisearch --test auth -- index_rules_formats` | ❌ Wave 0 |
| (gate) | JWT with `indexRules` is accepted on `DOCUMENTS_GET` action | integration | `cargo test -p meilisearch --test auth -- documents_get_tenant_token` | ❌ Wave 0 |
| (compile) | No regressions on existing `searchRules` path — existing tests still pass | integration | `cargo test -p meilisearch --test auth -- tenant_token` | ✅ exists |

### Sampling Rate
- **Per task commit:** `cargo test -p meilisearch --test auth -- tenant_token`
- **Per wave merge:** `cargo test -p meilisearch --test auth`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/meilisearch/tests/auth/tenant_token.rs` — add test functions for AUTH-01, AUTH-02, AUTH-03, and the `DOCUMENTS_GET` gate (extend existing file, no new file needed)

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read: `crates/meilisearch-auth/src/lib.rs` — `AuthFilter`, `SearchRules`, `IndexSearchRules`, `AuthController::get_key_filters`
- Direct codebase read: `crates/meilisearch/src/extractors/authentication/mod.rs` — `TenantTokenOutcome`, `Claims`, `ActionPolicy`, `authenticate_tenant_token`
- Direct codebase read: `crates/meilisearch-types/src/keys.rs` — `Action` enum, `actions::DOCUMENTS_GET` constant value
- Direct codebase read: `crates/meilisearch/src/routes/indexes/documents.rs` — `documents_by_query`, `retrieve_documents`, action policy usage
- Direct codebase read: `crates/meilisearch/src/search/mod.rs` — `add_search_rules`, `fuse_filters`

### Secondary (MEDIUM confidence)
- Direct codebase read: `crates/meilisearch/tests/auth/tenant_token.rs` — test patterns, `generate_tenant_token` helper

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies confirmed present in workspace, no new deps needed
- Architecture: HIGH — derived directly from existing code; `IndexRules` mirrors `SearchRules` exactly
- Pitfalls: HIGH — identified from direct code inspection (serde default, call sites, enum arity)

**Research date:** 2026-03-04
**Valid until:** 2026-04-04 (stable Rust codebase, no fast-moving external deps)
