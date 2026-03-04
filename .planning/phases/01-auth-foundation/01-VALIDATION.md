---
phase: 1
slug: auth-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-04
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `actix-rt` + `cargo test` (integration tests) |
| **Config file** | none — `#[actix_rt::test]` macro on each test function |
| **Quick run command** | `cargo test -p meilisearch --test auth -- tenant_token` |
| **Full suite command** | `cargo test -p meilisearch --test auth` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p meilisearch --test auth -- tenant_token`
- **After every plan wave:** Run `cargo test -p meilisearch --test auth`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | AUTH-01 | integration | `cargo test -p meilisearch --test auth -- index_rules` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 1 | AUTH-02 | integration | `cargo test -p meilisearch --test auth -- index_rules_independent` | ❌ W0 | ⬜ pending |
| 1-01-03 | 01 | 1 | AUTH-03 | integration | `cargo test -p meilisearch --test auth -- index_rules_formats` | ❌ W0 | ⬜ pending |
| 1-01-04 | 01 | 1 | (gate) | integration | `cargo test -p meilisearch --test auth -- documents_get_tenant_token` | ❌ W0 | ⬜ pending |
| 1-01-05 | 01 | 1 | (compile) | integration | `cargo test -p meilisearch --test auth -- tenant_token` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/meilisearch/tests/auth/tenant_token.rs` — add test functions for AUTH-01, AUTH-02, AUTH-03, and `DOCUMENTS_GET` gate (extend existing file)

*Existing `tenant_token` tests cover regression on `searchRules` path.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
