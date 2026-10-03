# Product Delta — Runtime & Opportunity UX v1.0.4

## 1. User-visible Principle

The user should not see “more opportunities” because the engine listens continuously.

The UI continues to show only formal Prediction Episodes and their states:

- WATCH
- EMERGING
- QUALIFIED
- TRACKING / IN POSITION
- COMPLETED

Market Pulse and raw Trigger Events are diagnostic data, not primary opportunity cards.

## 2. Main UI Behavior

### IDLE
No opportunity card is created. Optional compact status:

`BTC — Monitoring`

### CANDIDATE
A candidate may be held internally. Do not publish a full trade card until the existing Prediction Episode creation rule is satisfied.

### ACTIVE
Use the existing v1.0.3 stable Opportunity Card. Live probability, Net EV and drivers may refresh without creating duplicate cards.

### IN POSITION
Use existing Current/Target Position, Effective Leverage, Action and Reason display.

## 3. Optional Diagnostic Detail

On an expanded technical panel only:

- Triggered by: `ORDER_FLOW + AGGRESSOR_BURST`
- Severity: `ABNORMAL + ABNORMAL`
- Triggered at
- Full evaluation latency

Do not promote trigger severity to a BUY/SELL indicator.

## 4. History / Review

A completed Prediction Episode may show its originating trigger family for later review, e.g.:

`Origin: ORDER_BOOK_DEPTH + PRICE_VELOCITY`

This supports learning without creating a new “Trigger dashboard” in v1.0.4.

## 5. Output Volume

Production UX goal:

`many market events -> few trigger events -> fewer Prediction Episodes -> fewer Qualified opportunities -> selective trades`.

No target count per day is required. Zero qualified opportunities and zero trades are valid.
