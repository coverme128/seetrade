-- SQLite-oriented reference migration for the implemented personal deployment.
-- Adapt exact table/column names to the existing schema; do not introduce PostgreSQL solely for v1.1.

CREATE TABLE IF NOT EXISTS asset_trigger_profile (
  id TEXT PRIMARY KEY,
  asset_key TEXT NOT NULL,
  profile_version INTEGER NOT NULL,
  config_json TEXT NOT NULL,
  metric_distribution_version TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(asset_key, profile_version)
);

CREATE INDEX IF NOT EXISTS idx_asset_trigger_profile_asset
  ON asset_trigger_profile(asset_key, profile_version);

-- The existing AssetProfile record/profile_json should reference the current
-- asset_trigger_profile.id. Prefer the existing profile_json extension if the
-- deployed schema intentionally avoids additive columns.
