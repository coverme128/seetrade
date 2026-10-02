# Codex Handoff

## What Codex Should Build First

Do not start with UI polish or AUTO.

Start with:

1. Exchange market-data capture
2. deterministic local book
3. replay
4. feature calculation
5. prediction/outcome
6. OBSERVE Card

Then proceed through MASTER-CONTRACT order.

## Implementation Constraints

- single-user local/personal product
- no SaaS abstractions
- no multitenancy
- no billing
- no organization model
- no RBAC beyond local app safety if needed
- no LLM in hot path
- no queue in hot path
- no duplicate paper/live business logic

## Definition of Done

Codex must return for each phase:

- code changes
- tests added
- commands to run
- actual test results
- known gaps
- gate status PASS/FAIL
- screenshots only where UI is relevant
