//! Offline resimulation only. Never exposes a fixed-step switch to the production API.
use crate::{core::Engine, storage::Journal, types::Outcome};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub fn compare(path: &Path) -> Result<Value> {
    let mut event_driven = None;
    let mut reference = None;
    let mut events = 0;
    let mut ignored = 0;
    let mut event_outcomes = vec![];
    let mut reference_outcomes = vec![];
    for line in BufReader::new(File::open(path)?).lines() {
        let journal: Journal = serde_json::from_str(&line?)?;
        match journal {
            Journal::Start { engine } => {
                anyhow::ensure!(event_driven.is_none(), "multiple session starts");
                let mut e = Engine::new(engine.source == "demo");
                e.config = engine.config;
                e.config_version = engine.config_version;
                e.model = engine.model;
                e.trigger_config = engine.trigger_config;
                e.trigger_config_version = engine.trigger_config_version;
                let mut r = e.clone();
                r.reference_runtime = true;
                event_driven = Some(e);
                reference = Some(r);
            }
            Journal::Event { event } => {
                for (e, outcomes) in [
                    (&mut event_driven, &mut event_outcomes),
                    (&mut reference, &mut reference_outcomes),
                ] {
                    let e = e.as_mut().context("recording missing start")?;
                    e.ingest(event.clone());
                    for row in &e.records {
                        if row["kind"] == "outcome" {
                            outcomes
                                .push(serde_json::from_value::<Outcome>(row["payload"].clone())?);
                        }
                    }
                    e.records.clear();
                }
                events += 1;
            }
            Journal::Command { command, payload }
                if command == "config" || command == "trigger_config" =>
            {
                for e in [&mut event_driven, &mut reference] {
                    let e = e.as_mut().context("recording missing start")?;
                    e.command(&command, payload.clone())
                        .map_err(anyhow::Error::msg)?;
                    for row in &e.records {
                        if row["kind"] == "outcome" {
                            let o = serde_json::from_value::<Outcome>(row["payload"].clone())?;
                            if e.reference_runtime {
                                reference_outcomes.push(o);
                            } else {
                                event_outcomes.push(o);
                            }
                        }
                    }
                    e.records.clear();
                }
            }
            _ => {
                ignored += 1;
            }
        }
    }
    let e = event_driven.context("empty recording")?;
    let r = reference.unwrap();
    let mut matched = BTreeSet::new();
    let mut missed = vec![];
    let mut useful = 0;
    for o in &reference_outcomes {
        if !o.learning_eligible
            || !(o.prediction.episode.highest_qualification == "QUALIFIED"
                || o.observed_net_edge > 0. && o.prediction.qualification.net_edge > 0.)
        {
            continue;
        }
        useful += 1;
        let hit = event_outcomes
            .iter()
            .filter(|x| {
                !matched.contains(&x.prediction.prediction_id)
                    && x.learning_eligible
                    && x.prediction.symbol == o.prediction.symbol
                    && x.prediction.direction == o.prediction.direction
                    && x.prediction.config_version == o.prediction.config_version
                    && x.prediction.ts.abs_diff(o.prediction.ts)
                        <= e.trigger_config.candidate_ttl_ms
            })
            .min_by_key(|x| x.prediction.ts.abs_diff(o.prediction.ts));
        if let Some(x) = hit {
            matched.insert(x.prediction.prediction_id.clone());
        } else {
            missed.push(json!({"episode_id":o.prediction.prediction_id,"symbol":o.prediction.symbol,"opened_at":o.prediction.ts,"outcome":o.outcome_type,"observed_net_edge":o.observed_net_edge}));
        }
    }
    Ok(
        json!({"schema":"trigger-reference-comparison-v1","source":e.source,"events":events,"ignored_control_records":ignored,
        "trigger_config":e.trigger_config,"event_driven":{"evaluations":e.integrity.realtime_evaluations,"episodes":e.integrity.prediction_episodes_opened,"outcomes":event_outcomes.len(),"funnel":e.trigger_analytics.report()},
        "fixed_step_reference":{"evaluations":r.integrity.realtime_evaluations,"episodes":r.integrity.prediction_episodes_opened,"outcomes":reference_outcomes.len()},
        "useful_reference_episodes":useful,"matched_useful_episodes":matched.len(),"potentially_missed_episodes":missed,
        "evaluation_reduction":if r.integrity.realtime_evaluations==0 {0.}else{1.-e.integrity.realtime_evaluations as f64/r.integrity.realtime_evaluations as f64},
        "matching":"one-to-one same symbol/direction/config; opening time within candidate TTL; eligible qualified or positive observed/expected net edge reference outcomes",
        "limitations":["OBSERVE resimulation from initial frozen model/config, fresh warmup; not canonical order replay.","Only risk/trigger config commands are applied; model promotions, annotations, trading and private reports are excluded and counted.","Mid-price labels minus configured costs are not executable profit. Unclosed terminal episodes are right-censored.","No automatic threshold change, production reference mode, or model promotion."]}),
    )
}
