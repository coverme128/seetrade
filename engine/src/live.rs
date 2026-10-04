//! Durable execution intents. Network code only consumes intents after the journal commits.
use crate::{core::Engine, types::*};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountSnapshot {
    pub ts: u64,
    pub equity: f64,
    pub available: f64,
    pub daily_pnl: f64,
    pub one_way: bool,
    pub positions: BTreeMap<String, AccountPosition>,
    pub open_order_ids: Vec<String>,
    pub funding: Vec<FundingSettlement>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FundingSettlement {
    pub symbol: String,
    pub ts: u64,
    pub income: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveFeedback {
    pub model_version: String,
    pub config_version: u64,
    pub expected_cost: f64,
    pub realized_cost: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveTrade {
    #[serde(default)]
    pub feedback: Option<LiveFeedback>,
    pub parent_id: String,
    pub prediction_id: String,
    pub symbol: String,
    pub closed_ts: u64,
    pub entry_notional: f64,
    pub exit_notional: f64,
    pub fees: f64,
    pub funding_income: f64,
    pub net_pnl: f64,
    pub expected_net_pnl: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountPosition {
    pub quantity: f64, // signed base asset quantity, never contracts
    pub entry_price: f64,
    pub mark_price: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
}
/// Frozen when risk approves an intent; never reconstructed for legacy orders.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionReference {
    pub model_version: String,
    pub config_version: u64,
    pub mid: f64,
    pub expected_round_trip_cost: f64,
    pub expected_net_edge: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    #[serde(default)]
    pub feedback: Option<ExecutionReference>,
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub admission: Option<crate::admission::AdmissionDecision>,
    pub client_id: String,
    pub parent_id: Option<String>,
    pub action: String,
    pub plan: TradePlan,
    pub qualification: Qualification,
    pub created_ts: u64,
    pub deadline_ts: u64,
    pub quantity: f64,
    pub limit_price: f64,
    pub status: String,
    pub exchange_order_id: Option<String>,
    pub filled_qty: f64,
    pub average_price: f64,
    pub fee: f64,
    pub send_ts: u64,
    pub ack_ts: u64,
    pub fill_ts: u64,
    pub reason: String,
    pub alpha_id: Option<String>,
}
impl Intent {
    pub fn pending(&self) -> bool {
        ["QUEUED", "SUBMITTING", "UNKNOWN", "NEW", "PARTIALLY_FILLED"]
            .contains(&self.status.as_str())
    }
    pub fn reduce_only(&self) -> bool {
        ["EXIT", "REDUCE"].contains(&self.action.as_str())
    }
    pub fn buy(&self) -> bool {
        (self.plan.direction == "LONG") != self.reduce_only()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveState {
    pub configured: bool,
    pub account_id: String,
    pub venue: String,
    pub environment: String,
    pub cap: f64,
    pub auto_allowed: bool,
    pub account: Option<AccountSnapshot>,
    pub peak_equity: f64,
    pub healthy: bool,
    pub error: String,
    pub orders: BTreeMap<String, Intent>,
    pub skipped: Vec<String>,
    pub counter: u64,
    pub trades: Vec<LiveTrade>,
    pub protection: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiveHealth {
    pub samples: usize,
    pub excluded_samples: usize,
    pub execution_drift: Option<f64>,
    pub ev_drift: Option<f64>,
    pub status: String,
}
impl Engine {
    pub fn live_feedback(&self, symbol: &str) -> LiveHealth {
        let rows: Vec<_> = self
            .live
            .trades
            .iter()
            .filter(|t| t.symbol == symbol)
            .collect();
        let mut comparable: Vec<_> = rows
            .iter()
            .copied()
            .filter(|t| {
                t.entry_notional > 0.
                    && t.feedback.as_ref().is_some_and(|f| {
                        f.model_version == self.asset_model(symbol).model_version
                            && f.config_version == self.asset_config_version(symbol)
                    })
            })
            .collect();
        let excluded = rows.len() - comparable.len();
        comparable.sort_by_key(|t| t.closed_ts);
        let recent: Vec<_> = comparable.iter().rev().take(50).collect();
        let basis = recent.iter().map(|t| t.entry_notional).sum::<f64>();
        let execution = (basis > 0.).then(|| {
            recent
                .iter()
                .map(|t| {
                    let f = t.feedback.as_ref().unwrap();
                    f.realized_cost - f.expected_cost
                })
                .sum::<f64>()
                / basis
        });
        let ev = (basis > 0.).then(|| {
            recent
                .iter()
                .map(|t| t.net_pnl - t.expected_net_pnl)
                .sum::<f64>()
                / basis
        });
        let status = if execution.is_some_and(|x| x > self.asset_config(symbol).max_slippage)
            || (recent.len() >= self.asset_config(symbol).min_samples
                && ev.is_some_and(|x| x < -self.asset_config(symbol).ev_drift_limit))
        {
            "DEGRADED"
        } else if recent.len() < self.asset_config(symbol).min_samples {
            "INSUFFICIENT"
        } else {
            "NORMAL"
        };
        LiveHealth {
            samples: recent.len(),
            excluded_samples: excluded,
            execution_drift: execution,
            ev_drift: ev,
            status: status.into(),
        }
    }
    pub fn apply_live_feedback(&self, symbol: &str, health: &mut ModelHealth) {
        health.live = self.live_feedback(symbol);
        if health.live.status == "DEGRADED" {
            if health.trust != "LOW" {
                health.trust = "DEGRADED".into();
            }
            health
                .reasons
                .push("实盘成交成本或净收益偏移，降低仓位".into());
            health.retrain_recommended |=
                health.live.samples >= self.asset_config(symbol).min_samples;
        }
    }
    pub fn live_positions_view(&self) -> Value {
        let mut positions = serde_json::Map::new();
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            let symbol = symbol.as_str();
            let account = self.live.account.as_ref();
            let position = account.and_then(|a| a.positions.get(symbol));
            let tracked = self.tracked_live_quantity(symbol);
            let actual = position.map_or(0., |p| p.quantity);
            let mid = self.symbols[symbol]
                .features
                .as_ref()
                .map(|f| f.mid)
                .or_else(|| position.map(|p| p.mark_price))
                .unwrap_or(0.);
            let equity = account.map_or(0., |a| a.equity);
            let pending: Vec<_> = self
                .live
                .orders
                .values()
                .filter(|o| o.plan.symbol == symbol && o.pending())
                .collect();
            let projected = pending.iter().fold(tracked, |qty, o| {
                qty + (o.quantity - o.filled_qty).max(0.) * if o.buy() { 1. } else { -1. }
            });
            let protection = self.live.protection.get(symbol);
            let current = tracked.abs() * mid;
            // A target is the signed exposure after already-approved pending orders, not an unapproved model wish.
            let target = if protection.is_some() {
                0.
            } else {
                projected.abs() * mid
            };
            let action = if protection.is_some() {
                "EXIT"
            } else if pending.iter().any(|o| o.reduce_only()) {
                "REDUCE"
            } else if pending.iter().any(|o| o.action == "ADD") {
                "ADD"
            } else if pending.iter().any(|o| o.action == "ENTRY") {
                "ENTRY"
            } else if tracked.abs() > 1e-10 {
                "HOLD"
            } else {
                "FLAT"
            };
            let latest = self
                .live
                .orders
                .values()
                .filter(|o| o.plan.symbol == symbol)
                .max_by_key(|o| o.created_ts);
            let parent = self
                .live
                .orders
                .values()
                .filter(|o| o.plan.symbol == symbol && o.action == "ENTRY" && o.filled_qty > 0.)
                .max_by_key(|o| o.created_ts);
            positions.insert(symbol.into(), json!({"quantity":tracked,"account_quantity":actual,
                "account_age_ms":account.map(|a|self.now.abs_diff(a.ts)),"reconciled":(actual-tracked).abs()<1e-8,
                "current_notional":current,"target_notional":target,
                "effective_leverage":(equity>0.).then_some(current/equity),"target_leverage":(equity>0.).then_some(target/equity),
                "entry_price":position.map(|p|p.entry_price),"mark_price":position.map(|p|p.mark_price),
                "unrealized_pnl":position.map(|p|p.unrealized_pnl),
                "liquidation_buffer":position.filter(|p|p.liquidation_price>0. && p.mark_price>0.).map(|p|(p.mark_price-p.liquidation_price).abs()/p.mark_price),
                "sl":parent.map(|p|p.plan.sl),"tp":parent.map(|p|p.plan.tp),"deadline_ts":parent.map(|p|p.deadline_ts),
                "action":action,"reason":protection.cloned().unwrap_or_else(||latest.map(|o|o.reason.clone()).unwrap_or_default()),
                "pending":pending.iter().map(|o|json!({"action":o.action,"status":o.status})).collect::<Vec<_>>(),
                "feedback":self.live_feedback(symbol)}));
        }
        Value::Object(positions)
    }

    pub fn execution_metrics(&self) -> Value {
        let orders: Vec<_> = self.live.orders.values().collect();
        let rejected = orders.iter().filter(|o| o.status == "REJECTED").count();
        let p95 = |mut values: Vec<u64>| -> Option<u64> {
            values.sort_unstable();
            values
                .get(values.len().saturating_sub(1) * 95 / 100)
                .copied()
        };
        json!({"orders":orders.len(),"rejected":rejected,"rejection_rate":if orders.is_empty(){None}else{Some(rejected as f64/orders.len() as f64)},
            "unknown":orders.iter().filter(|o|o.status=="UNKNOWN").count(),
            "ack_p95_ms":p95(orders.iter().filter(|o|o.ack_ts>0&&o.send_ts>0).map(|o|o.ack_ts.saturating_sub(o.send_ts)).collect()),
            "fill_p95_ms":p95(orders.iter().filter(|o|o.filled_qty>0.&&o.fill_ts>0&&o.send_ts>0).map(|o|o.fill_ts.saturating_sub(o.send_ts)).collect())})
    }
    pub fn tracked_live_quantity(&self, symbol: &str) -> f64 {
        self.live
            .orders
            .values()
            .filter(|o| o.plan.symbol == symbol)
            .map(|o| o.filled_qty * if o.buy() { 1. } else { -1. })
            .sum()
    }
    pub fn close_live_trades(&mut self) {
        let Some(a) = self.live.account.as_ref() else {
            return;
        };
        let mut results = vec![];
        for o in self
            .live
            .orders
            .values()
            .filter(|o| o.action == "ENTRY" && o.filled_qty > 0.)
        {
            let children: Vec<_> = self
                .live
                .orders
                .values()
                .filter(|x| x.parent_id.as_ref() == Some(&o.client_id))
                .collect();
            if o.pending() || children.iter().any(|x| x.pending()) {
                continue;
            }
            let mut entries = o.filled_qty;
            let mut exits = 0.;
            let mut basis = o.filled_qty * o.average_price;
            let mut proceeds = 0.;
            let mut fees = o.fee;
            for x in &children {
                fees += x.fee;
                if x.reduce_only() {
                    exits += x.filled_qty;
                    proceeds += x.filled_qty * x.average_price;
                } else {
                    entries += x.filled_qty;
                    basis += x.filled_qty * x.average_price;
                }
            }
            if (entries - exits).abs() > 1e-8 {
                continue;
            }
            let close = children.iter().map(|x| x.fill_ts).max().unwrap_or(self.now);
            let funding = a
                .funding
                .iter()
                .filter(|f| f.symbol == o.plan.symbol && f.ts >= o.created_ts && f.ts <= close)
                .map(|f| f.income)
                .sum::<f64>();
            let sign = if o.plan.direction == "LONG" { 1. } else { -1. };
            let legs: Vec<_> = std::iter::once(o)
                .chain(children.iter().copied())
                .filter(|x| x.filled_qty > 0.)
                .collect();
            let comparable = o.feedback.as_ref().filter(|reference| {
                legs.iter().all(|x| {
                    x.feedback.as_ref().is_some_and(|r| {
                        r.model_version == reference.model_version
                            && r.config_version == reference.config_version
                    })
                })
            });
            let feedback = comparable.map(|reference| LiveFeedback {
                model_version: reference.model_version.clone(),
                config_version: reference.config_version,
                expected_cost: legs
                    .iter()
                    .filter(|x| !x.reduce_only())
                    .map(|x| {
                        let r = x.feedback.as_ref().unwrap();
                        x.filled_qty * r.mid * r.expected_round_trip_cost
                    })
                    .sum(),
                realized_cost: fees - funding
                    + legs
                        .iter()
                        .map(|x| {
                            x.filled_qty
                                * (x.average_price - x.feedback.as_ref().unwrap().mid)
                                * if x.buy() { 1. } else { -1. }
                        })
                        .sum::<f64>(),
            });
            let expected_net_pnl = legs
                .iter()
                .filter(|x| !x.reduce_only())
                .map(|x| {
                    x.filled_qty
                        * x.feedback.as_ref().map_or(x.average_price, |r| r.mid)
                        * x.feedback
                            .as_ref()
                            .map_or(x.qualification.net_edge, |r| r.expected_net_edge)
                })
                .sum();
            results.push(LiveTrade {
                feedback,
                parent_id: o.client_id.clone(),
                prediction_id: o.plan.prediction_id.clone(),
                symbol: o.plan.symbol.clone(),
                closed_ts: close,
                entry_notional: basis,
                exit_notional: proceeds,
                fees,
                funding_income: funding,
                net_pnl: (proceeds - basis) * sign - fees + funding,
                expected_net_pnl,
            });
        }
        for r in results {
            if let Some(i) = self
                .live
                .trades
                .iter()
                .position(|t| t.parent_id == r.parent_id)
            {
                if (self.live.trades[i].funding_income - r.funding_income).abs() > 1e-12 {
                    self.log(
                        "live_trade_adjustment",
                        json!({"reason":"funding_reconciled","trade":r}),
                    );
                    self.live.trades[i] = r;
                }
            } else {
                self.integrity.live_trades += 1;
                self.daily_integrity.live_trades += 1;
                self.log("live_trade", json!(r));
                self.live.trades.push(r);
            }
        }
    }
    pub fn live_ready(&self) -> Result<(), String> {
        let l = &self.live;
        if self.source != "live" || !l.configured || !l.healthy {
            return Err(format!("执行通道未就绪：{}", l.error));
        }
        if !l.protection.is_empty() {
            return Err("保护性退出尚未完成，禁止新增风险".into());
        }
        if self.kill_switch || !self.storage_healthy {
            return Err("紧急停止或存储不可用".into());
        }
        if !self.positions.is_empty() {
            return Err("请先关闭模拟持仓".into());
        }
        let a = l.account.as_ref().ok_or("缺少账户对账")?;
        if self.now.abs_diff(a.ts) > 10_000 || !a.one_way {
            return Err("账户快照过期或不是单向持仓模式".into());
        }
        if a.equity <= 0.
            || a.available <= 0.
            || a.daily_pnl <= -self.config.max_daily_loss
            || (l.peak_equity - a.equity) / l.peak_equity.max(1.) >= self.config.max_drawdown
        {
            return Err("真实账户风险预算耗尽".into());
        }
        if a.open_order_ids.iter().any(|id| !l.orders.contains_key(id)) {
            return Err("账户存在外部挂单，请使用独立账户".into());
        }
        for (s, p) in &a.positions {
            if p.quantity.abs() > 1e-10
                && !l
                    .orders
                    .values()
                    .any(|o| o.plan.symbol == *s && !o.reduce_only() && o.filled_qty > 0.)
            {
                return Err("账户存在未管理持仓".into());
            }
            if p.quantity.abs() > 1e-10
                && p.liquidation_price > 0.
                && (p.mark_price - p.liquidation_price).abs() / p.mark_price
                    < self.asset_config(s).min_liquidation_buffer
            {
                return Err("真实强平距离低于阈值".into());
            }
        }
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            let symbol = symbol.as_str();
            let actual = a.positions.get(symbol).map(|p| p.quantity).unwrap_or(0.);
            if (actual - self.tracked_live_quantity(symbol)).abs() > 1e-8 {
                return Err("账户持仓与成交账本不一致，暂停新增风险".into());
            }
        }
        if l.orders
            .values()
            .any(|o| ["SUBMITTING", "UNKNOWN"].contains(&o.status.as_str()))
        {
            return Err("订单状态待核实，禁止新增风险".into());
        }
        if l.orders
            .values()
            .rev()
            .filter(|o| o.filled_qty > 0. && !o.reduce_only())
            .take(20)
            .any(|o| {
                (o.average_price - o.limit_price) * if o.buy() { 1. } else { -1. } > 1e-6
                    || o.fee / (o.filled_qty * o.average_price).max(1.)
                        > self.asset_execution_config(&o.plan.symbol).fee_bps / 10_000.
                            + self.asset_config(&o.plan.symbol).max_slippage
            })
        {
            return Err("实际执行成本超出预算，需复核费率或滑点".into());
        }
        Ok(())
    }
    pub fn auto_evidence(&self) -> bool {
        let rows: Vec<_> = self
            .trades
            .iter()
            .filter(|t| {
                t.position.config == self.asset_execution_config(&t.position.plan.symbol)
                    && self.outcomes.iter().any(|o| {
                        o.prediction.prediction_id == t.position.plan.prediction_id
                            && o.prediction.model_version
                                == self.asset_model(&o.prediction.symbol).model_version
                    })
            })
            .collect();
        let n = rows.len();
        if n < self.config.high_trust_samples.max(90) {
            return false;
        }
        let mut equity = self.config.equity;
        let mut peak = equity;
        for t in &rows {
            equity += t.net_pnl;
            peak = peak.max(equity);
            if (peak - equity) / peak.max(1.) >= self.config.max_drawdown {
                return false;
            }
        }
        (0..3).all(|i| {
            rows[i * n / 3..(i + 1) * n / 3]
                .iter()
                .map(|t| t.net_pnl)
                .sum::<f64>()
                > 0.
        }) && rows.iter().all(|t| {
            t.realized_net_edge >= -t.position.config.stop * t.position.config.max_mae_stop_multiple
                && (t.realized_cost - t.expected_cost) / t.position.cost_basis.max(1.)
                    <= t.position.config.max_slippage
        }) && self
            .symbols
            .iter()
            .filter(|(symbol, _)| self.asset_mode_allowed(symbol, "AUTO"))
            .all(|(symbol, s)| {
                ["NORMAL", "HIGH"].contains(&s.health.trust.as_str())
                    && s.health.calibration_error <= self.asset_config(symbol).calibration_warning
            })
    }
    pub fn live_execute(&mut self, symbol: &str, prediction_id: &str) -> Result<(), String> {
        if !["CONFIRM", "AUTO"].contains(&self.mode.as_str()) {
            return Err("请先启用 CONFIRM 或 AUTO".into());
        }
        self.live_ready()?;
        if self.live.account.as_ref().is_some_and(|a| {
            a.positions
                .get(symbol)
                .is_some_and(|p| p.quantity.abs() > 1e-10)
        }) {
            return Err("已有真实持仓，禁止重复开仓".into());
        }
        let s = self.symbols.get(symbol).ok_or("未知标的")?;
        let p = s.active.as_ref().ok_or("没有当前预测")?;
        if p.prediction_id != prediction_id
            || p.config_version != self.asset_config_version(symbol)
            || self.now >= p.ts + p.horizon * 1000
            || self.live.skipped.iter().any(|id| id == prediction_id)
            || self
                .live
                .orders
                .values()
                .any(|o| o.plan.prediction_id == prediction_id && o.action == "ENTRY")
        {
            return Err("预测已过期、跳过或执行过".into());
        }
        let (q, plan) = self.refresh_execution_evaluation(symbol)?;
        let plan = plan.ok_or_else(|| format!("资格拒绝：{}", q.reasons.join(",")))?;
        if self.mode == "AUTO" {
            self.require_admission(symbol, None)?;
        }
        let result = self.queue_live(plan, q, "ENTRY", None, None);
        if let Err(reason) = &result {
            self.log(
                "live_risk_decision",
                json!({"status":"FAIL","prediction_id":prediction_id,"reason":reason}),
            );
            if self.mode == "AUTO" {
                if let Some(mut d) = self.admission.latest.get(symbol).cloned() {
                    d.decision = "SKIPPED".into();
                    d.reason_codes = vec!["RISK_BUDGET_UNAVAILABLE".into()];
                    d.risk_budget_after_if_admitted = d.risk_budget_before.clone();
                    self.record_admission(d, true);
                }
            }
        }
        result
    }
    fn refresh_execution_evaluation(
        &mut self,
        symbol: &str,
    ) -> Result<(Qualification, Option<TradePlan>), String> {
        let s = self.symbols.get(symbol).ok_or("未知标的")?;
        let mut p = s.active.clone().ok_or("EPISODE_EXPIRED")?;
        let f = s.features.clone().ok_or("缺少特征")?;
        if self.now.saturating_sub(f.ts) > self.asset_config(symbol).max_stale_ms
            || !f.data_quality.is_empty()
            || p.config_version != self.asset_config_version(symbol)
            || self.now >= p.ts + p.horizon * 1000
        {
            return Err("STALE_EVALUATION".into());
        }
        let (raw, cal) = self
            .asset_model(symbol)
            .probabilities(&f, self.asset_config(symbol));
        p.raw_probabilities = raw;
        p.calibrated_probabilities = cal;
        let (q, plan) = self.qualify(&p, &f, &s.health);
        if q.status == "QUALIFIED" && p.episode.highest_qualification != "QUALIFIED" {
            self.integrity.qualified_episodes += 1;
            self.daily_integrity.qualified_episodes += 1;
            self.log(
                "episode_qualification",
                json!({"episode_id":p.prediction_id,"highest_qualification":"QUALIFIED"}),
            );
        }
        let s = self.symbols.get_mut(symbol).unwrap();
        s.evaluation = Some(AlphaEvaluation {
            symbol: symbol.into(),
            evaluated_at: self.now,
            qualification_evaluated_at: self.now,
            raw_probabilities: raw,
            calibrated_probabilities: cal,
            features: f.clone(),
            qualification: Some(q.clone()),
            target_position: plan.clone(),
        });
        s.qualification = Some(q.clone());
        s.plan = plan.clone();
        if let Some(p) = s.active.as_mut() {
            if q.status == "QUALIFIED" {
                p.episode.highest_qualification = "QUALIFIED".into();
            }
            p.episode.latest_evaluated_at = self.now;
            p.episode.latest_raw_probabilities = Some(raw);
            p.episode.latest_calibrated_probabilities = cal;
            p.episode.latest_feature_snapshot_id = f.feature_snapshot_id;
        }
        self.integrity.realtime_evaluations += 1;
        self.daily_integrity.realtime_evaluations += 1;
        Ok((q, plan))
    }
    fn queue_live(
        &mut self,
        mut plan: TradePlan,
        q: Qualification,
        action: &str,
        parent: Option<String>,
        alpha: Option<String>,
    ) -> Result<(), String> {
        let config = self.asset_execution_config(&plan.symbol);
        let admission = if action == "ENTRY" && self.mode == "AUTO" {
            let d = self
                .admission
                .latest
                .get(&plan.symbol)
                .filter(|d| {
                    d.decision == "ADMITTED"
                        && d.context == "LIVE"
                        && d.episode_id == plan.prediction_id
                        && d.evaluated_at == self.now
                        && d.config_version == self.asset_config_version(&plan.symbol)
                })
                .cloned()
                .ok_or("AUTO_ADMISSION_REQUIRED")?;
            if q.status != "QUALIFIED" || q.net_edge < config.auto_min_net_edge {
                return Err("AUTO_EDGE_THRESHOLD_NOT_MET".into());
            }
            plan.position_notional = plan.position_notional.min(d.approved_notional);
            Some(d)
        } else {
            None
        };
        if !["EXIT", "REDUCE"].contains(&action) {
            self.apply_portfolio(&mut plan, true, None)?;
        }
        let approved_notional = plan.position_notional;
        let account = self.live.account.clone().unwrap_or_default();
        let a = &account;
        let reduce = ["EXIT", "REDUCE"].contains(&action);
        let signed = self.tracked_live_quantity(&plan.symbol);
        let current = signed.abs();
        if reduce && current > 0. && (signed > 0.) != (plan.direction == "LONG") {
            return Err("成交账本方向与平仓计划不一致".into());
        }
        let mid = self.symbols[&plan.symbol]
            .features
            .as_ref()
            .map(|f| f.mid)
            .or_else(|| {
                parent
                    .as_ref()
                    .and_then(|id| self.live.orders.get(id))
                    .map(|o| o.average_price)
            })
            .filter(|m| m.is_finite() && *m > 0.)
            .ok_or("缺少有效参考价格")?;
        let buy = (plan.direction == "LONG") != reduce;
        let price = mid
            * (1.
                + if buy {
                    config.max_slippage
                } else {
                    -config.max_slippage
                });
        let sign = if plan.direction == "LONG" { 1. } else { -1. };
        let stop_distance = ((price - plan.sl) * sign / price).max(0.);
        let risk_cost = q.costs.total + (2. * config.max_slippage - q.costs.slippage).max(0.);
        let quantity = if reduce {
            if current <= 0. {
                return Err("没有可减仓的真实持仓".into());
            }
            if action == "EXIT" {
                current
            } else {
                (plan.position_notional / mid).min(current)
            }
        } else {
            self.live_ready()?;
            if q.status != "QUALIFIED" {
                return Err("风险门槛未通过".into());
            }
            let other = a
                .positions
                .values()
                .map(|p| p.quantity.abs() * p.mark_price)
                .sum::<f64>()
                + self
                    .live
                    .orders
                    .values()
                    .filter(|o| o.pending() && !o.reduce_only())
                    .map(|o| o.quantity * o.limit_price)
                    .sum::<f64>();
            let existing_risk = a
                .positions
                .iter()
                .map(|(symbol, p)| {
                    let stop = self
                        .live
                        .orders
                        .values()
                        .filter(|o| {
                            o.action == "ENTRY" && o.plan.symbol == *symbol && o.filled_qty > 0.
                        })
                        .max_by_key(|o| o.created_ts)
                        .map(|o| o.plan.sl)
                        .unwrap_or(p.mark_price);
                    p.quantity.abs() * ((p.mark_price - stop) * p.quantity.signum()).max(0.)
                        + p.quantity.abs() * p.mark_price * q.costs.total
                })
                .sum::<f64>()
                + self
                    .live
                    .orders
                    .values()
                    .filter(|o| o.pending() && !o.reduce_only())
                    .map(|o| o.plan.planned_max_loss)
                    .sum::<f64>();
            let cap = (self.live.cap - other)
                .min(config.max_position_notional - other)
                .min(a.equity * config.max_effective_leverage - other)
                .min(
                    (a.available * 0.95
                        - self
                            .live
                            .orders
                            .values()
                            .filter(|o| o.pending() && !o.reduce_only())
                            .map(|o| o.quantity * o.limit_price)
                            .sum::<f64>())
                    .max(0.),
                )
                .min(
                    (a.equity * config.risk_per_trade)
                        .min((config.max_daily_loss + a.daily_pnl - existing_risk).max(0.))
                        / (stop_distance + risk_cost),
                );
            plan.position_notional = plan.position_notional.min(cap);
            if plan.position_notional < 10.
                || plan.position_notional * q.net_edge < config.min_net_pnl
            {
                return Err("小额实盘上限下净收益或名义金额不足".into());
            }
            plan.position_notional / price.max(mid)
        };
        if self
            .live
            .orders
            .values()
            .any(|o| o.plan.symbol == plan.symbol && o.pending() && (!reduce || o.reduce_only()))
        {
            return Err("标的已有待处理订单".into());
        }
        if reduce
            && self.live.orders.values().any(|o| {
                o.plan.symbol == plan.symbol
                    && o.reduce_only()
                    && ["REJECTED", "CANCELED", "EXPIRED"].contains(&o.status.as_str())
                    && self.now.saturating_sub(o.created_ts) < 5000
            })
        {
            return Err("保护订单拒绝后等待重试".into());
        }
        if !reduce {
            plan.effective_leverage = plan.position_notional / a.equity;
            plan.expected_net_pnl = plan.position_notional * q.net_edge;
            plan.expected_loss *= plan.position_notional / approved_notional.max(1.);
            plan.planned_max_loss = plan.position_notional * (stop_distance + risk_cost);
        }
        self.live.counter += 1;
        let id = format!("ma{}{:x}", self.now, self.live.counter);
        plan.entry_type = if reduce {
            "REDUCE_ONLY_MARKET"
        } else {
            "LIMIT_IOC"
        }
        .into();
        let deadline = parent
            .as_ref()
            .and_then(|id| self.live.orders.get(id))
            .map(|o| o.deadline_ts)
            .unwrap_or_else(|| {
                self.symbols[&plan.symbol]
                    .active
                    .as_ref()
                    .map_or(self.now, |p| p.ts)
                    + plan.horizon * 1000
            });
        let feedback = Some(ExecutionReference {
            model_version: self.asset_model(&plan.symbol).model_version.clone(),
            config_version: self.asset_config_version(&plan.symbol),
            mid,
            expected_round_trip_cost: q.costs.total,
            expected_net_edge: q.net_edge,
        });
        let reason = self
            .live
            .protection
            .get(&plan.symbol)
            .cloned()
            .unwrap_or_else(|| action.into());
        let intent = Intent {
            feedback,
            execution_mode: self.mode.clone(),
            admission,
            client_id: id.clone(),
            parent_id: parent,
            action: action.into(),
            plan,
            qualification: q,
            created_ts: self.now,
            deadline_ts: deadline,
            quantity,
            limit_price: price,
            status: "QUEUED".into(),
            exchange_order_id: None,
            filled_qty: 0.,
            average_price: 0.,
            fee: 0.,
            send_ts: 0,
            ack_ts: 0,
            fill_ts: 0,
            reason,
            alpha_id: alpha,
        };
        self.log(
            "live_risk_decision",
            json!({"status":"PASS","account":a,"cap":self.live.cap,"intent":intent}),
        );
        self.live.orders.insert(id, intent);
        Ok(())
    }
    pub fn live_tick(&mut self) {
        if !self.live.configured {
            return;
        }
        let entries: Vec<_> = self
            .live
            .orders
            .values()
            .filter(|o| o.action == "ENTRY" && o.filled_qty > 0.)
            .cloned()
            .collect();
        for o in entries {
            let symbol = &o.plan.symbol;
            let qty = self.tracked_live_quantity(symbol).abs();
            if qty <= 1e-10 {
                if !self
                    .live
                    .orders
                    .values()
                    .any(|x| x.plan.symbol == *symbol && x.pending())
                {
                    self.live.protection.remove(symbol);
                }
                continue;
            }
            if self
                .live
                .orders
                .values()
                .any(|x| x.plan.symbol == *symbol && x.pending() && x.reduce_only())
            {
                continue;
            }
            // Only the newest parent for a symbol manages its current position.
            if self.live.orders.values().any(|x| {
                x.action == "ENTRY"
                    && x.plan.symbol == *symbol
                    && x.created_ts > o.created_ts
                    && x.filled_qty > 0.
            }) {
                continue;
            }
            let s = &self.symbols[symbol];
            let f = s.features.as_ref();
            let long = o.plan.direction == "LONG";
            let barrier = f.is_some_and(|f| {
                if long {
                    f.mid <= o.plan.sl || f.mid >= o.plan.tp
                } else {
                    f.mid >= o.plan.sl || f.mid <= o.plan.tp
                }
            });
            let q = s.qualification.clone().unwrap_or(o.qualification.clone());
            let reason = if self.kill_switch && self.config.kill_behavior == "flatten" {
                Some("紧急停止：平仓")
            } else if s
                .plan
                .as_ref()
                .is_some_and(|p| p.direction != o.plan.direction)
                || s.active
                    .as_ref()
                    .is_some_and(|p| p.direction != o.plan.direction)
            {
                Some("当前建议方向与持仓相反")
            } else if barrier {
                Some("止盈或止损触发")
            } else if self.now >= o.deadline_ts {
                Some("持仓期限结束")
            } else if !self.live.healthy
                || self
                    .live
                    .account
                    .as_ref()
                    .is_none_or(|a| self.now.abs_diff(a.ts) > 10000)
            {
                Some("账户通道异常或快照过期")
            } else if f.is_none_or(|f| {
                !f.data_quality.is_empty()
                    || self.now.saturating_sub(f.ts) > self.asset_config(symbol).max_stale_ms
            }) {
                Some("行情不可用或过期")
            } else if s.health.trust == "LOW"
                || s.qualification
                    .as_ref()
                    .is_none_or(|q| q.status != "QUALIFIED")
            {
                Some("模型或资格失效")
            } else {
                None
            };
            if let Some(reason) = reason {
                self.live
                    .protection
                    .entry(symbol.clone())
                    .or_insert_with(|| reason.into());
            }
            if self.live.protection.contains_key(symbol) {
                let _ = self.queue_live(o.plan.clone(), q, "EXIT", Some(o.client_id.clone()), None);
                continue;
            }
            if self.kill_switch
                || self
                    .live
                    .orders
                    .values()
                    .any(|x| x.plan.symbol == *symbol && x.pending())
            {
                continue;
            }
            let f = f.unwrap();
            let Some(plan) = s.plan.clone() else {
                continue;
            };
            let children: Vec<_> = self
                .live
                .orders
                .values()
                .filter(|x| x.parent_id.as_ref() == Some(&o.client_id) && x.filled_qty > 0.)
                .collect();
            let last = children
                .iter()
                .map(|x| x.created_ts)
                .max()
                .unwrap_or(o.created_ts);
            if self.now.saturating_sub(last)
                < self.asset_config(symbol).rebalance_interval_secs * 1000
            {
                continue;
            }
            let current = qty * f.mid;
            if plan.position_notional < current * (1. - self.asset_config(symbol).position_deadband)
            {
                let mut reduce_plan = o.plan.clone();
                reduce_plan.position_notional = current - plan.position_notional;
                let _ = self.queue_live(reduce_plan, q, "REDUCE", Some(o.client_id.clone()), None);
            } else if self.mode == "AUTO"
                && children.len() < (self.asset_config(symbol).max_rebalances as usize)
            {
                let alpha = format!("alpha-{symbol}-{}", s.candidate_since);
                let sign = if long { 1. } else { -1. };
                if s.candidate_since > o.created_ts
                    && self.now.saturating_sub(s.candidate_since)
                        >= self.asset_config(symbol).signal_confirm_ms
                    && s.candidate_direction == o.plan.direction
                    && (f.mid - o.average_price) * sign >= 0.
                    && q.net_edge >= self.asset_config(symbol).incremental_ev
                    && plan.position_notional
                        > current * (1. + self.asset_config(symbol).position_deadband)
                    && !self
                        .live
                        .orders
                        .values()
                        .any(|x| x.alpha_id.as_ref() == Some(&alpha))
                {
                    let mut add = o.plan.clone();
                    add.position_notional = plan.position_notional - current;
                    // Incremental quantity stays within the original stop loss budget.
                    let risk = (o.plan.planned_max_loss
                        - qty * ((o.average_price - o.plan.sl) * sign).max(0.)
                        - current * q.costs.total)
                        .max(0.);
                    add.position_notional = add
                        .position_notional
                        .min(risk / (((f.mid - o.plan.sl) * sign / f.mid).max(0.) + q.costs.total));
                    let _ = self.queue_live(add, q, "ADD", Some(o.client_id.clone()), Some(alpha));
                }
            }
        }
        if self.mode == "AUTO" {
            let decisions = self.admission_decisions(true, None);
            for d in decisions {
                let admitted = d.decision == "ADMITTED";
                let symbol = d.symbol.clone();
                let id = d.episode_id.clone();
                self.record_admission(d, false);
                if admitted {
                    let _ = self.live_execute(&symbol, &id);
                }
            }
        }
    }

    pub fn live_command(&mut self, command: &str, payload: Value) -> Result<(), String> {
        match command {
            "execute" => self.live_execute(
                payload["symbol"].as_str().ok_or("缺少 symbol")?,
                payload["prediction_id"]
                    .as_str()
                    .ok_or("缺少 prediction_id")?,
            ),
            "skip" => {
                let id = payload["prediction_id"]
                    .as_str()
                    .ok_or("缺少 prediction_id")?;
                self.live.skipped.push(id.into());
                if self.live.skipped.len() > 2000 {
                    self.live.skipped.remove(0);
                }
                Ok(())
            }
            "live_flatten" => {
                let symbol = payload["symbol"].as_str().ok_or("缺少 symbol")?;
                let o = self
                    .live
                    .orders
                    .values()
                    .filter(|o| o.plan.symbol == symbol && o.action == "ENTRY" && o.filled_qty > 0.)
                    .max_by_key(|o| o.created_ts)
                    .cloned()
                    .ok_or("没有管理中的真实持仓")?;
                self.live
                    .protection
                    .insert(symbol.into(), "手动平仓".into());
                self.queue_live(o.plan, o.qualification, "EXIT", Some(o.client_id), None)
            }
            "broker_config" => {
                self.live.configured = payload["configured"] == true;
                self.live.account_id = payload["account_id"].as_str().unwrap_or("").into();
                self.live.venue = payload["venue"].as_str().unwrap_or("").into();
                self.live.environment = payload["environment"].as_str().unwrap_or("").into();
                self.live.cap = payload["cap"].as_f64().unwrap_or(0.);
                self.live.auto_allowed = payload["auto_allowed"] == true;
                self.live.healthy = false;
                Ok(())
            }
            "broker_snapshot" => {
                let mut a: AccountSnapshot =
                    serde_json::from_value(payload).map_err(|e| e.to_string())?;
                if let Some(previous) = &self.live.account {
                    for f in &previous.funding {
                        if !a
                            .funding
                            .iter()
                            .any(|new| new.symbol == f.symbol && new.ts == f.ts)
                        {
                            a.funding.push(f.clone());
                        }
                    }
                }
                self.live.peak_equity = self.live.peak_equity.max(a.equity);
                self.live.account = Some(a);
                self.live.healthy = true;
                self.live.error.clear();
                self.close_live_trades();
                Ok(())
            }
            "broker_error" => {
                self.live.healthy = false;
                self.live.error = payload["error"].as_str().unwrap_or("执行通道异常").into();
                Ok(())
            }
            "broker_order" => {
                let mut o: Intent = serde_json::from_value(payload).map_err(|e| e.to_string())?;
                if !self.live.orders.contains_key(&o.client_id) {
                    return Err("未知订单".into());
                }
                if o.status == "SUBMITTING" && self.live.orders[&o.client_id].status != "QUEUED" {
                    return Err("订单不能重复提交".into());
                }
                if o.status == "SUBMITTING" && !o.reduce_only() {
                    let original = &self.live.orders[&o.client_id];
                    if original.execution_mode != self.mode {
                        return Err("执行授权已变化".into());
                    }
                    let automatic_entry = original.execution_mode == "AUTO" && o.action == "ENTRY";
                    if !automatic_entry {
                        self.live_ready()?;
                        let (q, target) = self.refresh_execution_evaluation(&o.plan.symbol)?;
                        let target = target.ok_or("发送前资格或风险预算失效")?;
                        if target.direction != o.plan.direction {
                            return Err("发送前建议方向已变化".into());
                        }
                        let current = if o.action == "ADD" {
                            self.tracked_live_quantity(&o.plan.symbol).abs()
                                * self.symbols[&o.plan.symbol].features.as_ref().unwrap().mid
                        } else {
                            0.
                        };
                        if current + o.quantity * o.limit_price > target.position_notional + 1e-8 {
                            return Err("发送前目标仓位已下降".into());
                        }
                        if o.action == "ADD" {
                            let parent = o
                                .parent_id
                                .as_ref()
                                .and_then(|id| self.live.orders.get(id))
                                .ok_or("缺少加仓父订单")?;
                            let mid = self.symbols[&o.plan.symbol].features.as_ref().unwrap().mid;
                            let sign = if o.plan.direction == "LONG" { 1. } else { -1. };
                            if (mid - parent.average_price) * sign < 0.
                                || q.net_edge < self.asset_config(&o.plan.symbol).incremental_ev
                            {
                                return Err("加仓盈利或增量优势条件已失效".into());
                            }
                        }
                        o.qualification = q;
                        self.log("live_risk_recheck", json!({"status":"PASS","intent":o}));
                    }
                    if automatic_entry {
                        if self.live.orders[&o.client_id]
                            .admission
                            .as_ref()
                            .is_none_or(|d| {
                                d.decision != "ADMITTED" || d.episode_id != o.plan.prediction_id
                            })
                        {
                            return Err("AUTO_ADMISSION_REQUIRED".into());
                        }
                        self.refresh_execution_evaluation(&o.plan.symbol)?;
                        let d = self.require_admission(&o.plan.symbol, Some(&o.client_id))?;
                        if d.episode_id != o.plan.prediction_id {
                            return Err("EPISODE_EXPIRED".into());
                        }
                        if o.quantity * o.limit_price > d.approved_notional + 1e-8 {
                            return Err("RISK_BUDGET_UNAVAILABLE".into());
                        }
                        o.qualification = d.qualification.clone();
                        o.admission = Some(d);
                        self.log("live_risk_recheck", json!({"status":"PASS","intent":o}));
                    }
                }
                if o.status == "SUBMITTING" && !o.reduce_only() {
                    let mut target = o.plan.clone();
                    self.apply_portfolio(&mut target, true, Some(&o.client_id))?;
                    let current = if o.action == "ADD" {
                        self.tracked_live_quantity(&o.plan.symbol).abs() * o.limit_price
                    } else {
                        0.
                    };
                    if current + o.quantity * o.limit_price > target.position_notional + 1e-8 {
                        return Err("PORTFOLIO_CHANGED_BEFORE_SEND".into());
                    }
                    o.plan.portfolio_decision_id = target.portfolio_decision_id;
                }
                // Network workers may only advance confirmed fills, never erase them.
                let previous = &self.live.orders[&o.client_id];
                if o.filled_qty + 1e-12 < previous.filled_qty {
                    return Err("成交数量不能回退".into());
                }
                self.live.orders.insert(o.client_id.clone(), o);
                self.close_live_trades();
                Ok(())
            }
            _ => Err("未知执行操作".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feeds::demo_events;
    fn ready() -> (Engine, TradePlan, Qualification) {
        let mut e = Engine::new_reference(true);
        for i in 0..8 {
            for v in demo_events(i, 1_000_000 + i * 1000) {
                e.ingest(v);
            }
        }
        let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
        p.direction = "LONG".into();
        p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        e.config.min_net_pnl = 0.01;
        let mut h = e.symbols["BTCUSDT"].health.clone();
        h.trust = "NORMAL".into();
        h.samples = 100;
        h.calibration_error = 0.;
        let (q, plan) = e.qualify(&p, e.symbols["BTCUSDT"].features.as_ref().unwrap(), &h);
        e.source = "live".into();
        e.mode = "CONFIRM".into();
        e.live.configured = true;
        e.live.cap = 100.;
        e.live.healthy = true;
        e.live.account = Some(AccountSnapshot {
            ts: e.now,
            equity: 10000.,
            available: 10000.,
            one_way: true,
            ..Default::default()
        });
        (e, plan.unwrap(), q)
    }
    #[test]
    fn universe_portfolio_veto_is_rechecked_before_confirm_submission() {
        let (mut e, _, _) = auto_ready();
        e.mode = "CONFIRM".into();
        e.initialize_universe();
        let (q, plan) = e.refresh_execution_evaluation("BTCUSDT").unwrap();
        let plan = plan.unwrap();
        assert!(e
            .queue_live(plan.clone(), q.clone(), "ENTRY", None, None)
            .unwrap_err()
            .contains("PORTFOLIO_BLOCK"));
        let u = e.universe.as_mut().unwrap();
        u.portfolio.live_enabled = true;
        let p = u.profiles.get_mut("BTCUSDT").unwrap();
        p.lifecycle_state = "ELIGIBLE".into();
        p.trigger.calibrated = true;
        p.mode_permissions.push("CONFIRM".into());
        p.qualification=Some(serde_json::from_value(json!({"id":"fixture","asset_key":"BTCUSDT","rule_version":1,"evaluated_at":e.now,"overall_state":"PASS","components":{},"blocking_reasons":[],"warnings":[],"research_priority":"HIGH","next_action":"reviewed"})).unwrap());
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let mut order = e.live.orders.values().next().unwrap().clone();
        assert!(order.plan.portfolio_decision_id.is_some());
        order.status = "SUBMITTING".into();
        let mut accepted = e.clone();
        accepted.live_command("broker_order", json!(order)).unwrap();
        e.universe.as_mut().unwrap().portfolio.live_enabled = false;
        let error = e.live_command("broker_order", json!(order)).unwrap_err();
        assert!(error.contains("PORTFOLIO_BLOCK"), "{error}");
        assert_eq!(e.live.orders[&order.client_id].status, "QUEUED");
        // Protective reductions do not depend on new-risk permissions.
        let o = e.live.orders.get_mut(&order.client_id).unwrap();
        o.status = "FILLED".into();
        o.filled_qty = o.quantity;
        o.average_price = o.limit_price;
        let mut exit = o.plan.clone();
        exit.position_notional = o.quantity * o.limit_price;
        e.queue_live(
            exit,
            order.qualification,
            "EXIT",
            Some(order.client_id),
            None,
        )
        .unwrap();
        assert!(e.live.orders.values().any(|o| o.reduce_only()));
    }
    #[test]
    fn independent_asset_slippage_is_used_at_live_intent_boundary() {
        let (mut e, _, _) = auto_ready();
        e.mode = "CONFIRM".into();
        e.initialize_universe();
        let mut c = e.config.clone();
        c.max_slippage = 0.0001;
        c.auto_min_net_edge = 0.01;
        let u = e.universe.as_mut().unwrap();
        u.portfolio.live_enabled = true;
        let profile = u.profiles.get_mut("BTCUSDT").unwrap();
        profile.config = Some(c);
        profile.lifecycle_state = "ELIGIBLE".into();
        profile.trigger.calibrated = true;
        profile.mode_permissions.push("CONFIRM".into());
        profile.qualification = Some(serde_json::from_value(json!({"id":"test","asset_key":"BTCUSDT","rule_version":1,"evaluated_at":e.now,"overall_state":"PASS","components":{},"blocking_reasons":[],"warnings":[],"research_priority":"HIGH","next_action":"reviewed"})).unwrap());
        let (q, plan) = e.refresh_execution_evaluation("BTCUSDT").unwrap();
        assert!(e
            .admission_decisions(true, None)
            .iter()
            .any(|d| d.symbol == "BTCUSDT"
                && d.reason_codes
                    .contains(&"AUTO_EDGE_THRESHOLD_NOT_MET".into())));
        let mid = e.symbols["BTCUSDT"].features.as_ref().unwrap().mid;
        e.queue_live(plan.unwrap(), q, "ENTRY", None, None).unwrap();
        let order = e.live.orders.values().next().unwrap();
        assert!(((order.limit_price / mid - 1.).abs() - 0.0001).abs() < 1e-10);
    }
    #[test]
    fn live_cap_pending_reservation_unknown_restart_and_risk_rejection() {
        let (mut e, plan, q) = ready();
        e.queue_live(plan.clone(), q.clone(), "ENTRY", None, None)
            .unwrap();
        let o = e.live.orders.values().next().unwrap().clone();
        assert!(o.quantity * o.limit_price <= 100.00000001);
        assert!((o.plan.expected_net_pnl - o.plan.position_notional * q.net_edge).abs() < 1e-9);
        assert!(e
            .queue_live(plan.clone(), q.clone(), "ENTRY", None, None)
            .is_err());
        e.live.orders.get_mut(&o.client_id).unwrap().status = "SUBMITTING".into();
        let mut restored: Engine =
            serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        restored.restart();
        assert_eq!(restored.live.orders[&o.client_id].status, "UNKNOWN");
        assert_eq!(restored.mode, "OBSERVE");
        e.live.orders.clear();
        e.kill_switch = true;
        assert!(e
            .queue_live(plan.clone(), q.clone(), "ENTRY", None, None)
            .is_err());
        e.kill_switch = false;
        let mut denied = q;
        denied.status = "NOT_QUALIFIED".into();
        assert!(e.queue_live(plan, denied, "ENTRY", None, None).is_err());
    }
    #[test]
    fn production_runtime_account_error_protects_confirmed_fill_without_waiting_for_clock() {
        let (mut e, plan, q) = ready();
        e.reference_runtime = false;
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let id = e.live.orders.keys().next().unwrap().clone();
        let entry = e.live.orders.get_mut(&id).unwrap();
        entry.status = "PARTIALLY_FILLED".into();
        entry.filled_qty = entry.quantity * 0.4;
        entry.average_price = entry.limit_price;
        let quantity = entry.filled_qty;
        let now = e.now;
        e.command("broker_error", json!({"error":"account query unavailable"}))
            .unwrap();
        assert_eq!(e.now, now);
        let exit = e.live.orders.values().find(|o| o.action == "EXIT").unwrap();
        assert!(exit.reduce_only());
        assert!((exit.quantity - quantity).abs() < 1e-10);
        assert!(e.records.iter().any(|r| r["kind"] == "protection_trigger"));
    }
    #[test]
    fn live_round_trip_fees_funding_and_external_position_mismatch() {
        let (mut e, plan, q) = ready();
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let mut entry = e.live.orders.values().next().unwrap().clone();
        entry.status = "FILLED".into();
        entry.filled_qty = 0.001;
        entry.average_price = 100.;
        entry.fee = 0.01;
        entry.fill_ts = e.now;
        e.live.orders.insert(entry.client_id.clone(), entry.clone());
        e.live.account.as_mut().unwrap().positions.insert(
            "BTCUSDT".into(),
            AccountPosition {
                quantity: 0.002,
                mark_price: 100.,
                ..Default::default()
            },
        );
        assert!(e.live_ready().is_err());
        e.live.account.as_mut().unwrap().positions.clear();
        let mut exit = entry.clone();
        exit.client_id = "exit1".into();
        exit.parent_id = Some(entry.client_id.clone());
        exit.action = "EXIT".into();
        exit.average_price = 101.;
        exit.fill_ts = e.now + 1000;
        e.live.orders.insert(exit.client_id.clone(), exit);
        e.live
            .account
            .as_mut()
            .unwrap()
            .funding
            .push(FundingSettlement {
                symbol: "BTCUSDT".into(),
                ts: e.now + 500,
                income: -0.002,
            });
        e.close_live_trades();
        e.close_live_trades();
        assert_eq!(e.live.trades.len(), 1);
        assert!((e.live.trades[0].net_pnl - (0.001 - 0.02 - 0.002)).abs() < 1e-9);
        assert!(e.tracked_live_quantity("BTCUSDT").abs() < 1e-10);
    }
    #[test]
    fn protective_exit_uses_confirmed_partial_fills_despite_stale_account_and_pending_entry() {
        let (mut e, plan, q) = ready();
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let id = e.live.orders.keys().next().unwrap().clone();
        let entry = e.live.orders.get_mut(&id).unwrap();
        entry.status = "PARTIALLY_FILLED".into();
        entry.filled_qty = entry.quantity * 0.4;
        entry.average_price = entry.limit_price;
        let qty = entry.filled_qty;
        // Account still says flat and then becomes unavailable. Neither may hide confirmed fills.
        e.live.healthy = false;
        e.live.account = None;
        e.symbols.get_mut("BTCUSDT").unwrap().features = None;
        e.live_tick();
        let exit = e
            .live
            .orders
            .values()
            .find(|o| o.action == "EXIT")
            .unwrap()
            .clone();
        assert_eq!(exit.quantity, qty);
        assert!(exit.reduce_only());
        assert!(e.live.protection.contains_key("BTCUSDT"));
        e.live_tick();
        assert_eq!(
            e.live
                .orders
                .values()
                .filter(|o| o.action == "EXIT")
                .count(),
            1
        );
        assert_eq!(e.live_positions_view()["BTCUSDT"]["target_notional"], 0.);
        // More fills can arrive after a cancellation request; the latched exit covers them too.
        let order = e.live.orders.get_mut(&exit.client_id).unwrap();
        order.status = "FILLED".into();
        order.filled_qty = qty;
        order.average_price = exit.limit_price;
        let entry = e.live.orders.get_mut(&id).unwrap();
        entry.status = "CANCELED".into();
        entry.filled_qty = qty * 1.5;
        e.live_tick();
        let next = e
            .live
            .orders
            .values()
            .find(|o| o.action == "EXIT" && o.status == "QUEUED")
            .unwrap();
        assert!((next.quantity - qty * 0.5).abs() < 1e-12);
    }
    #[test]
    fn live_cost_feedback_downgrades_without_paper_masking_and_filters_versions() {
        let (mut e, plan, q) = ready();
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let mut entry = e.live.orders.values().next().unwrap().clone();
        entry.status = "FILLED".into();
        entry.filled_qty = 1.;
        entry.average_price = 100.;
        entry.fee = 1.;
        entry.feedback.as_mut().unwrap().mid = 100.;
        entry.feedback.as_mut().unwrap().expected_round_trip_cost = 0.001;
        entry.feedback.as_mut().unwrap().expected_net_edge = 0.01;
        e.live.orders.insert(entry.client_id.clone(), entry.clone());
        let mut exit = entry.clone();
        exit.client_id = "exit-feedback".into();
        exit.parent_id = Some(entry.client_id.clone());
        exit.action = "EXIT".into();
        exit.average_price = 99.;
        exit.fill_ts = e.now + 1000;
        e.live.orders.insert(exit.client_id.clone(), exit);
        e.close_live_trades();
        let trade = &e.live.trades[0];
        assert!((trade.feedback.as_ref().unwrap().realized_cost - 3.).abs() < 1e-9);
        assert!((trade.feedback.as_ref().unwrap().expected_cost - 0.1).abs() < 1e-9);
        assert_eq!(trade.net_pnl, -3.);
        assert_eq!(trade.expected_net_pnl, 1.);
        let mut health = e.symbols["BTCUSDT"].health.clone();
        health.trust = "HIGH".into();
        e.apply_live_feedback("BTCUSDT", &mut health);
        assert_eq!(health.trust, "DEGRADED");
        assert_eq!(health.live.samples, 1);
        health.trust = "LOW".into();
        e.apply_live_feedback("BTCUSDT", &mut health);
        assert_eq!(health.trust, "LOW");
        e.live
            .account
            .as_mut()
            .unwrap()
            .funding
            .push(FundingSettlement {
                symbol: "BTCUSDT".into(),
                ts: e.now + 500,
                income: -0.5,
            });
        e.close_live_trades();
        assert_eq!(e.live.trades.len(), 1);
        assert_eq!(
            e.live.trades[0].feedback.as_ref().unwrap().realized_cost,
            3.5
        );
        e.config_version += 1;
        let health = e.live_feedback("BTCUSDT");
        assert_eq!(health.samples, 0);
        assert_eq!(health.excluded_samples, 1);
        assert_eq!(health.execution_drift, None);
    }
    #[test]
    fn rejected_protection_retries_with_backoff_and_cannot_duplicate_pending_reduction() {
        let (mut e, plan, q) = ready();
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let entry = e.live.orders.values_mut().next().unwrap();
        entry.status = "FILLED".into();
        entry.filled_qty = entry.quantity;
        entry.average_price = entry.limit_price;
        e.live.healthy = false;
        e.live_tick();
        let exit = e
            .live
            .orders
            .values_mut()
            .find(|o| o.action == "EXIT")
            .unwrap();
        exit.status = "REJECTED".into();
        e.live_tick();
        assert_eq!(e.live.orders.len(), 2);
        e.now += 5000;
        e.live_tick();
        assert_eq!(e.live.orders.len(), 3);
        e.live_tick();
        assert_eq!(e.live.orders.len(), 3);
    }
    #[test]
    fn live_qualification_uses_exchange_budget_and_scales_capped_exposure_on_downgrade() {
        let (mut e, _, _) = ready();
        e.live.account.as_mut().unwrap().equity = 250.;
        e.live.account.as_mut().unwrap().available = 250.;
        e.live.peak_equity = 250.;
        e.daily_pnl = -1000.;
        e.equity_peak = 20_000.;
        let mut prediction = e.symbols["BTCUSDT"].active.clone().unwrap();
        prediction.direction = "LONG".into();
        prediction.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        let mut health = e.symbols["BTCUSDT"].health.clone();
        health.trust = "NORMAL".into();
        health.samples = 100;
        health.calibration_error = 0.;
        let f = e.symbols["BTCUSDT"].features.as_ref().unwrap();
        let (q, normal) = e.qualify(&prediction, f, &health);
        assert_eq!(q.status, "QUALIFIED", "{:?}", q.reasons);
        let normal = normal.unwrap();
        assert_eq!(normal.position_notional, 100.);
        assert_eq!(normal.effective_leverage, 0.4);
        health.trust = "DEGRADED".into();
        let (_, reduced) = e.qualify(&prediction, f, &health);
        assert_eq!(reduced.unwrap().position_notional, 50.);
        e.live.account.as_mut().unwrap().daily_pnl = -e.config.max_daily_loss;
        let (q, plan) = e.qualify(
            &prediction,
            e.symbols["BTCUSDT"].features.as_ref().unwrap(),
            &health,
        );
        assert!(plan.is_none());
        assert!(q.reasons.contains(&"risk".into()));
        // The separate paper account must not inherit exchange equity or its drawdown peak.
        assert_eq!(e.account_equity(), e.config.equity + e.realized_pnl);
    }
    #[test]
    fn queued_orders_reserve_available_cash_without_double_counting_their_own_target() {
        let (mut e, plan, q) = ready();
        e.live.cap = 500.;
        e.live.account.as_mut().unwrap().available = 100.;
        e.queue_live(plan, q, "ENTRY", None, None).unwrap();
        let mut h = e.symbols["BTCUSDT"].health.clone();
        h.trust = "NORMAL".into();
        h.samples = 100;
        h.calibration_error = 0.;
        let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
        p.direction = "LONG".into();
        p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        let (_, own) = e.qualify(&p, e.symbols["BTCUSDT"].features.as_ref().unwrap(), &h);
        assert!(
            own.is_some(),
            "same-symbol target includes its pending entry"
        );
        p.symbol = "ETHUSDT".into();
        p.prediction_id = "eth-other".into();
        let (q, other) = e.qualify(&p, e.symbols["ETHUSDT"].features.as_ref().unwrap(), &h);
        assert!(other.is_none());
        assert!(q.reasons.contains(&"risk".into()));
    }
    #[test]
    fn live_requalification_exits_on_opposite_plan_without_adding_to_old_direction() {
        let (mut e, mut plan, q) = ready();
        e.symbols.get_mut("BTCUSDT").unwrap().health.trust = "NORMAL".into();
        e.symbols.get_mut("BTCUSDT").unwrap().qualification = Some(q.clone());
        e.queue_live(plan.clone(), q, "ENTRY", None, None).unwrap();
        let o = e.live.orders.values_mut().next().unwrap();
        o.status = "FILLED".into();
        o.filled_qty = o.quantity;
        o.average_price = o.limit_price;
        plan.direction = "SHORT".into();
        e.symbols.get_mut("BTCUSDT").unwrap().plan = Some(plan);
        e.live_tick();
        let exit = e.live.orders.values().find(|o| o.action == "EXIT").unwrap();
        assert_eq!(exit.reason, "当前建议方向与持仓相反");
        assert!(exit.reduce_only());
    }
    #[test]
    fn confirm_submission_rechecks_fresh_account_budget_and_journals_the_recheck() {
        let (mut e, _, _) = auto_ready();
        e.mode = "CONFIRM".into();
        let (q, plan) = e.refresh_execution_evaluation("BTCUSDT").unwrap();
        e.queue_live(plan.unwrap(), q, "ENTRY", None, None).unwrap();
        let mut order = e.live.orders.values().next().unwrap().clone();
        order.status = "SUBMITTING".into();
        let mut accepted = e.clone();
        accepted.live_command("broker_order", json!(order)).unwrap();
        assert!(accepted
            .records
            .iter()
            .any(|r| r["kind"] == "live_risk_recheck"));
        e.live.account.as_mut().unwrap().available = 5.;
        assert!(e.live_command("broker_order", json!(order)).is_err());
        assert_eq!(e.live.orders[&order.client_id].status, "QUEUED");
    }
    #[test]
    fn paper_actual_ev_feedback_is_separate_from_prediction_and_cost_drift() {
        let (e, _, _) = auto_ready();
        let mut trades = e.trades.clone();
        for trade in &mut trades {
            trade.net_pnl = -1.;
            trade.realized_net_edge = -0.01;
        }
        let healthy = crate::model::health(&e.outcomes, &e.trades, &e.config, false);
        let h = crate::model::health(&e.outcomes, &trades, &e.config, false);
        assert_eq!(h.ev_drift, healthy.ev_drift);
        assert_eq!(h.execution_drift, healthy.execution_drift);
        assert_eq!(h.paper_samples, 50);
        assert!((h.paper_ev_drift.unwrap() + 0.011).abs() < 1e-12);
        assert!(["DEGRADED", "LOW"].contains(&h.trust.as_str()));
    }
    fn auto_ready() -> (Engine, TradePlan, Qualification) {
        let (mut e, plan, q) = ready();
        e.live.cap = 500.;
        e.live.auto_allowed = true;
        e.config.auto_required_model_trust = "NORMAL".into();
        e.config.auto_min_net_edge = e.config.min_net_edge;
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            let h = &mut e.symbols.get_mut(symbol).unwrap().health;
            h.trust = "NORMAL".into();
            h.samples = 100;
            h.calibration_error = 0.01;
        }
        let p = e.symbols["BTCUSDT"].active.clone().unwrap();
        for i in 0..100 {
            let mut prediction = p.clone();
            prediction.prediction_id = format!("history-{i}");
            let mut history_plan = plan.clone();
            history_plan.prediction_id = prediction.prediction_id.clone();
            e.outcomes.push(Outcome {
                horizon_observation: None,
                prediction,
                outcome_type: "TARGET_HIT".into(),
                first_hit_ts: e.now,
                mfe: 0.003,
                mae: 0.,
                final_move: 0.003,
                prediction_error: String::new(),
                brier: 0.01,
                observed_net_edge: 0.001,
                learning_eligible: true,
                annotation: None,
            });
            let position = Position {
                execution_venue: None,
                plan: history_plan,
                qty: 0.,
                entry_price: 100.,
                current_price: 101.,
                fees: 0.01,
                funding: 0.,
                realized_pnl: 0.11,
                ts: e.now,
                last_rebalance: e.now,
                adjustments: 0,
                action: "EXIT".into(),
                reason: String::new(),
                target_notional: 0.,
                executions: vec![],
                config: e.config.clone(),
                expected_cost: 0.01,
                expected_edge: 0.001,
                cost_basis: 100.,
                alpha_ids: vec![],
                last_funding_ts: 0,
                funding_observed: false,
            };
            e.trades.push(TradeResult {
                position,
                exit_ts: e.now,
                exit_reason: "TAKE_PROFIT".into(),
                net_pnl: 0.1,
                realized_cost: 0.01,
                expected_cost: 0.01,
                realized_net_edge: 0.001,
            });
        }
        assert!(e.auto_evidence());
        let version = e.model.model_version.clone();
        e.model.model_version = "new-champion".into();
        assert!(!e.auto_evidence());
        e.model.model_version = version;
        assert!(e.command("mode", json!({"mode":"AUTO"})).is_err());
        e.command("mode", json!({"mode":"AUTO","acknowledge_auto":true}))
            .unwrap();
        e.refresh_execution_evaluation("BTCUSDT").unwrap();
        e.symbols.get_mut("ETHUSDT").unwrap().active = None;
        (e, plan, q)
    }
    #[test]
    fn auto_requires_current_model_evidence_and_manages_add_reduce_exit() {
        let (mut e, plan, q) = auto_ready();
        e.live_tick();
        assert!(!e.live.orders.is_empty(), "{:?}", e.admission.latest);
        assert_eq!(
            e.symbols["BTCUSDT"]
                .active
                .as_ref()
                .unwrap()
                .episode
                .highest_qualification,
            "QUALIFIED"
        );
        let mut entry = e
            .live
            .orders
            .values()
            .find(|o| o.plan.symbol == "BTCUSDT")
            .unwrap()
            .clone();
        e.live.orders.clear();
        entry.status = "EXPIRED".into();
        entry.filled_qty = entry.quantity * 0.2;
        entry.average_price = e.symbols["BTCUSDT"].features.as_ref().unwrap().mid;
        e.live.orders.insert(entry.client_id.clone(), entry.clone());
        e.live.account.as_mut().unwrap().positions.insert(
            "BTCUSDT".into(),
            AccountPosition {
                quantity: entry.filled_qty,
                entry_price: entry.average_price,
                mark_price: entry.average_price,
                liquidation_price: 0.,
                unrealized_pnl: 0.,
            },
        );
        e.now += 20_000;
        e.live.account.as_mut().unwrap().ts = e.now;
        let s = e.symbols.get_mut("BTCUSDT").unwrap();
        s.candidate_since = e.now - 3000;
        s.candidate_direction = "LONG".into();
        s.features.as_mut().unwrap().ts = e.now;
        s.qualification = Some(q);
        s.plan = Some(plan);
        for v in s.venues.values_mut() {
            v.book.receive_ts = e.now;
            v.book.exchange_ts = e.now;
        }
        e.live_tick();
        assert!(e.live.orders.values().any(|o| o.action == "ADD"));
        e.live.orders.retain(|_, o| o.action == "ENTRY");
        e.mode = "CONFIRM".into();
        e.symbols
            .get_mut("BTCUSDT")
            .unwrap()
            .plan
            .as_mut()
            .unwrap()
            .position_notional = 20.;
        e.live_tick();
        assert!(e.live.orders.values().any(|o| o.action == "REDUCE"));
        e.live.orders.retain(|_, o| o.action == "ENTRY");
        e.config.kill_behavior = "flatten".into();
        e.command("kill", json!({"active":true})).unwrap();
        e.live_tick();
        assert!(e
            .live
            .orders
            .values()
            .any(|o| o.action == "EXIT" && o.reduce_only()));
    }
    #[test]
    fn auto_missing_admission_is_rejected_at_intent_and_submission_boundaries() {
        let (mut e, plan, q) = auto_ready();
        assert_eq!(
            e.queue_live(plan, q, "ENTRY", None, None).unwrap_err(),
            "AUTO_ADMISSION_REQUIRED"
        );
        e.live_tick();
        let mut o = e.live.orders.values().next().unwrap().clone();
        assert_eq!(o.admission.as_ref().unwrap().decision, "ADMITTED");
        e.live.orders.get_mut(&o.client_id).unwrap().admission = None;
        o.status = "SUBMITTING".into();
        assert_eq!(
            e.live_command("broker_order", json!(o)).unwrap_err(),
            "AUTO_ADMISSION_REQUIRED"
        );
        assert!(e.live.orders.values().all(|o| o.status == "QUEUED"));
    }
    #[test]
    fn auto_submission_rechecks_current_edge_and_persists_fresh_admission() {
        let (mut e, _, _) = auto_ready();
        e.live_tick();
        let mut o = e.live.orders.values().next().unwrap().clone();
        let original_admission = o.admission.as_ref().unwrap().admission_id.clone();
        o.status = "SUBMITTING".into();
        let mut accepted = e.clone();
        accepted.live_command("broker_order", json!(o)).unwrap();
        let sent = &accepted.live.orders[&o.client_id];
        assert_ne!(
            sent.admission.as_ref().unwrap().admission_id,
            original_admission
        );
        assert!(accepted
            .records
            .iter()
            .any(|r| r["kind"] == "live_risk_recheck" && r["payload"]["status"] == "PASS"));
        e.config.auto_min_net_edge = 0.01;
        assert!(e
            .live_command("broker_order", json!(o))
            .unwrap_err()
            .contains("AUTO_EDGE_THRESHOLD_NOT_MET"));
        assert_eq!(e.live.orders[&o.client_id].status, "QUEUED");
    }
    #[test]
    fn daily_and_window_caps_never_lower_thresholds_or_force_orders() {
        let (mut e, _, _) = auto_ready();
        e.config.max_daily_trades = 1;
        // Config changes invalidate evidence; keep research fixtures explicitly comparable.
        for t in &mut e.trades {
            t.position.config = e.config.clone();
        }
        e.live_tick();
        let mut previous = e.live.orders.values().next().unwrap().clone();
        previous.status = "CANCELED".into();
        previous.plan.prediction_id = "older-episode".into();
        e.live.orders.insert(previous.client_id.clone(), previous);
        let d = e.admission_decisions(true, None);
        assert!(d[0].reason_codes.contains(&"DAILY_TRADE_CAP".into()));
        let threshold = e.config.auto_min_net_edge;
        e.live_tick();
        assert_eq!(e.live.orders.len(), 1);
        assert_eq!(e.config.auto_min_net_edge, threshold);
        e.config.max_daily_trades = 20;
        e.config.max_auto_entries_per_window = 1;
        assert!(e.admission_decisions(true, None)[0]
            .reason_codes
            .contains(&"ORDER_WINDOW_CAP".into()));
        e.live.orders.clear();
        e.config.auto_min_net_edge = 0.01;
        e.live_tick();
        assert!(e.live.orders.is_empty());
    }
}
