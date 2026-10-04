# Dynamic Universe & Asset Lifecycle Contract

## States

- `DISCOVERED`
- `COLLECTING`
- `CALIBRATING`
- `OBSERVE`
- `ELIGIBLE`
- `DEGRADED`
- `SUSPENDED`
- `REJECTED`
- `RETIRED`

## Normal path

`DISCOVERED → COLLECTING → CALIBRATING → OBSERVE → ELIGIBLE`

## Safety paths

- any pre-eligible state → `REJECTED`
- `ELIGIBLE → DEGRADED → SUSPENDED`
- `SUSPENDED → COLLECTING/CALIBRATING/OBSERVE` after remediation
- delisted/unsupported → `RETIRED`

## State meaning

### DISCOVERED
Symbol found by scanner or manual add; no trading rights.

### COLLECTING
Collecting enough clean market data to build distributions/cost profile.

### CALIBRATING
Labels, feature normalization, model/calibration and cost estimates are being validated.

### OBSERVE
System may publish live prediction cards and outcomes; no live trading by default.

### ELIGIBLE
Asset can participate in Trade Qualification and may use PAPER/CONFIRM/AUTO only according to its `mode_permissions` and production gates.

### DEGRADED
Asset remains visible, but live exposure is reduced/blocked based on severity.

### SUSPENDED
No new live trades. Existing exposure follows risk-close policy.

### REJECTED
Does not meet qualification requirements.

### RETIRED
No longer supported/listed/desired.

## Promotion rules

Promotion requires evidence persistence. One favorable scanner run is insufficient.

Required evidence categories:

- data continuity;
- feature stability;
- outcome volume;
- calibration quality;
- cost/slippage feasibility;
- Model Trust not LOW;
- no blocking Asset Qualification failures.

Numeric thresholds are mutable configuration and MUST be recorded with the decision.

## Demotion rules

Hard safety events may demote immediately. Soft deterioration requires configured persistence windows to avoid thrashing.

Every transition must write:

- from_state
- to_state
- reason_code
- evidence snapshot
- rule/config version
- initiated_by: SCANNER / SYSTEM / USER
- timestamp
