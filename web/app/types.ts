export type Config = {
  equity: number;
  risk_per_trade: number;
  max_daily_loss: number;
  max_drawdown: number;
  max_position_notional: number;
  max_effective_leverage: number;
  min_liquidation_buffer: number;
  max_slippage: number;
  min_net_edge: number;
  min_net_pnl: number;
  fee_bps: number;
  slippage_bps: number;
  adverse_selection_bps: number;
  impact_bps: number;
  funding_bps: number;
  horizon: number;
  target: number;
  stop: number;
  min_probability: number;
  max_spread_bps: number;
  max_volatility: number;
  max_cross_venue_bps: number;
  max_stale_ms: number;
  min_depth: number;
  depth_participation: number;
  min_samples: number;
  calibration_warning: number;
  calibration_low: number;
  ev_drift_limit: number;
  max_rebalances: number;
  rebalance_interval_secs: number;
  position_deadband: number;
  incremental_ev: number;
  kill_behavior: string;
  signal_enter: number;
  signal_exit: number;
  signal_confirm_ms: number;
  result_hold_ms: number;
  max_opposite_rate: number;
  max_mae_stop_multiple: number;
  high_trust_samples: number;
  alpha_eval_interval_ms: number;
  qualification_eval_interval_ms: number;
  auto_min_net_edge: number;
  auto_required_model_trust: string;
  auto_max_concurrent_positions: number;
  reentry_cooldown_seconds: number;
  max_auto_entries_per_window: number;
  auto_entry_window_seconds: number;
  max_daily_trades: number;
  admission_priority_policy_version: string;
};
export type Features = {
  mid: number;
  spread_bps: number;
  book_imbalance: number;
  flow_imbalance: number;
  aggressive_imbalance: number;
  microprice_bps: number;
  momentum: number;
  volatility: number;
  depth: number;
  cross_venue_bps: number | null;
  cross_venue_lead_bps: number | null;
  cross_venue_lag?: {
    clock: string;
    leader: string;
    lag_ms: number;
    correlation: number;
    samples: number;
  } | null;
  mark: number | null;
  index: number | null;
  data_quality: string[];
};
export type Costs = {
  spread: number;
  entry_fee: number;
  exit_fee: number;
  slippage: number;
  impact: number;
  adverse_selection: number;
  funding: number;
  total: number;
};
export type Qualification = {
  status: string;
  net_edge: number;
  gross_edge: number;
  costs: Costs;
  reasons: string[];
  gates: { code: string; name: string; pass: boolean; detail: string }[];
};
export type Plan = {
  plan_id: string;
  prediction_id: string;
  symbol: string;
  direction: string;
  position_notional: number;
  effective_leverage: number;
  expected_net_pnl: number;
  expected_loss: number;
  planned_max_loss: number;
  break_even_probability: number;
  expected_mae: number;
  liquidity_capacity: number;
  liquidation_buffer: number;
  tp: number;
  sl: number;
  horizon: number;
  entry_type: string;
  exit_rules: string[];
};
export type Prediction = {
  episode?: {
    episode_id: string;
    opened_at: number;
    closed_at: number | null;
    upper_barrier: number;
    lower_barrier: number;
    latest_evaluated_at: number;
    latest_raw_probabilities: number[] | null;
    latest_calibrated_probabilities: number[] | null;
    latest_feature_snapshot_id: string;
    highest_qualification: string;
    close_reason: string | null;
  };
  prediction_id: string;
  symbol: string;
  ts: number;
  horizon: number;
  direction: string;
  target: number;
  stop: number;
  start_price: number;
  current_price: number;
  expected_move: number | null;
  signal_strength: number;
  raw_probabilities: number[];
  calibrated_probabilities: number[] | null;
  model_version: string;
  status: string;
  mfe: number;
  mae: number;
  qualification: Qualification;
  plan: Plan | null;
  config_version: number;
};
export type Outcome = {
  horizon_observation?: {
    deadline_ts: number;
    final_move_at_timeout: number | null;
    status: string;
  } | null;
  prediction: Prediction;
  outcome_type: string;
  first_hit_ts: number;
  final_move: number;
  mfe: number;
  mae: number;
  prediction_error: string;
  brier: number;
  observed_net_edge: number;
  learning_eligible: boolean;
  annotation: string | null;
};
export type Execution = {
  order_id: string;
  plan_id: string;
  side: string;
  qty: number;
  expected_price: number;
  fill_price: number;
  fee: number;
  slippage: number;
  send_ts: number;
  ack_ts: number;
  fill_ts: number;
  liquidity_limited: boolean;
};
export type Position = {
  plan: Plan;
  qty: number;
  entry_price: number;
  current_price: number;
  fees: number;
  realized_pnl: number;
  action: string;
  reason: string;
  target_notional: number;
  executions: Execution[];
  cost_basis: number;
  expected_cost: number;
};
export type Trade = {
  position: Position;
  exit_ts: number;
  exit_reason: string;
  net_pnl: number;
  realized_cost: number;
  expected_cost: number;
  realized_net_edge: number;
};
export type Health = {
  trust: string;
  samples: number;
  predicted_hit_rate: number;
  actual_hit_rate: number;
  calibration_error: number;
  baseline_error: number | null;
  brier: number;
  expected_edge: number;
  observed_edge: number;
  ev_drift: number;
  execution_drift: number | null;
  paper_samples?: number;
  paper_ev_drift?: number | null;
  strong_opposite_rate: number;
  mae_p95: number;
  regime_drift: boolean;
  regime: string;
  regime_samples: number;
  regime_calibration_error: number | null;
  distribution_score: number | null;
  distribution_status: string;
  retrain_recommended: boolean;
  reasons: string[];
  bins: { lower: number; count: number; predicted: number; actual: number }[];
};
export type SymbolState = {
  venues: Record<
    string,
    {
      connected: boolean;
      ready: boolean;
      age_ms: number;
      exchange_age_ms: number;
      gaps: number;
      reconnects: number;
      error: string;
      mid: number | null;
      sequence: number;
    }
  >;
  features: Features | null;
  active: Prediction | null;
  lifecycle: string;
  lifecycle_since: number;
  signal_direction: string;
  last_result: Outcome | null;
  qualification: Qualification | null;
  plan: Plan | null;
  health: Health;
  chart: [number, number][];
};
export type ShadowMetrics = {
  samples: number;
  selected: number;
  coverage: number;
  ece: number;
  brier: number;
  expected_net_edge: number;
  observed_net_edge: number;
  drawdown: number;
  tail_p05: number;
  strong_opposite_rate: number;
};
export type ShadowRun = {
  run_id: string;
  status: string;
  started_at: number;
  config_version: number;
  champion_version: string;
  candidate_version: string;
  candidate_hash: string;
  policy: Record<string, number>;
  offline_status: string;
  operational_checks: Record<string, boolean>;
  promotion_ready: boolean;
  comparison: {
    ready: boolean;
    checks: Record<string, boolean>;
    champion: ShadowMetrics;
    candidate: ShadowMetrics;
    champion_economics_comparable: boolean;
    symbols: Record<string, number>;
    invalid_rate: number;
    pending: number;
    retained: number;
    windows: {
      window: number;
      champion: ShadowMetrics;
      candidate: ShadowMetrics;
    }[];
    regimes: Record<string, ShadowMetrics>;
  };
};
export type AdmissionDecision = {
  admission_id: string;
  episode_id: string;
  symbol: string;
  evaluated_at: number;
  evaluation_ts: number;
  decision: string;
  reason_codes: string[];
  priority_tuple: {
    policy_version: string;
    net_edge_after_cost: number;
    model_trust: number;
    execution_quality: number;
    risk_consumption: number;
    episode_opened_at: number;
  };
  competing_episode_ids: string[];
  risk_budget_before: {
    available_loss: number;
    available_notional: number;
    available_slots: number;
  };
  risk_budget_after_if_admitted: {
    available_loss: number;
    available_notional: number;
    available_slots: number;
  };
  config_version: number;
  context: string;
};
export type IntegrityCounters = {
  realtime_evaluations: number;
  prediction_episodes_opened: number;
  prediction_episodes_closed: number;
  qualified_episodes: number;
  paper_trades: number;
  live_trades: number;
};
export type State = {
  integrity: IntegrityCounters;
  daily_integrity: IntegrityCounters;
  admission: { latest: Record<string, AdmissionDecision> };
  shadow: ShadowRun | null;
  rollback_available: boolean;
  shadow_policy_defaults: Record<string, number>;
  source: string;
  mode: string;
  now: number;
  config: Config;
  config_version: number;
  kill_switch: boolean;
  storage_healthy: boolean;
  model: {
    model_version: string;
    model_type: string;
    calibration_version: string;
    artifact_hash: string;
    training_window: string;
    validation_window: string;
    validated: boolean;
  };
  symbols: Record<string, SymbolState>;
  positions: Record<string, Position>;
  outcomes: Outcome[];
  trades: Trade[];
  audit: { kind: string; ts: number; payload: unknown }[];
  account: {
    equity: number;
    realized_pnl: number;
    daily_pnl: number;
    peak: number;
  };
  execution_metrics: {
    orders: number;
    rejected: number;
    rejection_rate: number | null;
    unknown: number;
    ack_p95_ms: number | null;
    fill_p95_ms: number | null;
  };
  live_positions?: Record<
    string,
    {
      quantity: number;
      account_quantity: number;
      account_age_ms: number | null;
      reconciled: boolean;
      current_notional: number;
      target_notional: number;
      effective_leverage: number | null;
      target_leverage: number | null;
      entry_price: number | null;
      mark_price: number | null;
      unrealized_pnl: number | null;
      liquidation_buffer: number | null;
      sl: number | null;
      tp: number | null;
      deadline_ts: number | null;
      action: string;
      reason: string;
      pending: { action: string; status: string }[];
      feedback: {
        samples: number;
        excluded_samples: number;
        execution_drift: number | null;
        ev_drift: number | null;
        status: string;
      };
    }
  >;
  ai_mode?: "OFF" | "ASSIST";
  live_enabled: boolean;
  auto_evidence_ready: boolean;
  live: {
    configured: boolean;
    venue: string;
    environment: string;
    cap: number;
    account: null | {
      equity: number;
      available: number;
      positions: Record<string, { quantity: number }>;
    };
    trades: {
      parent_id: string;
      symbol: string;
      net_pnl: number;
      expected_net_pnl: number;
      fees: number;
      funding_income: number;
    }[];
    orders: Record<
      string,
      {
        client_id: string;
        action: string;
        plan: Plan;
        created_ts: number;
        status: string;
        filled_qty: number;
        average_price: number;
        fee: number;
        send_ts: number;
        ack_ts: number;
        reason: string;
      }
    >;
  };
  research: {
    status: string;
    model: string;
    interval_secs: number;
    output: string;
    result: unknown;
  };
  live_gate_reason: string | null;
};
