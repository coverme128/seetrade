//! Read-only review of durable evidence. Never feeds labels, risk or execution.
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use time::{Date, Month, OffsetDateTime, UtcOffset};

type TradeCohortKey = (String, String, String, String, String, Option<u64>);
const DAY: u64 = 86_400_000;
const KINDS: &str = "'prediction','outcome','outcome_horizon','episode_qualification','mode','restart','broker_config','paper_entry','paper_reject','plan','trade','auto_admission','skip','live_risk_decision','broker_order','live_trade','live_trade_adjustment','portfolio_gate_decision'";
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReviewQuery {
    pub date: Option<String>,
    pub symbol: Option<String>,
    pub source: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub through_id: Option<u64>,
    pub as_of: Option<u64>,
    pub export: Option<bool>,
}
impl ReviewQuery {
    pub fn validate(&self, now: u64) -> Result<()> {
        window(self.date.as_deref(), now)?;
        if self
            .source
            .as_deref()
            .is_some_and(|s| !["live", "demo"].contains(&s))
        {
            bail!("source 必须是 live 或 demo");
        }
        if self.symbol.as_ref().is_some_and(|s| {
            s.is_empty() || s.len() > 64 || s.chars().any(|c| !c.is_alphanumeric())
        }) {
            bail!("symbol 必须是规范化币种名称");
        }
        if self.through_id.is_some_and(|id| id > i64::MAX as u64) {
            bail!("无效的报告快照编号");
        }
        Ok(())
    }
}
pub fn date_at(now: u64) -> Result<String> {
    let t = OffsetDateTime::from_unix_timestamp((now / 1000).try_into()?)?
        .to_offset(UtcOffset::from_hms(8, 0, 0)?);
    Ok(t.date().to_string())
}
fn window(date: Option<&str>, now: u64) -> Result<(String, u64, u64)> {
    let date = date.map(str::to_owned).unwrap_or(date_at(now)?);
    if date == "all" {
        return Ok((date, 0, now.saturating_add(1)));
    }
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        bail!("日期格式应为 YYYY-MM-DD 或 all");
    }
    let d = Date::from_calendar_date(
        parts[0].parse()?,
        Month::try_from(parts[1].parse::<u8>()?)?,
        parts[2].parse()?,
    )?;
    let seconds = d
        .midnight()
        .assume_offset(UtcOffset::from_hms(8, 0, 0)?)
        .unix_timestamp();
    if seconds < 0 {
        bail!("日期必须晚于 1970-01-01");
    }
    let start = seconds as u64 * 1000;
    if start > now {
        bail!("不能评估未来日期");
    }
    Ok((date, start, start + DAY))
}
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("UNKNOWN").to_owned()
}
fn number(v: &Value, key: &str) -> Option<f64> {
    v[key].as_f64().filter(|n| n.is_finite())
}
fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    *map.entry(key.into()).or_default() += 1;
}
fn key(source: &str, id: &str) -> String {
    format!("{source}:{id}")
}
fn in_window(ts: u64, start: u64, end: u64) -> bool {
    ts >= start && ts < end
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn calibrated_win_probability(p: &Value) -> Option<f64> {
    let a = p["calibrated_probabilities"].as_array()?;
    if a.len() != 3
        || a.iter()
            .any(|v| v.as_f64().is_none_or(|v| !(0. ..=1.).contains(&v)))
        || (a.iter().filter_map(Value::as_f64).sum::<f64>() - 1.).abs() >= 0.001
    {
        return None;
    }
    a[if p["direction"] == "LONG" { 0 } else { 1 }].as_f64()
}
fn grade(p: &Value) -> &'static str {
    let q = &p["qualification"];
    let gates = q["gates"].as_array().cloned().unwrap_or_default();
    if gates.is_empty() || q["prediction_id"] != p["prediction_id"] {
        return "inactive";
    }
    let trust = gates
        .iter()
        .find(|g| g["code"] == "model_trust")
        .and_then(|g| g["detail"].as_str())
        .unwrap_or("");
    let calibrated = calibrated_win_probability(p).is_some();
    if !calibrated
        || !["HIGH", "DEGRADED"].contains(&trust)
        || ["calibration", "model_trust"].iter().any(|code| {
            !gates
                .iter()
                .any(|g| g["code"] == *code && g["pass"] == true)
        })
    {
        return "unverified";
    }
    if q["status"] != "QUALIFIED"
        || gates.iter().any(|g| g["pass"] != true)
        || !strings(&q["reasons"]).is_empty()
        || !number(q, "net_edge").is_some_and(|n| n > 0.)
    {
        return "blocked";
    }
    if trust == "HIGH" {
        "priority"
    } else {
        "qualified"
    }
}
#[derive(Default)]
struct Opportunity {
    prediction: Value,
    outcome: Option<Value>,
    horizon: Option<Value>,
    mode: String,
    qualified: bool,
    paper: bool,
    skipped: bool,
    reasons: BTreeSet<String>,
    admission: Option<Value>,
    shadow_admission: Option<Value>,
}
#[derive(Clone)]
struct Order {
    source: String,
    environment: String,
    venue: String,
    value: Value,
    updated: u64,
}
#[derive(Clone, Serialize)]
pub struct ClosedTrade {
    pub id: String,
    pub source: String,
    pub channel: String,
    pub venue: String,
    pub prediction_id: String,
    pub symbol: String,
    pub model_version: String,
    pub config_version: Option<u64>,
    pub closed_at: u64,
    pub exit_reason: String,
    pub net_pnl: Option<f64>,
    pub expected_cost: Option<f64>,
    pub realized_cost: Option<f64>,
}
#[derive(Default)]
struct Evidence {
    opportunities: BTreeMap<String, Opportunity>,
    modes: BTreeMap<String, String>,
    environments: BTreeMap<String, (String, String)>,
    plans: BTreeMap<String, String>,
    orders: BTreeMap<String, Order>,
    trades: BTreeMap<String, ClosedTrade>,
    pending_reasons: Vec<(String, String, String)>,
    malformed: usize,
}
impl Evidence {
    fn observe(&mut self, kind: &str, source: &str, ts: u64, p: Value) {
        let id = text(&p, "prediction_id");
        match kind {
            "mode" => {
                self.modes.insert(source.into(), text(&p, "mode"));
            }
            "restart" => {
                self.modes.insert(source.into(), "OBSERVE".into());
            }
            "broker_config" => {
                let environment = match p["environment"].as_str() {
                    Some("testnet") => "TESTNET",
                    Some("live") => "LIVE",
                    _ => "UNKNOWN",
                };
                self.environments
                    .insert(source.into(), (environment.into(), text(&p, "venue")));
            }
            "prediction" | "outcome" => {
                let prediction = if kind == "prediction" {
                    &p
                } else {
                    &p["prediction"]
                };
                if !prediction["prediction_id"].is_string() || !prediction["ts"].is_u64() {
                    self.malformed += 1;
                    return;
                }
                let row = self
                    .opportunities
                    .entry(key(source, &text(prediction, "prediction_id")))
                    .or_default();
                // Prefer the original opening record; an outcome only supplies missing legacy context.
                if row.prediction.is_null() || kind == "prediction" && row.outcome.is_some() {
                    row.prediction = prediction.clone();
                    row.mode = self
                        .modes
                        .get(source)
                        .cloned()
                        .unwrap_or_else(|| "UNKNOWN".into());
                }
                row.qualified |= prediction["qualification"]["status"] == "QUALIFIED"
                    || prediction["episode"]["highest_qualification"] == "QUALIFIED";
                if let Some(plan) = prediction["plan"]["plan_id"].as_str() {
                    self.plans
                        .insert(key(source, plan), text(prediction, "prediction_id"));
                }
                if kind == "outcome" {
                    if !p["horizon_observation"].is_null() {
                        row.horizon = Some(p["horizon_observation"].clone());
                    }
                    row.outcome = Some(p);
                }
            }
            "outcome_horizon" => {
                self.opportunities
                    .entry(key(source, &id))
                    .or_default()
                    .horizon = Some(p["observation"].clone());
            }
            "episode_qualification" => {
                self.opportunities
                    .entry(key(source, &text(&p, "episode_id")))
                    .or_default()
                    .qualified |= p["highest_qualification"] == "QUALIFIED";
            }
            "paper_entry" => {
                self.opportunities
                    .entry(key(source, &id))
                    .or_default()
                    .paper = true;
            }
            "skip" => {
                self.opportunities
                    .entry(key(source, &id))
                    .or_default()
                    .skipped = true;
            }
            "plan" => {
                self.plans.insert(key(source, &text(&p, "plan_id")), id);
            }
            "paper_reject" => {
                self.pending_reasons.push((
                    source.into(),
                    text(&p, "plan_id"),
                    format!("PAPER: {}", text(&p, "reason")),
                ));
            }
            "portfolio_gate_decision" if p["enforced"] == true && p["decision"] == "BLOCK" => {
                self.pending_reasons.push((
                    source.into(),
                    text(&p, "trade_plan_id"),
                    format!("组合限制: {}", strings(&p["reason_codes"]).join(", ")),
                ));
            }
            "auto_admission" => {
                let row = self
                    .opportunities
                    .entry(key(source, &text(&p, "episode_id")))
                    .or_default();
                if p["context"] == "LIVE" {
                    row.admission = Some(p);
                } else {
                    row.shadow_admission = Some(p);
                }
            }
            "live_risk_decision" | "broker_order" => {
                if kind == "live_risk_decision" && p["status"] == "FAIL" {
                    self.opportunities
                        .entry(key(source, &id))
                        .or_default()
                        .reasons
                        .insert(text(&p, "reason"));
                } else {
                    let intent = if kind == "broker_order" {
                        p
                    } else {
                        p["intent"].clone()
                    };
                    if let Some(client) = intent["client_id"].as_str() {
                        let k = key(source, client);
                        let (environment, venue) = self
                            .orders
                            .get(&k)
                            .map(|o| (o.environment.clone(), o.venue.clone()))
                            .or_else(|| self.environments.get(source).cloned())
                            .unwrap_or_else(|| ("UNKNOWN".into(), "UNKNOWN".into()));
                        self.orders.insert(
                            k,
                            Order {
                                source: source.into(),
                                environment,
                                venue,
                                value: intent,
                                updated: ts,
                            },
                        );
                    }
                }
            }
            "trade" => {
                let plan = &p["position"]["plan"];
                let id = text(plan, "plan_id");
                let prediction_id = text(plan, "prediction_id");
                let row = self.opportunities.get(&key(source, &prediction_id));
                self.trades.insert(
                    format!("{source}:PAPER:{id}"),
                    ClosedTrade {
                        id,
                        source: source.into(),
                        channel: "PAPER".into(),
                        venue: text(&p["position"], "execution_venue"),
                        prediction_id,
                        symbol: text(plan, "symbol"),
                        model_version: row
                            .map(|r| text(&r.prediction, "model_version"))
                            .unwrap_or_else(|| "UNKNOWN".into()),
                        config_version: row.and_then(|r| r.prediction["config_version"].as_u64()),
                        closed_at: p["exit_ts"].as_u64().unwrap_or(ts),
                        exit_reason: text(&p, "exit_reason"),
                        net_pnl: number(&p, "net_pnl"),
                        expected_cost: number(&p, "expected_cost"),
                        realized_cost: number(&p, "realized_cost"),
                    },
                );
            }
            "live_trade" | "live_trade_adjustment" => {
                let t = if kind == "live_trade" {
                    p
                } else {
                    p["trade"].clone()
                };
                let id = text(&t, "parent_id");
                let order = self.orders.get(&key(source, &id));
                let channel = order
                    .map(|o| o.environment.clone())
                    .unwrap_or_else(|| "UNKNOWN".into());
                let venue = order
                    .map(|o| o.venue.clone())
                    .unwrap_or_else(|| "UNKNOWN".into());
                let exit = self
                    .orders
                    .values()
                    .filter(|o| {
                        o.source == source
                            && o.value["parent_id"] == id
                            && o.value["action"] == "EXIT"
                    })
                    .max_by_key(|o| o.updated)
                    .map(|o| text(&o.value, "reason"))
                    .unwrap_or_else(|| "UNKNOWN".into());
                self.trades.insert(
                    format!("{source}:EXCHANGE:{id}"),
                    ClosedTrade {
                        id,
                        source: source.into(),
                        channel,
                        venue,
                        prediction_id: text(&t, "prediction_id"),
                        symbol: text(&t, "symbol"),
                        model_version: text(&t["feedback"], "model_version"),
                        config_version: t["feedback"]["config_version"].as_u64(),
                        closed_at: t["closed_ts"].as_u64().unwrap_or(ts),
                        exit_reason: exit,
                        net_pnl: number(&t, "net_pnl"),
                        expected_cost: number(&t["feedback"], "expected_cost"),
                        realized_cost: number(&t["feedback"], "realized_cost"),
                    },
                );
            }
            _ => {}
        }
    }
}
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<_> = values.collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
fn trade_summary(rows: &[&ClosedTrade]) -> Value {
    let pnl: Vec<_> = rows.iter().filter_map(|t| t.net_pnl).collect();
    let mut curve = 0_f64;
    let mut peak = 0_f64;
    let mut drawdown = 0_f64;
    let mut losses = 0;
    let mut max_losses = 0;
    for p in &pnl {
        curve += p;
        peak = peak.max(curve);
        drawdown = drawdown.max(peak - curve);
        losses = if *p < 0. { losses + 1 } else { 0 };
        max_losses = max_losses.max(losses);
    }
    json!({"closed_trades":rows.len(),"pnl_samples":pnl.len(),"net_pnl":(!pnl.is_empty()).then_some(curve),
        "average_net_pnl":mean(pnl.iter().copied()),"win_rate":mean(pnl.iter().map(|v| if *v > 0. {1.} else {0.})),
        "realized_pnl_drawdown":(!pnl.is_empty()).then_some(drawdown),"max_consecutive_losses":(!pnl.is_empty()).then_some(max_losses),
        "cost_samples":rows.iter().filter(|t| t.realized_cost.is_some() && t.expected_cost.is_some()).count(),
        "mean_cost_overrun":mean(rows.iter().filter_map(|t| Some(t.realized_cost? - t.expected_cost?)))})
}
/// One consistent, read-only WAL snapshot. Aggregation is not limited to history's 500 rows.
pub fn build(path: &Path, query: &ReviewQuery, now: u64) -> Result<Value> {
    query.validate(now)?;
    let (date, start, end) = window(query.date.as_deref(), now)?;
    let cutoff = now.min(end - 1).min(query.as_of.unwrap_or(now));
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("BEGIN")?;
    let max_id: u64 =
        conn.query_row("SELECT coalesce(max(id),0) FROM records", [], |r| r.get(0))?;
    let through_id = max_id.min(query.through_id.unwrap_or(max_id));
    let latest_record_ts: Option<u64> = conn.query_row(
        "SELECT max(ts) FROM records WHERE id<=?1 AND ts<=?2",
        rusqlite::params![through_id, cutoff],
        |r| r.get(0),
    )?;
    let mut stmt=conn.prepare(&format!("SELECT kind,ts,payload FROM records WHERE id<=?1 AND ts<=?2 AND kind IN ({KINDS}) ORDER BY id"))?;
    let mut cursor = stmt.query(rusqlite::params![through_id, cutoff])?;
    let mut evidence = Evidence::default();
    while let Some(row) = cursor.next()? {
        let kind: String = row.get(0)?;
        let ts: u64 = row.get(1)?;
        let encoded: String = row.get(2)?;
        match serde_json::from_str::<Value>(&encoded) {
            Ok(record) => {
                let source = text(&record, "source");
                if query.source.as_ref().is_none_or(|s| s == &source) {
                    evidence.observe(&kind, &source, ts, record["payload"].clone());
                }
            }
            Err(_) => evidence.malformed += 1,
        }
    }
    for (source, plan, reason) in &evidence.pending_reasons {
        // Legacy portfolio/paper rejects only saved the canonical plan ID.
        let linked = evidence.plans.get(&key(source, plan)).cloned().or_else(|| {
            let (id, timestamp) = plan.strip_prefix("plan-")?.rsplit_once('-')?;
            timestamp.parse::<u64>().ok()?;
            evidence
                .opportunities
                .contains_key(&key(source, id))
                .then(|| id.to_owned())
        });
        if let Some(id) = linked {
            evidence
                .opportunities
                .entry(key(source, &id))
                .or_default()
                .reasons
                .insert(reason.clone());
        }
    }
    let mut ledger: Vec<_> = evidence
        .trades
        .values()
        .filter(|t| {
            in_window(t.closed_at, start, end)
                && t.closed_at <= cutoff
                && query.symbol.as_ref().is_none_or(|s| s == &t.symbol)
        })
        .cloned()
        .collect();
    ledger.sort_by(|a, b| a.closed_at.cmp(&b.closed_at).then(a.id.cmp(&b.id)));
    let mut trade_groups: BTreeMap<(String, String, String), Vec<&ClosedTrade>> = BTreeMap::new();
    for t in &ledger {
        trade_groups
            .entry((t.source.clone(), t.channel.clone(), t.venue.clone()))
            .or_default()
            .push(t);
    }
    let trading:Vec<_>=trade_groups.into_iter().map(|((source,channel,venue),trades)| json!({"source":source,"channel":channel,"venue":venue,"metrics":trade_summary(&trades)})).collect();
    let mut opportunities = vec![];
    for (k, row) in &evidence.opportunities {
        let p = &row.prediction;
        let ts = p["ts"].as_u64().unwrap_or(u64::MAX);
        if !in_window(ts, start, end)
            || ts > cutoff
            || query.symbol.as_ref().is_some_and(|s| p["symbol"] != *s)
        {
            continue;
        }
        let source = k.split_once(':').map(|v| v.0).unwrap_or("UNKNOWN");
        let id = text(p, "prediction_id");
        let linked: Vec<_> = evidence
            .orders
            .values()
            .filter(|o| {
                o.source == source
                    && o.value["plan"]["prediction_id"] == id
                    && o.value["action"] == "ENTRY"
            })
            .collect();
        let filled = linked
            .iter()
            .any(|o| number(&o.value, "filled_qty").is_some_and(|v| v > 0.));
        let pending = linked.iter().any(|o| {
            ["QUEUED", "SUBMITTING", "UNKNOWN", "NEW", "PARTIALLY_FILLED"]
                .contains(&o.value["status"].as_str().unwrap_or(""))
        });
        let closed: Vec<_> = evidence
            .trades
            .values()
            .filter(|t| t.source == source && t.prediction_id == id)
            .collect();
        let mut reasons = row.reasons.clone();
        let execution = if !closed.is_empty() {
            "CLOSED"
        } else if filled || row.paper {
            "OPEN"
        } else if pending {
            "ORDER_PENDING"
        } else if !linked.is_empty() {
            "UNFILLED"
        } else if row.skipped {
            "MANUAL_SKIP"
        } else if row
            .admission
            .as_ref()
            .is_some_and(|a| a["decision"] == "SKIPPED")
        {
            "ADMISSION_BLOCKED"
        } else if !row.reasons.is_empty() {
            "EXECUTION_BLOCKED"
        } else if row.mode == "OBSERVE" {
            "OBSERVE_ONLY"
        } else if !row.qualified {
            "NOT_QUALIFIED"
        } else {
            "NO_EXECUTION_EVIDENCE"
        };
        if !filled && !row.paper && closed.is_empty() {
            if let Some(a) = &row.admission {
                if a["decision"] == "SKIPPED" {
                    reasons.extend(strings(&a["reason_codes"]));
                }
            }
            if !row.qualified {
                reasons.extend(strings(&p["qualification"]["reasons"]));
            }
            for o in &linked {
                if let Some(r) = o.value["reason"].as_str() {
                    reasons.insert(r.into());
                }
            }
        }
        let o = row.outcome.as_ref();
        let eligible = o.is_some_and(|o| o["learning_eligible"] == true);
        opportunities.push(json!({"prediction_id":id,"source":source,"symbol":p["symbol"],"opened_at":ts,"direction":p["direction"],"mode_at_open":row.mode,
            "model_version":p["model_version"],"config_version":p["config_version"],"horizon":p["horizon"],"target":p["target"],"stop":p["stop"],
            "opening_grade":grade(p),"opening_net_edge":p["qualification"]["net_edge"],"ever_qualified":row.qualified,
            "calibrated_win_probability":calibrated_win_probability(p),
            "outcome":o.map(|o|&o["outcome_type"]),"closed_at":o.map(|o|&o["first_hit_ts"]),"learning_eligible":eligible,"invalid_reason":o.map(|o|&o["prediction_error"]),
            "observed_net_edge":if eligible{o.and_then(|o|number(o,"observed_net_edge"))}else{None},
            "horizon_observation":row.horizon,"execution":execution,"reasons":reasons,"shadow_admission":row.shadow_admission.as_ref().map(|a|json!({"decision":a["decision"],"reasons":a["reason_codes"]})),
            "orders":linked.iter().map(|o|json!({"id":o.value["client_id"],"channel":o.environment,"venue":o.venue,"status":o.value["status"],"filled_qty":o.value["filled_qty"]})).collect::<Vec<_>>(),
            "closed_trades":closed,"paper_entry":row.paper,"exchange_filled":filled}));
    }
    opportunities.sort_by(|a, b| {
        b["opened_at"].as_u64().cmp(&a["opened_at"].as_u64()).then(
            a["prediction_id"]
                .as_str()
                .cmp(&b["prediction_id"].as_str()),
        )
    });
    let summary = summarize(&opportunities);
    let mut cohorts: BTreeMap<(String, String, String, u64), Vec<Value>> = BTreeMap::new();
    for r in &opportunities {
        cohorts
            .entry((
                text(r, "source"),
                text(r, "symbol"),
                text(r, "model_version"),
                r["config_version"].as_u64().unwrap_or(0),
            ))
            .or_default()
            .push(r.clone());
    }
    let cohorts:Vec<_>=cohorts.into_iter().map(|((source,symbol,model,version),rows)| json!({"source":source,"symbol":symbol,"model_version":model,"config_version":version,"metrics":summarize(&rows)})).collect();
    let mut order_groups: BTreeMap<(String, String, String), Vec<&Order>> = BTreeMap::new();
    for o in evidence.orders.values().filter(|o| {
        in_window(o.updated, start, end)
            && query
                .symbol
                .as_ref()
                .is_none_or(|s| o.value["plan"]["symbol"] == *s)
    }) {
        order_groups
            .entry((o.source.clone(), o.environment.clone(), o.venue.clone()))
            .or_default()
            .push(o);
    }
    let execution:Vec<_>=order_groups.into_iter().map(|((source,channel,venue),orders)| {
        let mut statuses=BTreeMap::new(); for o in &orders { increment(&mut statuses,&text(&o.value,"status")); }
        json!({"source":source,"channel":channel,"venue":venue,"orders":orders.len(),"filled_orders":orders.iter().filter(|o|number(&o.value,"filled_qty").is_some_and(|n|n>0.)).count(),"statuses":statuses})
    }).collect();
    let mut version_trades: BTreeMap<TradeCohortKey, Vec<&ClosedTrade>> = BTreeMap::new();
    for t in &ledger {
        version_trades
            .entry((
                t.source.clone(),
                t.channel.clone(),
                t.venue.clone(),
                t.symbol.clone(),
                t.model_version.clone(),
                t.config_version,
            ))
            .or_default()
            .push(t);
    }
    let trade_cohorts: Vec<_> = version_trades.into_iter().map(|((source,channel,venue,symbol,model_version,config_version),trades)|json!({"source":source,"channel":channel,"venue":venue,"symbol":symbol,"model_version":model_version,"config_version":config_version,"metrics":trade_summary(&trades)})).collect();
    let mut diagnostics = diagnose(&summary, &trading, evidence.malformed);
    if cohorts.len() > 1 {
        diagnostics.push(json!({"code":"MULTIPLE_COHORTS","message":"汇总包含多个币种或版本；请按分组评估，不能用累计样本数替代当前单币 / 版本的验证门槛。"}));
    }
    if trading.iter().any(|g| {
        g["channel"] == "UNKNOWN" || g["metrics"]["pnl_samples"] != g["metrics"]["closed_trades"]
    }) {
        diagnostics.push(json!({"code":"MISSING_TRADE_EVIDENCE","message":"部分交易缺少环境或损益证据，相关汇总不完整，不能据此判断实盘效果。"}));
    }
    let total = opportunities.len();
    let offset = query.offset.unwrap_or(0).min(total);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let rows = if query.export == Some(true) {
        opportunities
    } else {
        opportunities.into_iter().skip(offset).take(limit).collect()
    };
    Ok(
        json!({"schema":"daily-review-v1","date":date,"timezone":"Asia/Shanghai","start":start,"end_exclusive":end,"as_of":cutoff,"through_id":through_id,"latest_record_ts":latest_record_ts,
        "source_filter":query.source,"symbol_filter":query.symbol,"provisional":end>now,"summary":summary,"cohorts":cohorts,"trade_cohorts":trade_cohorts,"trading":trading,"execution":execution,"closed_trades":ledger,
        "diagnostics":diagnostics,"malformed_records":evidence.malformed,"rows":rows,"pagination":{"offset":offset,"limit":limit,"total":total,"exported_all":query.export==Some(true)},
        "limitations":["按北京时间开场日统计正式预测；扫描 HIGH 候选不计为交易机会。", "历史日报仅使用当日结束前已知证据；跨日未结束显示待完成，最终结果请查看累计评估。", "成交汇总按平仓日统计，包含此前开场的持仓；资金费修订按报告截止时已知记录去重。", "加仓与分批退出汇总到原始入场交易，不为每条加仓信号重复分配整笔损益。", "观察净优势是价格路径减假设成本，不是成交损益；不合格、失效和无成交都不会计为零收益交易。", "回撤仅为按平仓顺序累计的已平仓净损益回撤（USDT），不含浮动盈亏，不等于账户最大回撤。", "旧记录缺少环境、版本或执行原因时保留 UNKNOWN；未确认不能自动归因为人工忽略。", "诊断仅辅助复盘，不修改标签、模型、风险参数或交易权限；累计结果应结合各版本分组阅读。"]}),
    )
}
fn summarize(rows: &[Value]) -> Value {
    let mut outcomes = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut execution = BTreeMap::new();
    let mut grades = BTreeMap::new();
    for r in rows {
        increment(&mut outcomes, r["outcome"].as_str().unwrap_or("PENDING"));
        increment(&mut execution, &text(r, "execution"));
        increment(&mut grades, &text(r, "opening_grade"));
        if r["learning_eligible"] != true && !r["outcome"].is_null() {
            increment(&mut reasons, &text(r, "invalid_reason"));
        }
    }
    let valid: Vec<_> = rows
        .iter()
        .filter(|r| r["learning_eligible"] == true)
        .collect();
    let mut non_execution = BTreeMap::new();
    for r in rows.iter().filter(|r| {
        r["paper_entry"] != true
            && r["exchange_filled"] != true
            && r["closed_trades"].as_array().is_none_or(Vec::is_empty)
    }) {
        for reason in strings(&r["reasons"]) {
            increment(&mut non_execution, &reason);
        }
    }
    let calibrated: Vec<_> = valid
        .iter()
        .filter_map(|r| {
            number(r, "calibrated_win_probability")
                .map(|p| (p, if r["outcome"] == "TARGET_HIT" { 1. } else { 0. }))
        })
        .collect();
    let mut bins = [(0usize, 0_f64, 0_f64); 10];
    for (p, hit) in &calibrated {
        let bin = &mut bins[((p * 10.) as usize).min(9)];
        bin.0 += 1;
        bin.1 += p;
        bin.2 += hit;
    }
    let ece = (!calibrated.is_empty())
        .then(|| bins.iter().map(|(_, p, h)| (p - h).abs()).sum::<f64>() / calibrated.len() as f64);
    json!({"calibration_samples":calibrated.len(),"calibration_ece":ece,
        "hit_probability_brier":mean(calibrated.iter().map(|(p,h)|(p-h).powi(2))),
        "opportunities":rows.len(),"qualified":rows.iter().filter(|r|r["ever_qualified"]==true).count(),"pending":rows.iter().filter(|r|r["outcome"].is_null()).count(),
        "eligible":valid.len(),"invalid":rows.iter().filter(|r|r["learning_eligible"]!=true && !r["outcome"].is_null()).count(),
        "paper_entries":rows.iter().filter(|r|r["paper_entry"]==true).count(),"exchange_filled_opportunities":rows.iter().filter(|r|r["exchange_filled"]==true).count(),
        "observed_net_edge":mean(valid.iter().filter_map(|r|number(r,"observed_net_edge"))),
        "target_hit_rate":mean(valid.iter().map(|r|if r["outcome"]=="TARGET_HIT"{1.}else{0.})),
        "outcomes":outcomes,"invalid_reasons":reasons,"execution":execution,"opening_grades":grades,"non_execution_reasons":non_execution})
}
fn diagnose(summary: &Value, trading: &[Value], malformed: usize) -> Vec<Value> {
    let mut d = vec![];
    let eligible = summary["eligible"].as_u64().unwrap_or(0);
    let total = summary["opportunities"].as_u64().unwrap_or(0);
    if total == 0 {
        d.push(
            json!({"code":"NO_OPPORTUNITIES","message":"该范围暂无正式预测，不能评价策略效果。"}),
        );
    }
    if eligible < 100 {
        d.push(json!({"code":"INSUFFICIENT_SAMPLES","message":"有效样本不足 100 条；这只是最低样本提示，达到后仍需分版本进行时序验证。"}));
    }
    if summary["invalid"].as_u64().unwrap_or(0) > eligible {
        d.push(json!({"code":"HIGH_ATTRITION","message":"失效结果多于有效结果，先核对方向变化与数据中断；有效样本是筛选后的子集。"}));
    }
    if number(summary, "observed_net_edge").is_some_and(|v| v < 0.) {
        d.push(json!({"code":"NEGATIVE_OBSERVED_EDGE","message":"有效样本的平均观察净优势为负；检查费用与期限 / 屏障，不等同于实际账户亏损。"}));
    }
    if eligible > 0 && summary["calibration_samples"] == 0 {
        d.push(json!({"code":"NO_CALIBRATED_EVIDENCE","message":"有效结果没有可用的开场校准概率，不能把原始模型概率视为已验证命中率。"}));
    }
    if trading.is_empty() {
        d.push(json!({"code":"NO_CLOSED_TRADES","message":"尚无已平仓成交验证，不能将零成交判为策略通过。"}));
    }
    if malformed > 0 {
        d.push(json!({"code":"INCOMPLETE_RECORDS","message":"存在无法解析的历史记录，报告不完整；检查数据后再判断。"}));
    }
    d.push(json!({"code":"RESEARCH_REQUIRED","message":"复盘结果不是模型晋升结论；修改后仍需独立样本、影子比较和 PAPER 验证。"}));
    d
}
/// Daily export on startup and every five minutes; catches up the previous calendar day.
pub fn archive_previous_day(db: &Path, dir: &Path, now: u64) -> Result<Option<std::path::PathBuf>> {
    let date = date_at(now.saturating_sub(DAY))?;
    let path = dir.join(format!("review-{date}.json"));
    if path.exists() {
        return Ok(None);
    }
    let report = build(
        db,
        &ReviewQuery {
            date: Some(date),
            export: Some(true),
            ..Default::default()
        },
        now,
    )?;
    std::fs::create_dir_all(dir)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(&report)?).context("写入复盘日报失败")?;
    std::fs::rename(temp, &path)?;
    Ok(Some(path))
}
