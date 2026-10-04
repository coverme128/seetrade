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
    #[serde(default)]
    pub index_ts: u64,
    #[serde(default)]
    pub funding_rate: Option<f64>,
    #[serde(default)]
    pub funding_ts: u64,
    #[serde(default)]
    pub next_funding_ts: Option<u64>,
    pub prices: VecDeque<(u64, f64)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolState {
    #[serde(default)]
    pub runtime: crate::market_pulse::Runtime,
    #[serde(default)]
    pub evaluation: Option<AlphaEvaluation>,
    #[serde(default)]
    pub consumed_signal_epoch: Option<u64>,
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
    pub(crate) fn new(c: &Config) -> Self {
        Self {
            runtime: Default::default(),
            evaluation: None,
            consumed_signal_epoch: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universe: Option<crate::universe::Universe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_asset: Option<String>,
    #[serde(default)]
    pub trigger_config: crate::market_pulse::TriggerConfig,
    #[serde(default)]
    pub trigger_config_version: u64,
    #[serde(default)]
    pub trigger_analytics: crate::trigger_analytics::TriggerAnalytics,
    #[serde(default)]
    pub reference_runtime: bool,
    #[serde(skip)]
    pub telemetry: std::sync::Arc<std::sync::Mutex<crate::telemetry::Telemetry>>,
    #[serde(default)]
    pub live: crate::live::LiveState,
    #[serde(default)]
    pub research: crate::research::ResearchState,
    #[serde(default)]
    pub integrity: IntegrityCounters,
    #[serde(default)]
    pub horizon_pending: BTreeMap<String, crate::horizon::HorizonTracker>,
    #[serde(default)]
    pub daily_integrity: IntegrityCounters,
    #[serde(default)]
    pub admission: crate::admission::AdmissionState,
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
            universe: None,
            shadow_asset: None,
            trigger_config: Default::default(),
            trigger_config_version: 1,
            trigger_analytics: Default::default(),
            reference_runtime: false,
            telemetry: Default::default(),
            live: crate::live::LiveState::default(),
            research: crate::research::ResearchState::default(),
            integrity: Default::default(),
            horizon_pending: Default::default(),
            daily_integrity: Default::default(),
            admission: Default::default(),
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
    /// Fixed-step policy for offline contract regression and replay comparisons only.
    pub fn new_reference(demo: bool) -> Self {
        let mut e = Self::new(demo);
        e.reference_runtime = true;
        e
    }
    pub fn log(&mut self, kind: &str, mut payload: Value) {
        if kind == "protection_trigger" {
            if let Some(p) = payload["symbol"]
                .as_str()
                .and_then(|s| self.universe.as_ref()?.profiles.get(s))
            {
                payload["asset_trigger_profile_id"] = json!(p.trigger.id);
            }
        }
        self.trigger_analytics.observe(kind, &payload);
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
    pub(crate) fn id(&mut self, prefix: &str) -> String {
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
        if self.trigger_config_version == 0 {
            self.trigger_config_version = 1;
            self.config_version = self.next_config_version();
            self.log("runtime_migration", json!({"version":"v1.0.4","reason":"new trigger sampling policy; retain history, collect comparable evidence under a new config version"}));
        }
        if self.config.auto_min_net_edge < self.config.min_net_edge {
            self.config.auto_min_net_edge = self.config.min_net_edge;
            self.log("config_migration", json!({"reason":"AUTO threshold must not be lower than qualification","auto_min_net_edge":self.config.auto_min_net_edge}));
        }
        self.mode = "OBSERVE".into();
        self.live.healthy = false;
        if self.research.status == "RUNNING" {
            self.research.status = "FAILED".into();
            self.research.result = json!({"error":"进程重启中断研究；Champion 未改变"});
        }
        for o in self.live.orders.values_mut() {
            if o.status == "SUBMITTING" {
                o.status = "UNKNOWN".into();
            }
        }
        let interrupted: Vec<_> = self
            .symbols
            .values()
            .filter_map(|s| s.runtime.origin.as_ref().map(|o| o.trigger_id.clone()))
            .collect();
        for id in interrupted {
            self.log(
                "trigger_candidate_closed",
                json!({"trigger_id":id,"reason":"restart"}),
            );
        }
        for s in self.symbols.values_mut() {
            s.runtime = Default::default();
            for v in s.venues.values_mut() {
                v.connected = false;
                v.book.valid = false;
            }
            s.plan = None;
            s.qualification = None;
            s.evaluation = None;
            s.candidate_direction = "FLAT".into();
            s.candidate_since = self.now;
        }
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            let symbol = symbol.as_str();
            self.invalidate(symbol, "process_restart");
        }
        self.update_horizons();
        self.log(
            "restart",
            json!({"mode":"OBSERVE","positions_restored":self.positions.len()}),
        );
    }
    pub fn command(&mut self, command: &str, payload: Value) -> Result<(), String> {
        if command.starts_with("scanner_")
            || command.starts_with("asset_")
            || ["universe_rollback", "portfolio_config"].contains(&command)
        {
            self.universe_command(command, payload.clone())?;
            self.log(command, payload);
            return Ok(());
        }
        match command {
            "trigger_config" => self.set_trigger_config(payload.clone())?,
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
                let asset = payload["symbol"]
                    .as_str()
                    .map(crate::universe::normalize)
                    .transpose()?;
                if asset
                    .as_ref()
                    .is_some_and(|s| !self.symbols.contains_key(s))
                {
                    return Err("币种尚未采集".into());
                }
                if asset.is_some() {
                    self.initialize_universe();
                }
                let champion = asset
                    .as_ref()
                    .map(|s| self.asset_model(s))
                    .unwrap_or(&self.model);
                let config = asset
                    .as_ref()
                    .map(|s| self.asset_execution_config(s))
                    .unwrap_or_else(|| self.config.resolved_costs(self.cost_venue()));
                let mut run = ShadowRun::prepare(
                    artifact,
                    payload["report"].clone(),
                    policy,
                    champion,
                    &config,
                    self.now,
                    asset
                        .as_ref()
                        .map_or(self.config_version, |s| self.asset_config_version(s)),
                )?;
                if asset.as_ref().is_some_and(|s| {
                    run.candidate.distributions.len() != 1
                        || !run.candidate.distributions.contains_key(s)
                }) {
                    return Err("候选模型必须只包含所选币种的分布与校准".into());
                }
                if asset.is_none()
                    && self.universe.as_ref().is_some_and(|u| {
                        ["BTCUSDT", "ETHUSDT"]
                            .iter()
                            .any(|s| u.profiles.get(*s).is_some_and(|p| p.config.is_some()))
                    })
                {
                    return Err("BTC/ETH 存在独立配置，请分别运行币种影子验证".into());
                }
                if asset.is_none()
                    && !run.candidate.distributions.is_empty()
                    && ["BTCUSDT", "ETHUSDT"]
                        .iter()
                        .any(|s| !run.candidate.distributions.contains_key(*s))
                {
                    return Err("单币候选模型需要指定 symbol；全局模型必须覆盖 BTC/ETH".into());
                }
                self.shadow_asset = asset;
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
                if let Some(symbol) = self.shadow_asset.clone() {
                    let profile = self
                        .universe
                        .as_mut()
                        .unwrap()
                        .profiles
                        .get_mut(&symbol)
                        .unwrap();
                    profile.calibration_artifact_id = Some(candidate.calibration_version.clone());
                    profile.feature_normalization_version = candidate.artifact_hash.clone();
                    profile.previous_model = profile.model.replace(candidate);
                    self.save_profile(&symbol);
                } else {
                    let previous = std::mem::replace(&mut self.model, candidate);
                    self.previous_model = Some(previous);
                    self.sync_shared_model_profiles();
                }
                self.mode = "OBSERVE".into();
                let promoted = self
                    .shadow_asset
                    .as_ref()
                    .map(|s| self.asset_model(s))
                    .unwrap_or(&self.model);
                self.log("model_promotion", json!({"model":promoted.model_version,"hash":promoted.artifact_hash,"evidence":evidence,"mode":"OBSERVE"}));
            }
            "model_rollback" => {
                if payload["current_hash"] != self.model.artifact_hash
                    || self.previous_model.is_none()
                {
                    return Err("没有可回退模型或当前模型已变化".into());
                }
                if self.mode != "OBSERVE"
                    || !self.positions.is_empty()
                    || !self.storage_healthy
                    || self.live.orders.values().any(|o| o.pending())
                    || self
                        .live
                        .account
                        .as_ref()
                        .is_some_and(|a| a.positions.values().any(|p| p.quantity.abs() > 1e-10))
                {
                    return Err("回退要求 OBSERVE、无持仓且存储健康".into());
                }
                self.pause_shadow("ROLLED_BACK");
                self.clear_predictions_for_model_change();
                self.model = self.previous_model.take().unwrap();
                self.sync_shared_model_profiles();
                self.log(
                    "model_rollback_result",
                    json!({"model":self.model.model_version,"hash":self.model.artifact_hash}),
                );
            }
            "mode" => {
                let m = payload["mode"].as_str().ok_or("缺少 mode")?;
                if !["OBSERVE", "PAPER", "CONFIRM", "AUTO"].contains(&m) {
                    return Err("未知模式".into());
                }
                if ["CONFIRM", "AUTO"].contains(&m) {
                    self.live_ready()?;
                }
                if m == "AUTO"
                    && (payload["acknowledge_auto"] != true
                        || !self.live.auto_allowed
                        || !self.auto_evidence())
                {
                    return Err("AUTO 需要显式授权、独立开启和多窗口成本后验证证据".into());
                }
                if m == "PAPER"
                    && self
                        .live
                        .account
                        .as_ref()
                        .is_some_and(|a| a.positions.values().any(|p| p.quantity.abs() > 1e-10))
                {
                    return Err("请先平掉真实持仓".into());
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
                if !self.positions.is_empty()
                    || self.live.orders.values().any(|o| o.pending())
                    || self
                        .live
                        .account
                        .as_ref()
                        .is_some_and(|a| a.positions.values().any(|p| p.quantity.abs() > 1e-10))
                {
                    return Err("存在持仓或待处理订单时不能修改风险与成本配置，请先平仓".into());
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
                self.config_version = self.next_config_version();
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
            "research_schedule" | "research_run" | "research_started" | "research_finished" => {
                self.research_command(command, payload.clone())?
            }
            "execute" | "skip" | "live_flatten" | "broker_config" | "broker_snapshot"
            | "broker_error" | "broker_order" => self.live_command(command, payload.clone())?,
            _ => return Err("未知操作".into()),
        }
        self.log(command, payload);
        if !self.reference_runtime
            && ["broker_error", "broker_snapshot", "broker_order"].contains(&command)
        {
            for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
                let symbol = symbol.as_str();
                let (_, force) = self.protection_check(symbol);
                if force {
                    self.evaluate_symbol(symbol, true);
                    self.protection_result(symbol);
                }
            }
            self.live_tick();
        }
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
        if self.day != self.now / 86_400_000 {
            self.day = self.now / 86_400_000;
            self.daily_pnl = 0.;
            self.daily_integrity = IntegrityCounters::default();
        }
        self.settle_funding();
        if let EventData::Clock = e.data {
            self.tick();
            self.update_horizons();
            return;
        }
        let stale_ms = self.asset_config(&e.symbol).max_stale_ms;
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
                    && e.receive_ts.abs_diff(e.exchange_ts) <= stale_ms
                {
                    v.trades.push_back((e.receive_ts, *quantity, *buy));
                }
            }
            EventData::Mark {
                mark,
                index,
                funding,
            } => {
                if mark.is_finite()
                    && *mark > 0.
                    && index.is_finite()
                    && *index > 0.
                    && funding.is_finite()
                    && e.receive_ts.abs_diff(e.exchange_ts) <= stale_ms
                {
                    v.mark = Some(*mark);
                    v.index = Some(*index);
                    v.mark_ts = e.receive_ts;
                    v.index_ts = e.receive_ts;
                    v.funding_rate = Some(*funding);
                    v.funding_ts = e.receive_ts;
                }
            }
            EventData::Context {
                mark,
                index,
                funding,
                next_funding_ts,
            } => {
                if e.receive_ts.abs_diff(e.exchange_ts) <= stale_ms {
                    if let Some(p) = mark.filter(|p| p.is_finite() && *p > 0.) {
                        v.mark = Some(p);
                        v.mark_ts = e.receive_ts;
                    }
                    if let Some(p) = index.filter(|p| p.is_finite() && *p > 0.) {
                        v.index = Some(p);
                        v.index_ts = e.receive_ts;
                    }
                    if let Some(r) = funding.filter(|r| r.is_finite() && r.abs() <= 0.1) {
                        v.funding_rate = Some(r);
                        v.funding_ts = e.receive_ts;
                        v.next_funding_ts = *next_funding_ts;
                    }
                }
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
        if invalid {
            v.prices.clear();
            if let Some(p) = self
                .universe
                .as_mut()
                .and_then(|u| u.profiles.get_mut(&e.symbol))
            {
                p.clean_since = None;
            }
            self.invalidate(&e.symbol, "market_data_integrity");
        }
        if e.exchange == "binance" {
            if invalid {
                self.invalidate(&e.symbol, "market_data_integrity");
            } else if let Some(p) = price {
                if self.symbols[&e.symbol].venues["binance"]
                    .book
                    .ready(self.now, stale_ms)
                {
                    self.track(&e.symbol, p);
                    self.check_position_barrier(&e.symbol, p);
                }
            }
        }
        if self.reference_runtime {
            let f = self.features(&e.symbol);
            self.symbols.get_mut(&e.symbol).unwrap().features = f;
        } else {
            self.runtime_market_event(&e);
        }
        self.update_horizons();
    }
    pub(crate) fn features(&self, symbol: &str) -> Option<Features> {
        let started = std::time::Instant::now();
        let result = self.features_inner(symbol);
        self.telemetry
            .lock()
            .unwrap()
            .record("features", started.elapsed().as_micros() as u64);
        result
    }
    fn features_inner(&self, symbol: &str) -> Option<Features> {
        let s = &self.symbols[symbol];
        let primary = &s.venues["binance"];
        let b = &primary.book;
        let (bid, ask, bq, aq) = b.top()?;
        let mid = (bid + ask) / 2.;
        let (bd, ad) = b.depth();
        let mut flags = vec![];
        if !b.ready(self.now, self.asset_config(symbol).max_stale_ms) {
            flags.push("PRIMARY_STALE_OR_GAP".into());
        }
        let other = &s.venues["okx"].book;
        let cross = if other.ready(self.now, self.asset_config(symbol).max_stale_ms) {
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
        let lead = cross.and_then(|_| {
            let at = self.now.checked_sub(1000)?;
            let p = crate::cross_venue::asof(&primary.prices, at, 750)?;
            let o = crate::cross_venue::asof(&s.venues["okx"].prices, at, 750)?;
            Some(((other.mid()? / o - 1.) - (mid / p - 1.)) * 10_000.)
        });
        let cross_lag = cross.and_then(|_| {
            crate::cross_venue::estimate(&primary.prices, &s.venues["okx"].prices, self.now)
        });
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
            cross_venue_lag: cross_lag,
            mark: primary
                .mark
                .filter(|_| self.now.saturating_sub(primary.mark_ts) < 10_000),
            index: primary
                .index
                .filter(|_| self.now.saturating_sub(primary.index_ts) < 10_000),
            funding_rate: primary
                .funding_rate
                .filter(|_| self.now.saturating_sub(primary.funding_ts) < 60_000),
            next_funding_ts: primary
                .next_funding_ts
                .filter(|_| self.now.saturating_sub(primary.funding_ts) < 60_000),
            feature_lag_ms: self.now.saturating_sub(b.exchange_ts),
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
        let c = self.asset_config(&f.symbol);
        let venue = if self.live.configured {
            self.live.venue.as_str()
        } else {
            "binance"
        };
        let assumptions = c.costs_for_venue(venue);
        let (spread_bps, funding_rate, next_funding_ts) = if venue == "okx" {
            let v = self
                .symbols
                .get(&f.symbol)
                .and_then(|s| s.venues.get(venue));
            let spread = v
                .and_then(|v| v.book.top())
                .map(|(bid, ask, _, _)| (ask - bid) / ((ask + bid) / 2.) * 10_000.)
                .unwrap_or(f.spread_bps)
                .max(f.spread_bps);
            let fresh = v.filter(|v| self.now.saturating_sub(v.funding_ts) < 60_000);
            (
                spread,
                fresh.and_then(|v| v.funding_rate),
                fresh.and_then(|v| v.next_funding_ts),
            )
        } else {
            (f.spread_bps, f.funding_rate, f.next_funding_ts)
        };
        let mut cost = Costs {
            spread: spread_bps / 10_000.,
            entry_fee: assumptions.fee_bps / 10_000.,
            exit_fee: assumptions.fee_bps / 10_000.,
            slippage: 2. * assumptions.slippage_bps / 10_000.,
            impact: assumptions.impact_bps / 10_000.,
            adverse_selection: assumptions.adverse_selection_bps / 10_000.,
            funding: match (funding_rate, next_funding_ts) {
                (Some(rate), Some(ts)) if ts > self.now => {
                    if ts <= self.now + c.horizon * 1000 {
                        rate.abs()
                    } else {
                        0.
                    }
                }
                _ => assumptions.funding_bps / 10_000.,
            },
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
        let started = std::time::Instant::now();
        let result = self.qualify_inner(p, f, h);
        self.telemetry
            .lock()
            .unwrap()
            .record("qualification", started.elapsed().as_micros() as u64);
        result
    }
    fn qualify_inner(
        &self,
        p: &Prediction,
        f: &Features,
        h: &ModelHealth,
    ) -> (Qualification, Option<TradePlan>) {
        let c = self.asset_config(&p.symbol);
        let costs = self.costs(f);
        let probs = p.probabilities();
        let win = p.win_probability();
        let loss = probs[if p.direction == "LONG" { 1 } else { 0 }];
        let gross = win * p.target - loss * p.stop;
        let net = gross - costs.total;
        let risk = self.qualification_risk(p);
        let equity = risk.equity;
        let multiplier = if h.trust == "DEGRADED" { 0.5 } else { 1. };
        let budget = (equity * c.risk_per_trade).min(risk.daily_remaining);
        let capacity = f.depth * c.depth_participation;
        let notional = (budget / (p.stop + costs.total).max(1e-9))
            .min((risk.notional_cap - risk.other_notional).max(0.))
            .min((equity * c.max_effective_leverage - risk.other_notional).max(0.))
            .min(risk.available_notional)
            .min(capacity)
            * multiplier;
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
            h.execution_drift.is_none_or(|d| d <= c.max_slippage)
                && h.live.execution_drift.is_none_or(|d| d <= c.max_slippage),
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
            risk.available && budget > 0. && notional >= 10. && buffer >= c.min_liquidation_buffer,
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
            asset_profile_id: self
                .universe
                .as_ref()
                .and_then(|u| u.profiles.get(&p.symbol))
                .map(|p| p.id.clone()),
            portfolio_decision_id: None,
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
            entry_type: if risk.live {
                "LIMIT_IOC"
            } else {
                "PAPER_MARKET_IOC"
            }
            .into(),
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
            self.daily_integrity = IntegrityCounters::default();
        }
        self.equity_peak = self.equity_peak.max(self.account_equity());
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            let symbol = symbol.as_str();
            self.evaluate_symbol(symbol, false);
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
        if self.mode != "AUTO" {
            self.admission_shadow_tick();
        }
        self.live_tick();
        self.research_tick();
        self.universe_tick();
    }
    pub(crate) fn evaluate_symbol(&mut self, symbol: &str, force: bool) {
        let config = self.asset_execution_config(symbol);
        let (blocked, protect) = self.protection_check(symbol);
        if blocked {
            return;
        }
        let force = force || protect;
        if !self.reference_runtime {
            let s = self.symbols.get_mut(symbol).unwrap();
            if let Some(mid) = s.venues["binance"].book.mid().filter(|_| {
                s.venues["binance"]
                    .book
                    .ready(self.now, config.max_stale_ms)
            }) {
                if s.chart
                    .back()
                    .is_none_or(|(ts, _)| self.now.saturating_sub(*ts) >= 1000)
                {
                    s.chart.push_back((self.now, mid));
                    while s.chart.len() > 180 {
                        s.chart.pop_front();
                    }
                }
                self.track(symbol, mid);
                self.check_position_barrier(symbol, mid);
            }
        }
        let ready = self.symbols[symbol]
            .venues
            .values()
            .all(|v| v.connected && v.book.ready(self.now, config.max_stale_ms));
        if !ready {
            self.invalidate(symbol, "stale_feed");
        }
        if !self.runtime_due(symbol, force) {
            return;
        }
        let model = self.asset_model(symbol).clone();
        let evaluation_started = std::time::Instant::now();
        let Some(f) = self.features(symbol) else {
            if self.symbols[symbol].last_result.is_none() {
                self.transition(symbol, "NO_SIGNAL", None);
            }
            return;
        };
        if ready {
            self.track(symbol, f.mid);
            self.check_position_barrier(symbol, f.mid);
        }
        let previous = self.symbols[symbol].evaluation.as_ref();
        let alpha_due = force
            || (!self.reference_runtime)
            || previous.is_none_or(|ev| {
                self.now.saturating_sub(ev.evaluated_at) >= config.alpha_eval_interval_ms
            });
        let qualification_due = force
            || previous.is_none_or(|ev| {
                self.now.saturating_sub(ev.qualification_evaluated_at)
                    >= config.qualification_eval_interval_ms
            });
        if !alpha_due && !qualification_due {
            return;
        }
        if alpha_due {
            self.record_runtime_evaluation(symbol);
            self.integrity.realtime_evaluations += 1;
            self.daily_integrity.realtime_evaluations += 1;
        }
        let comparable: Vec<_> = self
            .outcomes
            .iter()
            .filter(|o| {
                o.prediction.symbol == symbol
                    && o.prediction.model_version == model.model_version
                    && o.prediction.config_version == self.asset_config_version(symbol)
            })
            .cloned()
            .collect();
        let trades: Vec<_> = self
            .trades
            .iter()
            .filter(|t| {
                t.position.plan.symbol == symbol
                    && t.position.config == config
                    && self.outcomes.iter().any(|o| {
                        o.prediction.prediction_id == t.position.plan.prediction_id
                            && o.prediction.model_version == model.model_version
                            && o.prediction.config_version == self.asset_config_version(symbol)
                    })
            })
            .cloned()
            .collect();
        let mut health = model.health(&comparable, &trades, &config, &f);
        self.apply_live_feedback(symbol, &mut health);
        let direction = if !alpha_due {
            self.symbols[symbol].candidate_direction.as_str()
        } else if ready {
            signal_direction(
                Model::score(&f),
                &self.symbols[symbol].candidate_direction,
                &config,
            )
        } else {
            "FLAT"
        }
        .to_owned();
        {
            let s = self.symbols.get_mut(symbol).unwrap();
            if s.candidate_direction != direction {
                s.candidate_direction = direction.clone();
                s.candidate_since = self.now;
            }
            s.features = Some(f.clone());
            s.health = health.clone();
            if ready && self.reference_runtime {
                s.chart.push_back((self.now, f.mid));
                if s.chart.len() > 180 {
                    s.chart.pop_front();
                }
            }
        }
        if self.symbols[symbol].active.as_ref().is_some_and(|p| {
            direction != p.direction
                && self
                    .now
                    .saturating_sub(self.symbols[symbol].candidate_since)
                    >= config.signal_confirm_ms
        }) {
            self.finish(symbol, "SIGNAL_INVALIDATED", "confirmed_signal_change");
        }
        let (raw, cal) = if alpha_due {
            model.probabilities(&f, &config)
        } else {
            let ev = self.symbols[symbol].evaluation.as_ref().unwrap();
            (ev.raw_probabilities, ev.calibrated_probabilities)
        };
        let previous_eval = self.symbols[symbol].evaluation.clone();
        self.symbols.get_mut(symbol).unwrap().evaluation = Some(AlphaEvaluation {
            symbol: symbol.into(),
            evaluated_at: if alpha_due {
                self.now
            } else {
                previous_eval.as_ref().unwrap().evaluated_at
            },
            qualification_evaluated_at: previous_eval
                .as_ref()
                .map_or(0, |v| v.qualification_evaluated_at),
            raw_probabilities: raw,
            calibrated_probabilities: cal,
            features: f.clone(),
            qualification: previous_eval.as_ref().and_then(|v| v.qualification.clone()),
            target_position: previous_eval.and_then(|v| v.target_position),
        });
        let result_held = self.symbols[symbol].last_result.is_some()
            && self.now.saturating_sub(self.symbols[symbol].completed_at) < config.result_hold_ms;
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
            && alpha_due
            && self.symbols[symbol].consumed_signal_epoch
                != Some(self.symbols[symbol].candidate_since)
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
                >= config.signal_confirm_ms;
        if can_predict {
            let id = self.id("prediction");
            self.transition(symbol, "EMERGING", Some(&id));
            let (raw, cal) = model.probabilities(&f, &config);
            let mut p = Prediction {
                episode: EpisodeMetadata {
                    origin_trigger_id: self.symbols[symbol]
                        .runtime
                        .origin
                        .as_ref()
                        .map(|t| t.trigger_id.clone()),
                    origin_trigger_families: self.symbols[symbol]
                        .runtime
                        .origin
                        .as_ref()
                        .map(|t| t.trigger_families.clone())
                        .unwrap_or_default(),
                    episode_id: id.clone(),
                    opened_at: self.now,
                    last_price_ts: f.ts,
                    latest_evaluated_at: self.now,
                    latest_raw_probabilities: Some(raw),
                    latest_calibrated_probabilities: cal,
                    latest_feature_snapshot_id: f.feature_snapshot_id.clone(),
                    upper_barrier: f.mid
                        * (1.
                            + if direction == "LONG" {
                                config.target
                            } else {
                                config.stop
                            }),
                    lower_barrier: f.mid
                        * (1.
                            - if direction == "LONG" {
                                config.stop
                            } else {
                                config.target
                            }),
                    signal_epoch: self.symbols[symbol].candidate_since,
                    ..Default::default()
                },
                prediction_id: id.clone(),
                symbol: symbol.into(),
                ts: self.now,
                horizon: config.horizon,
                direction: direction.clone(),
                target: config.target,
                stop: config.stop,
                start_price: f.mid,
                current_price: f.mid,
                expected_move: cal.map(|v| {
                    let i = if direction == "LONG" { 0 } else { 1 };
                    v[i] * config.target - v[1 - i] * config.stop
                }),
                signal_strength: (Model::score(&f).abs() / 3.).min(1.),
                raw_probabilities: raw,
                calibrated_probabilities: cal,
                model_version: model.model_version.clone(),
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
                config_version: self.asset_config_version(symbol),
                cost_assumptions_bps: Some(config.round_trip_cost_bps()),
            };
            let (q, plan) = self.qualify(&p, &f, &health);
            p.episode.highest_qualification = q.status.clone();
            p.qualification = q;
            p.plan = plan;
            self.integrity.prediction_episodes_opened += 1;
            self.daily_integrity.prediction_episodes_opened += 1;
            if p.qualification.status == "QUALIFIED" {
                self.integrity.qualified_episodes += 1;
                self.daily_integrity.qualified_episodes += 1;
            }
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
                .filter(|_| self.shadow_asset.as_ref().is_none_or(|s| s == symbol))
                .and_then(|run| run.predict(&p, &model, &config))
            {
                self.log(
                    "shadow_prediction",
                    json!({"run_id":self.shadow.as_ref().unwrap().run_id,"prediction":pair}),
                );
            }
            let s = self.symbols.get_mut(symbol).unwrap();
            s.last_prediction_ts = self.now;
            s.consumed_signal_epoch = Some(s.candidate_since);
            s.active = Some(p);
        }
        if let Some(p) = self.symbols[symbol].active.clone() {
            self.transition(symbol, "TRACKING", Some(&p.prediction_id));
            // Requalification uses fresh features, probabilities and remaining holding time.
            let mut fresh = p.clone();
            fresh.raw_probabilities = raw;
            fresh.calibrated_probabilities = cal;
            if fresh.config_version != self.asset_config_version(symbol) {
                fresh.calibrated_probabilities = None;
            }
            let due = force
                || self.now == p.ts
                || self.symbols[symbol].evaluation.as_ref().is_none_or(|ev| {
                    self.now.saturating_sub(ev.qualification_evaluated_at)
                        >= config.qualification_eval_interval_ms
                });
            let (q, plan) = if due {
                self.qualify(&fresh, &f, &health)
            } else {
                (
                    self.symbols[symbol]
                        .qualification
                        .clone()
                        .unwrap_or_else(|| p.qualification.clone()),
                    self.symbols[symbol].plan.clone(),
                )
            };
            if q.status == "QUALIFIED" && p.episode.highest_qualification != "QUALIFIED" {
                self.integrity.qualified_episodes += 1;
                self.daily_integrity.qualified_episodes += 1;
                self.log(
                    "episode_qualification",
                    json!({"episode_id":p.prediction_id,"highest_qualification":"QUALIFIED"}),
                );
            }
            {
                let s = self.symbols.get_mut(symbol).unwrap();
                s.qualification = Some(q.clone());
                s.plan = plan.clone();
                let ev = s.evaluation.as_mut().unwrap();
                if due {
                    ev.qualification_evaluated_at = self.now;
                }
                ev.qualification = Some(q.clone());
                ev.target_position = plan.clone();
                let episode = &mut s.active.as_mut().unwrap().episode;
                episode.latest_evaluated_at = ev.evaluated_at;
                episode.latest_raw_probabilities = Some(raw);
                episode.latest_calibrated_probabilities = cal;
                episode.latest_feature_snapshot_id = f.feature_snapshot_id.clone();
                if q.status == "QUALIFIED" {
                    episode.highest_qualification = q.status.clone();
                }
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
            if let Some(ev) = s.evaluation.as_mut() {
                ev.qualification = None;
                ev.target_position = None;
                ev.qualification_evaluated_at = self.now;
            }
            if self.positions.contains_key(symbol) {
                self.close_position(symbol, "PREDICTION_COMPLETED");
            }
        }
        self.telemetry.lock().unwrap().record(
            "full_alpha_evaluation",
            evaluation_started.elapsed().as_micros() as u64,
        );
        self.sync_runtime(symbol);
        if protect {
            self.protection_result(symbol);
        }
    }
    pub(crate) fn invalidate(&mut self, symbol: &str, reason: &str) {
        if self
            .universe
            .as_ref()
            .and_then(|u| u.profiles.get(symbol))
            .is_some_and(|p| p.lifecycle_state == "ELIGIBLE")
        {
            self.transition_asset(symbol, "SUSPENDED", reason, "SYSTEM", json!({}));
        }
        if self.symbols[symbol].active.is_some() {
            let kind = if ["model_changed", "config_changed"].contains(&reason) {
                "SIGNAL_INVALIDATED"
            } else {
                "DATA_INVALIDATED"
            };
            self.finish(symbol, kind, reason);
        }
        if self.positions.contains_key(symbol) {
            self.close_position(symbol, "DATA_INVALID_EXIT");
        }
    }
    fn track(&mut self, symbol: &str, price: f64) {
        let price_ts = self.symbols[symbol].venues["binance"].book.receive_ts;
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
            p.episode.last_price_ts = price_ts;
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
        p.episode.closed_at = Some(self.now);
        p.episode.close_reason = Some(if reason.is_empty() {
            kind.into()
        } else {
            format!("{kind}:{reason}")
        });
        self.integrity.prediction_episodes_closed += 1;
        self.daily_integrity.prediction_episodes_closed += 1;
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
        if class != 2 {
            p.episode.first_hit = Some(if class == 0 { "UPPER" } else { "LOWER" }.into());
            p.episode.first_hit_at = Some(self.now);
        }
        let brier = p
            .probabilities()
            .iter()
            .enumerate()
            .map(|(i, v)| (v - if i == class { 1. } else { 0. }).powi(2))
            .sum();
        let timeout = kind.starts_with("TIMEOUT");
        let horizon_observation = self.watch_horizon(&p, kind);
        let o = Outcome {
            horizon_observation,
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
            learning_eligible: !kind.ends_with("INVALIDATED"),
            annotation: None,
            prediction_error: if kind.ends_with("INVALIDATED") {
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
        if let Some(u) = &mut self.universe {
            if o.learning_eligible {
                *u.outcome_counts
                    .entry(format!(
                        "{}:{}:{}",
                        o.prediction.symbol,
                        o.prediction.model_version,
                        o.prediction.config_version
                    ))
                    .or_default() += 1;
            }
        }
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
        let venue = self
            .positions
            .get(symbol)
            .and_then(|p| p.execution_venue.as_deref())
            .unwrap_or_else(|| self.cost_venue());
        let b = &self.symbols[symbol].venues[venue].book;
        if !b.ready(self.now, config.max_stale_ms) {
            return None;
        }
        let mid = b.mid()?;
        let levels: Vec<_> = if buy {
            b.asks
                .iter()
                .map(|(p, q)| (*p as f64 / b.scale(), *q))
                .collect()
        } else {
            b.bids
                .iter()
                .rev()
                .map(|(p, q)| (*p as f64 / b.scale(), *q))
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
    fn open_position(&mut self, mut plan: TradePlan, q: &Qualification) {
        if self.apply_portfolio(&mut plan, false, None).is_err() {
            return;
        }
        if !self.risk_available()
            || q.status != "QUALIFIED"
            || self.positions.contains_key(&plan.symbol)
        {
            return;
        }
        let symbol = plan.symbol.clone();
        let mid = self.symbols[&symbol].features.as_ref().unwrap().mid;
        let config = self.asset_execution_config(&symbol);
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
            execution_venue: self.universe.as_ref().map(|_| self.cost_venue().to_owned()),
            config,
            expected_cost: basis * q.costs.total,
            expected_edge: q.net_edge,
            cost_basis: basis,
            alpha_ids: vec![plan.prediction_id.clone()],
            last_funding_ts: self.now,
            funding_observed: self.symbols[&symbol]
                .features
                .as_ref()
                .is_some_and(|f| f.next_funding_ts.is_some() && f.funding_rate.is_some()),
        };
        self.log("qualification", json!(q));
        self.log("plan", json!(plan));
        self.log(
            "paper_entry",
            json!({"prediction_id":plan.prediction_id,"symbol":plan.symbol}),
        );
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
        let config = p.config.clone();
        if p.action == "EXIT_PENDING" {
            return;
        }
        if self.kill_switch {
            if config.kill_behavior == "flatten" {
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
            self.now.saturating_sub(p.last_rebalance) >= config.rebalance_interval_secs * 1000;
        let reduce = target < current * (1. - config.position_deadband) && due;
        let fresh_since = self.symbols[symbol].candidate_since;
        let alpha_id = format!("alpha-{symbol}-{fresh_since}");
        let add = self.mode == "PAPER"
            && due
            && target > current * (1. + config.position_deadband)
            && fresh_since > p.ts
            && self.now.saturating_sub(fresh_since) >= config.signal_confirm_ms
            && self.symbols[symbol].candidate_direction == p.plan.direction
            && !p.alpha_ids.contains(&alpha_id)
            && p.adjustments < config.max_rebalances
            && q.net_edge >= config.incremental_ev
            && (p.current_price - p.entry_price) * prediction.sign() >= 0.;
        if add {
            self.add_position(symbol, target - current, q, &alpha_id);
            return;
        }
        let p = self.positions.get_mut(symbol).unwrap();
        p.target_notional = target;
        p.action = "HOLD".into();
        p.reason = "持续复核；只有重新形成的独立信号才可加仓".into();
        if reduce {
            if p.adjustments >= config.max_rebalances {
                self.close_position(symbol, "REBALANCE_LIMIT");
            } else {
                self.reduce_position(symbol, (current - target) / current, "TARGET_RISK_REDUCED");
            }
        }
    }
    fn settle_funding(&mut self) {
        let mut records = vec![];
        for (symbol, p) in &mut self.positions {
            let v = &self.symbols[symbol].venues[p.execution_venue.as_deref().unwrap_or("binance")];
            if let (Some(ts), Some(rate), Some(mark)) = (v.next_funding_ts, v.funding_rate, v.mark)
            {
                if ts > p.ts && ts > p.last_funding_ts && ts <= self.now {
                    // Paper settlement uses the last announced rate, never pretends to be an exchange debit.
                    let fresh = self.now.saturating_sub(v.funding_ts) <= 60_000;
                    let debit = p.qty
                        * mark
                        * if fresh {
                            rate * if p.plan.direction == "LONG" { 1. } else { -1. }
                        } else {
                            p.config.funding_bps / 10_000.
                        };
                    p.funding += debit;
                    p.last_funding_ts = ts;
                    p.funding_observed = true;
                    records.push(json!({"symbol":symbol,"settlement_ts":ts,"debit":debit,"rate":rate,"estimated":true,"stale_fallback":!fresh}));
                }
            }
        }
        for r in records {
            self.log("paper_funding", r);
        }
    }
    fn add_position(&mut self, symbol: &str, requested: f64, q: &Qualification, alpha_id: &str) {
        if !self.risk_available() || q.status != "QUALIFIED" {
            return;
        }
        let p = self.positions[symbol].clone();
        let mut target = p.plan.clone();
        target.position_notional = p.qty * p.current_price + requested;
        target.planned_max_loss = target.position_notional
            * (q.costs.total + ((p.current_price - p.plan.sl) / p.current_price).abs());
        if self.apply_portfolio(&mut target, false, None).is_err() {
            return;
        }
        let requested = requested.min((target.position_notional - p.qty * p.current_price).max(0.));
        let sign = if p.plan.direction == "LONG" { 1. } else { -1. };
        let mid = p.current_price;
        // Keep the original absolute stop and deadline; never widen risk to accommodate an add.
        let stop_distance = ((mid - p.plan.sl) * sign / mid).max(0.);
        let existing_risk =
            ((p.entry_price - p.plan.sl) * sign).max(0.) * p.qty + p.qty * mid * q.costs.total;
        let budget = (self.account_equity() * p.config.risk_per_trade).min(p.plan.planned_max_loss);
        let allowed = ((budget - existing_risk).max(0.)
            / (stop_distance + q.costs.total).max(1e-9))
        .min(requested);
        if allowed < 10. || allowed * q.net_edge < p.config.min_net_pnl {
            return;
        }
        let config = p.config.clone();
        let Some(fill) = self.fill(
            symbol,
            &p.plan.plan_id,
            sign > 0.,
            allowed / (mid * (1. + q.costs.total)),
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
            return;
        }
        let basis = fill.qty * fill.fill_price;
        let pos = self.positions.get_mut(symbol).unwrap();
        pos.entry_price = (pos.entry_price * pos.qty + basis) / (pos.qty + fill.qty);
        pos.qty += fill.qty;
        pos.fees += fill.fee;
        pos.expected_edge =
            (pos.expected_edge * pos.cost_basis + basis * q.net_edge) / (pos.cost_basis + basis);
        pos.cost_basis += basis;
        pos.expected_cost += basis * q.costs.total;
        pos.last_rebalance = self.now;
        pos.adjustments += 1;
        pos.action = "ADD".into();
        pos.reason = "独立信号重新确认，增量 EV 与原止损风险预算通过".into();
        pos.alpha_ids.push(alpha_id.into());
        pos.executions.push(fill.clone());
        self.log("independent_alpha", json!({"alpha_id":alpha_id,"symbol":symbol,"feature_snapshot_id":self.symbols[symbol].features.as_ref().map(|f|&f.feature_snapshot_id),"qualification":q,"amount":basis}));
        self.log("execution", json!(fill));
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
            if !p.funding_observed {
                p.funding += p.cost_basis * p.config.funding_bps / 10_000.;
            }
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
            self.integrity.paper_trades += 1;
            self.daily_integrity.paper_trades += 1;
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
    pub(crate) fn pause_shadow(&mut self, status: &str) {
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
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            let symbol = symbol.as_str();
            self.invalidate(symbol, "model_changed");
        }
        for s in self.symbols.values_mut() {
            s.plan = None;
            s.qualification = None;
            s.evaluation = None;
            s.candidate_direction = "FLAT".into();
            s.candidate_since = self.now;
            s.health = model::health(&[], &[], &self.config, false);
        }
    }
    pub fn shadow_view(&self) -> Option<Value> {
        let run = self.shadow.as_ref()?;
        let champion = self
            .shadow_asset
            .as_ref()
            .map(|s| self.asset_model(s))
            .unwrap_or(&self.model);
        let config = self
            .shadow_asset
            .as_ref()
            .map(|s| self.asset_execution_config(s))
            .unwrap_or_else(|| self.config.resolved_costs(self.cost_venue()));
        let comparison = run.comparison(
            champion,
            self.shadow_asset
                .as_ref()
                .map_or(self.config_version, |s| self.asset_config_version(s)),
            self.now,
        );
        let market_ready = self
            .symbols
            .iter()
            .filter(|(symbol, _)| self.shadow_asset.as_ref().is_none_or(|a| a == *symbol))
            .all(|(symbol, s)| {
                let stale_ms = self.asset_config(symbol).max_stale_ms;
                s.venues
                    .values()
                    .all(|v| v.connected && v.book.ready(self.now, stale_ms))
                    && s.features.as_ref().is_some_and(|f| {
                        self.now.saturating_sub(f.ts) <= stale_ms
                            && run.candidate.probabilities(f, &config).1.is_some()
                            && run
                                .candidate
                                .distribution_score(f)
                                .zip(run.candidate.distributions.get(&f.symbol))
                                .is_some_and(|(z, d)| z <= d.max_z)
                    })
            });
        let operational_checks = json!({"observe_mode":self.mode == "OBSERVE", "no_positions":self.positions.is_empty() && !self.live.orders.values().any(|o|o.pending()) && self.live.account.as_ref().is_none_or(|a|a.positions.values().all(|p|p.quantity.abs()<1e-10)),
            "storage_healthy":self.storage_healthy,"kill_released":!self.kill_switch,"market_ready":market_ready});
        let ready = comparison["ready"] == true
            && operational_checks
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == true);
        Some(
            json!({"asset":self.shadow_asset,"run_id":run.run_id,"status":run.status,"started_at":run.started_at,"config_version":run.config_version,
            "champion_version":run.champion_version,"candidate_version":run.candidate.model_version,"candidate_hash":run.candidate.artifact_hash,
            "policy":run.policy,"offline_status":run.offline_report["status"],"comparison":comparison,
            "operational_checks":operational_checks,"promotion_ready":ready,
            "recent_outcomes":run.outcomes.iter().rev().take(20).collect::<Vec<_>>()}),
        )
    }
    pub fn view(&self) -> Value {
        let symbols:BTreeMap<_,_>=self.symbols.iter().map(|(name,s)|{
            let stale_ms = self.asset_config(name).max_stale_ms;
            let venues:BTreeMap<_,_>=s.venues.iter().map(|(name,v)|(name,json!({"connected":v.connected,"ready":v.book.ready(self.now,stale_ms),
                "age_ms":self.now.saturating_sub(v.book.receive_ts),"exchange_age_ms":self.now.abs_diff(v.book.exchange_ts),"gaps":v.book.gaps,"reconnects":v.reconnects,"error":v.error,"mid":v.book.mid(),"sequence":v.book.sequence,"mark":v.mark.filter(|_|self.now.saturating_sub(v.mark_ts)<10_000),"index":v.index.filter(|_|self.now.saturating_sub(v.index_ts)<10_000),"funding_rate":v.funding_rate.filter(|_|self.now.saturating_sub(v.funding_ts)<60_000),"next_funding_ts":v.next_funding_ts}))).collect();
            (name,json!({"config":self.asset_config(name),"config_version":self.asset_config_version(name),"model":self.asset_model(name),"runtime_state":s.runtime.state,"pulse":s.runtime.pulse.snapshot,"trigger_decision":s.runtime.decision,"trigger_origin":s.runtime.origin,"full_evaluations":s.runtime.full_evaluations,"venues":venues,"features":s.features,"active":s.active,"qualification":s.qualification,"plan":s.plan,"health":s.health,"chart":s.chart,"lifecycle":s.lifecycle,"lifecycle_since":s.lifecycle_since,"signal_direction":s.candidate_direction,"last_result":s.last_result,"evaluation":s.evaluation}))
        }).collect();
        json!({"universe":self.universe,"portfolio_exposure":self.portfolio_snapshot(self.portfolio_is_live()),"trigger_config":self.trigger_config,"trigger_config_version":self.trigger_config_version,"trigger_analytics":self.trigger_analytics.report(),"source":self.source,"mode":self.mode,"now":self.now,"config":self.config,"config_version":self.config_version,"model":self.model,
            "shadow":self.shadow_view(),"rollback_available":self.previous_model.is_some(),"shadow_policy_defaults":ShadowPolicy::default(),
            "kill_switch":self.kill_switch,"storage_healthy":self.storage_healthy,"symbols":symbols,"positions":self.positions,
            "outcomes":self.outcomes.iter().rev().take(100).collect::<Vec<_>>(),"trades":self.trades.iter().rev().take(100).collect::<Vec<_>>(),
            "audit":self.audit.iter().rev().take(30).collect::<Vec<_>>(),"account":{"equity":self.account_equity(),"realized_pnl":self.realized_pnl,"daily_pnl":self.daily_pnl,"peak":self.equity_peak},
            "integrity":self.integrity,"daily_integrity":self.daily_integrity,"admission":self.admission,
            "telemetry":self.telemetry.lock().unwrap().view(),"execution_metrics":self.execution_metrics(),"research":self.research,"live":self.live,"live_positions":self.live_positions_view(),"live_enabled":self.live_ready().is_ok(),"auto_evidence_ready":self.auto_evidence(),"live_gate_reason":self.live_ready().err()})
    }
}
pub fn unrealized(p: &Position) -> f64 {
    (p.current_price - p.entry_price) * p.qty * if p.plan.direction == "LONG" { 1. } else { -1. }
        + p.realized_pnl
        - p.fees
        - p.funding
}

#[cfg(test)]
mod position_tests {
    use super::*;
    use crate::feeds::demo_events;
    fn opened() -> (Engine, Prediction, Qualification, TradePlan) {
        let mut e = Engine::new_reference(true);
        for i in 0..8 {
            for ev in demo_events(i, 1_000_000 + i * 1000) {
                e.ingest(ev);
            }
        }
        e.mode = "PAPER".into();
        let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
        p.direction = "LONG".into();
        p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        let mut h = e.symbols["BTCUSDT"].health.clone();
        h.trust = "NORMAL".into();
        h.samples = 100;
        h.calibration_error = 0.;
        let (q, plan) = e.qualify(&p, e.symbols["BTCUSDT"].features.as_ref().unwrap(), &h);
        let plan = plan.unwrap();
        let mut small = plan.clone();
        small.position_notional *= 0.2;
        e.open_position(small, &q);
        (e, p, q, plan)
    }
    #[test]
    fn independent_add_once_no_loss_trigger_and_original_stop_unchanged() {
        let (mut e, p, q, plan) = opened();
        let symbol = "BTCUSDT";
        e.now += 20_000;
        for v in e.symbols.get_mut(symbol).unwrap().venues.values_mut() {
            v.book.receive_ts = e.now;
            v.book.exchange_ts = e.now;
        }
        let pos = e.positions.get_mut(symbol).unwrap();
        pos.current_price = pos.entry_price * 1.00001;
        let old_qty = pos.qty;
        let old_sl = pos.plan.sl;
        let old_ts = pos.ts;
        // A larger target using the entry signal cannot add.
        e.rebalance(symbol, &p, &q, Some(&plan));
        assert_eq!(e.positions[symbol].qty, old_qty);
        e.symbols.get_mut(symbol).unwrap().candidate_since = e.now - 3000;
        e.symbols.get_mut(symbol).unwrap().candidate_direction = "LONG".into();
        e.rebalance(symbol, &p, &q, Some(&plan));
        let qty = e.positions[symbol].qty;
        assert!(qty > old_qty);
        assert_eq!(e.positions[symbol].plan.sl, old_sl);
        assert_eq!(e.positions[symbol].ts, old_ts);
        e.now += 20_000;
        e.rebalance(symbol, &p, &q, Some(&plan));
        assert_eq!(e.positions[symbol].qty, qty);
        e.symbols.get_mut(symbol).unwrap().candidate_since = e.now - 3000;
        e.positions.get_mut(symbol).unwrap().current_price *= 0.999;
        e.rebalance(symbol, &p, &q, Some(&plan));
        assert_eq!(e.positions[symbol].qty, qty);
    }
    #[test]
    fn paper_uses_frozen_asset_rebalance_limits_and_venue_fees() {
        let (mut e, p, q, plan) = opened();
        e.initialize_universe();
        let mut c = e.config.clone();
        c.rebalance_interval_secs = 120;
        c.venue_costs.insert(
            "binance".into(),
            VenueCosts {
                fee_bps: 8.,
                slippage_bps: 3.,
                impact_bps: 2.,
                adverse_selection_bps: 1.,
                funding_bps: 0.7,
            },
        );
        e.universe
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("BTCUSDT")
            .unwrap()
            .config = Some(c);
        let resolved = e.asset_execution_config("BTCUSDT");
        e.positions.get_mut("BTCUSDT").unwrap().config = resolved.clone();
        e.now += 20_000;
        for v in e.symbols.get_mut("BTCUSDT").unwrap().venues.values_mut() {
            v.book.receive_ts = e.now;
            v.book.exchange_ts = e.now;
        }
        let pos = e.positions.get_mut("BTCUSDT").unwrap();
        pos.current_price = pos.entry_price * 1.00001;
        let qty = pos.qty;
        e.symbols.get_mut("BTCUSDT").unwrap().candidate_since = e.now - 3000;
        e.rebalance("BTCUSDT", &p, &q, Some(&plan));
        assert_eq!(e.positions["BTCUSDT"].qty, qty);
        let fill = e
            .fill("BTCUSDT", "venue-test", true, 0.001, true, &resolved)
            .unwrap();
        assert!((fill.fee / (fill.qty * fill.fill_price) - 0.0008).abs() < 1e-12);
    }
    #[test]
    fn independent_asset_paper_fills_remain_in_model_feedback() {
        let (mut e, _, _, _) = opened();
        e.initialize_universe();
        let mut config = e.config.clone();
        config.fee_bps = 8.;
        e.universe
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("BTCUSDT")
            .unwrap()
            .config = Some(config.clone());
        e.positions.get_mut("BTCUSDT").unwrap().config = config;
        e.close_position("BTCUSDT", "TEST_CLOSE");
        assert_eq!(e.trades.len(), 1);
        e.mode = "OBSERVE".into();
        for step in 8..120 {
            for event in demo_events(step, 1_000_000 + step * 1000) {
                e.ingest(event);
            }
        }
        assert_eq!(e.symbols["BTCUSDT"].health.paper_samples, 1);
    }
    #[test]
    fn funding_settles_once_with_direction_and_boundary() {
        let (mut e, _, _, _) = opened();
        let symbol = "BTCUSDT";
        let v = e
            .symbols
            .get_mut(symbol)
            .unwrap()
            .venues
            .get_mut("binance")
            .unwrap();
        v.funding_rate = Some(0.0001);
        v.next_funding_ts = Some(e.now + 1000);
        v.funding_ts = e.now;
        v.mark = Some(100.);
        let qty = e.positions[symbol].qty;
        e.settle_funding();
        assert_eq!(e.positions[symbol].funding, 0.);
        e.now += 1000;
        e.settle_funding();
        assert!((e.positions[symbol].funding - qty * 0.01).abs() < 1e-10);
        e.settle_funding();
        assert!((e.positions[symbol].funding - qty * 0.01).abs() < 1e-10);
    }
}
