# Changelog v1.0.3

## Why

v1.0.2 correctly separated high-frequency Evaluation from Prediction Episode publication, but AUTO still needed an explicit contract preventing every Qualified opportunity from becoming a real trade.

## Added

- Auto Trade Admission Gate
- explicit `QUALIFIED != AUTO_EXECUTE`
- stricter AUTO Net EV / Model Trust / execution-health checks
- deterministic selection when multiple Qualified candidates compete for limited risk budget
- ADMITTED / SKIPPED audit reasons
- re-entry cooldown / churn protection
- daily trade count as safety cap only, never an execution target

## Preserved

- recommendation-first product model
- Prediction Episode and automatic Outcome
- Trade Qualification rules
- OBSERVE / PAPER / CONFIRM / AUTO modes
- AI boundary
- risk veto authority
- no online self-learning / AutoML

## Version Status

`CODEX_IMPLEMENTATION_READY`

This does not mean `AUTO_LIVE_READY`.
