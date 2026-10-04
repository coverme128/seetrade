# CHANGELOG v1.1

## Added

- Coin Opportunity Scanner
- Manual Add Coin
- Asset Qualification Gate
- Dynamic Eligible Universe
- Asset Lifecycle state machine
- versioned AssetProfile
- per-asset normalization/calibration/cost/risk references
- Opportunity Radar
- Portfolio Exposure Gate
- controlled multi-coin rollout

## Preserved

- v1.0.4 recommendation-first product model
- Triple Barrier labels
- OBSERVE/PAPER/CONFIRM/AUTO
- AI optional/non-authoritative
- Model Trust/Drift self-evaluation model
- controlled retraining only

## Removed / Not introduced

No v1.0.4 capability is removed. v1.1 intentionally does not introduce AI coin picking, online self-learning, AutoML or direct scanner-to-trade execution.


## v1.0.4 Runtime Baseline Alignment

- production runtime now inherits Market Pulse → Trigger → selective Full Alpha Evaluation;
- Dynamic Eligible Universe no longer implies fixed ~1s Full Alpha evaluation for every idle coin;
- adds versioned per-asset AssetTriggerProfile;
- Opportunity Radar defaults to triggered/active Episodes, not every Eligible asset;
- keeps Candidate/Active/In-Position high-cadence evaluation and Risk/Data protection;
- keeps advanced cross-venue/OI/liquidation triggers out of v1.1 core;
- aligns implementation with personal SQLite + local Parquet architecture.
