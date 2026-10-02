//! Offline barrier/horizon search. No network, order submission, or production config writes.
use anyhow::{Context, Result};
use micro_alpha_engine::{core::Engine, storage::Journal, types::*};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
};
#[derive(Clone)]
struct Trial {
    p: Prediction,
    last: f64,
    mfe: f64,
    mae: f64,
}
fn key(p: &Prediction) -> String {
    format!("{}s/{:.4}/{:.4}", p.horizon, p.target, p.stop)
}
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: cargo run --example grid -- recording.jsonl")?;
    let mut engine: Option<Engine> = None;
    let mut active: Vec<Trial> = vec![];
    let mut rows = vec![];
    let mut invalid = 0;
    for line in BufReader::new(File::open(path)?).lines() {
        let j: Journal = serde_json::from_str(&line?)?;
        match j {
            Journal::Start { engine: e } => {
                engine = Some(*e);
                invalid += active.len();
                active.clear();
            }
            Journal::RejectedCommand {
                command,
                payload,
                error,
            } => {
                let result = engine
                    .as_mut()
                    .context("missing start")?
                    .command(&command, payload);
                anyhow::ensure!(result == Err(error), "rejected command replay mismatch");
            }
            Journal::Command { command, payload } => {
                engine
                    .as_mut()
                    .context("missing start")?
                    .command(&command, payload)
                    .map_err(anyhow::Error::msg)?;
            }
            Journal::Event { event } => {
                let e = engine.as_mut().context("missing start")?;
                e.ingest(event.clone());
                active.retain_mut(|t| {
                    let s = &e.symbols[&t.p.symbol];
                    let book = &s.venues["binance"].book;
                    if !book.valid
                        || e.now.saturating_sub(book.receive_ts) > e.config.max_stale_ms
                        || e.now.abs_diff(book.exchange_ts) > e.config.max_stale_ms
                    {
                        invalid += 1;
                        return false;
                    }
                    // A Binance REST snapshot needs its first delta before prices are usable.
                    if !book.bridged {
                        return true;
                    }
                    let deadline = t.p.ts + t.p.horizon * 1000;
                    let current = book.mid().unwrap();
                    let relevant = matches!(event.data, EventData::Clock)
                        || event.exchange == "binance"
                            && event.symbol == t.p.symbol
                            && matches!(
                                event.data,
                                EventData::Snapshot { .. } | EventData::Delta { .. }
                            );
                    if !relevant {
                        return true;
                    }
                    if e.now <= deadline {
                        t.last = current;
                        let m = (current / t.p.start_price - 1.) * t.p.sign();
                        t.mfe = t.mfe.max(m);
                        t.mae = t.mae.max(-m);
                    }
                    let m = (t.last / t.p.start_price - 1.) * t.p.sign();
                    if m >= t.p.target || m <= -t.p.stop || e.now >= deadline {
                        rows.push((
                            key(&t.p),
                            t.p.ts,
                            e.now,
                            m - t.p.qualification.costs.total,
                            t.mae,
                        ));
                        false
                    } else {
                        true
                    }
                });
                for r in &e.records {
                    if r["kind"] == "prediction" {
                        let original: Prediction = serde_json::from_value(r["payload"].clone())?;
                        for horizon in [5, 15, 30, 60, 120] {
                            for target in [0.0005, 0.001, 0.0015, 0.002, 0.003] {
                                for stop in [0.0005, 0.001, 0.002] {
                                    let mut p = original.clone();
                                    p.horizon = horizon;
                                    p.target = target;
                                    p.stop = stop;
                                    active.push(Trial {
                                        last: p.start_price,
                                        p,
                                        mfe: 0.,
                                        mae: 0.,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(e) = &mut engine {
            e.records.clear();
        }
    }
    let mut times: Vec<_> = rows.iter().map(|r| r.1).collect();
    times.sort_unstable();
    times.dedup();
    if times.len() < 100 {
        anyhow::bail!(
            "parameter search needs at least 100 forecast timestamps; observed {}",
            times.len()
        );
    }
    let split = times[times.len() * 7 / 10];
    let mut training: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (k, _, end, edge, _) in &rows {
        if *end < split {
            training.entry(k.clone()).or_default().push(*edge);
        }
    }
    let winner = training
        .iter()
        .filter(|(_, v)| v.len() >= 30)
        .max_by(|(_, a), (_, b)| {
            (a.iter().sum::<f64>() / a.len() as f64)
                .total_cmp(&(b.iter().sum::<f64>() / b.len() as f64))
        })
        .context("insufficient purged training samples")?
        .0
        .clone();
    let test: Vec<_> = rows
        .iter()
        .filter(|r| r.0 == winner && r.1 >= split)
        .collect();
    let mean = test.iter().map(|r| r.3).sum::<f64>() / test.len().max(1) as f64;
    let mut edge: Vec<_> = test.iter().map(|r| r.3).collect();
    edge.sort_by(f64::total_cmp);
    let training: BTreeMap<_, _> = training
        .iter()
        .map(|(k, v)| {
            (
                k,
                json!({"samples":v.len(),"net_edge":v.iter().sum::<f64>()/v.len() as f64}),
            )
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"barrier-search-v1","source":engine.as_ref().map(|e|&e.source),"split_ts":split,"train":training,"selected_using_train_only":winner,"test":{"samples":test.len(),"net_edge":mean,"tail_p05":edge.get(edge.len()/20),"max_mae":test.iter().map(|r|r.4).fold(0.,f64::max)},"invalidated":invalid,"right_censored":active.len(),"research_pass":test.len()>=30&&mean>0.,"limitations":["Forecast timestamps and directions are inherited from the recorded champion; selection bias remains.","This searches label economics, not calibrated probabilities or executable returns.","A changed definition needs new walk-forward training, Shadow, and explicit configuration approval.","No production configuration or model has been changed."]})
        )?
    );
    Ok(())
}
