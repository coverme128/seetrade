import type { Config, State } from "./types";
export type ScannerConfig = {
  enabled: boolean;
  dry_run: boolean;
  interval_secs: number;
  candidate_l2_slots: number;
  minimum_clean_hours: number;
  minimum_outcomes: number;
  persistence: number;
  min_quote_volume: number;
  max_spread_bps: number;
  min_depth: number;
  max_slippage_bps: number;
  max_volatility: number;
  max_tail_gap: number;
};
export type AssetProfile = {
  id: string;
  asset_key: string;
  profile_version: number;
  lifecycle_state: string;
  supported_exchanges: string[];
  mode_permissions: string[];
  collecting: boolean;
  clean_since: number | null;
  good_scans: number;
  bad_scans: number;
  model: State["model"] | null;
  calibration_artifact_id: string | null;
  cost_artifact_id: string | null;
  liquidity_artifact_id?: string | null;
  config: Config | null;
  feature_normalization_version: string;
  trigger: {
    id: string;
    profile_version: number;
    calibrated: boolean;
    metric_distribution_version: string;
    config: Record<string, unknown>;
  };
  observation: null | { metrics: Record<string, number | null> };
  qualification: null | {
    overall_state: string;
    research_priority: string;
    blocking_reasons: string[];
    warnings: string[];
    next_action: string;
    components: Record<string, { state: string; evidence: unknown }>;
  };
};
export type Universe = {
  config: ScannerConfig;
  config_version: number;
  frozen: boolean;
  requested: boolean;
  running: string | null;
  next_run: number;
  profiles: Record<string, AssetProfile>;
  last_run: null | {
    id: string;
    status: string;
    started_at: number;
    completed_at: number;
    considered: number;
    errors: Record<string, string>;
  };
  transitions: {
    id: string;
    asset_key: string;
    from_state: string;
    to_state: string;
    reason_code: string;
    created_at: number;
  }[];
  portfolio: Record<string, number | boolean>;
  portfolio_latest: Record<
    string,
    {
      id: string;
      decision: string;
      approved_notional: number;
      requested_notional: number;
      reason_codes: string[];
      enforced: boolean;
    }
  >;
};
