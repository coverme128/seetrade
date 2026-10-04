# v1.1 Acceptance Gates

## A. Scanner

PASS when:

- scheduled and manual runs use the same implementation;
- partial exchange failures are visible;
- duplicate scans are idempotent;
- research priority is deterministic and auditable.

## B. Asset Lifecycle

PASS when:

- legal transitions are enforced;
- manual add cannot jump to ELIGIBLE;
- hard safety failures can suspend immediately;
- all transitions preserve evidence/reason/version.

## C. Shared Engine

PASS when:

- BTC/ETH use the same core engine as newly admitted assets;
- asset-specific behavior is supplied by AssetProfile/artifacts;
- no duplicated per-coin business logic appears.

## D. Opportunity Radar

PASS when:

- only active/research assets appear according to filters;
- current recommendation is displayed without causing execution;
- each item drills into the canonical Prediction Card.

## E. Portfolio Exposure

PASS when:

- it can PASS, REDUCE and BLOCK;
- it never increases the individual recommended position;
- same-direction correlated exposure is controlled;
- the decision is attached to live execution audit trail.

## F. Live Safety

PASS when:

- non-ELIGIBLE asset order attempt fails closed;
- LOW/DEGRADED model trust can block according to inherited rules;
- Portfolio Gate is mandatory for multi-coin live;
- AUTO remains disabled by default for newly added assets.

## Release labels

After engineering acceptance:

`V1_1_IMPLEMENTATION_COMPLETE`

After multi-asset OBSERVE/PAPER evidence:

`V1_1_MULTI_ASSET_VALIDATED`

Only after live gates:

`V1_1_CONTROLLED_MULTI_COIN_LIVE_READY`


## G. AUTO Admission Compatibility

- [ ] Dynamic Coin `QUALIFIED` does not imply executed.
- [ ] AUTO requires inherited v1.0.4 `ADMITTED`.
- [ ] Portfolio Exposure can veto after Admission.
- [ ] Qualified-but-skipped Episodes remain visible and receive Outcome.
- [ ] Scanner priority cannot directly trigger orders.


## H. v1.0.4 Runtime Inheritance

- [ ] Eligible IDLE assets use lightweight Market Pulse, not unconditional 1s Full Alpha.
- [ ] Each Eligible asset resolves a versioned AssetTriggerProfile.
- [ ] Trigger severity is asset-normalized.
- [ ] Trigger launches evaluation only; it cannot create an order.
- [ ] Active Episodes and open positions preserve required high-cadence re-evaluation/protection.
- [ ] Opportunity Radar does not list every Eligible IDLE asset as an opportunity.
- [ ] Replay comparison shows event-driven runtime does not materially miss the validated opportunity class before live expansion.
