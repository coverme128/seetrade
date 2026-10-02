//! Deterministic AUTO capital allocation, independent of opportunity qualification.
use crate::{core::Engine, types::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AdmissionState {
    pub counter: u64,
    pub latest: BTreeMap<String, AdmissionDecision>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskBudget {
    pub available_notional: f64,
    pub available_loss: f64,
    pub available_slots: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityTuple {
    pub policy_version: String,
    pub net_edge_after_cost: f64,
    pub model_trust: u8,
    /// Negative expected execution cost: larger is better.
    pub execution_quality: f64,
    pub risk_consumption: f64,
    pub episode_opened_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdmissionDecision {
    pub admission_id: String,
    pub episode_id: String,
    pub symbol: String,
    pub evaluated_at: u64,
    pub evaluation_ts: u64,
    pub decision: String,
    pub reason_codes: Vec<String>,
    pub priority_tuple: PriorityTuple,
    pub competing_episode_ids: Vec<String>,
    pub risk_budget_before: RiskBudget,
    pub risk_budget_after_if_admitted: RiskBudget,
    pub config_version: u64,
    pub context: String,
    pub qualification: Qualification,
    pub approved_notional: f64,
}
struct Candidate {
    decision: AdmissionDecision,
    requested: f64,
    risk_per_notional: f64,
}
fn trust_rank(trust: &str) -> u8 {
    match trust {
        "HIGH" => 3,
        "NORMAL" => 2,
        "DEGRADED" => 1,
        _ => 0,
    }
}
fn compare(a: &AdmissionDecision, b: &AdmissionDecision) -> Ordering {
    let x = &a.priority_tuple;
    let y = &b.priority_tuple;
    y.net_edge_after_cost
        .total_cmp(&x.net_edge_after_cost)
        .then(y.model_trust.cmp(&x.model_trust))
        .then(y.execution_quality.total_cmp(&x.execution_quality))
        .then(x.risk_consumption.total_cmp(&y.risk_consumption))
        .then(x.episode_opened_at.cmp(&y.episode_opened_at))
        .then(a.episode_id.cmp(&b.episode_id))
}
impl Engine {
    /// Exclude the one queued order being rechecked; all other reservations remain charged.
    pub fn admission_decisions(
        &self,
        live: bool,
        exclude_order: Option<&str>,
    ) -> Vec<AdmissionDecision> {
        let c = &self.config;
        let orders: Vec<_> = self
            .live
            .orders
            .values()
            .filter(|o| Some(o.client_id.as_str()) != exclude_order)
            .collect();
        let account = self.live.account.as_ref();
        let equity = if live {
            account.map_or(0., |a| a.equity)
        } else {
            self.account_equity()
        };
        let daily_pnl = if live {
            account.map_or(0., |a| a.daily_pnl)
        } else {
            self.daily_pnl
        };
        let mut occupied = BTreeSet::new();
        let (exposure, used_risk) = if live {
            let mut exposure = 0.;
            let mut risk = 0.;
            if let Some(a) = account {
                for (symbol, p) in &a.positions {
                    if p.quantity.abs() <= 1e-10 {
                        continue;
                    }
                    occupied.insert(symbol.clone());
                    exposure += p.quantity.abs() * p.mark_price;
                    let stop = orders
                        .iter()
                        .filter(|o| {
                            o.action == "ENTRY" && o.plan.symbol == *symbol && o.filled_qty > 0.
                        })
                        .max_by_key(|o| o.created_ts)
                        .map(|o| o.plan.sl);
                    risk += stop.map_or(p.quantity.abs() * p.mark_price, |stop| {
                        p.quantity.abs() * ((p.mark_price - stop) * p.quantity.signum()).max(0.)
                            + p.quantity.abs()
                                * p.mark_price
                                * (c.fee_bps * 2. / 10000. + c.max_slippage * 2.)
                    });
                }
            }
            for o in &orders {
                if o.pending() && !o.reduce_only() {
                    occupied.insert(o.plan.symbol.clone());
                    exposure += o.quantity * o.limit_price;
                    risk += o.plan.planned_max_loss;
                }
            }
            (exposure, risk)
        } else {
            for symbol in self.positions.keys() {
                occupied.insert(symbol.clone());
            }
            (
                self.positions
                    .values()
                    .map(|p| p.qty * p.current_price)
                    .sum(),
                self.positions
                    .values()
                    .map(|p| p.plan.planned_max_loss)
                    .sum(),
            )
        };
        let cap = if live {
            self.live.cap.min(c.max_position_notional)
        } else {
            c.max_position_notional
        };
        let available = if live {
            account.map_or(0., |a| a.available * 0.95)
        } else {
            equity
        };
        let mut budget = RiskBudget {
            available_notional: (cap - exposure)
                .min(equity * c.max_effective_leverage - exposure)
                .min(available)
                .max(0.),
            available_loss: (c.max_daily_loss + daily_pnl - used_risk).max(0.),
            available_slots: c
                .auto_max_concurrent_positions
                .saturating_sub(occupied.len()),
        };
        let entry_times: Vec<u64> = if live {
            orders
                .iter()
                .filter(|o| o.action == "ENTRY")
                .map(|o| o.created_ts)
                .collect()
        } else {
            self.trades
                .iter()
                .map(|t| t.position.ts)
                .chain(self.positions.values().map(|p| p.ts))
                .collect()
        };
        let mut daily_count = entry_times
            .iter()
            .filter(|t| **t / 86_400_000 == self.now / 86_400_000)
            .count();
        let mut window_count = entry_times
            .iter()
            .filter(|t| self.now.saturating_sub(**t) < c.auto_entry_window_seconds * 1000)
            .count();
        let mut candidates = vec![];
        for (symbol, s) in &self.symbols {
            let Some(p) = &s.active else {
                continue;
            };
            let ev = s.evaluation.as_ref();
            let q = ev
                .and_then(|v| v.qualification.as_ref())
                .cloned()
                .or_else(|| s.qualification.clone())
                .unwrap_or_else(|| p.qualification.clone());
            let mut reasons = vec![];
            if q.status != "QUALIFIED" {
                reasons.push("NOT_QUALIFIED".into());
            }
            if p.config_version != self.config_version || self.now >= p.ts + p.horizon * 1000 {
                reasons.push("EPISODE_EXPIRED".into());
            }
            let max_age = c.max_stale_ms.min(
                c.alpha_eval_interval_ms
                    .max(c.qualification_eval_interval_ms)
                    * 2,
            );
            if ev.is_none_or(|v| {
                self.now.saturating_sub(v.evaluated_at) > max_age
                    || self.now.saturating_sub(v.qualification_evaluated_at) > max_age
            }) {
                reasons.push("STALE_EVALUATION".into());
            }
            if !q.net_edge.is_finite() || q.net_edge < c.auto_min_net_edge {
                reasons.push("AUTO_EDGE_THRESHOLD_NOT_MET".into());
            }
            if trust_rank(&s.health.trust) < trust_rank(&c.auto_required_model_trust)
                || ev.is_none_or(|v| v.calibrated_probabilities.is_none())
            {
                reasons.push("MODEL_TRUST_INSUFFICIENT".into());
            }
            if s.features.as_ref().is_none_or(|f| {
                !f.data_quality.is_empty()
                    || self.now.saturating_sub(f.ts) > c.max_stale_ms
                    || f.spread_bps > c.max_spread_bps
                    || f.depth < c.min_depth
            }) || s
                .venues
                .values()
                .any(|v| !v.connected || !v.book.ready(self.now, c.max_stale_ms))
                || q.gates.iter().any(|g| {
                    ["slippage", "liquidity", "exchange", "cost"].contains(&g.code.as_str())
                        && !g.pass
                })
                || s.health.execution_drift.is_some_and(|v| v > c.max_slippage)
                || (live && (!self.live.healthy || self.live_ready().is_err()))
            {
                reasons.push("EXECUTION_QUALITY_FAIL".into());
            }
            if self.kill_switch || !self.storage_healthy {
                reasons.push("RISK_BUDGET_UNAVAILABLE".into());
            }
            if occupied.contains(symbol) {
                reasons.push("POSITION_ALREADY_ACTIVE".into());
            }
            if live && (!self.live.auto_allowed || !self.auto_evidence()) {
                reasons.push("AUTO_LIVE_EVIDENCE_REQUIRED".into());
            }
            if live
                && (self.live.skipped.contains(&p.prediction_id)
                    || orders
                        .iter()
                        .any(|o| o.action == "ENTRY" && o.plan.prediction_id == p.prediction_id))
            {
                reasons.push("EPISODE_ALREADY_CONSUMED".into());
            }
            let last_exit = if live {
                self.live
                    .trades
                    .iter()
                    .filter(|t| &t.symbol == symbol)
                    .map(|t| t.closed_ts)
                    .max()
            } else {
                self.trades
                    .iter()
                    .filter(|t| &t.position.plan.symbol == symbol)
                    .map(|t| t.exit_ts)
                    .max()
            };
            if let Some(exit) = last_exit {
                if self.now.saturating_sub(exit) < c.reentry_cooldown_seconds * 1000 {
                    reasons.push("REENTRY_COOLDOWN".into());
                }
                if p.episode.signal_epoch <= exit {
                    reasons.push("FRESH_ALPHA_REQUIRED".into());
                }
                if q.net_edge < q.costs.total.max(c.incremental_ev) {
                    reasons.push("REENTRY_COST_NOT_COVERED".into());
                }
            }
            let plan = ev
                .and_then(|v| v.target_position.as_ref())
                .or(s.plan.as_ref());
            let requested = plan.map_or(0., |v| v.position_notional.min(cap));
            let risk_per_notional = plan.map_or(1., |v| {
                let price = s.features.as_ref().map_or(p.current_price, |f| f.mid)
                    * (1. + p.sign() * c.max_slippage);
                ((price - v.sl) * p.sign() / price).max(0.)
                    + q.costs.total
                    + (2. * c.max_slippage - q.costs.slippage).max(0.)
            });
            candidates.push(Candidate {
                decision: AdmissionDecision {
                    admission_id: String::new(),
                    episode_id: p.prediction_id.clone(),
                    symbol: symbol.clone(),
                    evaluated_at: self.now,
                    evaluation_ts: ev.map_or(0, |v| v.evaluated_at),
                    decision: "SKIPPED".into(),
                    reason_codes: reasons,
                    priority_tuple: PriorityTuple {
                        policy_version: c.admission_priority_policy_version.clone(),
                        net_edge_after_cost: q.net_edge,
                        model_trust: trust_rank(&s.health.trust),
                        execution_quality: -(q.costs.spread + q.costs.slippage + q.costs.impact),
                        risk_consumption: requested * risk_per_notional,
                        episode_opened_at: p.ts,
                    },
                    competing_episode_ids: vec![],
                    risk_budget_before: Default::default(),
                    risk_budget_after_if_admitted: Default::default(),
                    config_version: self.config_version,
                    context: if live { "LIVE" } else { "SHADOW" }.into(),
                    qualification: q,
                    approved_notional: 0.,
                },
                requested,
                risk_per_notional,
            });
        }
        candidates.sort_by(|a, b| compare(&a.decision, &b.decision));
        let competitors: Vec<_> = candidates
            .iter()
            .filter(|c| c.decision.reason_codes.is_empty())
            .map(|c| c.decision.episode_id.clone())
            .collect();
        let mut allocated = false;
        candidates
            .into_iter()
            .map(|candidate| {
                let mut d = candidate.decision;
                d.competing_episode_ids = competitors
                    .iter()
                    .filter(|id| **id != d.episode_id)
                    .cloned()
                    .collect();
                d.risk_budget_before = budget.clone();
                if daily_count >= c.max_daily_trades {
                    d.reason_codes.push("DAILY_TRADE_CAP".into());
                }
                if window_count >= c.max_auto_entries_per_window {
                    d.reason_codes.push("ORDER_WINDOW_CAP".into());
                }
                let notional = candidate.requested.min(budget.available_notional).min(
                    budget.available_loss.min(equity * c.risk_per_trade)
                        / candidate.risk_per_notional.max(1e-10),
                );
                if budget.available_slots == 0
                    || notional < 10.
                    || notional * d.qualification.net_edge < c.min_net_pnl
                {
                    d.reason_codes.push(
                        if allocated {
                            "BETTER_OPPORTUNITY_ACTIVE"
                        } else {
                            "RISK_BUDGET_UNAVAILABLE"
                        }
                        .into(),
                    );
                }
                if d.reason_codes.is_empty() {
                    d.decision = "ADMITTED".into();
                    d.approved_notional = notional;
                    budget.available_notional = (budget.available_notional - notional).max(0.);
                    budget.available_loss =
                        (budget.available_loss - notional * candidate.risk_per_notional).max(0.);
                    budget.available_slots -= 1;
                    daily_count += 1;
                    window_count += 1;
                    allocated = true;
                }
                d.risk_budget_after_if_admitted = if d.decision == "ADMITTED" {
                    budget.clone()
                } else {
                    d.risk_budget_before.clone()
                };
                d
            })
            .collect()
    }
    pub fn record_admission(&mut self, mut d: AdmissionDecision, force: bool) -> AdmissionDecision {
        let changed = self.admission.latest.get(&d.symbol).is_none_or(|old| {
            old.episode_id != d.episode_id
                || old.decision != d.decision
                || old.reason_codes != d.reason_codes
                || old.context != d.context
                || old.config_version != d.config_version
                || old.competing_episode_ids != d.competing_episode_ids
        });
        if force || changed {
            self.admission.counter += 1;
            d.admission_id = format!("admission-{}-{}", self.now, self.admission.counter);
            self.log("auto_admission", json!(&d));
        } else if let Some(old) = self.admission.latest.get(&d.symbol) {
            d.admission_id = old.admission_id.clone();
        }
        self.admission.latest.insert(d.symbol.clone(), d.clone());
        d
    }
    pub fn admission_shadow_tick(&mut self) {
        for d in self.admission_decisions(false, None) {
            self.record_admission(d, false);
        }
    }
    pub fn require_admission(
        &mut self,
        symbol: &str,
        exclude_order: Option<&str>,
    ) -> Result<AdmissionDecision, String> {
        let d = self
            .admission_decisions(true, exclude_order)
            .into_iter()
            .find(|d| d.symbol == symbol)
            .ok_or("EPISODE_EXPIRED")?;
        let d = self.record_admission(d, true);
        if d.decision != "ADMITTED" {
            return Err(d.reason_codes.join(","));
        }
        Ok(d)
    }
}
