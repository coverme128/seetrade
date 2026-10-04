# v1.1 Rollback

## Objective

Return safely to fixed-universe v1.0.4 event-driven behavior without deleting v1.1 research data.

## Switches

1. disable Scanner scheduling;
2. disable manual candidate activation;
3. freeze Dynamic Universe Manager;
4. set active universe to the configured legacy asset set (BTC/ETH by default);
5. disable Portfolio Gate only if returning to a single/fixed-position v1.0.4 mode where its absence is proven compatible; otherwise leave it enabled as extra protection;
6. preserve all scanner/profile/lifecycle tables read-only.

## Never roll back by

- dropping historical scanner/qualification evidence;
- rewriting Prediction/Outcome history;
- silently promoting/demoting assets;
- deleting model artifacts.

## Verification

Replay a known BTC/ETH dataset and confirm the inherited v1.0.4 Core Recommendation and Trade Qualification outputs remain reproducible.
