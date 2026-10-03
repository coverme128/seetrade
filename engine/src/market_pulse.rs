//! Bounded, causal market-change detector. No model, probability, or trade decision.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const FAMILIES: [&str; 4] = [
    "ORDER_FLOW_CHANGE",
    "AGGRESSIVE_TRADE_BURST",
    "BOOK_IMBALANCE_DEPTH",
    "PRICE_VELOCITY",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TriggerConfig {
    pub normalization_method: String,
    pub sample_interval_ms: u64,
    pub normalization_window_ms: u64,
    pub warmup_samples: usize,
    pub abnormal_z: f64,
    pub extreme_z: f64,
    pub combination_window_ms: u64,
    pub debounce_ms: u64,
    pub candidate_ttl_ms: u64,
    pub candidate_full_eval_interval_ms: u64,
    pub active_full_eval_interval_ms: u64,
    pub position_full_eval_interval_ms: u64,
    pub enabled_families: [bool; 4],
    pub protection_spread: bool,
    pub protection_depth: bool,
    pub depth_drop_fraction: f64,
    pub protection_slippage: bool,
    pub protection_volatility: bool,
    pub protection_data_stale: bool,
    pub protection_book_gap: bool,
}
impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            normalization_method: "robust_z_v1".into(),
            sample_interval_ms: 250,
            normalization_window_ms: 120_000,
            warmup_samples: 32,
            abnormal_z: 3.,
            extreme_z: 6.,
            combination_window_ms: 1000,
            debounce_ms: 2000,
            candidate_ttl_ms: 10_000,
            candidate_full_eval_interval_ms: 1500,
            active_full_eval_interval_ms: 1000,
            position_full_eval_interval_ms: 1000,
            enabled_families: [true; 4],
            protection_spread: true,
            protection_depth: true,
            depth_drop_fraction: 0.5,
            protection_slippage: true,
            protection_volatility: true,
            protection_data_stale: true,
            protection_book_gap: true,
        }
    }
}
impl TriggerConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.normalization_method != "robust_z_v1"
            || !(100..=1000).contains(&self.sample_interval_ms)
            || !(10_000..=600_000).contains(&self.normalization_window_ms)
            || !(8..=512).contains(&self.warmup_samples)
            || self.warmup_samples as u64 * self.sample_interval_ms > self.normalization_window_ms
            || !self.abnormal_z.is_finite()
            || !self.extreme_z.is_finite()
            || !(1.0..=20.).contains(&self.abnormal_z)
            || self.extreme_z <= self.abnormal_z
            || self.extreme_z > 50.
            || !(100..=2000).contains(&self.combination_window_ms)
            || !(250..=10_000).contains(&self.debounce_ms)
            || !(3000..=60_000).contains(&self.candidate_ttl_ms)
            || !(1000..=2000).contains(&self.candidate_full_eval_interval_ms)
            || self.active_full_eval_interval_ms != 1000
            || self.position_full_eval_interval_ms != 1000
            || !self.depth_drop_fraction.is_finite()
            || !(0.1..=0.9).contains(&self.depth_drop_fraction)
            || self.normalization_window_ms / self.sample_interval_ms > 2048
            || !self.protection_data_stale
            || !self.protection_book_gap
        {
            return Err("触发配置超出边界；数据过期/断档保护不能关闭，ACTIVE/持仓保持 1 秒".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    #[default]
    Normal,
    Abnormal,
    Extreme,
}
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeState {
    #[default]
    Idle,
    Candidate,
    Active,
    InPosition,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketPulseSnapshot {
    pub symbol: String,
    pub ts: u64,
    pub exchange_ts: u64,
    pub config_version: u64,
    pub raw_metrics: BTreeMap<String, f64>,
    pub normalized_metrics: BTreeMap<String, f64>,
    pub severity_by_family: BTreeMap<String, Severity>,
    pub baseline_samples: usize,
    pub data_freshness: bool,
    pub spread_bps: f64,
    pub depth: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pulse {
    pub snapshot: MarketPulseSnapshot,
    pub baseline: VecDeque<(u64, [f64; 8])>,
    pub trades: VecDeque<(u64, f64, f64)>,
    pub previous_mid: f64,
    pub previous_depth: f64,
    pub previous_ofi: f64,
    pub previous_velocity: f64,
    pub family_seen: BTreeMap<String, (u64, Severity)>,
}
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    if values.len().is_multiple_of(2) {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.
    } else {
        values[values.len() / 2]
    }
}
pub fn robust_z(value: f64, history: &[f64]) -> f64 {
    if history.is_empty() {
        return 0.;
    }
    let center = median(history.to_vec());
    let mad = median(history.iter().map(|v| (v - center).abs()).collect());
    // Relative scale floor avoids a constant window dividing by zero. Epsilon is numerical only.
    ((value - center) / (1.4826 * mad).max(center.abs() * 0.05).max(1e-9)).clamp(0., 1000.)
}
pub fn combined(severity: &BTreeMap<String, Severity>) -> bool {
    severity.values().any(|s| *s == Severity::Extreme)
        || severity
            .values()
            .filter(|s| **s >= Severity::Abnormal)
            .count()
            >= 2
}
pub struct PulseInput<'a> {
    pub symbol: &'a str,
    pub ts: u64,
    pub exchange_ts: u64,
    pub config_version: u64,
    pub mid: f64,
    pub ofi: f64,
    pub bid_depth: f64,
    pub ask_depth: f64,
    pub spread_bps: f64,
}
impl Pulse {
    pub fn trade(&mut self, ts: u64, notional: f64, buy: bool) {
        self.trades
            .push_back((ts, notional, if buy { notional } else { -notional }));
        while self.trades.len() > 4096
            || self
                .trades
                .front()
                .is_some_and(|v| ts.saturating_sub(v.0) > 1000)
        {
            self.trades.pop_front();
        }
    }
    pub fn sample(&mut self, x: PulseInput<'_>, c: &TriggerConfig) -> Option<MarketPulseSnapshot> {
        if x.ts.saturating_sub(self.snapshot.ts) < c.sample_interval_ms {
            return None;
        }
        while self
            .baseline
            .front()
            .is_some_and(|v| x.ts.saturating_sub(v.0) > c.normalization_window_ms)
        {
            self.baseline.pop_front();
        }
        while self
            .trades
            .front()
            .is_some_and(|v| x.ts.saturating_sub(v.0) > 1000)
        {
            self.trades.pop_front();
        }
        let depth = x.bid_depth + x.ask_depth;
        let notional: f64 = self.trades.iter().map(|v| v.1).sum();
        let signed: f64 = self.trades.iter().map(|v| v.2).sum();
        let velocity = if self.previous_mid > 0. {
            (x.mid / self.previous_mid - 1.)
                / (x.ts.saturating_sub(self.snapshot.ts).max(1) as f64 / 1000.)
        } else {
            0.
        };
        let values = [
            x.ofi.abs(),
            (x.ofi - self.previous_ofi).abs(),
            (signed / notional.max(1e-9)).abs(),
            notional.ln_1p(),
            ((x.bid_depth - x.ask_depth) / depth.max(1e-9)).abs(),
            if self.previous_depth > 0. {
                (1. - depth / self.previous_depth).max(0.)
            } else {
                0.
            },
            velocity.abs(),
            (velocity - self.previous_velocity).abs(),
        ];
        let names = [
            "order_flow_level",
            "order_flow_delta",
            "aggressor_imbalance",
            "trade_notional_log",
            "book_imbalance",
            "depth_depletion",
            "return_velocity",
            "acceleration",
        ];
        let mut normalized = [0.; 8];
        if self.baseline.len() >= c.warmup_samples {
            for i in 0..8 {
                normalized[i] = robust_z(
                    values[i],
                    &self.baseline.iter().map(|(_, v)| v[i]).collect::<Vec<_>>(),
                );
            }
        }
        let mut severity = BTreeMap::new();
        for (i, family) in FAMILIES.iter().enumerate() {
            let score = normalized[i * 2].max(normalized[i * 2 + 1]);
            let current = if !c.enabled_families[i] {
                Severity::Normal
            } else if score >= c.extreme_z {
                Severity::Extreme
            } else if score >= c.abnormal_z {
                Severity::Abnormal
            } else {
                Severity::Normal
            };
            if current > Severity::Normal {
                self.family_seen.insert((*family).into(), (x.ts, current));
            }
            let held = self
                .family_seen
                .get(*family)
                .filter(|(ts, _)| {
                    x.ts.saturating_sub(*ts) <= c.combination_window_ms && c.enabled_families[i]
                })
                .map_or(current, |(_, v)| (*v).max(current));
            severity.insert((*family).into(), held);
        }
        let snapshot = MarketPulseSnapshot {
            symbol: x.symbol.into(),
            ts: x.ts,
            exchange_ts: x.exchange_ts,
            config_version: x.config_version,
            raw_metrics: names
                .iter()
                .zip(values)
                .map(|(k, v)| ((*k).into(), v))
                .collect(),
            normalized_metrics: names
                .iter()
                .zip(normalized)
                .map(|(k, v)| ((*k).into(), v))
                .collect(),
            severity_by_family: severity,
            baseline_samples: self.baseline.len(),
            data_freshness: true,
            spread_bps: x.spread_bps,
            depth,
        };
        self.baseline.push_back((x.ts, values));
        while self.baseline.len() > 2048 {
            self.baseline.pop_front();
        }
        self.previous_mid = x.mid;
        self.previous_depth = depth;
        self.previous_ofi = x.ofi;
        self.previous_velocity = velocity;
        self.snapshot = snapshot.clone();
        Some(snapshot)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketPulseTriggerEvent {
    pub trigger_id: String,
    pub symbol: String,
    pub exchange_scope: String,
    pub occurred_at_exchange_ts: u64,
    pub received_at_local_ts: u64,
    pub trigger_families: Vec<String>,
    pub severity_by_family: BTreeMap<String, Severity>,
    pub normalized_metric_snapshot: MarketPulseSnapshot,
    pub normalization_config_version: u64,
    pub runtime_state_before: RuntimeState,
    pub action: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Runtime {
    pub state: RuntimeState,
    pub pulse: Pulse,
    pub candidate_until: u64,
    pub origin: Option<MarketPulseTriggerEvent>,
    pub last_trigger_ts: u64,
    pub trigger_latched: bool,
    pub protection_reasons: Vec<String>,
    pub event_freshness: BTreeMap<String, bool>,
    pub decision: Option<TriggerDecision>,
    pub last_protection_ts: u64,
    pub last_protection_id: Option<String>,
    pub full_evaluations: u64,
    pub last_full_eval: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriggerDecision {
    pub triggered: bool,
    pub trigger_families: Vec<String>,
    pub severity_by_family: BTreeMap<String, Severity>,
    pub reason_codes: Vec<String>,
    pub evaluated_at: u64,
    pub normalization_config_version: u64,
}
impl MarketPulseSnapshot {
    pub fn decision(&self) -> TriggerDecision {
        let triggered = combined(&self.severity_by_family);
        TriggerDecision {
            triggered,
            trigger_families: self
                .severity_by_family
                .iter()
                .filter(|(_, s)| **s > Severity::Normal)
                .map(|(f, _)| f.clone())
                .collect(),
            severity_by_family: self.severity_by_family.clone(),
            reason_codes: vec![if triggered {
                "EXTREME_OR_TWO_ABNORMAL"
            } else {
                "INSUFFICIENT_COMBINATION_OR_WARMUP"
            }
            .into()],
            evaluated_at: self.ts,
            normalization_config_version: self.config_version,
        }
    }
}
