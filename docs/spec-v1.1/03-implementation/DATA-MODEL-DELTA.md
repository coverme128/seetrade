# Data Model Delta

## New entities

### asset_profile
Versioned current profile for a canonical asset/contract.

### asset_trigger_profile
Versioned per-asset trigger normalization/threshold profile inherited by the v1.0.4 Market Pulse runtime.

### scanner_run
One scanner execution and its metadata.

### scanner_observation
Per-symbol metrics captured during a scanner run.

### asset_qualification
Versioned qualification decision and evidence.

### universe_membership
Current lifecycle state + mode permissions.

### asset_state_transition
Immutable audit history of lifecycle transitions.

### portfolio_gate_decision
Portfolio-level approval/reduction/block decision for a TradePlan.

## Key constraints

- canonical unique key: `(venue, normalized_symbol, contract_type)` for venue instruments;
- canonical asset identity separated from venue instrument identity;
- only one current universe membership row per canonical trading asset;
- state transitions append-only;
- qualification decision references the exact scanner/data/config versions used;
- TradePlan references AssetProfile version;
- live Execution references Portfolio Gate decision.

## Compatibility

Existing Prediction/Qualification/TradePlan/Execution/Outcome IDs remain unchanged. v1.1 adds foreign keys/metadata; it does not redefine existing semantics.

## Runtime compatibility

Eligible Universe membership does not imply fixed-step Alpha evaluation. AssetProfile must resolve an AssetTriggerProfile before production event-driven trigger evaluation is enabled.
