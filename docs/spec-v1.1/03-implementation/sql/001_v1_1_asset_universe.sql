-- SQLite-oriented reference migration contract for personal single-machine v1.1.
-- Keep naming/types consistent with the implemented schema. JSON is stored as TEXT/JSON payloads as currently supported.

CREATE TABLE IF NOT EXISTS asset_profile (
  id TEXT PRIMARY KEY,
  asset_key TEXT NOT NULL,
  profile_version INTEGER NOT NULL,
  lifecycle_state TEXT NOT NULL,
  profile_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(asset_key, profile_version)
);

CREATE TABLE IF NOT EXISTS scanner_run (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  status TEXT NOT NULL,
  config_version TEXT NOT NULL,
  error_json TEXT
);

CREATE TABLE IF NOT EXISTS scanner_observation (
  id TEXT PRIMARY KEY,
  scanner_run_id TEXT NOT NULL,
  asset_key TEXT NOT NULL,
  metrics_json TEXT NOT NULL,
  research_priority TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(scanner_run_id) REFERENCES scanner_run(id)
);

CREATE TABLE IF NOT EXISTS asset_qualification (
  id TEXT PRIMARY KEY,
  asset_key TEXT NOT NULL,
  rule_version TEXT NOT NULL,
  overall_state TEXT NOT NULL,
  component_results_json TEXT NOT NULL,
  blocking_reasons_json TEXT NOT NULL,
  evaluated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS universe_membership (
  asset_key TEXT PRIMARY KEY,
  lifecycle_state TEXT NOT NULL,
  mode_permissions_json TEXT NOT NULL,
  current_asset_profile_id TEXT,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(current_asset_profile_id) REFERENCES asset_profile(id)
);

CREATE TABLE IF NOT EXISTS asset_state_transition (
  id TEXT PRIMARY KEY,
  asset_key TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  initiated_by TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS portfolio_gate_decision (
  id TEXT PRIMARY KEY,
  trade_plan_id TEXT NOT NULL,
  decision TEXT NOT NULL,
  requested_notional REAL,
  approved_notional REAL,
  reason_codes_json TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
