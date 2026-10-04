# Asset Profile Contract

## Purpose

Every active/researchable coin uses the same Shared Engine and Trigger Framework, while carrying versioned asset-specific calibration/configuration.

## Required fields

```text
asset_id
symbol
contract_type
quote_asset
supported_exchanges[]
profile_version
lifecycle_state
feature_schema_version
feature_normalization_version
asset_trigger_profile_id
candidate_horizons[]
candidate_targets[]
candidate_stops[]
model_artifact_id
calibration_artifact_id
cost_profile_id
liquidity_profile_id
risk_profile_id
model_trust_state
mode_permissions
created_at
updated_at
```

## Rules

- Profiles are versioned and auditable.
- Raw feature and trigger-family definitions are shared where possible; normalization/thresholds are asset-specific.
- Horizon/Target/Stop are research outputs, not globally fixed constants.
- Cost/slippage profiles are asset- and venue-aware.
- Risk limits may differ by asset.
- Missing optional features are explicit `UNAVAILABLE`; never fabricate real-time values.
- Live mode requires compatible model/calibration/cost/risk/trigger profiles.
- A newly admitted asset does not inherit BTC/ETH absolute trigger thresholds.

## Same Engine Rule

Do not implement per-asset strategy classes unless a future evidence-driven version introduces a new strategy family. Asset-specific behavior belongs in profiles/config/artifacts, not duplicated business logic.
