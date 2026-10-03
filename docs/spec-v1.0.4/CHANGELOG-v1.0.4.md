# Changelog — v1.0.4

## Added

- Event-driven production runtime
- Market Pulse lightweight listener
- `IDLE / CANDIDATE / ACTIVE / IN_POSITION` runtime states
- 4 P0 Alpha Trigger families
- Risk Trigger
- Data Integrity Trigger
- trigger severity + reason codes
- trigger debounce/deduplication
- Trigger Outcome Analytics
- replay comparison between trigger runtime and fixed-step reference

## Changed

- Removed unconditional production requirement that every asset runs Full Alpha Evaluation every ~1s while IDLE.
- Candidate opportunities use 1–2s Full Alpha Evaluation.
- Active Prediction Episodes and open positions retain ~1s Full Alpha Evaluation.
- Risk/Data protection may evaluate immediately on events.

## Not Changed

Prediction Episode, First-Hit labels, Trade Qualification, Auto Trade Admission, risk limits, OBSERVE/PAPER/CONFIRM/AUTO, Calibration, Model Trust, Drift, Cost Model and Outcome contracts remain v1.0.3-compatible.

## Explicitly Deferred

OI, liquidation-flow triggers, advanced cross-venue trigger logic, AI trigger models, news/social/on-chain triggers, AutoML and online learning.
