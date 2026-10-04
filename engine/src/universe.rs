//! Dynamic universe is research infrastructure. No scanner score authorizes an order.
use crate::{
    core::{Engine, SymbolState},
    market_pulse::TriggerConfig,
    model::Model,
    types::Config,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub fn normalize(input: &str) -> Result<String, String> {
    let s = input
        .trim()
        .to_ascii_uppercase()
        .replace("-USDT-SWAP", "USDT")
        .replace("/USDT", "USDT");
    if !(5..=28).contains(&s.chars().count())
        || !s.ends_with("USDT")
        || !s.chars().all(char::is_alphanumeric)
    {
        return Err("只支持交易所确认的 USDT 永续合约，例如 SOLUSDT".into());
    }
    Ok(s)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UniverseConfig {
    pub enabled: bool,
    pub dry_run: bool,
    pub interval_secs: u64,
    pub candidate_l2_slots: usize,
    pub minimum_clean_hours: u64,
    pub minimum_outcomes: usize,
    pub persistence: u32,
    pub min_quote_volume: f64,
    pub max_spread_bps: f64,
    pub min_depth: f64,
    pub max_slippage_bps: f64,
    pub max_volatility: f64,
    pub max_tail_gap: f64,
}
impl Default for UniverseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dry_run: true,
            interval_secs: 300,
            candidate_l2_slots: 12,
            minimum_clean_hours: 24,
            minimum_outcomes: 2000,
            persistence: 3,
            min_quote_volume: 10_000_000.,
            max_spread_bps: 10.,
            min_depth: 20_000.,
            max_slippage_bps: 10.,
            max_volatility: 0.01,
            max_tail_gap: 0.03,
        }
    }
}
impl UniverseConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(60..=86400).contains(&self.interval_secs)
            || !(2..=24).contains(&self.candidate_l2_slots)
            || !(1..=720).contains(&self.minimum_clean_hours)
            || self.minimum_outcomes < 100
            || self.persistence < 2
            || self.persistence > 20
            || [
                self.min_quote_volume,
                self.max_spread_bps,
                self.min_depth,
                self.max_slippage_bps,
                self.max_volatility,
                self.max_tail_gap,
            ]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.)
        {
            return Err(
                "扫描配置范围无效；最短 60 秒、2–24 个采集槽位、至少 1 小时/100 条结果".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instrument {
    pub venue: String,
    pub symbol: String,
    pub instrument_id: String,
    pub contract_type: String,
    pub active: bool,
    pub listed_at: u64,
    pub contract_value: f64,
    pub observed_at: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub quote_volume: Option<f64>,
    pub trade_count: Option<u64>,
    pub median_spread_bps: Option<f64>,
    pub tail_spread_bps: Option<f64>,
    pub depth: Option<f64>,
    pub slippage_bps: Option<f64>,
    pub volatility: Option<f64>,
    pub tail_gap: Option<f64>,
    pub clean_since: Option<u64>,
    pub resets: u64,
    #[serde(default)]
    pub resets_last_hour: Option<u64>,
    pub samples: usize,
    pub funding_rate: Option<f64>,
    pub oi: Option<f64>,
    pub liquidation_flow: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub state: String,
    pub evidence: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetQualification {
    pub id: String,
    pub asset_key: String,
    pub rule_version: u64,
    pub evaluated_at: u64,
    pub overall_state: String,
    pub components: BTreeMap<String, Component>,
    pub blocking_reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub research_priority: String,
    pub next_action: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub asset_key: String,
    pub instruments: Vec<Instrument>,
    pub metrics: Metrics,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScannerRun {
    pub id: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub status: String,
    pub config_version: u64,
    pub target: Option<String>,
    pub considered: usize,
    pub errors: BTreeMap<String, String>,
    pub observations: Vec<Observation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetTriggerProfile {
    pub id: String,
    pub asset_key: String,
    pub profile_version: u64,
    pub config: TriggerConfig,
    pub metric_distribution_version: String,
    pub calibrated: bool,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetProfile {
    pub id: String,
    pub asset_key: String,
    pub profile_version: u64,
    pub lifecycle_state: String,
    pub contract_type: String,
    pub quote_asset: String,
    pub supported_exchanges: Vec<String>,
    pub feature_schema_version: String,
    pub feature_normalization_version: String,
    pub trigger: AssetTriggerProfile,
    pub mode_permissions: Vec<String>,
    pub model: Option<Model>,
    #[serde(default)]
    pub previous_model: Option<Model>,
    pub calibration_artifact_id: Option<String>,
    pub cost_artifact_id: Option<String>,
    #[serde(default)]
    pub liquidity_artifact_id: Option<String>,
    pub risk_artifact_id: Option<String>,
    pub config: Option<Config>,
    #[serde(default)]
    pub config_version: Option<u64>,
    pub candidate_horizons: Vec<u64>,
    pub candidate_targets: Vec<f64>,
    pub candidate_stops: Vec<f64>,
    pub collecting: bool,
    pub clean_since: Option<u64>,
    pub last_health_check: u64,
    #[serde(default)]
    pub last_reset_count: Option<u64>,
    #[serde(default)]
    pub reset_history: std::collections::VecDeque<(u64, u64)>,
    pub good_scans: u32,
    pub bad_scans: u32,
    pub qualification: Option<AssetQualification>,
    pub observation: Option<Observation>,
    pub created_at: u64,
    pub updated_at: u64,
}
impl AssetProfile {
    fn new(symbol: &str, now: u64, config: &Config, trigger: &TriggerConfig) -> Self {
        Self {
            id: format!("asset-{symbol}-1"),
            asset_key: symbol.into(),
            profile_version: 1,
            lifecycle_state: "DISCOVERED".into(),
            contract_type: "PERPETUAL".into(),
            quote_asset: "USDT".into(),
            supported_exchanges: vec![],
            feature_schema_version: "micro-v1".into(),
            feature_normalization_version: "robust_z_v1".into(),
            trigger: AssetTriggerProfile {
                id: format!("trigger-profile-{symbol}-1"),
                asset_key: symbol.into(),
                profile_version: 1,
                config: trigger.clone(),
                metric_distribution_version: "warmup-unvalidated".into(),
                calibrated: false,
                created_at: now,
            },
            mode_permissions: vec!["OBSERVE".into()],
            model: None,
            previous_model: None,
            calibration_artifact_id: None,
            cost_artifact_id: None,
            liquidity_artifact_id: None,
            risk_artifact_id: None,
            config: None,
            config_version: None,
            candidate_horizons: vec![config.horizon],
            candidate_targets: vec![config.target],
            candidate_stops: vec![config.stop],
            collecting: false,
            clean_since: None,
            last_health_check: 0,
            last_reset_count: None,
            reset_history: Default::default(),
            good_scans: 0,
            bad_scans: 0,
            qualification: None,
            observation: None,
            created_at: now,
            updated_at: now,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transition {
    pub id: String,
    pub asset_key: String,
    pub from_state: String,
    pub to_state: String,
    pub reason_code: String,
    pub evidence: Value,
    #[serde(default)]
    pub rule_version: u64,
    #[serde(default)]
    pub config_version: u64,
    pub initiated_by: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Universe {
    #[serde(default)]
    pub outcome_counts: BTreeMap<String, usize>,
    pub config: UniverseConfig,
    pub config_version: u64,
    pub frozen: bool,
    pub requested: bool,
    pub target: Option<String>,
    pub next_run: u64,
    pub running: Option<String>,
    pub last_run: Option<ScannerRun>,
    pub profiles: BTreeMap<String, AssetProfile>,
    pub instruments: BTreeMap<String, Instrument>,
    pub transitions: Vec<Transition>,
    #[serde(default)]
    pub spread_samples: BTreeMap<String, std::collections::VecDeque<(u64, f64)>>,
    pub portfolio: crate::portfolio::PortfolioConfig,
    pub portfolio_latest: BTreeMap<String, crate::portfolio::PortfolioDecision>,
}
pub fn qualify(o: &Observation, c: &UniverseConfig, version: u64, now: u64) -> AssetQualification {
    let mut parts: BTreeMap<String, Component> = BTreeMap::new();
    let mut add = |key: &str, value: Option<f64>, pass: bool, hard: bool| {
        parts.insert(
            key.into(),
            Component {
                state: if value.is_none() {
                    "UNKNOWN"
                } else if pass {
                    "PASS"
                } else if hard {
                    "FAIL"
                } else {
                    "WARN"
                }
                .into(),
                evidence: json!(value),
            },
        );
    };
    add(
        "volume",
        o.metrics.quote_volume,
        o.metrics
            .quote_volume
            .is_some_and(|x| x >= c.min_quote_volume),
        false,
    );
    add(
        "spread",
        o.metrics.tail_spread_bps,
        o.metrics
            .tail_spread_bps
            .is_some_and(|x| x <= c.max_spread_bps),
        true,
    );
    add(
        "depth",
        o.metrics.depth,
        o.metrics.depth.is_some_and(|x| x >= c.min_depth),
        true,
    );
    add(
        "slippage",
        o.metrics.slippage_bps,
        o.metrics
            .slippage_bps
            .is_some_and(|x| x <= c.max_slippage_bps),
        true,
    );
    add(
        "volatility",
        o.metrics.volatility,
        o.metrics.volatility.is_some_and(|x| x <= c.max_volatility),
        true,
    );
    add(
        "tail_gap",
        o.metrics.tail_gap,
        o.metrics.tail_gap.is_some_and(|x| x <= c.max_tail_gap),
        true,
    );
    let new_listing = o
        .instruments
        .iter()
        .any(|i| i.listed_at > 0 && now.saturating_sub(i.listed_at) < 7 * 86_400_000);
    let required_clean_hours = c.minimum_clean_hours * if new_listing { 2 } else { 1 };
    add(
        "continuity",
        o.metrics.clean_since.map(|x| now.saturating_sub(x) as f64),
        o.metrics
            .clean_since
            .is_some_and(|x| now.saturating_sub(x) >= required_clean_hours * 3600000),
        false,
    );
    // Fixed, versioned personal-use policy: >3 resets/reconnects in one hour is a hard failure.
    add(
        "book_stability",
        o.metrics.resets_last_hour.map(|x| x as f64),
        o.metrics.resets_last_hour.is_some_and(|x| x <= 3),
        true,
    );
    let active = o.instruments.iter().filter(|x| x.active).count();
    add("exchange_available", Some(active as f64), active > 0, true);
    add("cross_venue", Some(active as f64), active >= 2, false);
    let blocking: Vec<_> = parts
        .iter()
        .filter(|(_, c)| c.state == "FAIL")
        .map(|(k, _)| k.clone())
        .collect();
    let warnings: Vec<_> = parts
        .iter()
        .filter(|(_, c)| ["WARN", "UNKNOWN"].contains(&c.state.as_str()))
        .map(|(k, _)| k.clone())
        .collect();
    let priority = if !blocking.is_empty() {
        "REJECT"
    } else if o
        .metrics
        .quote_volume
        .is_some_and(|x| x >= c.min_quote_volume * 10.)
        && active >= 2
    {
        "HIGH"
    } else if o
        .metrics
        .quote_volume
        .is_some_and(|x| x >= c.min_quote_volume)
    {
        "MEDIUM"
    } else {
        "LOW"
    };
    AssetQualification {
        id: format!("aq-{}-{now}-{version}", o.asset_key),
        asset_key: o.asset_key.clone(),
        rule_version: version,
        evaluated_at: now,
        overall_state: if !blocking.is_empty() {
            "FAIL"
        } else if warnings.is_empty() {
            "PASS"
        } else {
            "CONDITIONAL"
        }
        .into(),
        components: parts,
        blocking_reasons: blocking,
        warnings,
        research_priority: priority.into(),
        next_action: if priority == "REJECT" {
            "修复阻断项后重新评估"
        } else {
            "采集 → 校准 → OBSERVE；研究排名不代表交易资格"
        }
        .into(),
    }
}
impl Engine {
    pub fn initialize_universe(&mut self) {
        if self.universe.is_some() {
            return;
        }
        let mut u = Universe {
            outcome_counts: BTreeMap::new(),
            config: Default::default(),
            config_version: 1,
            frozen: false,
            requested: false,
            target: None,
            next_run: 0,
            running: None,
            last_run: None,
            profiles: BTreeMap::new(),
            instruments: BTreeMap::new(),
            transitions: vec![],
            spread_samples: BTreeMap::new(),
            portfolio: Default::default(),
            portfolio_latest: BTreeMap::new(),
        };
        for symbol in self.symbols.keys() {
            let mut p = AssetProfile::new(symbol, self.now, &self.config, &self.trigger_config);
            p.lifecycle_state = "OBSERVE".into();
            p.collecting = true;
            p.supported_exchanges = vec!["binance".into(), "okx".into()];
            // Existing model stays available for research; migration never grants live permission.
            p.model = Some(self.model.clone());
            p.mode_permissions.push("PAPER".into());
            u.profiles.insert(symbol.clone(), p);
        }
        for o in &self.outcomes {
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
        self.universe = Some(u);
        for symbol in self.symbols.keys().cloned().collect::<Vec<_>>() {
            self.save_profile(&symbol);
        }
        self.log("universe_migration",json!({"version":"v1.1","scanner_enabled":false,"legacy_state":"OBSERVE","history_preserved":true}));
    }
    pub fn asset_config(&self, symbol: &str) -> &Config {
        self.universe
            .as_ref()
            .and_then(|u| u.profiles.get(symbol))
            .and_then(|p| p.config.as_ref())
            .unwrap_or(&self.config)
    }
    pub fn asset_config_version(&self, symbol: &str) -> u64 {
        self.universe
            .as_ref()
            .and_then(|u| u.profiles.get(symbol))
            .and_then(|p| p.config_version)
            .unwrap_or(0)
            .max(self.config_version)
    }
    pub(crate) fn next_config_version(&self) -> u64 {
        self.universe
            .as_ref()
            .map(|u| {
                u.profiles
                    .values()
                    .filter_map(|p| p.config_version)
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
            .max(self.config_version)
            + 1
    }
    pub fn cost_venue(&self) -> &str {
        if self.live.configured {
            &self.live.venue
        } else {
            "binance"
        }
    }
    pub fn asset_execution_config(&self, symbol: &str) -> Config {
        self.asset_config(symbol).resolved_costs(self.cost_venue())
    }
    pub(crate) fn sync_shared_model_profiles(&mut self) {
        let symbols: Vec<_> = self
            .universe
            .as_ref()
            .map(|u| {
                u.profiles
                    .iter()
                    .filter(|(s, p)| {
                        ["BTCUSDT", "ETHUSDT"].contains(&s.as_str()) && p.config.is_none()
                    })
                    .map(|(s, _)| s.clone())
                    .collect()
            })
            .unwrap_or_default();
        for symbol in symbols {
            let p = self
                .universe
                .as_mut()
                .unwrap()
                .profiles
                .get_mut(&symbol)
                .unwrap();
            p.previous_model = p.model.replace(self.model.clone());
            p.calibration_artifact_id = self
                .model
                .validated
                .then(|| self.model.calibration_version.clone());
            p.feature_normalization_version = self.model.artifact_hash.clone();
            self.save_profile(&symbol);
        }
    }
    pub fn asset_model(&self, symbol: &str) -> &Model {
        self.universe
            .as_ref()
            .and_then(|u| u.profiles.get(symbol))
            .and_then(|p| p.model.as_ref())
            .unwrap_or(&self.model)
    }
    pub fn asset_trigger(&self, symbol: &str) -> (&TriggerConfig, u64) {
        self.universe
            .as_ref()
            .and_then(|u| u.profiles.get(symbol))
            .map(|p| (&p.trigger.config, p.trigger.profile_version))
            .unwrap_or((&self.trigger_config, self.trigger_config_version))
    }
    pub fn asset_mode_allowed(&self, symbol: &str, mode: &str) -> bool {
        self.universe.as_ref().is_none_or(|u| {
            u.profiles.get(symbol).is_some_and(|p| {
                let live = ["AUTO", "CONFIRM"].contains(&mode);
                (!u.frozen || ["BTCUSDT", "ETHUSDT"].contains(&symbol))
                    && p.mode_permissions.iter().any(|x| x == mode)
                    && (mode == "OBSERVE"
                        || p.qualification
                            .as_ref()
                            .is_none_or(|q| q.overall_state != "FAIL"))
                    && !["SUSPENDED", "REJECTED", "RETIRED"].contains(&p.lifecycle_state.as_str())
                    && (!live
                        || (u.portfolio.live_enabled
                            && p.lifecycle_state == "ELIGIBLE"
                            && p.trigger.calibrated
                            && p.qualification.as_ref().is_some_and(|q| {
                                q.overall_state == "PASS"
                                    && self.now.saturating_sub(q.evaluated_at)
                                        <= u.config.interval_secs * 2000
                            })))
            })
        })
    }
    pub fn execution_symbols(&self) -> Vec<String> {
        self.symbols
            .keys()
            .filter(|s| {
                ["BTCUSDT", "ETHUSDT"].contains(&s.as_str())
                    || self.has_runtime_position(s)
                    || self
                        .live
                        .orders
                        .values()
                        .any(|o| o.plan.symbol == ***s && o.pending())
                    || self
                        .universe
                        .as_ref()
                        .and_then(|u| u.profiles.get(*s))
                        .is_some_and(|p| {
                            p.mode_permissions
                                .iter()
                                .any(|m| ["CONFIRM", "AUTO"].contains(&m.as_str()))
                        })
            })
            .cloned()
            .collect()
    }
    pub fn collection_symbols(&self) -> Vec<String> {
        self.symbols
            .keys()
            .filter(|s| {
                self.has_runtime_position(s)
                    || self
                        .live
                        .orders
                        .values()
                        .any(|o| o.pending() && o.plan.symbol == ***s)
                    || self.universe.as_ref().is_none_or(|u| {
                        u.profiles.get(*s).is_some_and(|p| {
                            p.collecting
                                && (!u.frozen || ["BTCUSDT", "ETHUSDT"].contains(&s.as_str()))
                        })
                    })
            })
            .cloned()
            .collect()
    }
    pub(crate) fn save_profile(&mut self, symbol: &str) {
        let p = self
            .universe
            .as_mut()
            .unwrap()
            .profiles
            .get_mut(symbol)
            .unwrap();
        p.profile_version += 1;
        p.id = format!("asset-{symbol}-{}", p.profile_version);
        p.updated_at = self.now;
        let p = p.clone();
        self.log("asset_trigger_profile", json!(p.trigger));
        self.log("asset_profile", json!(p));
    }
    pub(crate) fn transition_asset(
        &mut self,
        symbol: &str,
        to: &str,
        reason: &str,
        by: &str,
        evidence: Value,
    ) {
        let id = self.id("asset-transition");
        let config_version = self.asset_config_version(symbol);
        let u = self.universe.as_mut().unwrap();
        let p = u.profiles.get_mut(symbol).unwrap();
        if p.lifecycle_state == to {
            return;
        }
        let row = Transition {
            id,
            asset_key: symbol.into(),
            from_state: p.lifecycle_state.clone(),
            to_state: to.into(),
            reason_code: reason.into(),
            evidence: json!({"detail":evidence,"rule_version":u.config_version,"config_version":config_version,"scanner_rules":u.config,"fixed_policy":"asset-quality-v1.1.1-reset3h-slip100-500-1000"}),
            rule_version: u.config_version,
            config_version,
            initiated_by: by.into(),
            created_at: self.now,
        };
        p.lifecycle_state = to.into();
        p.profile_version += 1;
        p.id = format!("asset-{symbol}-{}", p.profile_version);
        p.updated_at = self.now;
        if to != "ELIGIBLE" {
            p.mode_permissions
                .retain(|m| ["OBSERVE", "PAPER"].contains(&m.as_str()));
        }
        u.transitions.push(row.clone());
        if u.transitions.len() > 300 {
            u.transitions.remove(0);
        }
        self.log("asset_state_transition", json!(row));
        self.save_profile(symbol);
    }
    pub fn universe_command(&mut self, command: &str, v: Value) -> Result<(), String> {
        self.initialize_universe();
        match command {
            "scanner_config" => {
                let c: UniverseConfig = serde_json::from_value(v).map_err(|e| e.to_string())?;
                c.validate()?;
                let u = self.universe.as_mut().unwrap();
                if u.running.is_some() {
                    return Err("扫描运行中".into());
                }
                if c.candidate_l2_slots < self.collection_symbols().len() {
                    return Err("槽位不能小于当前采集数量".into());
                }
                let u = self.universe.as_mut().unwrap();
                u.config = c;
                u.config_version += 1;
                u.next_run = self.now;
            }
            "scanner_run" | "asset_add" => {
                let target = if command == "asset_add" {
                    Some(normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?)
                } else {
                    None
                };
                let u = self.universe.as_mut().unwrap();
                if u.frozen {
                    return Err("动态币种池已冻结，请先解除回退".into());
                }
                if u.running.is_some() || u.requested {
                    return Err("已有扫描任务".into());
                }
                u.requested = true;
                u.target = target;
            }
            "scanner_started" => {
                let u = self.universe.as_mut().unwrap();
                if u.running.is_some() {
                    return Err("已有扫描任务".into());
                }
                u.running = Some(v["id"].as_str().ok_or("缺少 id")?.into());
                u.requested = false;
            }
            "scanner_result" => {
                let r: ScannerRun = serde_json::from_value(v).map_err(|e| e.to_string())?;
                self.apply_scan(r)?;
            }
            "asset_collect" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                let on = v["enabled"].as_bool().ok_or("缺少 enabled")?;
                let u = self.universe.as_ref().unwrap();
                let p = u.profiles.get(&symbol).ok_or("请先扫描并验证交易所合约")?;
                if on
                    && (u.frozen
                        || p.supported_exchanges.is_empty()
                        || p.lifecycle_state == "RETIRED")
                {
                    return Err("币种池冻结或合约不可用".into());
                }
                if on
                    && !p.collecting
                    && self.collection_symbols().len() >= u.config.candidate_l2_slots
                {
                    return Err("采集槽位已满，请先停止其他币种".into());
                }
                if !on && self.has_runtime_position(&symbol) {
                    return Err("有持仓时必须保留行情保护".into());
                }
                self.universe
                    .as_mut()
                    .unwrap()
                    .profiles
                    .get_mut(&symbol)
                    .unwrap()
                    .collecting = on;
                if on {
                    self.symbols
                        .entry(symbol.clone())
                        .or_insert_with(|| SymbolState::new(&self.config));
                    self.transition_asset(
                        &symbol,
                        "COLLECTING",
                        "USER_COLLECTION",
                        "USER",
                        json!({}),
                    );
                } else {
                    self.transition_asset(&symbol, "SUSPENDED", "USER_STOP", "USER", json!({}));
                }
                self.save_profile(&symbol);
            }
            "asset_review" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                let p = self
                    .universe
                    .as_ref()
                    .unwrap()
                    .profiles
                    .get(&symbol)
                    .ok_or("未知币种")?;
                let c = &self.universe.as_ref().unwrap().config;
                let key = format!(
                    "{}:{}:{}",
                    symbol,
                    self.asset_model(&symbol).model_version,
                    self.asset_config_version(&symbol)
                );
                let outcomes = self
                    .universe
                    .as_ref()
                    .unwrap()
                    .outcome_counts
                    .get(&key)
                    .copied()
                    .unwrap_or(0);
                if p.lifecycle_state != "OBSERVE"
                    || p.good_scans < c.persistence
                    || outcomes < c.minimum_outcomes
                    || !p.trigger.calibrated
                    || p.qualification
                        .as_ref()
                        .is_none_or(|q| q.overall_state != "PASS")
                    || self
                        .symbols
                        .get(&symbol)
                        .is_none_or(|s| s.health.trust == "LOW")
                    || !self.asset_model(&symbol).validated
                {
                    return Err("晋级证据不足：连续数据、重复扫描、同版本有效 Outcome、校准、触发参数及模型可信度必须全部通过".into());
                }
                self.transition_asset(
                    &symbol,
                    "ELIGIBLE",
                    "MANUAL_EVIDENCE_REVIEW",
                    "USER",
                    json!({"outcomes":outcomes,"config_version":self.asset_config_version(&symbol)}),
                );
            }
            "asset_permissions" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                let modes: Vec<String> =
                    serde_json::from_value(v["modes"].clone()).map_err(|e| e.to_string())?;
                let u = self.universe.as_mut().unwrap();
                let p = u.profiles.get_mut(&symbol).ok_or("未知币种")?;
                if modes
                    .iter()
                    .any(|m| !["OBSERVE", "PAPER", "CONFIRM", "AUTO"].contains(&m.as_str()))
                    || !modes.iter().any(|m| m == "OBSERVE")
                {
                    return Err("模式权限无效".into());
                }
                if modes
                    .iter()
                    .any(|m| ["CONFIRM", "AUTO"].contains(&m.as_str()))
                    && (p.lifecycle_state != "ELIGIBLE" || !u.portfolio.live_enabled)
                {
                    return Err("实盘权限要求 ELIGIBLE 且组合门禁已启用实盘".into());
                }
                p.mode_permissions = modes;
                p.profile_version += 1;
                p.id = format!("asset-{symbol}-{}", p.profile_version);
                p.updated_at = self.now;
                self.save_profile(&symbol);
            }
            "asset_model_rollback" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                if self.mode != "OBSERVE"
                    || self.has_runtime_position(&symbol)
                    || self.live.orders.values().any(|o| o.pending())
                {
                    return Err("回退要求 OBSERVE 且无持仓/订单".into());
                }
                let p = self
                    .universe
                    .as_mut()
                    .unwrap()
                    .profiles
                    .get_mut(&symbol)
                    .ok_or("未知币种")?;
                if p.model
                    .as_ref()
                    .is_none_or(|m| v["current_hash"] != m.artifact_hash)
                    || p.previous_model.is_none()
                {
                    return Err("没有匹配的回退工件".into());
                }
                p.model = p.previous_model.take();
                p.calibration_artifact_id = p
                    .model
                    .as_ref()
                    .filter(|m| m.validated)
                    .map(|m| m.calibration_version.clone());
                p.feature_normalization_version = p.model.as_ref().unwrap().artifact_hash.clone();
                self.save_profile(&symbol);
                self.invalidate(&symbol, "model_changed");
            }
            "asset_config" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                let config: Config =
                    serde_json::from_value(v["config"].clone()).map_err(|e| e.to_string())?;
                config.validate()?;
                if self.has_runtime_position(&symbol)
                    || self
                        .live
                        .orders
                        .values()
                        .any(|o| o.pending() && o.plan.symbol == symbol)
                {
                    return Err("有持仓或订单时不能修改配置".into());
                }
                let version = self.next_config_version();
                let baseline = Model::baseline(
                    self.source == "demo",
                    &config.resolved_costs(self.cost_venue()),
                );
                let p = self
                    .universe
                    .as_mut()
                    .unwrap()
                    .profiles
                    .get_mut(&symbol)
                    .ok_or("未知币种")?;
                p.config = Some(config.clone());
                p.model = Some(baseline);
                p.calibration_artifact_id = None;
                p.cost_artifact_id = Some(format!("cost-{symbol}-{}", p.profile_version + 1));
                p.risk_artifact_id = Some(format!("risk-{symbol}-{}", p.profile_version + 1));
                p.candidate_horizons = vec![config.horizon];
                p.candidate_targets = vec![config.target];
                p.candidate_stops = vec![config.stop];
                p.mode_permissions = vec!["OBSERVE".into()];
                p.config_version = Some(version);
                self.invalidate(&symbol, "asset_config_changed");
                self.transition_asset(
                    &symbol,
                    "CALIBRATING",
                    "ASSET_CONFIG_CHANGED",
                    "USER",
                    json!({}),
                );
                self.save_profile(&symbol);
            }
            "asset_trigger_calibrate" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                if self.has_runtime_position(&symbol)
                    || self
                        .symbols
                        .get(&symbol)
                        .is_some_and(|s| s.active.is_some())
                {
                    return Err("持仓或预测期间不能更换触发分布".into());
                }
                let u = self.universe.as_ref().unwrap();
                let p = u.profiles.get(&symbol).ok_or("未知币种")?;
                if p.clean_since.is_none_or(|ts| {
                    self.now.saturating_sub(ts) < u.config.minimum_clean_hours * 3600000
                }) {
                    return Err("连续行情预热时间不足".into());
                }
                let baseline = &self
                    .symbols
                    .get(&symbol)
                    .ok_or("尚未采集")?
                    .runtime
                    .pulse
                    .baseline;
                let warmup = p.trigger.config.warmup_samples;
                if baseline.len() < warmup * 2 {
                    return Err("触发分布样本不足".into());
                }
                let mut config = p.trigger.config.clone();
                for (family_index, family) in crate::market_pulse::FAMILIES.iter().enumerate() {
                    let mut scores = vec![];
                    for i in warmup..baseline.len() {
                        let score = (family_index * 2..family_index * 2 + 2)
                            .map(|metric| {
                                let past: Vec<_> =
                                    baseline.iter().take(i).map(|(_, v)| v[metric]).collect();
                                crate::market_pulse::robust_z(baseline[i].1[metric], &past)
                            })
                            .fold(0., f64::max);
                        scores.push(score);
                    }
                    scores.sort_by(f64::total_cmp);
                    let abnormal = scores[scores.len() * 95 / 100].clamp(3., 20.);
                    let extreme = scores[scores.len() * 99 / 100]
                        .max(abnormal + 1.)
                        .clamp(6., 50.);
                    config
                        .family_thresholds
                        .insert((*family).into(), [abnormal, extreme]);
                }
                config.validate()?;
                let evidence = json!({"asset_key":symbol,"source":self.source,"samples":baseline.len(),"clean_since":p.clean_since,"window_start":baseline.front().unwrap().0,"window_end":baseline.back().unwrap().0,"method":"causal_robust_z_p95_p99_floor_3_6","thresholds":config.family_thresholds,"baseline":baseline});
                let hash = format!("{:x}", Sha256::digest(evidence.to_string().as_bytes()));
                let p = self
                    .universe
                    .as_mut()
                    .unwrap()
                    .profiles
                    .get_mut(&symbol)
                    .unwrap();
                p.trigger.config = config;
                p.trigger.profile_version += 1;
                p.trigger.id = format!("trigger-profile-{symbol}-{}", p.trigger.profile_version);
                p.trigger.metric_distribution_version = hash;
                p.trigger.calibrated = true;
                p.trigger.created_at = self.now;
                self.symbols.get_mut(&symbol).unwrap().runtime = Default::default();
                self.log("asset_trigger_calibration", evidence);
                self.save_profile(&symbol);
            }
            "asset_trigger_config" => {
                let symbol = normalize(v["symbol"].as_str().ok_or("缺少 symbol")?)?;
                let c: TriggerConfig =
                    serde_json::from_value(v["config"].clone()).map_err(|e| e.to_string())?;
                c.validate()?;
                if self.has_runtime_position(&symbol)
                    || self
                        .symbols
                        .get(&symbol)
                        .is_some_and(|s| s.active.is_some())
                {
                    return Err("持仓或预测期间不能修改触发校准".into());
                }
                let evidence = v["evidence"]
                    .as_str()
                    .filter(|s| s.len() >= 16)
                    .ok_or("需要校准/回放证据引用")?;
                let u = self.universe.as_mut().unwrap();
                let p = u.profiles.get_mut(&symbol).ok_or("未知币种")?;
                if p.observation
                    .as_ref()
                    .is_none_or(|o| o.metrics.samples < c.warmup_samples)
                    || p.clean_since.is_none_or(|t| {
                        self.now.saturating_sub(t) < u.config.minimum_clean_hours * 3600000
                    })
                {
                    return Err("触发分布预热不足".into());
                }
                p.trigger.profile_version += 1;
                p.trigger.id = format!("trigger-profile-{symbol}-{}", p.trigger.profile_version);
                p.trigger.config = c;
                p.trigger.metric_distribution_version = evidence.into();
                p.trigger.calibrated = false;
                p.trigger.created_at = self.now;
                p.profile_version += 1;
                p.id = format!("asset-{symbol}-{}", p.profile_version);
                p.updated_at = self.now;
                if let Some(s) = self.symbols.get_mut(&symbol) {
                    s.runtime = Default::default();
                }
                self.save_profile(&symbol);
            }
            "universe_rollback" => {
                let frozen = v["frozen"].as_bool().ok_or("缺少 frozen")?;
                let u = self.universe.as_mut().unwrap();
                if u.running.is_some() {
                    return Err("请等待扫描结束再回退".into());
                }
                u.frozen = frozen;
                u.config.enabled = false;
                u.requested = false;
                self.mode = "OBSERVE".into();
            }
            "portfolio_config" => {
                let c: crate::portfolio::PortfolioConfig =
                    serde_json::from_value(v).map_err(|e| e.to_string())?;
                c.validate()?;
                self.universe.as_mut().unwrap().portfolio = c;
            }
            _ => return Err("未知币种池操作".into()),
        }
        Ok(())
    }
    fn apply_scan(&mut self, run: ScannerRun) -> Result<(), String> {
        if self.universe.as_ref().unwrap().running.as_deref() != Some(&run.id) {
            return Err("扫描任务已变化".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for o in &run.observations {
            if normalize(&o.asset_key)? != o.asset_key || !seen.insert(o.asset_key.clone()) {
                return Err("重复或无效币种".into());
            }
        }
        self.log("scanner_run_record", json!(run));
        for o in &run.observations {
            let mut q = qualify(
                o,
                &self.universe.as_ref().unwrap().config,
                run.config_version,
                run.completed_at,
            );
            q.id = format!("{}:{}", run.id, o.asset_key);
            if ["binance", "okx"]
                .iter()
                .any(|venue| run.errors.contains_key(*venue))
            {
                q.components.insert(
                    "scanner_sources".into(),
                    Component {
                        state: "UNKNOWN".into(),
                        evidence: json!(run.errors),
                    },
                );
                q.warnings.push("scanner_sources".into());
                if q.overall_state == "PASS" {
                    q.overall_state = "CONDITIONAL".into();
                }
            }
            let u = self.universe.as_mut().unwrap();
            for i in &o.instruments {
                u.instruments.insert(
                    format!("{}:{}:{}", i.venue, i.symbol, i.contract_type),
                    i.clone(),
                );
            }
            let p = u.profiles.entry(o.asset_key.clone()).or_insert_with(|| {
                AssetProfile::new(&o.asset_key, self.now, &self.config, &self.trigger_config)
            });
            p.supported_exchanges = o
                .instruments
                .iter()
                .filter(|i| i.active)
                .map(|i| i.venue.clone())
                .collect();
            if p.model.is_none() {
                p.model = Some(Model::baseline(self.source == "demo", &self.config));
            }
            let separated = p.qualification.as_ref().is_none_or(|previous| {
                q.evaluated_at.saturating_sub(previous.evaluated_at)
                    >= u.config.interval_secs * 1000
            });
            p.observation = Some(o.clone());
            p.liquidity_artifact_id = Some(format!("{}:{}", run.id, o.asset_key));
            p.qualification = Some(q.clone());
            if q.overall_state == "PASS" {
                p.bad_scans = 0;
                if separated {
                    p.good_scans += 1;
                }
            } else {
                p.good_scans = 0;
                if separated {
                    p.bad_scans += 1;
                }
            }
            let state = p.lifecycle_state.clone();
            let bad = p.bad_scans;
            let persistence = u.config.persistence;
            self.log(
                "scanner_observation",
                json!({"scanner_run_id":run.id,"observation":o,"qualification":q}),
            );
            self.log("asset_qualification", json!(q));
            if q.components["exchange_available"].state == "FAIL" {
                self.transition_asset(
                    &o.asset_key,
                    "RETIRED",
                    "EXPLICIT_EXCHANGE_UNAVAILABLE",
                    "SCANNER",
                    json!(q),
                );
            } else if q.overall_state == "FAIL" {
                self.transition_asset(
                    &o.asset_key,
                    if ["ELIGIBLE", "DEGRADED", "OBSERVE"].contains(&state.as_str()) {
                        "SUSPENDED"
                    } else {
                        "REJECTED"
                    },
                    "HARD_ASSET_GATE",
                    "SCANNER",
                    json!(q),
                );
            } else if state == "ELIGIBLE" && bad >= persistence {
                self.transition_asset(
                    &o.asset_key,
                    "DEGRADED",
                    "PERSISTENT_ASSET_WARNING",
                    "SCANNER",
                    json!(q),
                );
            } else if state == "DEGRADED" && bad >= persistence * 2 {
                self.transition_asset(
                    &o.asset_key,
                    "SUSPENDED",
                    "PERSISTENT_ASSET_WARNING",
                    "SCANNER",
                    json!(q),
                );
            }
            self.save_profile(&o.asset_key);
        }
        if !self.universe.as_ref().unwrap().config.dry_run {
            let mut candidates: Vec<_> = run
                .observations
                .iter()
                .filter_map(|o| {
                    let p = &self.universe.as_ref().unwrap().profiles[&o.asset_key];
                    (p.lifecycle_state == "DISCOVERED"
                        && p.qualification
                            .as_ref()
                            .is_some_and(|q| q.research_priority == "HIGH"))
                    .then_some((o.asset_key.clone(), o.metrics.quote_volume.unwrap_or(0.)))
                })
                .collect();
            candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            for (symbol, _) in candidates {
                if self.collection_symbols().len()
                    >= self.universe.as_ref().unwrap().config.candidate_l2_slots
                {
                    break;
                }
                let p = self
                    .universe
                    .as_mut()
                    .unwrap()
                    .profiles
                    .get_mut(&symbol)
                    .unwrap();
                p.collecting = true;
                self.symbols
                    .entry(symbol.clone())
                    .or_insert_with(|| SymbolState::new(&self.config));
                self.transition_asset(
                    &symbol,
                    "COLLECTING",
                    "BOUNDED_CANDIDATE_COLLECTION",
                    "SCANNER",
                    json!({"scanner_run_id":run.id}),
                );
            }
        }
        let u = self.universe.as_mut().unwrap();
        u.running = None;
        u.target = None;
        u.next_run = run.completed_at + u.config.interval_secs * 1000;
        u.last_run = Some(run.clone());

        Ok(())
    }
    pub fn universe_tick(&mut self) {
        if self.universe.is_none() {
            return;
        }
        let symbols = self.collection_symbols();
        for symbol in symbols {
            let stale_ms = self.asset_config(&symbol).max_stale_ms;
            let ready = self.symbols[&symbol]
                .venues
                .values()
                .all(|v| v.connected && v.book.ready(self.now, stale_ms));
            let u = self.universe.as_mut().unwrap();
            let Some(p) = u.profiles.get_mut(&symbol) else {
                continue;
            };
            if self.now.saturating_sub(p.last_health_check) < 1000 {
                continue;
            }
            p.last_health_check = self.now;
            let resets = self.symbols[&symbol]
                .venues
                .values()
                .map(|v| v.book.gaps + v.reconnects)
                .sum::<u64>();
            if let Some(previous) = p.last_reset_count {
                if resets > previous {
                    p.reset_history.push_back((self.now, resets - previous));
                    p.clean_since = None;
                }
            }
            p.last_reset_count = Some(resets);
            while p
                .reset_history
                .front()
                .is_some_and(|(ts, _)| self.now.saturating_sub(*ts) > 3_600_000)
            {
                p.reset_history.pop_front();
            }

            if ready {
                if let Some((bid, ask, _, _)) = self.symbols[&symbol].venues["binance"].book.top() {
                    let samples = u.spread_samples.entry(symbol.clone()).or_default();
                    samples.push_back((self.now, (ask - bid) / ((ask + bid) / 2.) * 10000.));
                    while samples.len() > 3600 {
                        samples.pop_front();
                    }
                }
                p.clean_since.get_or_insert(self.now);
            } else {
                p.clean_since = None;
            }
            let warm = p.clean_since.is_some_and(|t| {
                self.now.saturating_sub(t) >= u.config.minimum_clean_hours * 3600000
            });
            let state = p.lifecycle_state.clone();
            if warm && state == "COLLECTING" {
                self.transition_asset(
                    &symbol,
                    "CALIBRATING",
                    "CLEAN_DATA_WARMUP",
                    "SYSTEM",
                    json!({}),
                );
            } else if warm && state == "CALIBRATING" && self.asset_model(&symbol).validated {
                self.transition_asset(
                    &symbol,
                    "OBSERVE",
                    "CALIBRATED_MODEL_READY",
                    "SYSTEM",
                    json!({}),
                );
            } else if !ready && state == "ELIGIBLE" {
                self.transition_asset(&symbol, "SUSPENDED", "DATA_INTEGRITY", "SYSTEM", json!({}));
            }
        }
    }
}
