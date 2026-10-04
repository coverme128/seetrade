# v1.1 Compatibility Review

## Checked against v1.0.4 MASTER CONTRACT

### No conflict: product core
v1.1 keeps Recommendation + Qualification + Optional Execution. Scanner is upstream market selection only.

### No conflict: first-stage BTC/ETH
v1.0.4 defined BTC/ETH as the first-stage assets. v1.1 is a later-version expansion and therefore intentionally makes the universe dynamic while preserving BTC/ETH as baseline/bootstrap assets.

### No conflict: AI boundary
Scanner and asset qualification are deterministic. AI remains optional explanation/research assistance and cannot modify qualification/trade outputs.

### No conflict: learning boundary
v1.1 does not introduce online learning, AutoML or automatic production-model mutation. Per-asset calibration uses the inherited controlled adaptation approach.

### No conflict: risk authority
v1.1 adds Portfolio Exposure Gate, which is an additional risk veto downstream of Trade Qualification; it does not weaken Risk Engine authority.

### No conflict: OBSERVE
Every new asset must pass through research/OBSERVE evidence before live eligibility.

## Deliberate contract extension

v1.0.4: fixed first-stage asset set.
v1.1: controlled dynamic universe.

This is a versioned expansion, not an accidental contradiction.

## Anti-overengineering review

PASS:

- no SaaS features;
- no new distributed platform requirement;
- no AI coin picking;
- no auto-learning redesign;
- no per-coin strategy forks;
- no immediate all-coin live trading;
- scanner default 5m; Eligible IDLE assets inherit lightweight Market Pulse; only Candidate/Active/Open-Position states use Full Alpha cadence from v1.0.4.

## Overall review

`COMPATIBLE_WITH_V1_0_1 = PASS`
`SCANNER_NOT_DIRECT_TRADING = PASS`
`MANUAL_ADD_CANNOT_BYPASS_GATES = PASS`
`MULTI_COIN_RISK_GUARD_PRESENT = PASS`
`CODEX_UPGRADE_READY = PASS`


## Runtime alignment

v1.1 does not restore the superseded all-asset fixed 1s Full Alpha runtime. Dynamic-universe growth is explicitly constrained by the v1.0.4 event-driven Trigger contract.
