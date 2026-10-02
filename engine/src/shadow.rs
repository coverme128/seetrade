//! Paired, read-only forecasts. These records never enter the execution path.
use crate::{model::Model, types::*};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShadowPolicy {
    pub min_samples: usize,
    pub min_symbol_samples: usize,
    pub min_regime_samples: usize,
    pub min_selected: usize,
    pub min_window_selected: usize,
    pub max_ece: f64,
    pub min_coverage: f64,
    pub max_drawdown: f64,
    pub max_tail_loss: f64,
    pub max_opposite_rate: f64,
    pub max_invalid_rate: f64,
    pub max_evidence_age_ms: u64,
}
impl Default for ShadowPolicy {
    fn default() -> Self {
        Self {
            min_samples: 120,
            min_symbol_samples: 30,
            min_regime_samples: 20,
            min_selected: 30,
            min_window_selected: 5,
            max_ece: 0.12,
            min_coverage: 0.9,
            max_drawdown: 0.05,
            max_tail_loss: 0.005,
            max_opposite_rate: 0.2,
            max_invalid_rate: 0.1,
            max_evidence_age_ms: 300_000,
        }
    }
}
impl ShadowPolicy {
    pub fn validate(&self) -> Result<(), String> {
        let finite = [
            self.max_ece,
            self.min_coverage,
            self.max_drawdown,
            self.max_tail_loss,
            self.max_opposite_rate,
            self.max_invalid_rate,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite
            || !(120..=2000).contains(&self.min_samples)
            || !(30..=1000).contains(&self.min_symbol_samples)
            || !(20..=1000).contains(&self.min_regime_samples)
            || !(30..=1000).contains(&self.min_selected)
            || !(5..=500).contains(&self.min_window_selected)
            || !(0.01..=0.22).contains(&self.max_ece)
            || !(0.8..=1.).contains(&self.min_coverage)
            || !(0.001..=0.2).contains(&self.max_drawdown)
            || !(0.0001..=0.02).contains(&self.max_tail_loss)
            || !(0.0..=0.2).contains(&self.max_opposite_rate)
            || !(0.0..=0.2).contains(&self.max_invalid_rate)
            || !(10_000..=3_600_000).contains(&self.max_evidence_age_ms)
        {
            return Err("影子验证阈值超出研究边界".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Forecast {
    pub model_version: String,
    pub raw: [f64; 3],
    pub calibrated: Option<[f64; 3]>,
    pub in_distribution: bool,
    pub selected: bool,
    pub net_edge: f64,
}
impl Forecast {
    fn new(model: &Model, p: &Prediction, c: &Config) -> Self {
        let (raw, calibrated) = model.probabilities(&p.features, c);
        let prob = calibrated.unwrap_or(raw);
        let side = usize::from(p.direction != "LONG");
        let net_edge =
            prob[side] * p.target - prob[1 - side] * p.stop - p.qualification.costs.total;
        let in_distribution = model
            .distribution_score(&p.features)
            .zip(model.distributions.get(&p.symbol))
            .is_some_and(|(z, d)| z <= d.max_z);
        let selected = calibrated.is_some()
            && in_distribution
            && net_edge > c.min_net_edge
            && prob[side] >= c.min_probability
            && p.features.data_quality.is_empty()
            && p.features.spread_bps <= c.max_spread_bps
            && p.features.volatility <= c.max_volatility
            && p.features.depth >= c.min_depth;
        Self {
            model_version: model.model_version.clone(),
            raw,
            calibrated,
            in_distribution,
            selected,
            net_edge,
        }
    }
    fn probabilities(&self) -> [f64; 3] {
        self.calibrated.unwrap_or(self.raw)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedPrediction {
    pub prediction_id: String,
    pub feature_snapshot_id: String,
    pub symbol: String,
    pub ts: u64,
    pub direction: String,
    pub horizon: u64,
    pub target: f64,
    pub stop: f64,
    pub regime: String,
    pub champion: Forecast,
    pub candidate: Forecast,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedOutcome {
    pub prediction: PairedPrediction,
    pub outcome_type: String,
    pub first_hit_ts: u64,
    pub observed_net_edge: f64,
    pub learning_eligible: bool,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub samples: usize,
    pub selected: usize,
    pub coverage: f64,
    pub ece: f64,
    pub brier: f64,
    pub expected_net_edge: f64,
    pub observed_net_edge: f64,
    pub drawdown: f64,
    pub tail_p05: f64,
    pub strong_opposite_rate: f64,
}
impl Metrics {
    pub fn compute(rows: &[&PairedOutcome], candidate: bool) -> Self {
        let mut m = Self {
            samples: rows.len(),
            ..Self::default()
        };
        if rows.is_empty() {
            return m;
        }
        let mut bins = [(0usize, 0., 0.); 10];
        let mut equity: f64 = 0.;
        let mut peak: f64 = 0.;
        let mut edges = vec![];
        let mut ordered = rows.to_vec();
        ordered.sort_by_key(|r| (r.first_hit_ts, &r.prediction.prediction_id));
        for row in ordered {
            let p = &row.prediction;
            let f = if candidate { &p.candidate } else { &p.champion };
            let prob = f.probabilities();
            let side = usize::from(p.direction != "LONG");
            let class = match row.outcome_type.as_str() {
                "TARGET_HIT" => side,
                "STOP_HIT" | "STRONG_OPPOSITE_MOVE" => 1 - side,
                _ => 2,
            };
            let b = &mut bins[(prob[side] * 10.).floor().min(9.) as usize];
            b.0 += 1;
            b.1 += prob[side];
            b.2 += f64::from(row.outcome_type == "TARGET_HIT");
            m.brier += prob
                .iter()
                .enumerate()
                .map(|(i, v)| (v - f64::from(i == class)).powi(2))
                .sum::<f64>();
            m.coverage += f64::from(f.calibrated.is_some() && f.in_distribution);
            m.strong_opposite_rate += f64::from(row.outcome_type == "STRONG_OPPOSITE_MOVE");
            if f.selected {
                m.selected += 1;
                m.expected_net_edge += f.net_edge;
                m.observed_net_edge += row.observed_net_edge;
                equity += row.observed_net_edge;
                peak = peak.max(equity);
                m.drawdown = m.drawdown.max(peak - equity);
                edges.push(row.observed_net_edge);
            }
        }
        let n = rows.len() as f64;
        m.ece = bins
            .iter()
            .map(|(_, predicted, actual)| (predicted - actual).abs())
            .sum::<f64>()
            / n;
        m.brier /= n;
        m.coverage /= n;
        m.strong_opposite_rate /= n;
        m.expected_net_edge /= m.selected.max(1) as f64;
        m.observed_net_edge /= m.selected.max(1) as f64;
        edges.sort_by(f64::total_cmp);
        m.tail_p05 = edges
            .get((edges.len() as f64 * 0.05).floor() as usize)
            .copied()
            .unwrap_or(0.);
        m
    }
    fn stable(&self, policy: &ShadowPolicy, minimum_selected: usize) -> bool {
        self.samples >= policy.min_regime_samples
            && self.selected >= minimum_selected
            && self.ece <= policy.max_ece
            && self.coverage >= policy.min_coverage
            && self.observed_net_edge > 0.
            && self.drawdown <= policy.max_drawdown
            && self.tail_p05 >= -policy.max_tail_loss
            && self.strong_opposite_rate <= policy.max_opposite_rate
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowRun {
    pub run_id: String,
    pub status: String,
    pub started_at: u64,
    pub config_version: u64,
    pub champion_version: String,
    pub champion_hash: String,
    pub candidate: Model,
    pub offline_report: Value,
    pub policy: ShadowPolicy,
    pub pending: BTreeMap<String, PairedPrediction>,
    pub outcomes: Vec<PairedOutcome>,
}
impl ShadowRun {
    pub fn prepare(
        artifact: &str,
        report: Value,
        policy: ShadowPolicy,
        champion: &Model,
        c: &Config,
        now: u64,
        config_version: u64,
    ) -> Result<Self, String> {
        policy.validate()?;
        let mut candidate: Model =
            serde_json::from_str(artifact).map_err(|e| format!("模型 JSON 无效：{e}"))?;
        candidate.validate(champion.source == "demo")?;
        let hash = format!("{:x}", Sha256::digest(artifact.as_bytes()));
        if candidate.model_version == champion.model_version || hash == champion.artifact_hash {
            return Err("候选模型必须使用不同的版本与工件".into());
        }
        let cost = c.fee_bps * 2.
            + c.slippage_bps * 2.
            + c.impact_bps
            + c.adverse_selection_bps
            + c.funding_bps;
        if candidate.horizon != c.horizon
            || (candidate.target - c.target).abs() > 1e-9
            || (candidate.stop - c.stop).abs() > 1e-9
            || (candidate.cost_bps - cost).abs() > 1e-6
        {
            return Err("候选模型的期限、屏障或成本假设与当前配置不符".into());
        }
        let required = [
            "enough_oos",
            "both_symbols",
            "calibration",
            "calibration_coverage",
            "positive_observed_edge",
            "window_stability",
            "regime_calibration",
            "training_distribution",
            "tail",
            "baseline_comparison",
        ];
        if report["sha256"] != hash
            || report["source"] != candidate.source
            || report["status"] != "PAPER_RESEARCH_GATE_PASS"
            || required
                .iter()
                .any(|key| report["checks"][key].as_bool() != Some(true))
            || [
                "refit_each_fold",
                "purged_boundaries",
                "test_windows_disjoint",
            ]
            .iter()
            .any(|key| report["walk_forward"][key] != true)
            || report["counts"]["test"].as_u64().unwrap_or(0) < 100
            || ["BTCUSDT", "ETHUSDT"]
                .iter()
                .any(|s| report["counts"]["symbols"][s].as_u64().unwrap_or(0) < 30)
        {
            return Err("离线报告未通过、来源不符或 SHA-256 与模型不匹配".into());
        }
        let windows = report["chronological_test_windows"]
            .as_array()
            .ok_or("缺少滚动验证窗口")?;
        if windows.len() != 3 {
            return Err("需要三轮滚动验证证据".into());
        }
        let metric_ok = |v: &Value| -> bool {
            let n = |key: &str| v[key].as_f64().filter(|v| v.is_finite());
            n("ece").is_some_and(|v| (0.0..=policy.max_ece).contains(&v))
                && n("observed_net_edge").is_some_and(|v| v > 0.)
                && n("unit_notional_drawdown")
                    .is_some_and(|v| (0.0..=policy.max_drawdown).contains(&v))
                && n("tail_loss_p05").is_some_and(|v| v >= -policy.max_tail_loss)
                && n("strong_opposite_rate")
                    .is_some_and(|v| (0.0..=policy.max_opposite_rate).contains(&v))
                && n("selected").is_some_and(|v| v >= policy.min_window_selected as f64)
        };
        let mut previous_test_end = None;
        for w in windows {
            let ts = |section: &str, key: &str| w[section][key].as_u64().ok_or("验证窗口时间无效");
            let train_start = ts("train", "start")?;
            let train_end = ts("train", "end")?;
            let cal_start = ts("calibration", "start")?;
            let cal_end = ts("calibration", "end")?;
            let test_start = ts("test", "start")?;
            let test_end = ts("test", "end")?;
            if !(train_start <= train_end
                && train_end < cal_start
                && cal_start <= cal_end
                && cal_end < test_start
                && test_start <= test_end
                && test_end <= now)
                || previous_test_end.is_some_and(|v| v >= test_start)
                || !metric_ok(w)
                || w["calibration_coverage"]
                    .as_f64()
                    .is_none_or(|v| v < policy.min_coverage || v > 1.)
            {
                return Err("离线窗口的时间隔离、校准、收益或尾部指标不满足要求".into());
            }
            previous_test_end = Some(test_end);
        }
        let groups = report["final_model_regimes"]
            .as_object()
            .ok_or("缺少离线状态分组证据")?;
        if groups.is_empty()
            || groups.values().any(|v| {
                !metric_ok(v)
                    || v["samples"].as_u64().unwrap_or(0) < policy.min_regime_samples as u64
            })
        {
            return Err("离线市场状态分组证据不足或未通过".into());
        }
        let last = windows.last().unwrap();
        if serde_json::from_str::<Value>(&candidate.training_window)
            .ok()
            .as_ref()
            != Some(&last["train"])
            || serde_json::from_str::<Value>(&candidate.validation_window)
                .ok()
                .as_ref()
                != Some(&last["test"])
        {
            return Err("工件训练 / 验证窗口与报告不匹配".into());
        }
        candidate.artifact_hash = hash;
        Ok(Self {
            run_id: String::new(),
            status: "RUNNING".into(),
            started_at: now,
            config_version,
            champion_version: champion.model_version.clone(),
            champion_hash: champion.artifact_hash.clone(),
            candidate,
            offline_report: report,
            policy,
            pending: BTreeMap::new(),
            outcomes: vec![],
        })
    }
    pub fn predict(
        &mut self,
        p: &Prediction,
        champion: &Model,
        c: &Config,
    ) -> Option<PairedPrediction> {
        if self.status != "RUNNING"
            || p.config_version != self.config_version
            || champion.model_version != self.champion_version
            || champion.artifact_hash != self.champion_hash
        {
            return None;
        }
        let pair = PairedPrediction {
            prediction_id: p.prediction_id.clone(),
            feature_snapshot_id: p.feature_snapshot_id.clone(),
            symbol: p.symbol.clone(),
            ts: p.ts,
            direction: p.direction.clone(),
            horizon: p.horizon,
            target: p.target,
            stop: p.stop,
            regime: self.candidate.regime(&p.features),
            champion: Forecast::new(champion, p, c),
            candidate: Forecast::new(&self.candidate, p, c),
        };
        self.pending.insert(p.prediction_id.clone(), pair.clone());
        Some(pair)
    }
    pub fn finish(&mut self, o: &Outcome) -> Option<PairedOutcome> {
        let prediction = self.pending.remove(&o.prediction.prediction_id)?;
        let pair = PairedOutcome {
            prediction,
            outcome_type: o.outcome_type.clone(),
            first_hit_ts: o.first_hit_ts,
            observed_net_edge: o.observed_net_edge,
            learning_eligible: o.learning_eligible,
        };
        self.outcomes.push(pair.clone());
        if self.outcomes.len() > 2000 {
            self.outcomes.remove(0);
        }
        Some(pair)
    }
    pub fn comparison(&self, champion: &Model, config_version: u64, now: u64) -> Value {
        let mut valid: Vec<_> = self
            .outcomes
            .iter()
            .filter(|r| r.learning_eligible)
            .collect();
        valid.sort_by_key(|r| (r.prediction.ts, &r.prediction.prediction_id));
        let candidate = Metrics::compute(&valid, true);
        let baseline = Metrics::compute(&valid, false);
        let mut times: Vec<_> = valid.iter().map(|r| r.prediction.ts).collect();
        times.dedup();
        let windows: Vec<_> = (0..3)
            .map(|i| {
                let rows: Vec<_> = valid
                    .iter()
                    .copied()
                    .filter(|r| {
                        let index = times.binary_search(&r.prediction.ts).unwrap();
                        index >= times.len() * i / 3 && index < times.len() * (i + 1) / 3
                    })
                    .collect();
                (
                    Metrics::compute(&rows, false),
                    Metrics::compute(&rows, true),
                )
            })
            .collect();
        let mut groups: BTreeMap<String, Vec<&PairedOutcome>> = BTreeMap::new();
        for row in &valid {
            groups
                .entry(row.prediction.regime.clone())
                .or_default()
                .push(row);
        }
        let regimes: BTreeMap<_, _> = groups
            .iter()
            .map(|(key, rows)| (key.clone(), Metrics::compute(rows, true)))
            .collect();
        let symbols: BTreeMap<_, _> = ["BTCUSDT", "ETHUSDT"]
            .iter()
            .map(|s| {
                (
                    *s,
                    valid.iter().filter(|r| r.prediction.symbol == *s).count(),
                )
            })
            .collect();
        let baseline_comparable = baseline.coverage >= self.policy.min_coverage
            && baseline.selected >= self.policy.min_selected;
        let no_worse = |a: &Metrics, b: &Metrics| {
            a.ece <= b.ece + 1e-12
                && a.brier <= b.brier + 1e-12
                && (b.selected < self.policy.min_window_selected
                    || (a.observed_net_edge + 1e-12 >= b.observed_net_edge
                        && a.drawdown <= b.drawdown + 1e-12
                        && a.tail_p05 + 1e-12 >= b.tail_p05))
        };
        let invalid_rate =
            (self.outcomes.len() - valid.len()) as f64 / self.outcomes.len().max(1) as f64;
        let fresh = ["BTCUSDT", "ETHUSDT"].iter().all(|symbol| {
            valid
                .iter()
                .filter(|r| r.prediction.symbol == *symbol)
                .map(|r| r.first_hit_ts)
                .max()
                .is_some_and(|ts| ts <= now && now - ts <= self.policy.max_evidence_age_ms)
        });
        let checks = json!({
            "running":self.status == "RUNNING",
            "same_context":self.config_version == config_version && self.champion_version == champion.model_version && self.champion_hash == champion.artifact_hash,
            "samples":valid.len() >= self.policy.min_samples,
            "both_symbols":symbols.values().all(|n| *n >= self.policy.min_symbol_samples),
            "fresh_evidence":fresh,
            "data_quality":invalid_rate <= self.policy.max_invalid_rate,
            "candidate_metrics":candidate.stable(&self.policy,self.policy.min_selected),
            "window_stability":windows.iter().all(|(_,m)|m.stable(&self.policy,self.policy.min_window_selected)),
            "regime_stability":!regimes.is_empty() && regimes.values().all(|m|m.stable(&self.policy,self.policy.min_window_selected)),
            "champion_comparison":candidate.ece <= baseline.ece + 1e-12 && candidate.brier <= baseline.brier + 1e-12
                && (!baseline_comparable || (no_worse(&candidate,&baseline) && windows.iter().all(|(b,a)|no_worse(a,b)) && groups.values().all(|rows| no_worse(&Metrics::compute(rows,true),&Metrics::compute(rows,false)))))
        });
        let ready = checks.as_object().unwrap().values().all(|v| v == true);
        json!({"ready":ready,"checks":checks,"candidate":candidate,"champion":baseline,"champion_economics_comparable":baseline_comparable,
            "symbols":symbols,"windows":windows.iter().enumerate().map(|(i,(b,a))|json!({"window":i+1,"champion":b,"candidate":a})).collect::<Vec<_>>(),
            "regimes":regimes,"invalid_rate":invalid_rate,"pending":self.pending.len(),"retained":self.outcomes.len()})
    }
}
