# Trigger Outcome Analytics Contract v1.0.4

## 1. Goal

Measure whether a trigger is useful. Trigger count is not a success metric.

## 2. Required Funnel

For each trigger family and combination, track:

`Trigger -> Full Evaluation -> Prediction Episode -> QUALIFIED -> AUTO ADMITTED -> EXECUTED -> Prediction Outcome -> Trade Outcome`.

## 3. Minimum Metrics

- trigger_count
- full_evaluation_count
- episode_creation_rate
- qualified_rate
- auto_admitted_rate
- executed_rate
- target_hit / stop_hit / timeout distribution
- observed Net EV for episodes originated by trigger
- Paper/Live realized Net P&L when executed
- false/no-edge trigger rate

## 4. Missed Opportunity Comparison

Replay must periodically compare event-driven production logic to a fixed-step reference evaluation.

Purpose:
- estimate how many materially useful Episodes event-driven Trigger logic misses
- detect overly strict Trigger thresholds
- tune thresholds without enabling production full-scan mode

## 5. No Automatic Promotion

Trigger analytics may recommend threshold changes, but V1.0.4 does not automatically rewrite production thresholds or enable new trigger families.
