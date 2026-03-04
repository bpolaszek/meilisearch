# Project Research Summary

**Project:** Meilisearch — Document Multitenancy (`indexRules` JWT claim)
**Domain:** Codebase extension — JWT-based multitenancy for document read endpoints
**Researched:** 2026-03-04
**Confidence:** HIGH

---

## Executive Summary

This is a surgical codebase extension, not a new feature built from scratch. Meilisearch already
implements full JWT multitenancy for search via `searchRules`. The `indexRules` claim is a
parallel mechanism that routes through the exact same pipes — JWT decode, `AuthFilter` storage,
filter fusion via `fuse_filters`, injection at the route handler level — the only structural
difference is which action unlocks the tenant token path and which route handlers consume the
resulting filter. Every building block is already present; no new crates are needed.

The recommended approach is to mirror the `searchRules` implementation precisely: add an
`index_rules: Option<IndexRules>` field to both the `Claims` JWT struct and `AuthFilter`,
extend the action gate in `authenticate_tenant_token` to allow `DOCUMENTS_GET`, thread the
decoded claim through to `AuthFilter` via an extended `get_key_filters()`, and inject the
resulting filter in `get_documents` and `documents_by_query_post` using the existing
`add_search_rules` / `fuse_filters` utilities. The single-document `get_document` endpoint
requires special treatment (post-fetch tenant filter validation, returning 404 not 403) to
avoid leaking document existence to out-of-scope tenants.

The primary risk is a silent security bypass: if the action gate is not extended, a JWT
carrying `indexRules` is treated as a plain API key on document endpoints — it authenticates
successfully but no filter is applied, exposing all tenant data with no error. A second
independent risk is that `indexRules` must be fail-closed: a JWT without the claim must
return 403, not silently serve unfiltered documents. Both risks require explicit guard code
and integration tests before the feature can be considered safe to ship.

---

## Key Findings

### Recommended Stack

No new dependencies are required. The implementation touches exactly four files. Every
utility needed — JWT decode (`jsonwebtoken 10.3.0`), filter fusion (`fuse_filters` in
`search/mod.rs`), deserialization (`serde` + `serde_json`), index authorization
(`AuthFilter::is_index_authorized`) — already exists and is already exercised on the
`searchRules` path.

**Core technologies (already present):**
- `jsonwebtoken 10.3.0` — JWT HMAC decode — already used in `authenticate_tenant_token`; no change needed
- `serde` / `serde_json` — JWT claim deserialization — `Claims` struct extension adds one `Option` field
- `fuse_filters` / `add_search_rules` in `crates/meilisearch/src/search/mod.rs:1254` — filter AND-fusion — reused unchanged
- `GuardedData<ActionPolicy<A>, D>` extractor pattern — already wraps all document route handlers; exposes `filters()` for free

**Affected files:**

| File | Change |
|------|--------|
| `crates/meilisearch-auth/src/lib.rs` | Add `IndexRules` type, `index_rules` field to `AuthFilter`, `get_index_browse_rules()` method, extend `get_key_filters()` |
| `crates/meilisearch/src/extractors/authentication/mod.rs` | Add `index_rules` to `Claims`, extend action gate, extend `TenantTokenOutcome` |
| `crates/meilisearch/src/routes/indexes/documents.rs` | Inject `indexRules` filter in `get_documents` and `documents_by_query_post`; validate single-doc endpoint |
| `crates/meilisearch-types/src/error.rs` | Optional: new `Code` variant for `indexRules`-specific 403 message |

---

### Expected Features

**Must have (table stakes) — all required for correctness and security:**
- JWT `indexRules` claim parsed from `Claims` struct — entry point for the entire feature
- Filter injection on `GET /indexes/{uid}/documents` — the primary list endpoint
- Filter injection on `POST /indexes/{uid}/documents/fetch` — body-based equivalent, must be consistent
- Fail-closed: JWT without `indexRules` returns 403 on document endpoints — security default, explicit opt-in required
- `GET /indexes/{uid}/documents/{documentId}` respects tenant scope — post-fetch filter validation to prevent ID-based exfiltration
- `indexRules` and `searchRules` fully independent — separate fields in `Claims` and `AuthFilter`, no shared state

**Should have (differentiators, low effort):**
- Distinct `IndexRules` / `IndexDocumentRules` types (not aliases of `SearchRules`) — prevents future type confusion and enables independent evolution
- Wildcard index pattern support in `indexRules` — free if `SearchRules` resolution logic is reused via `get_index_search_rules`
- Meaningful error messages distinguishing `indexRules` vs `searchRules` failures — aids operator debugging

**Defer (v2+):**
- Extended `is_tenant_token()` semantics to cover `index_rules`-only tokens — low risk, can be addressed as follow-up
- Token introspection endpoint — out of scope per PROJECT.md
- Write-side filtering — explicitly out of scope per PROJECT.md

---

### Architecture Approach

The architecture is a direct extension of the existing `searchRules` data flow. The request
arrives bearing a JWT, is decoded in `authenticate_tenant_token`, the extracted `index_rules`
are carried through `TenantTokenOutcome::Valid` into `AuthFilter`, and the outer async
handlers (`get_documents`, `documents_by_query_post`) call `filters().get_index_browse_rules()`
and mutate `query.filter` before delegating to the inner `documents_by_query` helper — exactly
the pattern used by `search_with_post` at `routes/indexes/search.rs:671`. The inner helper and
all downstream code (milli filter evaluation, LMDB reads) require zero changes.

**Major components and their roles:**

1. **`Claims` struct** (`extractors/authentication/mod.rs:340`) — JWT deserialization boundary; gains `index_rules: Option<IndexRules>` field
2. **`authenticate_tenant_token` + action gate** (`mod.rs:308`) — the single line that currently blocks all `DOCUMENTS_GET` tenant token processing; must be extended to allow this action
3. **`TenantTokenOutcome`** (`mod.rs:157`) — carry `index_rules` out of JWT decode; extend `Valid` variant to carry both claims independently
4. **`AuthFilter`** (`meilisearch-auth/src/lib.rs:168`) — runtime auth context stored in `GuardedData`; gains `index_rules` field and `get_index_browse_rules()` method
5. **`get_documents` / `documents_by_query_post`** (`documents.rs:651, 566`) — injection sites; add 5-10 lines mirroring `search.rs:458-460`; 403 gate for tenant tokens without `indexRules`
6. **`get_document`** (`documents.rs`) — single-doc endpoint; requires post-fetch membership check against tenant RoaringBitmap; return 404 (not 403) to avoid leaking document existence

---

### Critical Pitfalls

1. **Silent bypass at the action gate** (Pitfall 1) — `authenticate_tenant_token` returns `NotATenantToken` for `DOCUMENTS_GET` today; a JWT with `indexRules` is accepted but treated as a plain API key with no filter applied. Prevention: extend the `if A != SEARCH && A != CHAT_COMPLETIONS` guard to include `A != DOCUMENTS_GET`; add an integration test that crosses tenant boundaries before merging.

2. **Filter injection missing in `documents_by_query`** (Pitfall 2) — even after fixing the gate, the handler never calls `filters().get_index_browse_rules()`. Prevention: mirror the 3-line injection pattern from `search.rs:458` in both `get_documents` and `documents_by_query_post`; test with cross-tenant data.

3. **Single-document endpoint ID-based exfiltration** (Pitfall 3) — `retrieve_document` does a raw LMDB lookup by external ID with no filter evaluation. Prevention: post-fetch check document membership in the tenant filter's RoaringBitmap (or redirect through `retrieve_documents` with filter applied); return 404 uniformly to avoid leaking document existence.

4. **Fail-closed logic for absent `indexRules`** (Pitfall 5) — the auth infrastructure defaults to permissive when a field is `None`; an explicit `is_tenant_token()` check is required before `get_index_browse_rules()`. Prevention: guard pattern must check `is_tenant_token()` AND `index_rules.is_some()`; test a JWT issued without `indexRules` → must return 403, not 200.

5. **`IndexRules` type aliasing** (Pitfall 6) — reusing `SearchRules` type for `index_rules` fields creates naming confusion and prevents independent evolution. Prevention: define distinct `IndexRules` / `IndexDocumentRules` types even if initially structurally identical; the Rust type system enforces intent at call sites.

---

## Implications for Roadmap

All research converges on the same dependency graph. The implementation is fully linear with
no parallel tracks possible: the JWT layer must exist before the auth layer, which must exist
before the route layer, which must exist before tests can be written meaningfully.

### Phase 1: Auth Foundation — JWT Claims and AuthFilter

**Rationale:** Everything downstream depends on `IndexRules` existing as a type and being
stored in `AuthFilter`. This is the load-bearing change. Without it, no other phase can
compile, let alone be tested.

**Delivers:** `IndexRules` type definition; `index_rules` field in `AuthFilter`; `get_index_browse_rules()` method; `get_key_filters()` extended signature.

**Addresses:** Table stakes features — JWT claim parsing, `indexRules` / `searchRules` independence.

**Avoids:** Pitfall 6 (type aliasing) — define `IndexRules` as a distinct type from the start.

**Files:** `crates/meilisearch-auth/src/lib.rs`

**Research flag:** No additional research needed — exact structural mirror of `SearchRules` precedent.

---

### Phase 2: JWT Decode and Action Gate

**Rationale:** The single gate at `mod.rs:308` is the primary architectural blocker. Until
`DOCUMENTS_GET` is admitted into the tenant token path, no JWT processing happens for document
endpoints. This phase also wires `index_rules` from `Claims` through `TenantTokenOutcome` into
`AuthFilter` via `authenticate()`.

**Delivers:** `DOCUMENTS_GET` accepted by `authenticate_tenant_token`; `Claims` struct carries `index_rules`; `TenantTokenOutcome::Valid` carries both claims independently; `AuthFilter` populated correctly for JWT requests on document endpoints.

**Addresses:** Table stakes — JWT `indexRules` claim parsed at authentication; action gate extension.

**Avoids:** Pitfall 1 (silent action gate bypass); Pitfall 7 (serde silent field ignore — explicit `Option` field prevents this).

**Files:** `crates/meilisearch/src/extractors/authentication/mod.rs`

**Research flag:** No additional research needed — all code paths identified precisely.

---

### Phase 3: Route Handler Filter Injection

**Rationale:** With the auth layer complete, the route handlers can safely call `filters().get_index_browse_rules()`. The injection pattern is a 5-10 line addition copied from `search.rs`. The single-document endpoint needs extra care (post-fetch validation, 404 semantics).

**Delivers:** Filter applied to all three document read endpoints; fail-closed 403 for tenant JWTs without `indexRules`; single-document endpoint safe against ID-based exfiltration.

**Addresses:** All table stakes features — filter injection on GET/POST list endpoints; single-doc endpoint; fail-closed behavior.

**Avoids:** Pitfall 2 (filter injection missing); Pitfall 3 (single-doc bypass); Pitfall 5 (fail-closed missing); Pitfall 4 (fuse semantics — reuse `fuse_filters` unchanged to avoid AND/OR confusion).

**Files:** `crates/meilisearch/src/routes/indexes/documents.rs`

**Research flag:** No additional research needed — injection pattern is copy-paste from search.rs with one structural difference (fail-closed vs. fail-open).

---

### Phase 4: Error Codes (Optional Enhancement)

**Rationale:** `Code::InvalidApiKey` is reusable for the 403 case and consistent with `searchRules` unauthorized access behavior. A dedicated `Code` variant improves operator UX but is not required for correctness. Keep this as a fast follow if time is constrained.

**Delivers:** Clear error messages distinguishing `indexRules`-missing 403 from generic auth failures.

**Addresses:** Differentiator — meaningful error messages.

**Files:** `crates/meilisearch-types/src/error.rs`

**Research flag:** Skip deeper research — error code addition is mechanical.

---

### Phase 5: Tests

**Rationale:** The security properties of this feature are not verifiable by code inspection alone. Integration tests crossing tenant boundaries are the only reliable way to confirm both the filter injection and the fail-closed behavior work end-to-end.

**Delivers:** Integration tests covering: (a) tenant JWT with `indexRules` → filtered results; (b) tenant JWT without `indexRules` → 403; (c) admin API key → 200 unfiltered; (d) single-doc endpoint with out-of-scope ID → 404; (e) wildcard index patterns; (f) `searchRules` path unaffected.

**Addresses:** All pitfall detection scenarios from PITFALLS.md.

**Files:** `crates/meilisearch/tests/auth/` (new file, e.g. `documents_multitenancy.rs`)

**Research flag:** Skip additional research — test patterns established in existing `tests/auth/` files.

---

### Phase Ordering Rationale

- Phase 1 before Phase 2: `AuthFilter::index_rules` must exist before `authenticate()` can store anything in it.
- Phase 2 before Phase 3: Route handlers call `filters().get_index_browse_rules()` — this method must be compiled and wired before the handler can use it.
- Phase 3 before Phase 5: Tests need the full injection chain to be meaningful; unit tests on `AuthFilter` methods can start earlier but integration tests require Phase 3 complete.
- Phase 4 is detachable: can be merged independently or deferred without blocking the feature.

### Research Flags

**Phases needing no additional research (standard patterns throughout):**
- All phases: 100% of implementation derives from direct source inspection. The `searchRules` precedent is a complete structural template. No niche patterns, no external API research needed.

**One open question requiring a decision (not research):**
- `TenantTokenOutcome::Valid` extension strategy: extend the existing variant `Valid(Uuid, SearchRules, Option<IndexRules>)` vs. add a new variant `ValidWithIndexRules(Uuid, Option<IndexRules>)`. The former is simpler at call sites for the combined JWT case; the latter preserves the independence guarantee more explicitly at the type level. Decision needed at Phase 2 start.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dependencies verified by direct `Cargo.lock` and source inspection; no inference |
| Features | HIGH | Table stakes derived from gap analysis of existing `searchRules` flow vs. document handlers |
| Architecture | HIGH | All integration points anchored to specific file:line references from source read |
| Pitfalls | HIGH | All pitfalls sourced from live code paths, not domain inference |

**Overall confidence: HIGH**

No external documentation or community sources were consulted — the entire research corpus
derives from direct Meilisearch source inspection. Confidence is therefore bounded by
codebase read accuracy rather than source reliability.

### Gaps to Address

- **`TenantTokenOutcome` extension strategy** — two valid approaches (extend existing variant vs. new variant); architectural preference needs a decision at Phase 2, not research.
- **`filterable_attributes` prerequisite documentation** — if `tenant_id` is not in `filterable_attributes`, the filter silently returns zero documents. This is an operational concern, not a code gap. Document it prominently in the PR description and user-facing docs.
- **`total` field in paginated responses** — exposes filtered set size to the tenant. Intentional per current design; the decision to round/omit `total` for tenant requests is not required for correctness but should be explicitly decided before ship.

---

## Sources

### Primary (HIGH confidence — direct source inspection)

- `crates/meilisearch-auth/src/lib.rs` — `AuthFilter`, `SearchRules`, `IndexSearchRules`, `get_key_filters`, `is_tenant_token`, `is_index_authorized`
- `crates/meilisearch/src/extractors/authentication/mod.rs` — `Claims`, `ActionPolicy`, `authenticate_tenant_token`, `TenantTokenOutcome`, action gate (line 308)
- `crates/meilisearch/src/routes/indexes/documents.rs` — `get_documents` (line 651), `documents_by_query_post` (line 566), `documents_by_query` (line 702), `retrieve_document` (line 1945)
- `crates/meilisearch/src/routes/indexes/search.rs` — reference filter injection pattern (lines 457-459, 670-672)
- `crates/meilisearch/src/search/mod.rs` — `add_search_rules`, `fuse_filters` (line 1254)
- `crates/meilisearch-types/src/keys.rs` — `actions::DOCUMENTS_GET` (value: 4), `actions::SEARCH`, `actions::CHAT_COMPLETIONS`
- `Cargo.lock` — exact crate versions: `jsonwebtoken 10.3.0`, `serde 1.0.228`, `serde_json 1.0.145`, `deserr 0.6.4`
- `.planning/PROJECT.md` — requirements, constraints, out-of-scope decisions

---

*Research completed: 2026-03-04*
*Ready for roadmap: yes*
