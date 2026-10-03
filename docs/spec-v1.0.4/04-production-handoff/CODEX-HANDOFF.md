# Codex Handoff — v1.0.4 Runtime Trigger Upgrade

## Instruction

Apply this package on top of the already implemented `micro-alpha-trading-commercial-v1.0.3`.

Do not rebuild v1.0.3 and do not begin v1.1 Coin Scanner in the same change set.

## Required Implementation Order

1. Market Pulse + runtime state
2. four P0 Alpha Triggers
3. Candidate/Active cadence
4. Risk/Data protection triggers
5. trigger analytics + replay comparison
6. regression + live-closure verification

## Do Not Add

- new exchanges
- new assets beyond current baseline for the purpose of this upgrade
- AI trigger model
- OI trigger
- liquidation trigger
- advanced cross-venue trigger
- news/social/on-chain sources
- online learning
- AutoML
- new scoring engine
- user-facing runtime-mode switch

## Return Format

Codex should report:

- changed files
- runtime state implementation
- exact Trigger metrics/normalization used
- default config values and why they are provisional
- tests added
- replay fixtures used
- actual test results
- trigger funnel sample output
- fixed-step vs event-driven comparison
- known gaps
- gate status PASS/FAIL

## Important

Threshold values are not alpha claims. Initial thresholds are engineering defaults for replay/observe validation and must remain configurable/versioned.
