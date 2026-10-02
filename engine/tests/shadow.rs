//! Synthetic fixtures verify control flow; they are not trading-performance evidence.
use micro_alpha_engine::{
    core::Engine,
    feeds::demo_events,
    model::{Distribution, Model, RegimeCalibration},
    shadow::{Forecast, PairedOutcome, PairedPrediction},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn warm() -> Engine {
    let mut e = Engine::new(true);
    for i in 0..8 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    e
}
fn bundle(e: &Engine) -> Value {
    let mut model = Model::baseline(true, &e.config);
    model.model_version = "synthetic-challenger-v1".into();
    model.weights = Some(
        vec![[0.8_f64.ln()], [0.1_f64.ln()], [0.1_f64.ln()]]
            .into_iter()
            .map(|v| {
                let mut row = vec![0.; 10];
                row[0] = v[0];
                row
            })
            .collect(),
    );
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        let f = e.symbols[symbol].features.as_ref().unwrap();
        model.distributions.insert(
            symbol.into(),
            Distribution {
                mean: Model::vector(f),
                scale: vec![100.; 10],
                max_z: 6.,
                volatility_boundary: 0.1,
                spread_boundary: 100.,
                samples: 100,
            },
        );
        model.regime_calibrations.insert(
            format!("{symbol}:NORMAL"),
            RegimeCalibration {
                temperature: 1.,
                samples: 100,
            },
        );
    }
    let metrics = json!({"samples":40,"selected":30,"ece":0.02,"brier":0.1,"observed_net_edge":0.001,"unit_notional_drawdown":0.01,"tail_loss_p05":-0.002,"strong_opposite_rate":0.01});
    let windows: Vec<_> = (0..3)
        .map(|i| {
            let mut w = metrics.clone();
            w["train"] = json!({"start":0,"end":100+i*100,"samples":100+i*100});
            w["calibration"] = json!({"start":110+i*100,"end":120+i*100,"samples":30});
            w["test"] = json!({"start":130+i*100,"end":190+i*100,"samples":40});
            w["calibration_coverage"] = json!(1.);
            w
        })
        .collect();
    model.training_window = windows[2]["train"].to_string();
    model.validation_window = windows[2]["test"].to_string();
    let artifact = serde_json::to_string_pretty(&model).unwrap();
    let mut checks = json!({});
    for key in [
        "enough_oos",
        "both_symbols",
        "calibration",
        "calibration_coverage",
        "positive_observed_edge",
        "window_stability",
        "regime_calibration",
        "training_distribution",
        "tail",
        "baseline_comparison",
    ] {
        checks[key] = json!(true);
    }
    json!({"artifact":artifact,"report":{
        "sha256":format!("{:x}",Sha256::digest(artifact.as_bytes())),"source":"demo","status":"PAPER_RESEARCH_GATE_PASS","checks":checks,
        "counts":{"test":120,"symbols":{"BTCUSDT":60,"ETHUSDT":60}},
        "walk_forward":{"refit_each_fold":true,"purged_boundaries":true,"test_windows_disjoint":true},
        "chronological_test_windows":windows,"final_model_regimes":{"BTCUSDT:NORMAL":metrics,"ETHUSDT:NORMAL":metrics}
    }})
}
fn start(e: &mut Engine) -> String {
    e.command("shadow_start", bundle(e)).unwrap();
    e.shadow.as_ref().unwrap().run_id.clone()
}
fn passing_evidence(e: &mut Engine) {
    let now = e.now;
    let run = e.shadow.as_mut().unwrap();
    run.outcomes = (0..120)
        .map(|i| {
            let symbol = if i % 2 == 0 { "BTCUSDT" } else { "ETHUSDT" };
            let good = i % 10 < 8;
            let p = PairedPrediction {
                prediction_id: format!("synthetic-pair-{i}"),
                feature_snapshot_id: format!("synthetic-feature-{i}"),
                symbol: symbol.into(),
                ts: now - 100_000 + i * 500,
                direction: "LONG".into(),
                horizon: 60,
                target: 0.003,
                stop: 0.001,
                regime: format!("{symbol}:NORMAL"),
                champion: Forecast {
                    model_version: run.champion_version.clone(),
                    raw: [0.5, 0.25, 0.25],
                    calibrated: None,
                    in_distribution: false,
                    selected: false,
                    net_edge: 0.,
                },
                candidate: Forecast {
                    model_version: run.candidate.model_version.clone(),
                    raw: [0.8, 0.1, 0.1],
                    calibrated: Some([0.8, 0.1, 0.1]),
                    in_distribution: true,
                    selected: true,
                    net_edge: 0.001,
                },
            };
            PairedOutcome {
                prediction: p,
                outcome_type: if good { "TARGET_HIT" } else { "STOP_HIT" }.into(),
                first_hit_ts: now - 90_000 + i * 500,
                observed_net_edge: if good { 0.0018 } else { -0.0022 },
                learning_eligible: true,
            }
        })
        .collect();
}

#[test]
fn import_rejects_mismatched_evidence_atomically() {
    let mut e = warm();
    let good = bundle(&e);
    let initial = serde_json::to_value(&e).unwrap();
    for bad in [
        {
            let mut v = good.clone();
            v["artifact"] = json!(format!("{} ", v["artifact"].as_str().unwrap()));
            v
        },
        {
            let mut v = good.clone();
            v["report"]["source"] = json!("live");
            v
        },
        {
            let mut v = good.clone();
            v["report"]["checks"]["window_stability"] = json!(false);
            v
        },
        {
            let mut v = good.clone();
            v["report"]["chronological_test_windows"][2]["test"]["end"] = json!(e.now + 1);
            v
        },
        {
            let mut v = good.clone();
            v["report"]["chronological_test_windows"][0]["unit_notional_drawdown"] = json!(0.9);
            v
        },
        {
            let mut v = good.clone();
            v["policy"] = json!({"min_samples":0});
            v
        },
    ] {
        assert!(e.command("shadow_start", bad).is_err());
        assert_eq!(serde_json::to_value(&e).unwrap(), initial);
    }
    e.command("shadow_start", good).unwrap();
    assert_eq!(e.model.model_version, "score-demo-v1");
    assert!(e.command("shadow_start", bundle(&e)).is_err());
}

#[test]
fn paired_shadow_does_not_change_champion_predictions_or_execute() {
    let mut e = warm();
    let mut control = e.clone();
    start(&mut e);
    for i in 8..240 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event.clone());
            control.ingest(event);
        }
    }
    let signature = |engine: &Engine| {
        engine
            .outcomes
            .iter()
            .map(|o| {
                (
                    o.prediction.ts,
                    o.prediction.direction.clone(),
                    o.prediction.probabilities(),
                    o.outcome_type.clone(),
                    o.observed_net_edge,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&e), signature(&control));
    assert_eq!(e.realized_pnl, control.realized_pnl);
    assert!(e.positions.is_empty() && e.trades.is_empty());
    let run = e.shadow.as_ref().unwrap();
    assert!(run.outcomes.len() > 4);
    for pair in &run.outcomes {
        let o = e
            .outcomes
            .iter()
            .find(|o| o.prediction.prediction_id == pair.prediction.prediction_id)
            .unwrap();
        assert_eq!(pair.outcome_type, o.outcome_type);
        assert_eq!(
            pair.prediction.feature_snapshot_id,
            o.prediction.feature_snapshot_id
        );
        assert_eq!(
            pair.prediction.champion.calibrated,
            o.prediction.calibrated_probabilities
        );
        assert_eq!(pair.observed_net_edge, o.observed_net_edge);
    }
    assert!(e.records.iter().any(|r| r["kind"] == "shadow_outcome"));
    assert!(e
        .outcomes
        .iter()
        .all(|o| o.prediction.model_version == "score-demo-v1"));
}

#[test]
fn no_backfill_and_config_changes_cancel_inflight_shadow() {
    let mut e = warm();
    let run_id = start(&mut e);
    assert!(e.shadow.as_ref().unwrap().pending.is_empty());
    for i in 8..80 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    let before = e.shadow.as_ref().unwrap().outcomes.len();
    let c = serde_json::to_value(&e.config).unwrap();
    e.command("config", c).unwrap();
    assert_eq!(e.shadow.as_ref().unwrap().status, "PAUSED_CONFIG");
    assert!(e.shadow.as_ref().unwrap().pending.is_empty());
    for i in 80..150 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    assert_eq!(e.shadow.as_ref().unwrap().outcomes.len(), before);
    assert!(e
        .command("shadow_promote", json!({"run_id":run_id}))
        .is_err());
}

#[test]
fn sample_quality_and_freshness_are_hard_promotion_gates() {
    let mut e = warm();
    let id = start(&mut e);
    let model = e.model.model_version.clone();
    assert!(e.command("shadow_promote", json!({"run_id":id})).is_err());
    passing_evidence(&mut e);
    assert_eq!(e.shadow_view().unwrap()["promotion_ready"], true);
    e.shadow.as_mut().unwrap().outcomes.truncate(119);
    assert_eq!(
        e.shadow_view().unwrap()["comparison"]["checks"]["samples"],
        false
    );
    passing_evidence(&mut e);
    for row in &mut e.shadow.as_mut().unwrap().outcomes[0..30] {
        row.learning_eligible = false;
    }
    assert_eq!(
        e.shadow_view().unwrap()["comparison"]["checks"]["data_quality"],
        false
    );
    passing_evidence(&mut e);
    for row in &mut e.shadow.as_mut().unwrap().outcomes {
        row.first_hit_ts = 1;
    }
    assert_eq!(
        e.shadow_view().unwrap()["comparison"]["checks"]["fresh_evidence"],
        false
    );
    assert!(e.command("shadow_promote", json!({"run_id":id})).is_err());
    assert_eq!(e.model.model_version, model);
}

#[test]
fn poor_window_or_regime_cannot_hide_behind_positive_average() {
    let mut e = warm();
    start(&mut e);
    passing_evidence(&mut e);
    for row in &mut e.shadow.as_mut().unwrap().outcomes[..40] {
        row.observed_net_edge = -0.0001;
    }
    let view = e.shadow_view().unwrap();
    assert!(
        view["comparison"]["candidate"]["observed_net_edge"]
            .as_f64()
            .unwrap()
            > 0.
    );
    assert_eq!(view["comparison"]["checks"]["window_stability"], false);
    passing_evidence(&mut e);
    for row in &mut e.shadow.as_mut().unwrap().outcomes[..10] {
        row.prediction.regime = "BTCUSDT:STRESSED".into();
    }
    assert_eq!(
        e.shadow_view().unwrap()["comparison"]["checks"]["regime_stability"],
        false
    );
}

#[test]
fn promotion_is_explicit_guarded_and_rollback_preserves_account() {
    let mut e = warm();
    let id = start(&mut e);
    passing_evidence(&mut e);
    let before = e.model.model_version.clone();
    assert_eq!(e.shadow_view().unwrap()["promotion_ready"], true);
    assert_eq!(
        e.model.model_version, before,
        "ready must never auto-promote"
    );
    assert!(e
        .command("shadow_promote", json!({"run_id":"stale-page"}))
        .is_err());
    e.mode = "PAPER".into();
    assert!(e.command("shadow_promote", json!({"run_id":id})).is_err());
    e.mode = "OBSERVE".into();
    e.kill_switch = true;
    assert!(e.command("shadow_promote", json!({"run_id":id})).is_err());
    e.kill_switch = false;
    e.daily_pnl = -12.;
    e.command("shadow_promote", json!({"run_id":id})).unwrap();
    assert_eq!(e.model.model_version, "synthetic-challenger-v1");
    assert_eq!(e.mode, "OBSERVE");
    assert_eq!(e.daily_pnl, -12.);
    assert!(e
        .symbols
        .values()
        .all(|s| s.active.is_none() && s.plan.is_none()));
    assert!(e
        .outcomes
        .iter()
        .filter(|o| o.outcome_type == "INVALIDATED")
        .all(|o| !o.learning_eligible));
    assert!(e
        .command("model_rollback", json!({"current_hash":"stale-hash"}))
        .is_err());
    e.command(
        "model_rollback",
        json!({"current_hash":e.model.artifact_hash}),
    )
    .unwrap();
    assert_eq!(e.model.model_version, before);
    assert_eq!(e.daily_pnl, -12.);
    assert!(e.previous_model.is_none());
    assert!(e.command("mode", json!({"mode":"AUTO"})).is_err());
}

#[test]
fn shadow_and_commands_survive_checkpoints_with_deterministic_results() {
    let mut a = warm();
    let initial = a.clone();
    let request = bundle(&a);
    a.command("shadow_start", request.clone()).unwrap();
    let mut b: Engine = serde_json::from_str(&serde_json::to_string(&initial).unwrap()).unwrap();
    b.command("shadow_start", request).unwrap();
    for i in 8..180 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            a.ingest(event.clone());
            b.ingest(event);
        }
        a.records.clear();
        b.records.clear();
        if i % 17 == 0 {
            b = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        }
    }
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let old = b.shadow.as_ref().unwrap().outcomes.len();
    let pending = b.shadow.as_ref().unwrap().pending.len();
    b.restart();
    let run = b.shadow.as_ref().unwrap();
    assert_eq!(run.status, "RUNNING");
    assert!(run.pending.is_empty());
    assert_eq!(run.outcomes.len(), old + pending);
    assert!(run.outcomes.iter().skip(old).all(|r| !r.learning_eligible));
}
