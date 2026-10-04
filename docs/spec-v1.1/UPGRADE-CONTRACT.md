# Micro Alpha Trading v1.1 — Upgrade Contract

## 0. Baseline

- Baseline: `Micro Alpha Trading Commercial Personal v1.0.4`
- Upgrade: `v1.1 Dynamic Coin Universe`
- Usage: single-user / personal trading system. No SaaS, multi-tenant, billing, workspace or customer administration requirements.
- Upgrade type: Product + Domain + Implementation + Production Handoff.
- This package is an additive upgrade. It does **not** replace the v1.0.4 core recommendation/trading contracts.

## 1. v1.1 Goal

v1.0.4 proves whether a short-horizon microstructure edge exists for a small fixed research set. v1.1 adds a controlled way to discover, warm up, qualify and monitor additional coins without turning the product into an indiscriminate all-coin auto trader.

Formal flow:

`Exchange Universe → Coin Opportunity Scanner → Asset Qualification Gate → Dynamic Eligible Universe → Shared Alpha Engine → Per-Asset Calibration/Cost/Risk → Core Recommendation → Trade Qualification Gate → [Auto Trade Admission when AUTO] → Portfolio Exposure Gate → Risk/Execution → Outcome`

The v1.1 core addition is **market selection + asset-specific runtime calibration**, not a new prediction engine or trigger strategy family.

## 2. Frozen Inherited Contracts

The following v1.0.4 contracts are inherited and MUST NOT be redesigned in v1.1:

1. Core product = Recommendation + Qualification + Optional Execution.
2. Triple Barrier / First-Hit prediction semantics.
3. `OBSERVE / PAPER / CONFIRM / AUTO` modes.
4. `QUALIFIED != AUTO_EXECUTE`; AUTO new entries require the inherited `Auto Trade Admission` contract.
5. `AI_MODE=OFF / ASSIST`; AI never changes Direction, Probability, Net EV, Position, Leverage, Admission or Exit.
6. Risk authority is higher than Alpha authority.
7. No positive cost-adjusted EV → no trade.
8. Leverage is risk scaling, never a source of alpha.
9. Every formal Prediction must resolve to an objective market Outcome.
10. Calibration, Model Trust and Drift remain formal inputs to Trade Qualification.
11. Production models do not self-modify online after individual prediction errors.
12. Production runtime follows v1.0.4 `Listen Continuously → Evaluate Selectively → Trade Rarely → Verify Everything`.

## 3. New v1.1 Domain Concepts

v1.1 adds:

- `CoinOpportunityScanner`
- `ScannerRun`
- `CoinCandidate`
- `AssetProfile`
- `AssetQualification`
- `UniverseMembership`
- `AssetLifecycle`
- `ResearchPriority`
- `DataWarmupState`
- `PortfolioExposureGate`
- `AssetTriggerProfile`

These concepts are upstream/downstream guards around the existing Shared Alpha Engine. They are not new strategy families.

## 4. Two-Gate Rule

Two gates MUST remain separate.

### Asset Qualification Gate
Answers: **Is this coin suitable for entering the Micro Alpha research/monitoring universe?**

It evaluates data quality, liquidity, spread, depth, slippage feasibility, volatility/tail risk, market continuity and exchange availability.

### Trade Qualification Gate
Answers: **Is the current short-horizon setup on an already-qualified asset worth trading now?**

It evaluates directional probability, calibration, Model Trust, cost-adjusted EV, execution quality and risk.

`ASSET_ELIGIBLE != TRADE_QUALIFIED`.

A coin may be eligible and have no trade. A coin may show a strong price move and still be rejected as an asset because execution quality is poor.

## 5. Scanner Boundary

The Scanner MAY:

- enumerate configured Binance/OKX derivative markets;
- compute deterministic asset-quality metrics;
- rank candidates for research priority;
- create/update candidate records;
- trigger data warmup for approved/eligible candidates;
- surface manual-add candidates.

The Scanner MUST NOT:

- place orders;
- create leverage decisions;
- bypass Asset Qualification;
- bypass Trade Qualification;
- turn a newly discovered coin directly into LIVE/AUTO;
- use LLM opinion as the asset qualification decision;
- rank coins by a hidden "buy score".

## 6. Dynamic Universe Rule

`Scanner Result != Active Universe`.

Universe membership changes only through the Asset Lifecycle state machine and debounce/persistence rules. A single scan must never instantly activate or remove a live-tradable coin unless a hard safety condition requires suspension.

## 7. Manual Add Rule

Manual Add is supported and is a first-class v1.1 feature.

Manual Add bypasses **discovery only**. It MUST NOT bypass:

- exchange symbol validation;
- market-data health checks;
- Asset Qualification;
- data warmup;
- calibration/model readiness;
- OBSERVE/PAPER gates;
- Portfolio Exposure Gate.

## 8. Shared Engine Rule

v1.1 uses:

`Same Engine + Different Asset Profile`

All supported assets reuse the same architectural pipeline:

`Book/Trades → Market Pulse → Trigger → Features/Model → Calibration → Cost → Trade Qualification → Auto Admission → Portfolio/Risk → Execution → Outcome`

Per-asset differences live in versioned Asset Profiles and model/calibration artifacts. Do not fork the codebase into BTCStrategy, ETHStrategy, SOLStrategy, etc.

## 9. Multi-Coin Risk Rule

Before any live order, an already-qualified TradePlan must also pass the Portfolio Exposure Gate.

This gate protects against apparent diversification that is actually repeated crypto-beta exposure.

At minimum it controls:

- maximum concurrent positions;
- total gross notional/effective leverage;
- total risk budget in use;
- same-direction exposure;
- per-asset cap;
- common-market/correlation concentration guard;
- exchange concentration.

## 10. v1.1 Non-Goals

Do NOT add in this release:

- all-exchange discovery;
- social/news/on-chain sentiment scanner;
- AI coin picking;
- meme-coin-specific strategy;
- cross-asset reinforcement learning;
- online self-learning production models;
- AutoML/model auto-promotion;
- universal global model replacing per-asset calibration;
- automatic live trading immediately after discovery;
- Kafka/Flink/Kubernetes unless a measured bottleneck proves necessary.

## 11. Implementation Priority

1. Asset domain + lifecycle
2. Scanner + Manual Add
3. Data warmup and qualification
4. Per-asset profile/calibration integration
5. Opportunity Radar
6. Portfolio Exposure Gate
7. OBSERVE/PAPER multi-asset validation
8. Controlled multi-coin live enablement

## 12. Upgrade Acceptance

v1.1 is implementation-complete when:

- scanner runs on schedule and on demand;
- manual add follows the same qualification path;
- candidate and active universe are demonstrably distinct;
- every active asset has a versioned AssetProfile and AssetTriggerProfile;
- existing BTC/ETH prediction/trade contracts remain unchanged;
- Opportunity Radar aggregates current recommendations without creating trades;
- no unqualified/new coin can reach live execution;
- Portfolio Exposure Gate can block an individually qualified trade;
- asset state transitions and reasons are auditable;
- rollback can disable v1.1 and return to fixed-asset v1.0.4 event-driven behavior without data loss.

## 13. Release Status

Target implementation status after Codex completion:

`V1_1_IMPLEMENTATION_READY_FOR_OBSERVE_AND_PAPER`

Not automatically:

`MULTI_COIN_AUTO_LIVE_READY`

Multi-coin AUTO requires separate evidence from OBSERVE/PAPER/Small Live gates.


## v1.0.4 Auto Admission Compatibility

Dynamic multi-coin scanning must not bypass the inherited v1.0.4 `Auto Trade Admission` contract.

AUTO live chain is:

`Trade Qualification → Auto Trade Admission → Portfolio Exposure Gate → Risk Gate → Execution`.

- Scanner ranking is research/discovery priority and cannot become AUTO admission priority directly.
- A Coin may be `ELIGIBLE` and an Episode may be `QUALIFIED` while AUTO still returns `SKIPPED`.
- v1.1 Portfolio Exposure Gate is an additional veto after Admission; it may block an ADMITTED candidate if portfolio-level constraints changed before execution.
- Multi-coin AUTO must not attempt to trade every Qualified Coin.


## v1.0.4 Trigger Runtime Compatibility

Dynamic Coin expansion MUST NOT restore an unconditional fixed-step Full Alpha loop.

Production per-asset states are inherited:

`IDLE (Market Pulse only) → CANDIDATE (triggered Full Evaluation) → ACTIVE (~1s Full Evaluation) → IN_POSITION (~1s Alpha + immediate Risk/Data protection)`.

The Scanner may discover and qualify assets, but Scanner ranking is not an Alpha Trigger. Eligible IDLE assets only start Full Alpha Evaluation when the v1.0.4 Trigger Framework detects meaningful state change.

The first trigger family remains exactly the v1.0.4 scope. v1.1 adds per-asset normalization, not more trigger categories.
