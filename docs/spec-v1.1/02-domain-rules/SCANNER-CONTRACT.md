# Coin Opportunity Scanner Domain Contract

## 1. Universe source

Default v1.1 discovery universe:

- configured USDT-margined perpetual markets from Binance and OKX;
- only active/tradable contracts;
- symbols normalized to canonical asset identity.

The exact product types are configuration, not hard-coded business truth.

## 2. Schedule

Default full scan cadence: 5 minutes.

Allowed configuration: 1m / 5m / 15m or explicit cron-like interval. Scanner cadence is independent from the v1.0.4 event-driven runtime. Eligible IDLE assets run lightweight Market Pulse; only triggered/active assets enter Full Alpha Evaluation cadence.

## 3. Scanner metrics

Use deterministic market-quality metrics. Minimum set:

- liquidity/volume
- median and tail spread
- near-mid L2 depth
- expected slippage at test notionals
- trade activity
- realized volatility and tail volatility
- feed continuity/data quality
- exchange availability
- cross-venue availability indicator

No LLM is required.

## 4. Output

Every ScannerRun stores:

- run_id / started_at / completed_at
- exchange snapshots used
- symbols considered
- per-symbol observations
- candidate rank/research priority
- rule/config version
- errors/incomplete sources

## 5. Idempotency

Repeated scans with the same market state must not create duplicate candidate objects. Upsert observations and append run evidence.

## 6. Rate limits

Respect exchange API/WebSocket limits. Prefer exchange metadata and already-collected market data over opening unnecessary per-symbol connections during the discovery phase.
