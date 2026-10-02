use crate::{
    book::Book,
    model::{self, Model},
    shadow::{ShadowPolicy, ShadowRun},
    types::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Venue {
    pub book: Book,
    pub connected: bool,
    pub error: String,
    pub reconnects: u64,
    pub trades: VecDeque<(u64, f64, bool)>,
    pub mark: Option<f64>,
    pub index: Option<f64>,
    pub mark_ts: u64,
    pub prices: VecDeque<(u64, f64)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolState {
    pub venues: BTreeMap<String, Venue>,
    pub features: Option<Features>,
    pub active: Option<Prediction>,
    pub last_prediction_ts: u64,
    pub chart: VecDeque<(u64, f64)>,
    pub qualification: Option<Qualification>,
    pub plan: Option<TradePlan>,
    pub health: ModelHealth,
    pub candidate_direction: String,
    pub candidate_since: u64,
    #[serde(default = "no_signal")]
    pub lifecycle: String,
    #[serde(default)]
    pub lifecycle_since: u64,
    #[serde(default)]
    pub last_result: Option<Outcome>,
    #[serde(default)]
    pub completed_at: u64,
}
fn no_signal() -> String {
    "NO_SIGNAL".into()
}

pub fn signal_direction(score: f64, previous: &str, c: &Config) -> &'static str {
    if score >= c.signal_enter {
        "LONG"
    } else if score <= -c.signal_enter {
        "SHORT"
    } else if previous == "LONG" && score > c.signal_exit {
        "LONG"
    } else if previous == "SHORT" && score < -c.signal_exit {
        "SHORT"
    } else {
        "FLAT"
    }
}
impl SymbolState {
    fn new(c: &Config) -> Self {
        Self {
            venues: BTreeMap::from([
                ("binance".into(), Venue::default()),
                ("okx".into(), Venue::default()),
            ]),
            features: None,
            active: None,
            last_prediction_ts: 0,
            chart: VecDeque::new(),
            qualification: None,
            plan: None,
            health: model::health(&[], &[], c, false),
            candidate_direction: "FLAT".into(),
            candidate_since: 0,
            lifecycle: no_signal(),
            lifecycle_since: 0,
            last_result: None,
            completed_at: 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Engine {
    pub source: String,
    pub mode: String,
    pub config: Config,
    pub config_version: u64,
    pub model: Model,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<ShadowRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_model: Option<Model>,
    pub now: u64,
    pub counter: u64,
    pub kill_switch: bool,
    pub storage_healthy: bool,
    pub symbols: BTreeMap<String, SymbolState>,
    pub outcomes: Vec<Outcome>,
    pub positions: BTreeMap<String, Position>,
    pub trades: Vec<TradeResult>,
    pub realized_pnl: f64,
    pub daily_pnl: f64,
    pub day: u64,
    pub equity_peak: f64,
    pub audit: Vec<Value>,
    #[serde(skip)]
    pub records: Vec<Value>,
}
impl Engine {
    pub fn new(demo: bool) -> Self {
        let config = Config::default();
        Self {
            source: if demo { "demo" } else { "live" }.into(),
            mode: "OBSERVE".into(),
            config_version: 1,
            model: Model::baseline(demo, &config),
            symbols: BTreeMap::from([
                ("BTCUSDT".into(), SymbolState::new(&config)),
                ("ETHUSDT".into(), SymbolState::new(&config)),
            ]),
            equity_peak: config.equity,
            config,
            shadow: None,
            previous_model: None,
            now: 0,
            counter: 0,
            kill_switch: false,
            storage_healthy: true,
            outcomes: vec![],
            positions: BTreeMap::new(),
            trades: vec![],
            realized_pnl: 0.,
            daily_pnl: 0.,
            day: 0,
            audit: vec![],
            records: vec![],
        }
    }
    pub fn log(&mut self, kind: &str, payload: Value) {
        let record = json!({"kind":kind,"ts":self.now,"source":self.source,"payload":payload});
        self.records.push(record.clone());
        if ![
            "prediction",
            "outcome",
            "features",
            "qualification",
            "plan",
            "shadow_prediction",
            "shadow_outcome",
        ]
        .contains(&kind)
        {
            self.audit.push(record);
            if self.audit.len() > 100 {
                self.audit.remove(0);
            }
        }
    }
    fn id(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}-{}-{}", self.now, self.counter)
    }
    fn transition(&mut self, symbol: &str, state: &str, prediction_id: Option<&str>) {
        let s = self.symbols.get_mut(symbol).unwrap();
        if s.lifecycle == state {
            return;
        }
        let previous = std::mem::replace(&mut s.lifecycle, state.into());
        s.lifecycle_since = self.now;
        self.log(
            "prediction_state",
            json!({"symbol":symbol,"from":previous,"to":state,"prediction_id":prediction_id}),
        );
    }
    pub fn restart(&mut self) {
        self.mode = "OBSERVE".into();
        for s in self.symbols.values_mut() {
            for v in s.venues.values_mut() {
                v.connected = false;
                v.book.valid = false;
            }
            s.plan = None;
            s.qualification = None;
            s.candidate_direction = "FLAT".into();
            s.candidate_since = self.now;
        }
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            self.invalidate(symbol, "process_restart");
        }
        self.log(
            "restart",
            json!({"mode":"OBSERVE","positions_restored":self.positions.len()}),
        );
    }
    pub fn command(&mut self, command: &str, payload: Value) -> Result<(), String> {
        match command {
            "shadow_start" => {
                if self.shadow.as_ref().is_some_and(|s| s.status == "RUNNING") {
                    return Err("请先停止当前影子验证，再载入新候选模型".into());
                }
                let artifact = payload["artifact"].as_str().ok_or("缺少模型文件内容")?;
                let policy: ShadowPolicy = if payload.get("policy").is_some() {
                    serde_json::from_value(payload["policy"].clone()).map_err(|e| e.to_string())?
                } else {
                    ShadowPolicy::default()
                };
                let mut run = ShadowRun::prepare(
                    artifact,
                    payload["report"].clone(),
                    policy,
                    &self.model,
                    &self.config,
                    self.now,
                    self.config_version,
                )?;
                run.run_id = self.id("shadow");
                self.shadow = Some(run);
            }
            "shadow_stop" => {
                self.check_shadow_id(&payload)?;
                self.pause_shadow("STOPPED");
            }
            "shadow_promote" => {
                self.check_shadow_id(&payload)?;
                let evidence = self.shadow_view().ok_or("没有影子验证")?;
                if evidence["promotion_ready"] != true {
                    return Err("晋升检查未全部通过，请查看样本、指标、当前行情与账户状态".into());
                }
                let candidate = self.shadow.as_ref().unwrap().candidate.clone();
                self.pause_shadow("PROMOTED");
                self.clear_predictions_for_model_change();
                let previous = std::mem::replace(&mut self.model, candidate);
                self.previous_model = Some(previous);
                self.mode = "OBSERVE".into();
                self.log("model_promotion", json!({"model":self.model.model_version,"hash":self.model.artifact_hash,"evidence":evidence,"mode":"OBSERVE"}));
            }
            "model_rollback" => {
                if payload["current_hash"] != self.model.artifact_hash
                    || self.previous_model.is_none()
                {
                    return Err("没有可回退模型或当前模型已变化".into());
                }
                if self.mode != "OBSERVE" || !self.positions.is_empty() || !self.storage_healthy {
                    return Err("回退要求 OBSERVE、无持仓且存储健康".into());
                }
                self.pause_shadow("ROLLED_BACK");
                self.clear_predictions_for_model_change();
                self.model = self.previous_model.take().unwrap();
                self.log(
                    "model_rollback_result",
                    json!({"model":self.model.model_version,"hash":self.model.artifact_hash}),
                );
            }
            "mode" => {
                let m = payload["mode"].as_str().ok_or("缺少 mode")?;
                if !["OBSERVE", "PAPER"].contains(&m) {
                    return Err("CONFIRM / AUTO 尚未通过真钱门槛；此版本不提供真实下单通道".into());
                }
                if self.kill_switch && m == "PAPER" {
                    return Err("请先解除紧急停止".into());
                }
                self.mode = m.into();
            }
            "kill" => {
                let active = payload["active"].as_bool().ok_or("缺少 active")?;
                self.kill_switch = active;
                if active {
                    self.mode = "OBSERVE".into();
                }
                if active && self.config.kill_behavior == "flatten" {
                    let keys: Vec<_> = self.positions.keys().cloned().collect();
                    for s in keys {
                        self.close_position(&s, "KILL_FLATTEN");
                    }
                }
            }
            "config" => {
                if !self.positions.is_empty() {
                    return Err("存在模拟持仓时不能修改风险与成本配置，请先平仓".into());
                }
                let c: Config =
                    serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
                c.validate()?;
                if c.equity != self.config.equity
                    && (self.realized_pnl != 0. || !self.trades.is_empty())
                {
                    return Err("已有交易后不能修改初始资金；请使用独立数据目录建立新账户".into());
                }
                if self
                    .shadow
                    .as_ref()
                    .is_some_and(|run| run.status == "RUNNING")
                {
                    self.pause_shadow("PAUSED_CONFIG");
                }
                self.config = c;
                self.config_version += 1;
                for s in self.symbols.values_mut() {
                    s.plan = None;
                    s.qualification = None;
                }
            }
            "flatten" => {
                let s = payload["symbol"].as_str().ok_or("缺少 symbol")?;
                if !self.positions.contains_key(s) {
                    return Err("没有该标的的模拟持仓".into());
                }
                self.close_position(s, "MANUAL_EXIT");
            }
            "annotate" => {
                let id = payload["prediction_id"]
                    .as_str()
                    .ok_or("缺少 prediction_id")?;
                let note = payload["annotation"].as_str().ok_or("缺少 annotation")?;
                if note.chars().count() > 500 {
                    return Err("注释最多 500 字".into());
                }
                let o = self
                    .outcomes
                    .iter_mut()
                    .find(|o| o.prediction.prediction_id == id)
                    .ok_or("结果不在当前会话缓存中")?;
                o.annotation = Some(note.into());
                for s in self.symbols.values_mut() {
                    if let Some(last) = s
                        .last_result
                        .as_mut()
                        .filter(|r| r.prediction.prediction_id == id)
                    {
                        last.annotation = Some(note.into());
                    }
                }
            }
            _ => return Err("未知操作".into()),
        }
        self.log(command, payload);
        Ok(())
    }
    pub fn ingest(&mut self, e: MarketEvent) {
        if e.receive_ts < self.now {
            self.log(
                "late_event",
                json!({"exchange":e.exchange,"symbol":e.symbol}),
            );
            return;
        }
        self.now = e.receive_ts;
        if let EventData::Clock = e.data {
            self.tick();
            return;
        }
        let Some(s) = self.symbols.get_mut(&e.symbol) else {
            return;
        };
        let Some(v) = s.venues.get_mut(&e.exchange) else {
            return;
        };
        let mut price = None;
        let mut invalid = false;
        match &e.data {
            EventData::Snapshot { .. } | EventData::Delta { .. } => match v.book.apply(&e) {
                Ok(true) => {
                    v.connected = true;
                    v.error.clear();
                    price = v.book.mid();
                    if let Some(p) = price {
                        v.prices.push_back((e.receive_ts, p));
                        while v.prices.len() > 2000
                            || v.prices
                                .front()
                                .is_some_and(|x| e.receive_ts.saturating_sub(x.0) > 60_000)
                        {
                            v.prices.pop_front();
                        }
                    }
                }
                Ok(false) => {}
                Err(reason) => {
                    v.error = reason;
                    invalid = true;
                }
            },
            EventData::Trade { quantity, buy, .. } => {
                if quantity.is_finite()
                    && *quantity >= 0.
                    && e.receive_ts.abs_diff(e.exchange_ts) <= self.config.max_stale_ms
                {
                    v.trades.push_back((e.receive_ts, *quantity, *buy));
                }
            }
            EventData::Mark { mark, index, .. } => {
                v.mark = Some(*mark);
                v.index = Some(*index);
                v.mark_ts = e.receive_ts;
            }
            EventData::Health { connected, reason } => {
                if !connected {
                    v.reconnects += 1;
                    v.book.valid = false;
                    invalid = true;
                }
                v.connected = *connected;
                v.error = reason.clone();
            }
            EventData::Clock => {}
        }
        while v
            .trades
            .front()
            .is_some_and(|x| self.now.saturating_sub(x.0) > 10_000)
        {
            v.trades.pop_front();
        }
        if e.exchange == "binance" {
            if invalid {
                self.invalidate(&e.symbol, "market_data_integrity");
            } else if let Some(p) = price {
                if self.symbols[&e.symbol].venues["binance"]
                    .book
                    .ready(self.now, self.config.max_stale_ms)
                {
                    self.track(&e.symbol, p);
                    self.check_position_barrier(&e.symbol, p);
                }
            }
        }
    }
    fn features(&self, symbol: &str) -> Option<Features> {
        let s = &self.symbols[symbol];
        let primary = &s.venues["binance"];
        let b = &primary.book;
        let (bid, ask, bq, aq) = b.top()?;
        let mid = (bid + ask) / 2.;
        let (bd, ad) = b.depth();
        let mut flags = vec![];
        if !b.ready(self.now, self.config.max_stale_ms) {
            flags.push("PRIMARY_STALE_OR_GAP".into());
        }
        let other = &s.venues["okx"].book;
        let cross = if other.ready(self.now, self.config.max_stale_ms) {
            other.mid().map(|m| (m / mid - 1.) * 10_000.)
        } else {
            flags.push("CROSS_VENUE_UNAVAILABLE".into());
            None
        };
        let changes: Vec<f64> = s
            .chart
            .iter()
            .zip(s.chart.iter().skip(1))
            .map(|(a, b)| (b.1 / a.1).ln())
            .collect();
        let vol = if changes.is_empty() {
            0.
        } else {
            (changes.iter().map(|r| r * r).sum::<f64>() / changes.len() as f64).sqrt()
        };
        let lag_price = |venue: &Venue| {
            venue
                .prices
                .iter()
                .rev()
                .find(|(t, _)| self.now.saturating_sub(*t) >= 1000)
                .map(|x| x.1)
        };
        let lead = match (
            lag_price(primary),
            lag_price(&s.venues["okx"]),
            other.mid(),
            cross,
        ) {
            (Some(p), Some(o), Some(current), Some(_)) => {
                Some(((current / o - 1.) - (mid / p - 1.)) * 10_000.)
            }
            _ => None,
        };
        let (buy, sell) = primary
            .trades
            .iter()
            .filter(|x| self.now.saturating_sub(x.0) <= 10_000)
            .fold(
                (0., 0.),
                |(a, b), (_, q, buy)| if *buy { (a + q, b) } else { (a, b + q) },
            );
        Some(Features {
            feature_snapshot_id: format!("f-{symbol}-{}", self.now),
            feature_schema_version: "micro-v1".into(),
            symbol: symbol.into(),
            ts: self.now,
            mid,
            spread_bps: (ask - bid) / mid * 10_000.,
            book_imbalance: (bd - ad) / (bd + ad).max(1.),
            flow_imbalance: b.ofi,
            aggressive_imbalance: (buy - sell) / (buy + sell).max(1e-9),
            microprice_bps: ((ask * bq + bid * aq) / (bq + aq) / mid - 1.) * 10_000.,
            momentum: s
                .chart
                .iter()
                .rev()
                .find(|(t, _)| self.now.saturating_sub(*t) >= 10_000)
                .map(|(_, p)| mid / p - 1.)
                .unwrap_or(0.),
            volatility: vol,
            depth: bd.min(ad),
            cross_venue_bps: cross,
            cross_venue_lead_bps: lead,
            mark: primary
                .mark
                .filter(|_| self.now.saturating_sub(primary.mark_ts) < 10_000),
            index: primary
                .index
                .filter(|_| self.now.saturating_sub(primary.mark_ts) < 10_000),
            data_quality: flags,
        })
    }
    pub fn account_equity(&self) -> f64 {
        self.config.equity
            + self.realized_pnl
            + self.positions.values().map(unrealized).sum::<f64>()
    }
    fn risk_available(&self) -> bool {
        let eq = self.account_equity();
        let unrealized_loss = self.positions.values().map(unrealized).sum::<f64>().min(0.);
        !self.kill_switch
            && self.storage_healthy
            && eq > 0.
            && self.daily_pnl + unrealized_loss > -self.config.max_daily_loss
            && (self.equity_peak - eq) / self.equity_peak.max(1.) < self.config.max_drawdown
    }
    pub fn costs(&self, f: &Features) -> Costs {
        let c = &self.config;
        let mut cost = Costs {
            spread: f.spread_bps / 10_000.,
            entry_fee: c.fee_bps / 10_000.,
            exit_fee: c.fee_bps / 10_000.,
            slippage: 2. * c.slippage_bps / 10_000.,
            impact: c.impact_bps / 10_000.,
            adverse_selection: c.adverse_selection_bps / 10_000.,
            funding: c.funding_bps / 10_000.,
            total: 0.,
        };
        cost.total = cost.spread
            + cost.entry_fee
            + cost.exit_fee
            + cost.slippage
            + cost.impact
            + cost.adverse_selection
            + cost.funding;
        cost
    }
    pub fn qualify(
        &self,
        p: &Prediction,
        f: &Features,
        h: &ModelHealth,
    ) -> (Qualification, Option<TradePlan>) {
        let c = &self.config;
        let costs = self.costs(f);
        let probs = p.probabilities();
        let win = p.win_probability();
        let loss = probs[if p.direction == "LONG" { 1 } else { 0 }];
        let gross = win * p.target - loss * p.stop;
        let net = gross - costs.total;
        let equity = self.account_equity().max(0.);
        let multiplier = if h.trust == "DEGRADED" { 0.5 } else { 1. };
        let other_notional = self
            .positions
            .iter()
            .filter(|(s, _)| *s != &p.symbol)
            .map(|(_, p)| p.qty * p.current_price)
            .sum::<f64>();
        let other_risk = self
            .positions
            .iter()
            .filter(|(s, _)| *s != &p.symbol)
            .map(|(_, p)| p.qty * p.current_price * (p.config.stop + self.costs(f).total))
            .sum::<f64>();
        let daily_remaining = (c.max_daily_loss
            + self.daily_pnl
            + self.positions.values().map(unrealized).sum::<f64>().min(0.)
            - other_risk)
            .max(0.);
        let budget = (equity * c.risk_per_trade * multiplier).min(daily_remaining);
        let capacity = f.depth * c.depth_participation;
        let notional = (budget / (p.stop + costs.total).max(1e-9))
            .min((c.max_position_notional - other_notional).max(0.))
            .min((equity * c.max_effective_leverage - other_notional).max(0.))
            .min(capacity);
        let leverage = notional / equity.max(1.);
        let buffer = if leverage <= 1. {
            1.
        } else {
            (1. / leverage - 0.01).max(0.)
        };
        let mut gates = vec![];
        let mut gate = |code: &str, name: &str, pass: bool, detail: String| {
            gates.push(Gate {
                code: code.into(),
                name: name.into(),
                pass,
                detail,
            })
        };
        gate(
            "probability",
            "概率优势",
            win >= c.min_probability + if h.trust == "DEGRADED" { 0.03 } else { 0. },
            format!("{:.1}% / 最低 {:.1}%", win * 100., c.min_probability * 100.),
        );
        gate(
            "calibration",
            "概率校准",
            p.calibrated_probabilities.is_some()
                && h.samples >= c.min_samples
                && h.calibration_error <= c.calibration_low,
            if p.calibrated_probabilities.is_none() {
                "模型尚无匹配的冻结校准参数".into()
            } else {
                format!(
                    "ECE {:.1}% · {} 个可比样本",
                    h.calibration_error * 100.,
                    h.samples
                )
            },
        );
        gate(
            "model_trust",
            "模型可信度",
            h.trust != "LOW",
            h.trust.clone(),
        );
        gate(
            "net_ev",
            "成本后 Net EV",
            net > c.min_net_edge,
            format!("{:+.4}%", net * 100.),
        );
        gate(
            "net_pnl",
            "预期净收益",
            net * notional >= c.min_net_pnl,
            format!("${:.2}", net * notional),
        );
        gate(
            "cost",
            "成本可信度",
            h.execution_drift.is_none_or(|d| d <= c.max_slippage),
            format!("总成本 {:.2} bps · 配置假设", costs.total * 10_000.),
        );
        gate(
            "liquidity",
            "流动性",
            f.depth >= c.min_depth && notional >= 10.,
            format!("近端深度 ${:.0}", f.depth),
        );
        gate(
            "slippage",
            "滑点 / 价差",
            c.slippage_bps / 10_000. <= c.max_slippage && f.spread_bps <= c.max_spread_bps,
            format!("价差 {:.2} bps", f.spread_bps),
        );
        gate(
            "volatility",
            "波动状态",
            f.volatility <= c.max_volatility,
            format!("1s RV {:.4}%", f.volatility * 100.),
        );
        gate(
            "risk",
            "账户风险预算",
            self.risk_available()
                && budget > 0.
                && notional >= 10.
                && buffer >= c.min_liquidation_buffer,
            format!("预算 ${budget:.2} · 杠杆 {leverage:.2}x"),
        );
        gate(
            "regime",
            "市场状态 / OOD",
            !h.regime_drift
                && f.cross_venue_bps
                    .is_some_and(|v| v.abs() <= c.max_cross_venue_bps),
            format!(
                "{} · {} · 最大偏差 {}",
                h.regime,
                h.distribution_status,
                h.distribution_score
                    .map(|z| format!("{z:.2}"))
                    .unwrap_or_else(|| "无训练分布".into())
            ),
        );
        gate(
            "freshness",
            "行情新鲜度",
            f.data_quality.is_empty(),
            if f.data_quality.is_empty() {
                "两交易所数据连续".into()
            } else {
                f.data_quality.join(", ")
            },
        );
        gate(
            "exchange",
            "交易所健康",
            self.symbols[&p.symbol]
                .venues
                .values()
                .all(|v| v.connected && v.book.valid),
            "Binance + OKX".into(),
        );
        let reasons = gates
            .iter()
            .filter(|g| !g.pass)
            .map(|g| g.code.clone())
            .collect::<Vec<_>>();
        let qualified = reasons.is_empty();
        let qualification = Qualification {
            prediction_id: p.prediction_id.clone(),
            gate_version: "gate-v2".into(),
            status: if qualified {
                "QUALIFIED"
            } else {
                "NOT_QUALIFIED"
            }
            .into(),
            gates,
            reasons,
            net_edge: net,
            gross_edge: gross,
            costs: costs.clone(),
        };
        let plan = qualified.then(|| TradePlan {
            plan_id: format!("plan-{}-{}", p.prediction_id, self.now),
            prediction_id: p.prediction_id.clone(),
            symbol: p.symbol.clone(),
            direction: p.direction.clone(),
            position_notional: notional,
            effective_leverage: leverage,
            expected_net_pnl: net * notional,
            expected_loss: loss * (p.stop + costs.total) * notional,
            planned_max_loss: (p.stop + costs.total) * notional,
            break_even_probability: (p.stop + costs.total) / (p.target + p.stop),
            expected_mae: h.mae_p95,
            liquidity_capacity: capacity,
            liquidation_buffer: buffer,
            tp: f.mid * (1. + p.sign() * p.target),
            sl: f.mid * (1. - p.sign() * p.stop),
            horizon: p.horizon,
            entry_type: "PAPER_MARKET_IOC".into(),
            exit_rules: vec![
                "TP / SL / Timeout".into(),
                "Gate 失效、模型 LOW 或负 Net EV → EXIT".into(),
                "目标仓位下降 → REDUCE；亏损不触发加仓".into(),
            ],
        });
        (qualification, plan)
    }
    fn tick(&mut self) {
        let day = self.now / 86_400_000;
        if self.day != day {
            self.daily_pnl = 0.;
            self.day = day;
        }
        self.equity_peak = self.equity_peak.max(self.account_equity());
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            let ready = self.symbols[symbol].venues["binance"]
                .book
                .ready(self.now, self.config.max_stale_ms);
            if !ready {
                self.invalidate(symbol, "stale_feed");
            }
            let Some(f) = self.features(symbol) else {
                if self.symbols[symbol].last_result.is_none() {
                    self.transition(symbol, "NO_SIGNAL", None);
                }
                continue;
            };
            if ready {
                self.track(symbol, f.mid);
                self.check_position_barrier(symbol, f.mid);
            }
            let comparable: Vec<_> = self
                .outcomes
                .iter()
                .filter(|o| {
                    o.prediction.symbol == symbol
                        && o.prediction.model_version == self.model.model_version
                        && o.prediction.config_version == self.config_version
                })
                .cloned()
                .collect();
            let trades: Vec<_> = self
                .trades
                .iter()
                .filter(|t| t.position.plan.symbol == symbol)
                .cloned()
                .collect();
            let health = self.model.health(&comparable, &trades, &self.config, &f);
            let direction = if ready {
                signal_direction(
                    Model::score(&f),
                    &self.symbols[symbol].candidate_direction,
                    &self.config,
                )
            } else {
                "FLAT"
            };
            {
                let s = self.symbols.get_mut(symbol).unwrap();
                if s.candidate_direction != direction {
                    s.candidate_direction = direction.into();
                    s.candidate_since = self.now;
                }
                s.features = Some(f.clone());
                s.health = health.clone();
                if ready {
                    s.chart.push_back((self.now, f.mid));
                    if s.chart.len() > 180 {
                        s.chart.pop_front();
                    }
                }
            }
            let result_held = self.symbols[symbol].last_result.is_some()
                && self.now.saturating_sub(self.symbols[symbol].completed_at)
                    < self.config.result_hold_ms;
            if self.symbols[symbol].active.is_none() && !result_held {
                self.transition(
                    symbol,
                    if direction == "FLAT" {
                        "NO_SIGNAL"
                    } else {
                        "WATCH"
                    },
                    None,
                );
            }
            let can_predict = ready
                && direction != "FLAT"
                && !result_held
                && self.symbols[symbol].active.is_none()
                && self
                    .now
                    .saturating_sub(self.symbols[symbol].last_prediction_ts)
                    >= 5000
                && self
                    .now
                    .saturating_sub(self.symbols[symbol].candidate_since)
                    >= self.config.signal_confirm_ms;
            if can_predict {
                let id = self.id("prediction");
                self.transition(symbol, "EMERGING", Some(&id));
                let (raw, cal) = self.model.probabilities(&f, &self.config);
                let mut p = Prediction {
                    prediction_id: id.clone(),
                    symbol: symbol.into(),
                    ts: self.now,
                    horizon: self.config.horizon,
                    direction: direction.into(),
                    target: self.config.target,
                    stop: self.config.stop,
                    start_price: f.mid,
                    current_price: f.mid,
                    raw_probabilities: raw,
                    calibrated_probabilities: cal,
                    model_version: self.model.model_version.clone(),
                    feature_snapshot_id: f.feature_snapshot_id.clone(),
                    features: f.clone(),
                    status: "TRACKING".into(),
                    mfe: 0.,
                    mae: 0.,
                    qualification: Qualification {
                        prediction_id: id,
                        gate_version: "gate-v1".into(),
                        status: "WATCH".into(),
                        gates: vec![],
                        reasons: vec![],
                        net_edge: 0.,
                        gross_edge: 0.,
                        costs: Costs::default(),
                    },
                    plan: None,
                    config_version: self.config_version,
                };
                let (q, plan) = self.qualify(&p, &f, &health);
                p.qualification = q;
                p.plan = plan;
                if p.qualification.status == "QUALIFIED" {
                    self.transition(symbol, "QUALIFIED", Some(&p.prediction_id));
                }
                if p.plan.is_some() {
                    self.transition(symbol, "TRADE_READY", Some(&p.prediction_id));
                }
                self.log("features", json!(&f));
                self.log("prediction", json!(&p));
                if let Some(pair) = self
                    .shadow
                    .as_mut()
                    .and_then(|run| run.predict(&p, &self.model, &self.config))
                {
                    self.log(
                        "shadow_prediction",
                        json!({"run_id":self.shadow.as_ref().unwrap().run_id,"prediction":pair}),
                    );
                }
                let s = self.symbols.get_mut(symbol).unwrap();
                s.last_prediction_ts = self.now;
                s.active = Some(p);
            }
            if let Some(p) = self.symbols[symbol].active.clone() {
                self.transition(symbol, "TRACKING", Some(&p.prediction_id));
                // Requalification uses fresh features, probabilities and remaining holding time.
                let mut fresh = p.clone();
                let (raw, cal) = self.model.probabilities(&f, &self.config);
                fresh.raw_probabilities = raw;
                fresh.calibrated_probabilities = cal;
                if fresh.config_version != self.config_version {
                    fresh.calibrated_probabilities = None;
                }
                let (q, plan) = self.qualify(&fresh, &f, &health);
                {
                    let s = self.symbols.get_mut(symbol).unwrap();
                    s.qualification = Some(q.clone());
                    s.plan = plan.clone();
                }
                if self.positions.contains_key(symbol) {
                    self.rebalance(symbol, &p, &q, plan.as_ref());
                } else if self.mode == "PAPER" && !self.kill_switch && self.now == p.ts {
                    if let Some(plan) = plan {
                        self.open_position(plan, &q);
                    }
                }
            } else {
                let s = self.symbols.get_mut(symbol).unwrap();
                s.qualification = None;
                s.plan = None;
                if self.positions.contains_key(symbol) {
                    self.close_position(symbol, "PREDICTION_COMPLETED");
                }
            }
        }
        // Pending exits never fill against stale quotes.
        let pending: Vec<_> = self
            .positions
            .iter()
            .filter(|(_, p)| p.action == "EXIT_PENDING")
            .map(|(s, p)| (s.clone(), p.reason.clone()))
            .collect();
        for (s, r) in pending {
            self.close_position(&s, &r);
        }
    }
    fn invalidate(&mut self, symbol: &str, reason: &str) {
        if self.symbols[symbol].active.is_some() {
            self.finish(symbol, "INVALIDATED", reason);
        }
        if self.positions.contains_key(symbol) {
            self.close_position(symbol, "DATA_INVALID_EXIT");
        }
    }
    fn track(&mut self, symbol: &str, price: f64) {
        let Some(p) = self.symbols.get_mut(symbol).unwrap().active.as_mut() else {
            return;
        };
        let deadline = p.ts + p.horizon * 1000;
        let outcome = if self.now > deadline {
            Some(if (p.current_price / p.start_price - 1.) * p.sign() >= 0. {
                "TIMEOUT_POSITIVE"
            } else {
                "TIMEOUT_NEGATIVE"
            })
        } else {
            p.current_price = price;
            let m = (price / p.start_price - 1.) * p.sign();
            p.mfe = p.mfe.max(m);
            p.mae = p.mae.max(-m);
            if m >= p.target {
                Some("TARGET_HIT")
            } else if m <= -p.stop * 2. {
                Some("STRONG_OPPOSITE_MOVE")
            } else if m <= -p.stop {
                Some("STOP_HIT")
            } else if self.now >= deadline {
                Some(if m >= 0. {
                    "TIMEOUT_POSITIVE"
                } else {
                    "TIMEOUT_NEGATIVE"
                })
            } else {
                None
            }
        };
        if let Some(kind) = outcome {
            self.finish(symbol, kind, "");
        }
    }
    fn finish(&mut self, symbol: &str, kind: &str, reason: &str) {
        let Some(mut p) = self.symbols.get_mut(symbol).unwrap().active.take() else {
            return;
        };
        p.status = "COMPLETED".into();
        let move_ = (p.current_price / p.start_price - 1.) * p.sign();
        let class = if kind == "TARGET_HIT" {
            if p.direction == "LONG" {
                0
            } else {
                1
            }
        } else if ["STOP_HIT", "STRONG_OPPOSITE_MOVE"].contains(&kind) {
            if p.direction == "LONG" {
                1
            } else {
                0
            }
        } else {
            2
        };
        let brier = p
            .probabilities()
            .iter()
            .enumerate()
            .map(|(i, v)| (v - if i == class { 1. } else { 0. }).powi(2))
            .sum();
        let timeout = kind.starts_with("TIMEOUT");
        let o = Outcome {
            first_hit_ts: if timeout {
                p.ts + p.horizon * 1000
            } else {
                self.now
            },
            final_move: move_,
            mfe: p.mfe,
            mae: p.mae,
            brier,
            observed_net_edge: move_ - p.qualification.costs.total,
            learning_eligible: kind != "INVALIDATED",
            annotation: None,
            prediction_error: if kind == "INVALIDATED" {
                reason.into()
            } else if class == 2 {
                "TIMEOUT".into()
            } else if kind == "TARGET_HIT" {
                "CORRECT".into()
            } else {
                "WRONG_WAY".into()
            },
            prediction: p,
            outcome_type: kind.into(),
        };
        self.log("outcome", json!(&o));
        if let Some(pair) = self.shadow.as_mut().and_then(|run| run.finish(&o)) {
            self.log(
                "shadow_outcome",
                json!({"run_id":self.shadow.as_ref().unwrap().run_id,"outcome":pair}),
            );
        }
        self.transition(symbol, "COMPLETED", Some(&o.prediction.prediction_id));
        let s = self.symbols.get_mut(symbol).unwrap();
        s.last_result = Some(o.clone());
        s.completed_at = self.now;
        s.qualification = None;
        s.plan = None;
        self.outcomes.push(o);
        if self.outcomes.len() > 2000 {
            self.outcomes.remove(0);
        }
    }
    fn fill(
        &mut self,
        symbol: &str,
        plan_id: &str,
        buy: bool,
        qty: f64,
        entry: bool,
        config: &Config,
    ) -> Option<Execution> {
        let b = &self.symbols[symbol].venues["binance"].book;
        if !b.ready(self.now, config.max_stale_ms) {
            return None;
        }
        let mid = b.mid()?;
        let levels: Vec<_> = if buy {
            b.asks.iter().map(|(p, q)| (*p as f64 / 1e6, *q)).collect()
        } else {
            b.bids
                .iter()
                .rev()
                .map(|(p, q)| (*p as f64 / 1e6, *q))
                .collect()
        };
        let mut remaining = qty;
        let mut value = 0.;
        for (p, q) in levels {
            let take = remaining.min(q);
            value += take * p;
            remaining -= take;
            if remaining < 1e-10 {
                break;
            }
        }
        let filled = qty - remaining;
        if filled <= 1e-10 {
            return None;
        }
        let extra = (config.slippage_bps
            + if entry {
                config.impact_bps + config.adverse_selection_bps
            } else {
                0.
            })
            / 10_000.;
        let price = value / filled * (1. + if buy { extra } else { -extra });
        Some(Execution {
            order_id: self.id("paper"),
            plan_id: plan_id.into(),
            side: if buy { "BUY" } else { "SELL" }.into(),
            qty: filled,
            expected_price: mid,
            fill_price: price,
            fee: filled * price * config.fee_bps / 10_000.,
            slippage: (price - mid).abs() / mid,
            send_ts: self.now,
            ack_ts: self.now,
            fill_ts: self.now,
            liquidity_limited: remaining > 1e-8,
        })
    }
    fn open_position(&mut self, plan: TradePlan, q: &Qualification) {
        if !self.risk_available()
            || q.status != "QUALIFIED"
            || self.positions.contains_key(&plan.symbol)
        {
            return;
        }
        let symbol = plan.symbol.clone();
        let mid = self.symbols[&symbol].features.as_ref().unwrap().mid;
        let config = self.config.clone();
        let reserve = q.costs.spread
            + config.max_slippage
            + (config.impact_bps + config.adverse_selection_bps) / 10_000.;
        let Some(fill) = self.fill(
            &symbol,
            &plan.plan_id,
            plan.direction == "LONG",
            plan.position_notional / (mid * (1. + reserve)),
            true,
            &config,
        ) else {
            return;
        };
        if fill.slippage
            > config.max_slippage
                + q.costs.spread / 2.
                + (config.impact_bps + config.adverse_selection_bps) / 10_000.
        {
            self.log(
                "paper_reject",
                json!({"reason":"slippage_limit","plan_id":plan.plan_id}),
            );
            return;
        }
        let basis = fill.qty * fill.fill_price;
        let p = Position {
            plan: plan.clone(),
            qty: fill.qty,
            entry_price: fill.fill_price,
            current_price: mid,
            fees: fill.fee,
            funding: 0.,
            realized_pnl: 0.,
            ts: self.now,
            last_rebalance: self.now,
            adjustments: 0,
            action: "HOLD".into(),
            reason: "已通过全部资格检查".into(),
            target_notional: plan.position_notional,
            executions: vec![fill.clone()],
            config,
            expected_cost: basis * q.costs.total,
            expected_edge: q.net_edge,
            cost_basis: basis,
        };
        self.log("qualification", json!(q));
        self.log("plan", json!(plan));
        self.log("execution", json!(fill));
        self.positions.insert(symbol, p);
    }
    fn check_position_barrier(&mut self, symbol: &str, price: f64) {
        let Some(p) = self.positions.get_mut(symbol) else {
            return;
        };
        p.current_price = price;
        let long = p.plan.direction == "LONG";
        let hit_tp = if long {
            price >= p.plan.tp
        } else {
            price <= p.plan.tp
        };
        let hit_sl = if long {
            price <= p.plan.sl
        } else {
            price >= p.plan.sl
        };
        let reason = if hit_sl {
            Some("STOP_LOSS")
        } else if hit_tp {
            Some("TAKE_PROFIT")
        } else if self.now >= p.ts + p.plan.horizon * 1000 {
            Some("TIMEOUT")
        } else {
            None
        };
        if let Some(r) = reason {
            self.close_position(symbol, r);
        }
    }
    fn rebalance(
        &mut self,
        symbol: &str,
        prediction: &Prediction,
        q: &Qualification,
        plan: Option<&TradePlan>,
    ) {
        let p = &self.positions[symbol];
        if p.action == "EXIT_PENDING" {
            return;
        }
        if self.kill_switch {
            if self.config.kill_behavior == "flatten" {
                self.close_position(symbol, "KILL_FLATTEN");
            }
            return;
        }
        if q.status != "QUALIFIED" || q.net_edge <= 0. || prediction.direction != p.plan.direction {
            self.close_position(symbol, "REQUALIFICATION_FAILED");
            return;
        }
        let target = plan.map(|p| p.position_notional).unwrap_or(0.);
        let current = p.qty * p.current_price;
        let due =
            self.now.saturating_sub(p.last_rebalance) >= self.config.rebalance_interval_secs * 1000;
        let reduce = target < current * (1. - self.config.position_deadband) && due;
        // This conservative V1 never averages down or adds to an existing prediction.
        let p = self.positions.get_mut(symbol).unwrap();
        p.target_notional = target;
        p.action = "HOLD".into();
        p.reason = "持续复核；同一预测内不加仓".into();
        if reduce {
            if p.adjustments >= self.config.max_rebalances {
                self.close_position(symbol, "REBALANCE_LIMIT");
            } else {
                self.reduce_position(symbol, (current - target) / current, "TARGET_RISK_REDUCED");
            }
        }
    }
    pub fn close_position(&mut self, symbol: &str, reason: &str) {
        self.reduce_position(symbol, 1., reason);
    }
    fn reduce_position(&mut self, symbol: &str, fraction: f64, reason: &str) {
        let Some(p) = self.positions.get(symbol).cloned() else {
            return;
        };
        let Some(fill) = self.fill(
            symbol,
            &p.plan.plan_id,
            p.plan.direction != "LONG",
            p.qty * fraction,
            false,
            &p.config,
        ) else {
            let p = self.positions.get_mut(symbol).unwrap();
            p.action = "EXIT_PENDING".into();
            p.reason = reason.into();
            return;
        };
        let pnl = (fill.fill_price - p.entry_price)
            * fill.qty
            * if p.plan.direction == "LONG" { 1. } else { -1. };
        let p = self.positions.get_mut(symbol).unwrap();
        p.qty = (p.qty - fill.qty).max(0.);
        p.fees += fill.fee;
        p.realized_pnl += pnl;
        p.last_rebalance = self.now;
        p.adjustments += 1;
        p.executions.push(fill.clone());
        p.action = if fraction >= 1. && p.qty > 1e-9 {
            "EXIT_PENDING"
        } else {
            "REDUCE"
        }
        .into();
        p.reason = reason.into();
        self.log("execution", json!(fill));
        if self.positions[symbol].qty < 1e-9 {
            let mut p = self.positions.remove(symbol).unwrap();
            // A conservative reserved funding debit, not an exchange settlement simulation.
            p.funding = p.cost_basis * p.config.funding_bps / 10_000.;
            let net = p.realized_pnl - p.fees - p.funding;
            self.realized_pnl += net;
            self.daily_pnl += net;
            let cost = p.fees
                + p.funding
                + p.executions
                    .iter()
                    .map(|e| (e.fill_price - e.expected_price).abs() * e.qty)
                    .sum::<f64>();
            let t = TradeResult {
                exit_ts: self.now,
                exit_reason: reason.into(),
                net_pnl: net,
                realized_cost: cost,
                expected_cost: p.expected_cost,
                realized_net_edge: net / p.cost_basis.max(1.),
                position: p,
            };
            self.log("trade", json!(&t));
            self.trades.push(t);
            if self.trades.len() > 2000 {
                self.trades.remove(0);
            }
        }
    }
    fn check_shadow_id(&self, payload: &Value) -> Result<(), String> {
        if self
            .shadow
            .as_ref()
            .is_none_or(|run| payload["run_id"] != run.run_id)
        {
            return Err("影子验证已变化，请刷新后重试".into());
        }
        Ok(())
    }
    fn pause_shadow(&mut self, status: &str) {
        let Some(run) = self.shadow.as_mut() else {
            return;
        };
        let pending: Vec<_> = run.pending.keys().cloned().collect();
        run.pending.clear();
        run.status = status.into();
        let id = run.run_id.clone();
        self.log(
            "shadow_status",
            json!({"run_id":id,"status":status,"cancelled_predictions":pending}),
        );
    }
    fn clear_predictions_for_model_change(&mut self) {
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            self.invalidate(symbol, "model_changed");
        }
        for s in self.symbols.values_mut() {
            s.plan = None;
            s.qualification = None;
            s.candidate_direction = "FLAT".into();
            s.candidate_since = self.now;
            s.health = model::health(&[], &[], &self.config, false);
        }
    }
    pub fn shadow_view(&self) -> Option<Value> {
        let run = self.shadow.as_ref()?;
        let comparison = run.comparison(&self.model, self.config_version, self.now);
        let market_ready = self.symbols.values().all(|s| {
            s.venues
                .values()
                .all(|v| v.connected && v.book.ready(self.now, self.config.max_stale_ms))
                && s.features.as_ref().is_some_and(|f| {
                    self.now.saturating_sub(f.ts) <= self.config.max_stale_ms
                        && run.candidate.probabilities(f, &self.config).1.is_some()
                        && run
                            .candidate
                            .distribution_score(f)
                            .zip(run.candidate.distributions.get(&f.symbol))
                            .is_some_and(|(z, d)| z <= d.max_z)
                })
        });
        let operational_checks = json!({"observe_mode":self.mode == "OBSERVE", "no_positions":self.positions.is_empty(),
            "storage_healthy":self.storage_healthy,"kill_released":!self.kill_switch,"market_ready":market_ready});
        let ready = comparison["ready"] == true
            && operational_checks
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == true);
        Some(
            json!({"run_id":run.run_id,"status":run.status,"started_at":run.started_at,"config_version":run.config_version,
            "champion_version":run.champion_version,"candidate_version":run.candidate.model_version,"candidate_hash":run.candidate.artifact_hash,
            "policy":run.policy,"offline_status":run.offline_report["status"],"comparison":comparison,
            "operational_checks":operational_checks,"promotion_ready":ready,
            "recent_outcomes":run.outcomes.iter().rev().take(20).collect::<Vec<_>>()}),
        )
    }
    pub fn view(&self) -> Value {
        let symbols:BTreeMap<_,_>=self.symbols.iter().map(|(name,s)|{
            let venues:BTreeMap<_,_>=s.venues.iter().map(|(name,v)|(name,json!({"connected":v.connected,"ready":v.book.ready(self.now,self.config.max_stale_ms),
                "age_ms":self.now.saturating_sub(v.book.receive_ts),"exchange_age_ms":self.now.abs_diff(v.book.exchange_ts),"gaps":v.book.gaps,"reconnects":v.reconnects,"error":v.error,"mid":v.book.mid(),"sequence":v.book.sequence}))).collect();
            (name,json!({"venues":venues,"features":s.features,"active":s.active,"qualification":s.qualification,"plan":s.plan,"health":s.health,"chart":s.chart,"lifecycle":s.lifecycle,"lifecycle_since":s.lifecycle_since,"signal_direction":s.candidate_direction,"last_result":s.last_result}))
        }).collect();
        json!({"source":self.source,"mode":self.mode,"now":self.now,"config":self.config,"config_version":self.config_version,"model":self.model,
            "shadow":self.shadow_view(),"rollback_available":self.previous_model.is_some(),"shadow_policy_defaults":ShadowPolicy::default(),
            "kill_switch":self.kill_switch,"storage_healthy":self.storage_healthy,"symbols":symbols,"positions":self.positions,
            "outcomes":self.outcomes.iter().rev().take(100).collect::<Vec<_>>(),"trades":self.trades.iter().rev().take(100).collect::<Vec<_>>(),
            "audit":self.audit.iter().rev().take(30).collect::<Vec<_>>(),"account":{"equity":self.account_equity(),"realized_pnl":self.realized_pnl,"daily_pnl":self.daily_pnl,"peak":self.equity_peak},
            "live_enabled":false,"live_gate_reason":"尚无经过批准的真钱执行适配器与实盘验证证据"})
    }
}
pub fn unrealized(p: &Position) -> f64 {
    (p.current_price - p.entry_price) * p.qty * if p.plan.direction == "LONG" { 1. } else { -1. }
        + p.realized_pnl
        - p.fees
}
