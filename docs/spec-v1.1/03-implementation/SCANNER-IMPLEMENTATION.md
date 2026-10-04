# Scanner Implementation

## Stage A — Market enumeration

1. fetch configured venue instrument metadata;
2. normalize symbols/contracts;
3. exclude inactive/delisted/non-target contract types;
4. upsert instrument registry.

## Stage B — lightweight observations

Use existing rolling market statistics where possible. Avoid opening full L2 subscriptions for the entire universe.

Compute:

- volume/trade activity
- median spread and spread percentile
- simple depth proxy if available
- realized volatility
- feed availability/health
- multi-venue presence

## Stage C — candidate selection

Apply deterministic pre-filter and Research Priority.

Potential candidates move to `DISCOVERED`/`COLLECTING`; rejected markets receive explicit reasons.

## Stage D — warmup

Candidate assets receive bounded L2/trade subscriptions and write Parquet partitions under the same dataset conventions as BTC/ETH.

## Stage E — qualification/calibration

Python research jobs produce:

- feature normalization profile
- label/outcome statistics
- candidate Horizon × Target × Stop evidence
- model artifact
- calibration artifact
- cost/slippage profile

Artifacts are versioned and referenced by AssetProfile.

## Stage F — OBSERVE

OBSERVE produces formal Prediction/Outcome records but no live orders. Only after sufficient evidence and explicit lifecycle promotion can the asset become `ELIGIBLE`.

## Scanner scheduler

- default 5-minute run
- `Scan Now` uses the same code path
- manual add triggers a targeted scan/qualification for one symbol, not a separate shortcut path

## Failure handling

- one exchange unavailable → scanner run marked PARTIAL; do not silently assume absence means delisted;
- one asset metric missing → UNKNOWN, not zero;
- repeated source errors → candidate cannot promote;
- all actions emit structured logs and run IDs.


## Prediction Runtime Handoff

Scanner output never becomes a Prediction directly. Assets admitted to the appropriate lifecycle stage enter the inherited v1.0.4 runtime:

`Eligible IDLE → lightweight Market Pulse → Alpha Trigger → Full Alpha Evaluation → PredictionEpisode`.

The Scanner does not force every Eligible Coin into a permanent ~1s Full Evaluation loop. Candidate/warmup data is also used to build the asset-specific Trigger Profile before broader live permissions are granted.
