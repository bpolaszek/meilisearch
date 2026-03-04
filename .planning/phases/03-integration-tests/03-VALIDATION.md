---
phase: 3
slug: integration-tests
status: draft
nyquist_compliant: false
wave_0_complete: true
created: 2026-03-04
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `actix-rt` + `cargo test` (integration tests via `Server` helper) |
| **Config file** | none — `#[actix_rt::test]` macro on each test function |
| **Quick run command** | `cargo test -p meilisearch --test auth -- index_rules` |
| **Full suite command** | `cargo test -p meilisearch --test auth` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p meilisearch --test auth -- index_rules`
- **After every plan wave:** Run `cargo test -p meilisearch --test auth`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | Crit-5 | integration | `cargo test -p meilisearch --test auth -- search_rules_filter_unaffected_by_index_rules` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | VERIF-gap | integration | `cargo test -p meilisearch --test auth -- index_rules_filter_fused_with_query_filter` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 1 | VERIF-gap | integration | `cargo test -p meilisearch --test auth -- index_rules_null_rule_returns_all_documents` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

None — existing test infrastructure covers all phase requirements. Phase 3 only adds new tests to existing `crates/meilisearch/tests/auth/tenant_token.rs`.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
