use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub equity: f64,
    pub risk_per_trade: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub max_position_notional: f64,
    pub max_effective_leverage: f64,
    pub min_liquidation_buffer: f64,
    pub max_slippage: f64,
    pub min_net_edge: f64,
    pub min_net_pnl: f64,
    pub fee_bps: f64,
    pub slippage_bps: f64,
    pub adverse_selection_bps: f64,
    pub impact_bps: f64,
    pub funding_bps: f64,
    pub horizon: u64,
    pub target: f64,
    pub stop: f64,
    pub min_probability: f64,
    pub max_spread_bps: f64,
    pub max_volatility: f64,
    pub max_cross_venue_bps: f64,
    pub max_stale_ms: u64,
    pub min_depth: f64,
    pub depth_participation: f64,
    pub min_samples: usize,
    pub calibration_warning: f64,
    pub calibration_low: f64,
    pub ev_drift_limit: f64,
    pub max_rebalances: u32,
    pub rebalance_interval_secs: u64,
    pub position_deadband: f64,
    pub incremental_ev: f64,
    pub kill_behavior: String,
    pub signal_enter: f64,
    pub signal_exit: f64,
    pub signal_confirm_ms: u64,
    pub result_hold_ms: u64,
    pub max_opposite_rate: f64,
    pub max_mae_stop_multiple: f64,
    pub high_trust_samples: usize,
    pub alpha_eval_interval_ms: u64,
    pub qualification_eval_interval_ms: u64,
    pub auto_min_net_edge: f64,
    pub auto_required_model_trust: String,
    pub auto_max_concurrent_positions: usize,
    pub reentry_cooldown_seconds: u64,
    pub max_auto_entries_per_window: usize,
    pub auto_entry_window_seconds: u64,
    pub max_daily_trades: usize,
    pub admission_priority_policy_version: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            equity: 10_000.,
            risk_per_trade: 0.0025,
            max_daily_loss: 100.,
            max_drawdown: 0.05,
            max_position_notional: 5_000.,
            max_effective_leverage: 1.,
            min_liquidation_buffer: 0.2,
            max_slippage: 0.0005,
            min_net_edge: 0.0002,
            min_net_pnl: 0.5,
            fee_bps: 4.,
            slippage_bps: 1.,
            adverse_selection_bps: 0.5,
            impact_bps: 0.5,
            funding_bps: 0.5,
            horizon: 60,
            target: 0.003,
            stop: 0.001,
            min_probability: 0.55,
            max_spread_bps: 5.,
            max_volatility: 0.005,
            max_cross_venue_bps: 15.,
            max_stale_ms: 3000,
            min_depth: 25_000.,
            depth_participation: 0.01,
            min_samples: 30,
            calibration_warning: 0.12,
            calibration_low: 0.22,
            ev_drift_limit: 0.001,
            max_rebalances: 3,
            rebalance_interval_secs: 15,
            position_deadband: 0.15,
            incremental_ev: 0.0003,
            kill_behavior: "hold".into(),
            signal_enter: 0.35,
            signal_exit: 0.15,
            signal_confirm_ms: 2000,
            result_hold_ms: 5000,
            max_opposite_rate: 0.2,
            max_mae_stop_multiple: 3.,
            high_trust_samples: 100,
            alpha_eval_interval_ms: 1000,
            qualification_eval_interval_ms: 1000,
            auto_min_net_edge: 0.0004,
            auto_required_model_trust: "HIGH".into(),
            auto_max_concurrent_positions: 1,
            reentry_cooldown_seconds: 60,
            max_auto_entries_per_window: 3,
            auto_entry_window_seconds: 300,
            max_daily_trades: 20,
            admission_priority_policy_version: "priority-v1".into(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        let v = serde_json::to_value(self).map_err(|e| e.to_string())?;
        if v.as_object()
            .unwrap()
            .values()
            .any(|v| v.is_null() || v.as_f64().is_some_and(|n| !n.is_finite() || n < 0.))
        {
            return Err("配置必须为有限非负值".into());
        }
        if self.auto_min_net_edge < self.min_net_edge
            || self.auto_min_net_edge > 0.1
            || !["NORMAL", "HIGH"].contains(&self.auto_required_model_trust.as_str())
            || !(1..=2).contains(&self.auto_max_concurrent_positions)
            || !(1..=86400).contains(&self.reentry_cooldown_seconds)
            || !(1..=100).contains(&self.max_auto_entries_per_window)
            || !(10..=86400).contains(&self.auto_entry_window_seconds)
            || !(1..=1000).contains(&self.max_daily_trades)
            || self.admission_priority_policy_version != "priority-v1"
            || !(100..=5000).contains(&self.alpha_eval_interval_ms)
            || !(100..=5000).contains(&self.qualification_eval_interval_ms)
            || self.equity < 100.
            || self.equity > 1_000_000.
            || self.risk_per_trade <= 0.
            || self.risk_per_trade > 0.02
            || self.max_effective_leverage <= 0.
            || self.max_effective_leverage > 3.
            || self.max_position_notional <= 0.
            || self.max_position_notional > self.equity * 3.
            || self.max_daily_loss <= 0.
            || self.max_daily_loss > self.equity * 0.1
            || self.max_drawdown <= 0.
            || self.max_drawdown > 0.2
            || ![5, 15, 30, 60, 120].contains(&self.horizon)
            || self.target < 0.0005
            || self.target > 0.01
            || self.stop < 0.0002
            || self.stop > 0.01
            || self.min_probability < 0.5
            || self.min_probability > 0.95
            || self.min_samples < 20
            || self.min_samples > 1000
            || self.calibration_warning <= 0.
            || self.calibration_warning >= self.calibration_low
            || self.calibration_low > 0.5
            || self.max_stale_ms < 500
            || self.max_stale_ms > 10_000
            || self.min_depth <= 0.
            || self.depth_participation <= 0.
            || self.depth_participation > 0.05
            || self.min_liquidation_buffer < 0.1
            || self.min_liquidation_buffer > 0.9
            || self.max_slippage > 0.005
            || self.max_spread_bps <= 0.
            || self.max_spread_bps > 50.
            || self.fee_bps > 100.
            || self.slippage_bps > 50.
            || self.max_volatility <= 0.
            || self.max_volatility > 0.1
            || self.position_deadband < 0.05
            || self.position_deadband > 0.8
            || self.rebalance_interval_secs < 5
            || self.max_rebalances > 10
            || self.signal_enter <= self.signal_exit
            || self.signal_enter > 3.
            || !(1000..=30_000).contains(&self.signal_confirm_ms)
            || !(1000..=60_000).contains(&self.result_hold_ms)
            || self.max_opposite_rate > 0.5
            || !(1.0..=10.0).contains(&self.max_mae_stop_multiple)
            || !(self.min_samples..=1000).contains(&self.high_trust_samples)
            || !["hold", "flatten"].contains(&self.kill_behavior.as_str())
        {
            return Err("配置超出个人版风险边界，请检查资金、杠杆、期限与阈值".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketEvent {
    pub exchange: String,
    pub symbol: String,
    pub exchange_ts: u64,
    pub receive_ts: u64,
    pub sequence_id: i64,
    #[serde(flatten)]
    pub data: EventData,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum EventData {
    Snapshot {
        bids: Vec<[f64; 2]>,
        asks: Vec<[f64; 2]>,
    },
    Delta {
        first: i64,
        previous: i64,
        bids: Vec<[f64; 2]>,
        asks: Vec<[f64; 2]>,
    },
    Trade {
        price: f64,
        quantity: f64,
        buy: bool,
    },
    Mark {
        mark: f64,
        index: f64,
        funding: f64,
    },
    Context {
        mark: Option<f64>,
        index: Option<f64>,
        funding: Option<f64>,
        next_funding_ts: Option<u64>,
    },
    Health {
        connected: bool,
        reason: String,
    },
    Clock,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Features {
    pub feature_snapshot_id: String,
    pub feature_schema_version: String,
    pub symbol: String,
    pub ts: u64,
    pub mid: f64,
    pub spread_bps: f64,
    pub book_imbalance: f64,
    pub flow_imbalance: f64,
    pub aggressive_imbalance: f64,
    pub microprice_bps: f64,
    pub momentum: f64,
    pub volatility: f64,
    pub depth: f64,
    pub cross_venue_bps: Option<f64>,
    /// Legacy field: synchronized one-second return difference (OKX minus Binance).
    pub cross_venue_lead_bps: Option<f64>,
    #[serde(default)]
    pub cross_venue_lag: Option<crate::cross_venue::LeadLag>,
    pub mark: Option<f64>,
    pub index: Option<f64>,
    #[serde(default)]
    pub funding_rate: Option<f64>,
    #[serde(default)]
    pub next_funding_ts: Option<u64>,
    #[serde(default)]
    pub feature_lag_ms: u64,
    pub data_quality: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gate {
    pub code: String,
    pub name: String,
    pub pass: bool,
    pub detail: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Costs {
    pub spread: f64,
    pub entry_fee: f64,
    pub exit_fee: f64,
    pub slippage: f64,
    pub impact: f64,
    pub adverse_selection: f64,
    pub funding: f64,
    pub total: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Qualification {
    pub prediction_id: String,
    pub gate_version: String,
    pub status: String,
    pub gates: Vec<Gate>,
    pub reasons: Vec<String>,
    pub net_edge: f64,
    pub gross_edge: f64,
    pub costs: Costs,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradePlan {
    pub plan_id: String,
    pub prediction_id: String,
    pub symbol: String,
    pub direction: String,
    pub position_notional: f64,
    pub effective_leverage: f64,
    pub expected_net_pnl: f64,
    pub expected_loss: f64,
    pub planned_max_loss: f64,
    pub break_even_probability: f64,
    pub expected_mae: f64,
    pub liquidity_capacity: f64,
    pub liquidation_buffer: f64,
    pub tp: f64,
    pub sl: f64,
    pub horizon: u64,
    pub entry_type: String,
    pub exit_rules: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EpisodeMetadata {
    pub origin_trigger_id: Option<String>,
    pub origin_trigger_families: Vec<String>,
    pub episode_id: String,
    pub opened_at: u64,
    pub closed_at: Option<u64>,
    pub upper_barrier: f64,
    pub lower_barrier: f64,
    pub latest_evaluated_at: u64,
    pub latest_raw_probabilities: Option<[f64; 3]>,
    pub latest_calibrated_probabilities: Option<[f64; 3]>,
    pub latest_feature_snapshot_id: String,
    pub highest_qualification: String,
    pub close_reason: Option<String>,
    pub signal_epoch: u64,
    pub last_price_ts: u64,
    pub first_hit: Option<String>,
    pub first_hit_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlphaEvaluation {
    pub symbol: String,
    pub evaluated_at: u64,
    pub qualification_evaluated_at: u64,
    pub raw_probabilities: [f64; 3],
    pub calibrated_probabilities: Option<[f64; 3]>,
    pub features: Features,
    pub qualification: Option<Qualification>,
    pub target_position: Option<TradePlan>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrityCounters {
    pub realtime_evaluations: u64,
    pub prediction_episodes_opened: u64,
    pub prediction_episodes_closed: u64,
    pub qualified_episodes: u64,
    pub paper_trades: u64,
    pub live_trades: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prediction {
    #[serde(default)]
    pub episode: EpisodeMetadata,
    pub prediction_id: String,
    pub symbol: String,
    pub ts: u64,
    pub horizon: u64,
    pub direction: String,
    pub target: f64,
    pub stop: f64,
    pub start_price: f64,
    pub current_price: f64,
    /// Calibrated first-hit payoff expectation in the forecast direction; timeout payoff is zero.
    #[serde(default)]
    pub expected_move: Option<f64>,
    /// Absolute deterministic score normalized to [0,1], not a probability.
    #[serde(default)]
    pub signal_strength: f64,
    pub raw_probabilities: [f64; 3],
    pub calibrated_probabilities: Option<[f64; 3]>,
    pub model_version: String,
    pub feature_snapshot_id: String,
    pub features: Features,
    pub status: String,
    pub mfe: f64,
    pub mae: f64,
    pub qualification: Qualification,
    pub plan: Option<TradePlan>,
    pub config_version: u64,
    #[serde(default)]
    pub cost_assumptions_bps: Option<f64>,
}
impl Prediction {
    pub fn sign(&self) -> f64 {
        if self.direction == "LONG" {
            1.
        } else {
            -1.
        }
    }
    pub fn probabilities(&self) -> [f64; 3] {
        self.calibrated_probabilities
            .unwrap_or(self.raw_probabilities)
    }
    pub fn win_probability(&self) -> f64 {
        self.probabilities()[if self.direction == "LONG" { 0 } else { 1 }]
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    #[serde(default)]
    pub horizon_observation: Option<crate::horizon::HorizonObservation>,
    pub prediction: Prediction,
    pub outcome_type: String,
    pub first_hit_ts: u64,
    pub final_move: f64,
    pub mfe: f64,
    pub mae: f64,
    pub prediction_error: String,
    pub brier: f64,
    pub observed_net_edge: f64,
    pub learning_eligible: bool,
    pub annotation: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Execution {
    pub order_id: String,
    pub plan_id: String,
    pub side: String,
    pub qty: f64,
    pub expected_price: f64,
    pub fill_price: f64,
    pub fee: f64,
    pub slippage: f64,
    pub send_ts: u64,
    pub ack_ts: u64,
    pub fill_ts: u64,
    pub liquidity_limited: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub plan: TradePlan,
    pub qty: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub fees: f64,
    pub funding: f64,
    pub realized_pnl: f64,
    pub ts: u64,
    pub last_rebalance: u64,
    pub adjustments: u32,
    pub action: String,
    pub reason: String,
    pub target_notional: f64,
    pub executions: Vec<Execution>,
    pub config: Config,
    pub expected_cost: f64,
    pub expected_edge: f64,
    pub cost_basis: f64,
    #[serde(default)]
    pub alpha_ids: Vec<String>,
    #[serde(default)]
    pub last_funding_ts: u64,
    #[serde(default)]
    pub funding_observed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeResult {
    pub position: Position,
    pub exit_ts: u64,
    pub exit_reason: String,
    pub net_pnl: f64,
    pub realized_cost: f64,
    pub expected_cost: f64,
    pub realized_net_edge: f64,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub lower: f64,
    pub count: usize,
    pub predicted: f64,
    pub actual: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelHealth {
    #[serde(default)]
    pub live: crate::live::LiveHealth,
    pub trust: String,
    pub samples: usize,
    pub predicted_hit_rate: f64,
    pub actual_hit_rate: f64,
    pub calibration_error: f64,
    pub baseline_error: Option<f64>,
    pub brier: f64,
    pub expected_edge: f64,
    pub observed_edge: f64,
    pub ev_drift: f64,
    pub execution_drift: Option<f64>,
    #[serde(default)]
    pub paper_samples: usize,
    #[serde(default)]
    pub paper_ev_drift: Option<f64>,
    pub strong_opposite_rate: f64,
    pub mae_p95: f64,
    pub regime_drift: bool,
    pub retrain_recommended: bool,
    pub reasons: Vec<String>,
    pub bins: Vec<CalibrationBin>,
    #[serde(default)]
    pub regime: String,
    #[serde(default)]
    pub regime_samples: usize,
    #[serde(default)]
    pub regime_calibration_error: Option<f64>,
    #[serde(default)]
    pub distribution_score: Option<f64>,
    #[serde(default)]
    pub distribution_status: String,
}
