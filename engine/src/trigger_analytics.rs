//! Auditable trigger funnel. Every link is persisted; API memory is a bounded recent window.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Link {
    pub trigger_id: String,
    pub symbol: String,
    pub families: Vec<String>,
    pub ts: u64,
    pub evaluations: u64,
    pub last_alpha_evaluation_id: Option<String>,
    pub episode_ids: BTreeSet<String>,
    pub qualified: BTreeSet<String>,
    pub admitted: BTreeSet<String>,
    pub executed: BTreeSet<String>,
    pub outcomes: BTreeMap<String, Value>,
    pub paper_trades: BTreeMap<String, f64>,
    pub live_trades: BTreeMap<String, f64>,
    pub closed_without_episode: bool,
    pub closed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TriggerAnalytics {
    pub links: BTreeMap<String, Link>,
    pub evicted_links: u64,
}
impl TriggerAnalytics {
    pub fn observe(&mut self, kind: &str, p: &Value) {
        let p = if kind == "live_trade_adjustment" {
            &p["trade"]
        } else {
            p
        };
        if kind == "market_pulse_trigger" {
            let id = p["trigger_id"].as_str().unwrap_or("").to_owned();
            let link = self.links.entry(id.clone()).or_insert_with(|| Link {
                trigger_id: id,
                symbol: p["symbol"].as_str().unwrap_or("").into(),
                ts: p["received_at_local_ts"].as_u64().unwrap_or(0),
                ..Default::default()
            });
            link.families =
                serde_json::from_value(p["trigger_families"].clone()).unwrap_or_default();
            while self.links.len() > 2000 {
                let oldest = self
                    .links
                    .values()
                    .filter(|l| {
                        l.closed
                            || (!l.episode_ids.is_empty()
                                && l.outcomes.len() == l.episode_ids.len())
                    })
                    .min_by_key(|l| l.ts)
                    .map(|l| l.trigger_id.clone());
                if let Some(id) = oldest {
                    self.links.remove(&id);
                    self.evicted_links += 1;
                } else {
                    break;
                }
            }
            return;
        }
        if kind == "trigger_evaluation" || kind == "trigger_candidate_closed" {
            if let Some(link) = p["trigger_id"]
                .as_str()
                .and_then(|id| self.links.get_mut(id))
            {
                if kind == "trigger_evaluation" {
                    link.evaluations += 1;
                    link.last_alpha_evaluation_id =
                        p["alpha_evaluation_id"].as_str().map(str::to_owned);
                } else {
                    link.closed = true;
                    link.closed_without_episode = link.episode_ids.is_empty()
                        && p["reason"] != "data_invalidated"
                        && p["reason"] != "config_changed"
                        && p["reason"] != "restart";
                }
            }
            return;
        }
        if kind == "prediction" {
            if let Some(link) = p["episode"]["origin_trigger_id"]
                .as_str()
                .and_then(|id| self.links.get_mut(id))
            {
                let id = p["prediction_id"].as_str().unwrap_or("").to_owned();
                link.episode_ids.insert(id.clone());
                if p["qualification"]["status"] == "QUALIFIED" {
                    link.qualified.insert(id);
                }
            }
            return;
        }
        let id = match kind {
            "episode_qualification" | "auto_admission" => p["episode_id"].as_str(),
            "outcome" => p["prediction"]["prediction_id"].as_str(),
            "trade" => p["position"]["plan"]["prediction_id"].as_str(),
            "paper_entry" | "live_trade" | "live_trade_adjustment" => p["prediction_id"].as_str(),
            "broker_order" => p["plan"]["prediction_id"].as_str(),
            _ => None,
        };
        let Some(id) = id else { return };
        let Some(link) = self.links.values_mut().find(|l| l.episode_ids.contains(id)) else {
            return;
        };
        match kind {
            "episode_qualification" => {
                link.qualified.insert(id.into());
            }
            "auto_admission" if p["decision"] == "ADMITTED" && p["context"] == "LIVE" => {
                link.admitted.insert(id.into());
            }
            "paper_entry" => {
                link.executed.insert(id.into());
            }
            "broker_order" if p["filled_qty"].as_f64().unwrap_or(0.) > 0. => {
                link.executed.insert(id.into());
            }
            "outcome" => {
                link.outcomes.insert(id.into(),json!({"type":p["outcome_type"],"eligible":p["learning_eligible"],"net_edge":p["observed_net_edge"]}));
            }
            "trade" => {
                link.paper_trades
                    .insert(id.into(), p["net_pnl"].as_f64().unwrap_or(0.));
            }
            "live_trade" | "live_trade_adjustment" => {
                link.live_trades.insert(
                    p["parent_id"].as_str().unwrap_or(id).into(),
                    p["net_pnl"].as_f64().unwrap_or(0.),
                );
            }
            _ => {}
        }
    }
    pub fn report(&self) -> Value {
        let mut groups: BTreeMap<String, Vec<&Link>> = BTreeMap::new();
        for link in self.links.values() {
            for f in &link.families {
                groups.entry(format!("family:{f}")).or_default().push(link);
            }
            groups
                .entry(format!("combination:{}", link.families.join("+")))
                .or_default()
                .push(link);
        }
        let rows:BTreeMap<_,_>=groups.into_iter().map(|(k,links)| {
            let count=links.len();let episodes:usize=links.iter().map(|l|l.episode_ids.len()).sum();
            let qualified:usize=links.iter().map(|l|l.qualified.len()).sum();let admitted:usize=links.iter().map(|l|l.admitted.len()).sum();
            let executed:usize=links.iter().map(|l|l.executed.len()).sum();
            let mut outcomes:BTreeMap<String,u64>=BTreeMap::new();let mut edges=vec![];
            for l in &links {for v in l.outcomes.values() {*outcomes.entry(v["type"].as_str().unwrap_or("UNKNOWN").into()).or_default()+=1;
                if v["eligible"]==true {if let Some(n)=v["net_edge"].as_f64(){edges.push(n);}}}}
            let no_edge=links.iter().filter(|l|l.closed_without_episode).count();
            (k,json!({"trigger_count":count,"full_evaluation_count":links.iter().map(|l|l.evaluations).sum::<u64>(),
                "episode_count":episodes,"episode_creation_rate":episodes as f64/count as f64,
                "qualified_count":qualified,"qualified_rate":qualified as f64/episodes.max(1) as f64,
                "auto_admitted_count":admitted,"auto_admitted_rate":admitted as f64/qualified.max(1) as f64,
                "executed_count":executed,"executed_rate":executed as f64/episodes.max(1) as f64,
                "outcome_distribution":outcomes,"observed_net_ev":if edges.is_empty(){None}else{Some(edges.iter().sum::<f64>()/edges.len() as f64)},
                "paper_realized_net_pnl":links.iter().flat_map(|l|l.paper_trades.values()).sum::<f64>(),
                "live_realized_net_pnl":links.iter().flat_map(|l|l.live_trades.values()).sum::<f64>(),
                "no_edge_count":no_edge,"false_no_edge_trigger_rate":no_edge as f64/count as f64}))
        }).collect();
        json!({"schema":"trigger-funnel-v1","scope":"most recent retained trigger links (up to 2000 completed plus unresolved)",
            "evicted_links":self.evicted_links,"groups":rows,"rates":"episode/trigger; qualified/episode; admitted/qualified (LIVE context only); executed/episode (confirmed PAPER or LIVE fill); no-edge/trigger excludes pending candidates"})
    }
}
