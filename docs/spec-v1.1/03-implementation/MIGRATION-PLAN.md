# Migration Plan v1.0.4 → v1.1

## Principle

No destructive reset. Preserve all existing market data, predictions, outcomes, trades, model-health data and risk settings.

## Steps

1. Apply additive schema migration.
2. Create canonical asset/instrument registry.
3. Create AssetProfile for existing BTC/ETH instruments.
4. Map BTC/ETH lifecycle conservatively:
   - if no runtime evidence exists: `COLLECTING` or `OBSERVE`;
   - if OBSERVE validation exists but no live gate: `OBSERVE`;
   - if existing production evidence explicitly proves the required gates: `ELIGIBLE` with recorded migration evidence.
5. Leave new scanner disabled.
6. Backfill recent spread/liquidity/data-health observations from available raw data.
7. Enable Scanner in dry-run mode.
8. Compare dry-run candidates with expected exchange universe.
9. Enable candidate creation but keep automatic live eligibility disabled.
10. Enable Opportunity Radar.
11. Enable Portfolio Exposure Gate in shadow/log-only mode, then enforcement before multi-coin live.

## No implicit promotion

Migration MUST NOT mark all newly discovered markets as `ELIGIBLE`.


## Runtime trigger profile migration

After asset-universe schema is available, add the AssetTriggerProfile reference/config. Seed BTC/ETH from the validated v1.0.4 trigger configuration. New assets remain COLLECTING/CALIBRATING until their trigger normalization is supported by sufficient data. Do not use one generic absolute threshold to force promotion.
