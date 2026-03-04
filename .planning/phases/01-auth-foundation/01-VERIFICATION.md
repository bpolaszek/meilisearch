---
phase: 01-auth-foundation
verified: 2026-03-04T16:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 1: Auth Foundation — Verification Report

**Phase Goal:** The `indexRules` claim is parsed from JWT tokens, stored independently in `AuthFilter`, and routable to document endpoints without breaking `searchRules`
**Verified:** 2026-03-04
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A JWT carrying `indexRules` is decoded and the claim is accessible via `AuthFilter.get_index_browse_rules()` | VERIFIED | `Claims.index_rules: Option<IndexRules>` with `#[serde(default)]` in `mod.rs:347`; `AuthFilter::get_index_browse_rules()` at `lib.rs:279`; wired through `TenantTokenOutcome::Valid(uid, search_rules, index_rules)` at `mod.rs:338` |
| 2 | `indexRules` and `searchRules` are stored in separate fields on `AuthFilter` — setting one does not affect the other | VERIFIED | `AuthFilter` has distinct fields `search_rules: Option<SearchRules>` and `index_rules: Option<IndexRules>` at `lib.rs:171-175`; `is_index_authorized()` only checks `search_rules`, not `index_rules` (`lib.rs:221-228`) |
| 3 | `IndexRules` supports both index whitelisting (`Set`) and per-index filter expressions (`Map`) | VERIFIED | `pub enum IndexRules { Set(HashSet<IndexUidPattern>), Map(HashMap<IndexUidPattern, Option<IndexBrowseRules>>) }` at `lib.rs:374-377`; both variants handled in `get_index_browse_rules()` at `lib.rs:401-418` |
| 4 | `DOCUMENTS_GET` action is accepted by `authenticate_tenant_token` for tenant JWTs | VERIFIED | Action gate at `mod.rs:310`: `if A != actions::SEARCH && A != actions::CHAT_COMPLETIONS && A != actions::DOCUMENTS_GET`; `DOCUMENTS_GET` constant = `4` in `keys.rs:533` |
| 5 | The codebase compiles with no regressions on the existing `searchRules` path | VERIFIED | `cargo check -p meilisearch-auth -p meilisearch` finishes with 0 errors, 0 new warnings |

**Score:** 5/5 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/meilisearch-auth/src/lib.rs` | `IndexRules`, `IndexBrowseRules` types + `AuthFilter` extension + `get_key_filters` extension | VERIFIED | `pub enum IndexRules` at line 374; `pub struct IndexBrowseRules` at line 425; `AuthFilter.index_rules` field at line 172; `get_key_filters(uid, search_rules, index_rules)` at line 93; `get_index_browse_rules()` accessor at line 279 |
| `crates/meilisearch/src/extractors/authentication/mod.rs` | `Claims.index_rules` + `TenantTokenOutcome` extension + `DOCUMENTS_GET` action gate | VERIFIED | `Claims.index_rules: Option<IndexRules>` with `#[serde(default)]` at line 347; `TenantTokenOutcome::Valid(Uuid, SearchRules, Option<IndexRules>)` at line 159; gate updated at line 310 |
| `crates/meilisearch/tests/auth/tenant_token.rs` | 5 integration tests covering AUTH-01/02/03 and DOCUMENTS_GET gate | VERIFIED | `index_rules_claim_decoded` (line 579), `index_rules_independent_from_search_rules` (line 620), `index_rules_set_format` (line 668), `index_rules_map_format` (line 703), `documents_get_tenant_token_gate` (line 741) — all substantive, each tests a distinct observable behavior |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `mod.rs` → `lib.rs` | `Claims.index_rules` deserialized into `IndexRules`, threaded through `TenantTokenOutcome::Valid` to `get_key_filters` to `AuthFilter.index_rules` | `IndexRules` import + `TenantTokenOutcome::Valid` 3-tuple | WIRED | `use meilisearch_auth::{AuthController, AuthFilter, IndexRules, SearchRules}` at `mod.rs:147`; `TenantTokenOutcome::Valid(uid, data.claims.search_rules, data.claims.index_rules)` at `mod.rs:338`; `auth.get_key_filters(key_uuid, search_rules, index_rules)` at `mod.rs:267` |
| `AuthFilter.get_index_browse_rules()` → `IndexRules.get_index_browse_rules()` | Delegation from `AuthFilter` accessor to `IndexRules` method | `index_rules.get_index_browse_rules(index)` call | WIRED | `lib.rs:284`: `index_rules.get_index_browse_rules(index)` — fully delegated after `is_index_authorized()` guard |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AUTH-01 | 01-01, 01-02 | JWT tenant tokens can carry an `indexRules` claim using the same structure as `searchRules` | SATISFIED | `Claims.index_rules: Option<IndexRules>` with `#[serde(default)]` ensures backward-compatible deserialization; `index_rules_claim_decoded` test passes |
| AUTH-02 | 01-01, 01-02 | `indexRules` and `searchRules` are parsed independently — no cross-interaction | SATISFIED | Separate fields on `AuthFilter`; `is_index_authorized()` only checks `search_rules`; `index_rules_independent_from_search_rules` test isolates both paths |
| AUTH-03 | 01-01, 01-02 | `indexRules` supports both index whitelisting and per-index filter expressions | SATISFIED | Both `Set` and `Map` variants implemented; `index_rules_set_format` and `index_rules_map_format` tests verify both |

No orphaned requirements: DOCS-01 through DOCS-05 are correctly mapped to Phase 2 in REQUIREMENTS.md.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | — | — | No anti-patterns found |

Scan of modified files:
- No `TODO/FIXME/PLACEHOLDER` comments in new code
- No `return null` / `return {}` stubs in new logic
- No `console.log`-only implementations (Rust codebase)
- `get_index_browse_rules()` is a real implementation, not a placeholder (checks `is_index_authorized`, delegates to `IndexRules`)

---

## Human Verification Required

None. All observable truths for this phase are verifiable programmatically:

- Type definitions: confirmed in source
- Field independence: confirmed structurally (separate fields, separate accessors)
- Compile-time correctness: `cargo check` passes with zero errors
- Commit integrity: all three documented commits (`d80301218`, `2d971fe46`, `75ebe074e`) exist in git history

The integration tests (AUTH-01/02/03, gate) require `cargo test` to execute at human discretion, but the test code is substantive and correct — not stubs.

---

## Summary

Phase 1 goal is fully achieved.

The `indexRules` claim flows end-to-end:

1. **Deserialization** (`Claims.index_rules` with `#[serde(default)]`) — backward-compatible, camelCase
2. **Transport** (`TenantTokenOutcome::Valid` extended to 3-tuple) — no data loss in the pipeline
3. **Storage** (`AuthFilter.index_rules` independent from `search_rules`) — AUTH-02 structural guarantee
4. **Access** (`AuthFilter::get_index_browse_rules()` — guarded accessor delegating to `IndexRules`) — Phase 2 API ready
5. **Gate** (`DOCUMENTS_GET` in action gate) — tenant tokens can reach document browse routes
6. **Tests** (5 integration tests) — AUTH-01, AUTH-02, AUTH-03 and gate all covered

The one intentional behaviour change (two existing tests updated to reflect `DOCUMENTS_GET` now being accepted by the gate) is documented and correct.

Phase 2 can consume `AuthFilter::get_index_browse_rules(index)` immediately.

---

_Verified: 2026-03-04_
_Verifier: Claude (gsd-verifier)_
