//! Offline H/T/S research reuses the same event-triggered engine and first-hit outcomes.
use crate::{core::Engine, model::Model, storage::Journal};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Read};
pub fn replay(path: &std::path::Path, symbol: &str) -> Result<Value> {
    let symbol = crate::universe::normalize(symbol).map_err(anyhow::Error::msg)?;
    let file = std::fs::File::open(path)?;
    let bytes = file.metadata()?.len();
    let mut reader = std::io::BufReader::new(file.take(bytes));
    let mut line = String::new();
    let mut variants: Vec<(Engine, usize, usize, f64, f64)> = vec![];
    let mut source = String::new();
    let mut events = 0_u64;
    let mut start = 0;
    let mut end = 0;
    while reader.read_line(&mut line)? > 0 {
        // A concurrent recorder may have a final uncommitted partial line; freeze the byte boundary.
        if !line.ends_with('\n') {
            break;
        }
        match serde_json::from_str::<Journal>(&line).context("invalid recording")? {
            Journal::Start { engine } => {
                if !variants.is_empty() {
                    anyhow::bail!("expected one recording session");
                }
                let template =
                    engine.symbols.get(&symbol).cloned().unwrap_or_else(|| {
                        crate::core::SymbolState::new(engine.asset_config(&symbol))
                    });
                source = engine.source.clone();
                start = engine.now;
                for horizon in [15, 30, 60, 120] {
                    for target_scale in [0.75, 1., 1.5] {
                        for stop_scale in [1., 1.5] {
                            let mut e = Engine::new(source == "demo");
                            let mut config = engine.asset_config(&symbol).clone();
                            config.horizon = horizon;
                            config.target = (config.target * target_scale).clamp(0.0005, 0.01);
                            config.stop = (config.stop * stop_scale).clamp(0.0002, 0.01);
                            config.validate().map_err(anyhow::Error::msg)?;
                            e.config = config;
                            e.model = Model::baseline(source == "demo", &e.config);
                            e.trigger_config = engine.asset_trigger(&symbol).0.clone();
                            e.symbols.clear();
                            let mut state = template.clone();
                            state.active = None;
                            state.plan = None;
                            state.qualification = None;
                            state.evaluation = None;
                            state.runtime = Default::default();
                            state.consumed_signal_epoch = None;
                            e.symbols.insert(symbol.clone(), state);
                            e.now = start;
                            e.mode = "OBSERVE".into();
                            variants.push((e, 0, 0, 0., 0.));
                        }
                    }
                }
            }
            Journal::Event { event } if event.symbol == symbol || event.symbol.is_empty() => {
                end = event.receive_ts;
                events += 1;
                for (e, n, hits, net, mae) in &mut variants {
                    e.ingest(event.clone());
                    for record in std::mem::take(&mut e.records) {
                        if record["kind"] == "outcome"
                            && record["payload"]["learning_eligible"] == true
                        {
                            *n += 1;
                            *hits += usize::from(record["payload"]["outcome_type"] == "TARGET_HIT");
                            *net += record["payload"]["observed_net_edge"]
                                .as_f64()
                                .unwrap_or(0.);
                            *mae = mae.max(record["payload"]["mae"].as_f64().unwrap_or(0.));
                        }
                    }
                }
            }
            _ => {}
        }
        line.clear();
    }
    anyhow::ensure!(!variants.is_empty(), "recording missing Start");
    let rows:Vec<_>=variants.into_iter().map(|(e,n,hits,net,mae)|json!({"horizon":e.config.horizon,"target":e.config.target,"stop":e.config.stop,"samples":n,"target_hits":hits,"observed_net_edge":if n>0{Some(net/n as f64)}else{None},"max_mae":mae,"episodes":e.integrity.prediction_episodes_opened,"evaluations":e.integrity.realtime_evaluations})).collect();
    Ok(
        json!({"schema":"asset-grid-v1","asset_key":symbol,"source":source,"recording":path,"snapshot_bytes":bytes,"events":events,"start":start,"end":end,"candidates":rows,"selection":"RESEARCH_ONLY_NO_AUTOMATIC_SELECTION","limitations":["24 H/T/S candidates use shared event-trigger runtime and objective first-hit labels.","Grid outcomes are exploratory, not a held-out profitability claim. Selected parameters require new chronological calibration and shadow validation.","Unfinished and invalidated episodes are excluded; recorded commands cannot change research parameters or authorize orders."]}),
    )
}
