use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Distribution {
    pub mean: Vec<f64>,
    pub scale: Vec<f64>,
    pub max_z: f64,
    pub volatility_boundary: f64,
    pub spread_boundary: f64,
    pub samples: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegimeCalibration {
    pub temperature: f64,
    pub samples: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub model_version: String,
    pub model_type: String,
    pub feature_schema_version: String,
    pub label_version: String,
    pub calibration_version: String,
    pub artifact_hash: String,
    pub training_window: String,
    pub validation_window: String,
    pub source: String,
    pub temperature: f64,
    pub weights: Option<Vec<Vec<f64>>>,
    pub validated: bool,
    pub horizon: u64,
    pub target: f64,
    pub stop: f64,
    pub cost_bps: f64,
    #[serde(default)]
    pub distributions: BTreeMap<String, Distribution>,
    #[serde(default)]
    pub regime_calibrations: BTreeMap<String, RegimeCalibration>,
}
impl Model {
    pub fn baseline(demo: bool, c: &Config) -> Self {
        Self {
            model_version: if demo {
                "score-demo-v1"
            } else {
                "score-v1-unvalidated"
            }
            .into(),
            model_type: "deterministic-score".into(),
            feature_schema_version: "micro-v1".into(),
            label_version: "first-hit-v1".into(),
            calibration_version: if demo { "synthetic-only" } else { "none" }.into(),
            artifact_hash: "built-in-score-v1".into(),
            training_window: "none".into(),
            validation_window: "none".into(),
            source: if demo { "demo" } else { "live" }.into(),
            temperature: 1.25,
            weights: None,
            validated: demo,
            horizon: c.horizon,
            target: c.target,
            stop: c.stop,
            cost_bps: (c.fee_bps * 2.)
                + c.slippage_bps * 2.
                + c.impact_bps
                + c.adverse_selection_bps
                + c.funding_bps,
            distributions: BTreeMap::new(),
            regime_calibrations: BTreeMap::new(),
        }
    }
    pub fn vector(f: &Features) -> Vec<f64> {
        vec![
            1.,
            f.book_imbalance,
            f.flow_imbalance,
            f.aggressive_imbalance,
            f.microprice_bps / 5.,
            f.momentum * 1000.,
            f.volatility * 1000.,
            f.cross_venue_bps.unwrap_or(0.) / 10.,
            f.spread_bps / 5.,
            f.depth.max(1.).ln() / 15.,
        ]
    }
    pub fn score(f: &Features) -> f64 {
        (1.5 * f.book_imbalance
            + 0.8 * f.flow_imbalance
            + 1.2 * f.aggressive_imbalance
            + f.microprice_bps * 0.15
            + (f.momentum * 500.).clamp(-1., 1.))
        .clamp(-3., 3.)
    }
    pub fn probabilities(&self, f: &Features, c: &Config) -> ([f64; 3], Option<[f64; 3]>) {
        let score = Self::score(f);
        let logits = if let Some(w) = &self.weights {
            let x = Self::vector(f);
            [dot(&w[0], &x), dot(&w[1], &x), dot(&w[2], &x)]
        } else {
            [score, -score, -0.2]
        };
        let raw = softmax(logits, 1.);
        let compatible = self.validated
            && self.horizon == c.horizon
            && (self.target - c.target).abs() < 1e-9
            && (self.stop - c.stop).abs() < 1e-9;
        let temperature = if self.distributions.is_empty() && self.source == "demo" {
            Some(self.temperature)
        } else {
            self.regime_calibrations
                .get(&self.regime(f))
                .map(|r| r.temperature)
        };
        (
            raw,
            temperature
                .filter(|_| compatible)
                .map(|t| softmax(logits, t)),
        )
    }
    pub fn regime(&self, f: &Features) -> String {
        let Some(d) = self.distributions.get(&f.symbol) else {
            return "UNKNOWN".into();
        };
        let name = match (
            f.volatility > d.volatility_boundary,
            f.spread_bps > d.spread_boundary,
        ) {
            (false, false) => "NORMAL",
            (true, false) => "HIGH_VOL",
            (false, true) => "WIDE_SPREAD",
            (true, true) => "STRESSED",
        };
        format!("{}:{name}", f.symbol)
    }
    pub fn distribution_score(&self, f: &Features) -> Option<f64> {
        let d = self.distributions.get(&f.symbol)?;
        Some(
            Self::vector(f)
                .iter()
                .zip(&d.mean)
                .zip(&d.scale)
                .skip(1)
                .map(|((x, m), s)| ((x - m) / s).abs())
                .fold(0., f64::max),
        )
    }
    pub fn health(
        &self,
        outcomes: &[Outcome],
        trades: &[TradeResult],
        c: &Config,
        f: &Features,
    ) -> ModelHealth {
        let score = self.distribution_score(f);
        let synthetic = self.source == "demo" && self.distributions.is_empty();
        let in_distribution = score
            .zip(self.distributions.get(&f.symbol))
            .is_some_and(|(z, d)| z <= d.max_z);
        let fixed_limit = f.volatility > c.max_volatility
            || f.cross_venue_bps
                .is_some_and(|v| v.abs() > c.max_cross_venue_bps);
        let mut h = health(
            outcomes,
            trades,
            c,
            fixed_limit || (!synthetic && !in_distribution),
        );
        h.regime = self.regime(f);
        h.distribution_score = score;
        h.distribution_status = if synthetic {
            "SYNTHETIC_ONLY"
        } else if score.is_none() {
            "UNAVAILABLE"
        } else if in_distribution {
            "IN_DISTRIBUTION"
        } else {
            "OUT_OF_DISTRIBUTION"
        }
        .into();
        let recent: Vec<_> = outcomes
            .iter()
            .rev()
            .filter(|o| o.learning_eligible && self.regime(&o.prediction.features) == h.regime)
            .take(1000)
            .collect();
        h.regime_samples = recent.len();
        h.regime_calibration_error =
            (!recent.is_empty()).then(|| ece(&calibration_bins(&recent), recent.len()));
        if !synthetic
            && (recent.len() < c.min_samples
                || !self.regime_calibrations.contains_key(&h.regime)
                || h.regime_calibration_error
                    .is_some_and(|e| e > c.calibration_low))
        {
            h.trust = "LOW".into();
            h.reasons.push(format!(
                "市场状态 {} 的校准证据不足或失效：{} 条",
                h.regime,
                recent.len()
            ));
        } else if !synthetic
            && h.regime_calibration_error
                .is_some_and(|e| e > c.calibration_warning)
            && h.trust != "LOW"
        {
            h.trust = "DEGRADED".into();
            h.reasons
                .push("当前市场状态的校准误差偏高，降低仓位".into());
        }
        if !synthetic && !in_distribution {
            h.reasons.push("训练分布缺失或当前特征超出训练分布".into());
        }
        h.retrain_recommended =
            h.samples >= c.min_samples && ["LOW", "DEGRADED"].contains(&h.trust.as_str());
        h
    }
    pub fn validate(&self, demo: bool) -> Result<(), String> {
        if self.source != if demo { "demo" } else { "live" }
            || self.feature_schema_version != "micro-v1"
            || self.label_version != "first-hit-v1"
            || !self.temperature.is_finite()
            || !(0.1..=10.).contains(&self.temperature)
            || !self.validated
            || self.model_version.is_empty()
            || (!demo && (self.distributions.len() != 2 || self.regime_calibrations.is_empty()))
            || self.distributions.iter().any(|(s, d)| {
                !["BTCUSDT", "ETHUSDT"].contains(&s.as_str())
                    || d.samples < 20
                    || d.mean.len() != 10
                    || d.scale.len() != 10
                    || d.mean.iter().any(|v| !v.is_finite())
                    || d.scale.iter().any(|v| !v.is_finite() || *v <= 0.)
                    || !d.max_z.is_finite()
                    || !(1.0..=10.0).contains(&d.max_z)
                    || !d.volatility_boundary.is_finite()
                    || d.volatility_boundary < 0.
                    || !d.spread_boundary.is_finite()
                    || d.spread_boundary < 0.
            })
            || self.regime_calibrations.iter().any(|(key, r)| {
                let Some((symbol, regime)) = key.split_once(':') else {
                    return true;
                };
                !self.distributions.contains_key(symbol)
                    || !["NORMAL", "HIGH_VOL", "WIDE_SPREAD", "STRESSED"].contains(&regime)
                    || r.samples < 20
                    || !r.temperature.is_finite()
                    || !(0.1..=10.).contains(&r.temperature)
            })
            || self.weights.as_ref().is_some_and(|w| {
                w.len() != 3
                    || w.iter()
                        .any(|r| r.len() != 10 || r.iter().any(|v| !v.is_finite()))
            })
        {
            return Err("模型来源、schema、校准或权重不兼容".into());
        }
        Ok(())
    }
}
fn dot(w: &[f64], x: &[f64]) -> f64 {
    w.iter().zip(x).map(|(a, b)| a * b).sum()
}
pub fn softmax(x: [f64; 3], temperature: f64) -> [f64; 3] {
    let m = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let y = x.map(|v| ((v - m) / temperature).exp());
    let sum: f64 = y.iter().sum();
    y.map(|v| v / sum)
}

pub fn health(
    outcomes: &[Outcome],
    trades: &[TradeResult],
    c: &Config,
    regime_drift: bool,
) -> ModelHealth {
    let valid: Vec<_> = outcomes.iter().filter(|o| o.learning_eligible).collect();
    let window = 100.max(c.min_samples).max(c.high_trust_samples);
    let recent: Vec<_> = valid.iter().rev().take(window).copied().collect();
    let n = recent.len();
    let denom = n.max(1) as f64;
    let bins = calibration_bins(&recent);
    let error = ece(&bins, n);
    let baseline: Vec<_> = valid
        .iter()
        .rev()
        .skip(window)
        .take(window * 2)
        .copied()
        .collect();
    let expected = recent
        .iter()
        .map(|o| o.prediction.qualification.net_edge)
        .sum::<f64>()
        / denom;
    let observed = recent.iter().map(|o| o.observed_net_edge).sum::<f64>() / denom;
    let recent_trades: Vec<_> = trades.iter().rev().take(50).collect();
    let execution_drift = (!recent_trades.is_empty()).then(|| {
        recent_trades
            .iter()
            .map(|t| (t.realized_cost - t.expected_cost) / t.position.cost_basis.max(1.))
            .sum::<f64>()
            / recent_trades.len() as f64
    });
    let opposite = recent
        .iter()
        .filter(|o| o.outcome_type == "STRONG_OPPOSITE_MOVE")
        .count() as f64
        / denom;
    let mut maes: Vec<f64> = recent.iter().map(|o| o.mae).collect();
    maes.sort_by(f64::total_cmp);
    let mae_p95 = maes
        .get((n as f64 * 0.95).floor() as usize)
        .or(maes.last())
        .copied()
        .unwrap_or(0.);
    let mut reasons = vec![];
    let trust = if n < c.min_samples {
        reasons.push(format!("近期样本不足：{n}/{}", c.min_samples));
        "LOW"
    } else if error > c.calibration_low
        || regime_drift
        || opposite > c.max_opposite_rate
        || mae_p95 > c.stop * c.max_mae_stop_multiple
    {
        reasons.push("校准 / 市场状态 / 尾部风险失效，暂停新交易".into());
        "LOW"
    } else if error > c.calibration_warning
        || expected - observed > c.ev_drift_limit
        || execution_drift.is_some_and(|d| d > c.max_slippage)
    {
        reasons.push("校准或成本后收益偏移，降低仓位".into());
        "DEGRADED"
    } else if n >= c.high_trust_samples && error < c.calibration_warning / 2. {
        "HIGH"
    } else {
        "NORMAL"
    };
    ModelHealth {
        trust: trust.into(),
        samples: n,
        predicted_hit_rate: recent
            .iter()
            .map(|o| o.prediction.win_probability())
            .sum::<f64>()
            / denom,
        actual_hit_rate: recent
            .iter()
            .filter(|o| o.outcome_type == "TARGET_HIT")
            .count() as f64
            / denom,
        calibration_error: error,
        baseline_error: (baseline.len() >= c.min_samples)
            .then(|| ece(&calibration_bins(&baseline), baseline.len())),
        brier: recent.iter().map(|o| o.brier).sum::<f64>() / denom,
        expected_edge: expected,
        observed_edge: observed,
        ev_drift: observed - expected,
        execution_drift,
        strong_opposite_rate: opposite,
        mae_p95,
        regime_drift,
        retrain_recommended: n >= c.min_samples && ["LOW", "DEGRADED"].contains(&trust),
        reasons,
        bins,
        regime: "UNKNOWN".into(),
        regime_samples: 0,
        regime_calibration_error: None,
        distribution_score: None,
        distribution_status: "UNAVAILABLE".into(),
    }
}
fn calibration_bins(data: &[&Outcome]) -> Vec<CalibrationBin> {
    (0..10)
        .map(|i| {
            let rows: Vec<_> = data
                .iter()
                .filter(|o| (o.prediction.win_probability() * 10.).floor().min(9.) as usize == i)
                .collect();
            let n = rows.len();
            CalibrationBin {
                lower: i as f64 / 10.,
                count: n,
                predicted: rows
                    .iter()
                    .map(|o| o.prediction.win_probability())
                    .sum::<f64>()
                    / n.max(1) as f64,
                actual: rows
                    .iter()
                    .filter(|o| o.outcome_type == "TARGET_HIT")
                    .count() as f64
                    / n.max(1) as f64,
            }
        })
        .collect()
}
fn ece(bins: &[CalibrationBin], n: usize) -> f64 {
    bins.iter()
        .map(|b| (b.predicted - b.actual).abs() * b.count as f64)
        .sum::<f64>()
        / n.max(1) as f64
}
