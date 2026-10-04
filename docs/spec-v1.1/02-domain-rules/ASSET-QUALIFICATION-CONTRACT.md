# Asset Qualification Contract

## 1. Purpose

Asset Qualification determines whether a market is suitable for Micro Alpha research and, later, trading. It is not a directional predictor.

## 2. Inputs

At minimum:

- configured exchange availability;
- symbol/contract metadata validity;
- market-data continuity and timestamp quality;
- trade activity / volume;
- bid-ask spread distribution;
- L2 depth around mid;
- estimated slippage for configured test notionals;
- book stability / reset rate;
- realized volatility distribution;
- tail-move / gap frequency;
- funding/OI availability when applicable;
- liquidation feed availability when applicable;
- cross-venue coverage when available.

## 3. Qualification Dimensions

Each dimension returns `PASS / WARN / FAIL / UNKNOWN` plus evidence.

Hard FAIL dimensions must be configurable and versioned. A single hidden score must not replace these dimension outcomes.

## 4. Result

`AssetQualification` contains:

- asset_id
- evaluated_at
- rule_version
- overall_state: PASS / CONDITIONAL / FAIL
- component_results[]
- blocking_reasons[]
- warnings[]
- research_priority
- next_action

## 5. Safety Rules

- Unknown market-data continuity cannot produce live eligibility.
- Persistent sequence gaps force data-health FAIL until resynced.
- Poor depth/slippage may allow OBSERVE research while blocking live modes.
- Very high volatility is not automatically an opportunity; it may be a risk FAIL.
- Listing/new-market recency may require extended warmup.
- A coin cannot self-promote to LIVE purely because price movement is large.

## 6. Re-evaluation

Asset Qualification is periodic and event-driven. It should run after each full scanner cycle and on hard safety events.

A hard safety FAIL can immediately move an active asset to DEGRADED/SUSPENDED. Positive scanner changes must satisfy persistence/debounce requirements before promotion.
