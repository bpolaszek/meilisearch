---
phase: 2
slug: route-injection
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-04
---

# Phase 2 — Validation Strategy

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
| 02-00-01 | 00 | 1 | DOCS-01..05 | integration (stubs) | `cargo test -p meilisearch --test auth -- index_rules_list_filtered index_rules_fetch_filtered index_rules_single_doc_out_of_scope index_rules_fail_closed index_rules_admin_key_unaffected --no-run` | Plan 02-00 creates | ⬜ pending |
| 02-01-01 | 01 | 2 | DOCS-01 | integration | `cargo test -p meilisearch --test auth -- index_rules_list_filtered` | ✅ W0 | ⬜ pending |
| 02-01-02 | 01 | 2 | DOCS-02 | integration | `cargo test -p meilisearch --test auth -- index_rules_fetch_filtered` | ✅ W0 | ⬜ pending |
| 02-01-03 | 01 | 2 | DOCS-04 | integration | `cargo test -p meilisearch --test auth -- index_rules_fail_closed` | ✅ W0 | ⬜ pending |
| 02-01-04 | 01 | 2 | DOCS-05 | integration | `cargo test -p meilisearch --test auth -- index_rules_admin_key_unaffected` | ✅ W0 | ⬜ pending |
| 02-02-01 | 02 | 3 | DOCS-03 | integration | `cargo test -p meilisearch --test auth -- index_rules_single_doc_out_of_scope` | ✅ W0 | ⬜ pending |
| 02-02-02 | 02 | 3 | DOCS-04 | integration | `cargo test -p meilisearch --test auth -- index_rules_fail_closed` | ✅ W0 | ⬜ pending |
| 02-02-03 | 02 | 3 | DOCS-05 | integration | `cargo test -p meilisearch --test auth -- index_rules_admin_key_unaffected` | ✅ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `crates/meilisearch/tests/auth/tenant_token.rs` — Plan 02-00 creates 5 test function stubs for DOCS-01 through DOCS-05
- [x] No new test infrastructure needed — `Server`, `server.index()`, `wait_task()`, `generate_tenant_token` all available

*Wave 0 is covered by Plan 02-00 (wave 1).*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved
