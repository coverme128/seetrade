# CHANGELOG v1.0.1

## Why

The original v1.0 retained a stronger Champion/Challenger automation emphasis than necessary for a personal-use first release. v1.0.1 keeps the useful feedback loop while reducing overengineering.

## Changes

- Freeze V1 intelligence as Realtime Intelligence + Self Evaluation + Controlled Adaptation.
- Make Outcome Tracking, Calibration, Model Trust and Drift mandatory.
- Move scheduled retraining / Challenger / Shadow to later phase.
- Remove any implication that production should continuously self-learn online.
- Add Model Trust as a formal Trade Qualification input.
- Define Drift response as risk reduction / pause before retraining.
- Reorder Codex implementation to build Model Health before PAPER/live automation.
