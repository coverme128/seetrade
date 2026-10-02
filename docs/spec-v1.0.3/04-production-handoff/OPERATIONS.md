# Personal Operations

## Daily Start

1. Check exchange WS health
2. Check local clock / timestamp drift
3. Check model version
4. Check fee configuration
5. Check balance and max risk caps
6. Run replay smoke test
7. Start in OBSERVE/PAPER unless live criteria already approved

## Live Runtime Health

Monitor:

- WS reconnects
- book gaps
- stale data
- feature lag
- prediction latency
- gate latency
- order ack/fill latency
- expected vs actual slippage
- daily P&L
- drawdown
- rejection rate

## Kill Switch

Must support:

- stop new orders immediately
- cancel pending orders
- configurable: hold existing positions OR flatten positions

Flatten behavior must be an explicit config, not hidden default.


## Daily Prediction Integrity Checks

Track separately:

- realtime_evaluations
- prediction_episodes_opened
- prediction_episodes_closed
- qualified_episodes
- paper/live trades

A large `realtime_evaluations` count is normal. A 1:1 relationship between evaluations and formal episodes is a defect.
