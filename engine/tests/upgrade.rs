use micro_alpha_engine::{core::Engine, feeds::demo_events, live::AccountSnapshot, types::*};
use serde_json::json;

fn warm() -> Engine {
    let mut e = Engine::new_reference(true);
    for i in 0..8 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    e
}
fn candidates() -> Engine {
    let mut e = warm();
    e.config.auto_required_model_trust = "NORMAL".into();
    e.config.min_net_pnl = 0.01;
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        let s = &e.symbols[symbol];
        let mut p = s.active.clone().unwrap();
        p.direction = "LONG".into();
        p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        let mut h = s.health.clone();
        h.trust = "HIGH".into();
        h.samples = 100;
        h.calibration_error = 0.;
        let (q, plan) = e.qualify(&p, s.features.as_ref().unwrap(), &h);
        assert_eq!(q.status, "QUALIFIED", "{:?}", q.reasons);
        let s = e.symbols.get_mut(symbol).unwrap();
        s.health = h;
        s.qualification = Some(q.clone());
        s.plan = plan.clone();
        let ev = s.evaluation.as_mut().unwrap();
        ev.qualification = Some(q);
        ev.target_position = plan;
        ev.calibrated_probabilities = p.calibrated_probabilities;
    }
    e
}
fn reason(e: &Engine, code: &str) {
    let d = e.admission_decisions(false, None);
    assert!(
        d.iter()
            .all(|d| d.decision == "SKIPPED" && d.reason_codes.iter().any(|r| r == code)),
        "{d:?}"
    );
}
#[test]
fn qualification_does_not_imply_admission_or_execution() {
    let mut e = candidates();
    let original = serde_json::to_value(&e.symbols["BTCUSDT"].active).unwrap();
    e.config.auto_min_net_edge = 0.01;
    e.admission_shadow_tick();
    reason(&e, "AUTO_EDGE_THRESHOLD_NOT_MET");
    assert!(e.live.orders.is_empty());
    assert!(e
        .symbols
        .values()
        .all(|s| s.qualification.as_ref().unwrap().status == "QUALIFIED"));
    assert_eq!(
        serde_json::to_value(&e.symbols["BTCUSDT"].active).unwrap(),
        original
    );
    assert!(e.records.iter().any(|r| r["kind"] == "auto_admission"));
    // Skipping capital allocation never interrupts prediction truth.
    for i in 8..100 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    assert!(!e.outcomes.is_empty());
}
#[test]
fn admission_fails_on_staleness_trust_execution_and_budget_independently() {
    let e = candidates();
    let mut stale = e.clone();
    stale.now += 4000;
    reason(&stale, "STALE_EVALUATION");
    let mut low = e.clone();
    for s in low.symbols.values_mut() {
        s.health.trust = "LOW".into();
    }
    reason(&low, "MODEL_TRUST_INSUFFICIENT");
    let mut bad = e.clone();
    for s in bad.symbols.values_mut() {
        s.venues.get_mut("okx").unwrap().connected = false;
    }
    reason(&bad, "EXECUTION_QUALITY_FAIL");
    let mut no_budget = e.clone();
    no_budget.daily_pnl = -no_budget.config.max_daily_loss;
    reason(&no_budget, "RISK_BUDGET_UNAVAILABLE");
    let mut negative = e;
    for s in negative.symbols.values_mut() {
        s.evaluation
            .as_mut()
            .unwrap()
            .qualification
            .as_mut()
            .unwrap()
            .net_edge = -0.001;
    }
    reason(&negative, "AUTO_EDGE_THRESHOLD_NOT_MET");
}
#[test]
fn concurrent_priority_is_versioned_deterministic_and_reserves_budget() {
    let mut e = candidates();
    for (symbol, edge) in [("BTCUSDT", 0.001), ("ETHUSDT", 0.002)] {
        e.symbols
            .get_mut(symbol)
            .unwrap()
            .evaluation
            .as_mut()
            .unwrap()
            .qualification
            .as_mut()
            .unwrap()
            .net_edge = edge;
    }
    let a = e.admission_decisions(false, None);
    let b = e.admission_decisions(false, None);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    assert_eq!(a[0].symbol, "ETHUSDT");
    assert_eq!(a[0].decision, "ADMITTED");
    assert_eq!(a[1].decision, "SKIPPED");
    assert!(a[1]
        .reason_codes
        .contains(&"BETTER_OPPORTUNITY_ACTIVE".into()));
    assert!(
        a[0].risk_budget_after_if_admitted.available_loss < a[0].risk_budget_before.available_loss
    );
    assert_eq!(a[0].priority_tuple.policy_version, "priority-v1");
    assert_eq!(a[0].competing_episode_ids.len(), 1);
    // A failing higher-ranked candidate cannot steal budget.
    e.symbols.get_mut("ETHUSDT").unwrap().health.trust = "LOW".into();
    let d = e.admission_decisions(false, None);
    assert_eq!(
        d.iter().find(|d| d.symbol == "BTCUSDT").unwrap().decision,
        "ADMITTED"
    );
}
#[test]
fn config_cannot_weaken_auto_below_qualification_or_use_unknown_priority() {
    let mut c = Config::default();
    c.auto_min_net_edge = c.min_net_edge / 2.;
    assert!(c.validate().is_err());
    c.auto_min_net_edge = c.min_net_edge;
    assert!(c.validate().is_ok());
    c.admission_priority_policy_version = "llm-ranking".into();
    assert!(c.validate().is_err());
    c.admission_priority_policy_version = "priority-v1".into();
    c.auto_required_model_trust = "LOW".into();
    assert!(c.validate().is_err());
}
#[test]
fn auto_reentry_requires_cooldown_fresh_alpha_and_cost_coverage() {
    let mut e = candidates();
    e.source = "live".into();
    e.live.configured = true;
    e.live.healthy = true;
    e.live.cap = 500.;
    e.live.account = Some(AccountSnapshot {
        ts: e.now,
        equity: 10000.,
        available: 10000.,
        one_way: true,
        ..Default::default()
    });
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        e.live.trades.push(micro_alpha_engine::live::LiveTrade {
            feedback: None,
            parent_id: symbol.into(),
            prediction_id: "old".into(),
            symbol: symbol.into(),
            closed_ts: e.now - 1000,
            entry_notional: 100.,
            exit_notional: 100.,
            fees: 0.,
            funding_income: 0.,
            net_pnl: -1.,
            expected_net_pnl: 1.,
        });
    }
    let d = e.admission_decisions(true, None);
    assert!(d
        .iter()
        .all(|d| d.reason_codes.contains(&"REENTRY_COOLDOWN".into())
            && d.reason_codes.contains(&"FRESH_ALPHA_REQUIRED".into())));
    e.config.reentry_cooldown_seconds = 1;
    for s in e.symbols.values_mut() {
        s.active.as_mut().unwrap().episode.signal_epoch = e.now;
        let q = s
            .evaluation
            .as_mut()
            .unwrap()
            .qualification
            .as_mut()
            .unwrap();
        q.net_edge = e.config.auto_min_net_edge;
    }
    let d = e.admission_decisions(true, None);
    assert!(d
        .iter()
        .all(|d| !d.reason_codes.contains(&"REENTRY_COOLDOWN".into())
            && !d.reason_codes.contains(&"FRESH_ALPHA_REQUIRED".into())
            && d.reason_codes.contains(&"REENTRY_COST_NOT_COVERED".into())));
}
#[test]
fn evaluation_ticks_do_not_inflate_episodes_and_opening_snapshot_is_immutable() {
    let mut e = warm();
    let before = e.integrity.realtime_evaluations;
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    let opened = e.integrity.prediction_episodes_opened;
    for i in 1..10 {
        e.ingest(MarketEvent {
            exchange: "clock".into(),
            symbol: String::new(),
            exchange_ts: 1_007_000 + i * 100,
            receive_ts: 1_007_000 + i * 100,
            sequence_id: 0,
            data: EventData::Clock,
        });
    }
    assert_eq!(e.integrity.realtime_evaluations, before);
    assert_eq!(e.integrity.prediction_episodes_opened, opened);
    for event in demo_events(8, 1_008_000) {
        e.ingest(event);
    }
    let active = e.symbols["BTCUSDT"].active.as_ref().unwrap();
    assert_eq!(active.prediction_id, p.prediction_id);
    assert_eq!(active.raw_probabilities, p.raw_probabilities);
    assert!(active.episode.latest_evaluated_at > p.episode.latest_evaluated_at);
    assert_eq!(e.integrity.realtime_evaluations, before + 2);
    assert_eq!(e.integrity.prediction_episodes_opened, opened);
    assert!(e.live.orders.is_empty() && e.trades.is_empty());
}
#[test]
fn episode_results_are_unique_under_restart_and_replay() {
    let mut a = Engine::new_reference(true);
    let mut b = Engine::new_reference(true);
    for i in 0..240 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            a.ingest(event.clone());
            b.ingest(event);
        }
    }
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert!(a.integrity.realtime_evaluations > a.integrity.prediction_episodes_opened * 10);
    let ids: std::collections::BTreeSet<_> = a
        .outcomes
        .iter()
        .map(|o| o.prediction.prediction_id.clone())
        .collect();
    assert_eq!(ids.len(), a.outcomes.len());
    a.restart();
    let count = a.outcomes.len();
    a.restart();
    assert_eq!(a.outcomes.len(), count);
    assert!(a
        .outcomes
        .iter()
        .filter(|o| o.outcome_type.ends_with("INVALIDATED"))
        .all(|o| !o.learning_eligible));
    assert_eq!(
        a.integrity.prediction_episodes_closed as usize,
        a.outcomes.len()
    );
}
#[test]
fn config_changes_and_new_admission_fields_survive_checkpoint() {
    let mut e = candidates();
    e.admission_shadow_tick();
    let restored: Engine = serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&e.admission).unwrap(),
        serde_json::to_value(restored.admission).unwrap()
    );
    let mut config = serde_json::to_value(&e.config).unwrap();
    config["auto_min_net_edge"] = json!(0.0007);
    e.command("config", config).unwrap();
    assert!(e
        .admission_decisions(false, None)
        .iter()
        .all(|d| d.reason_codes.contains(&"EPISODE_EXPIRED".into())));
}

fn stationary_book(e: &mut Engine, ts: u64, mid: f64) {
    for venue in ["binance", "okx"] {
        let seq = ts as i64;
        e.ingest(MarketEvent {
            exchange: venue.into(),
            symbol: "BTCUSDT".into(),
            exchange_ts: ts,
            receive_ts: ts,
            sequence_id: seq,
            data: EventData::Snapshot {
                bids: vec![[mid * 0.999999, 1000.]],
                asks: vec![[mid * 1.000001, 1000.]],
            },
        });
        if venue == "binance" {
            e.ingest(MarketEvent {
                exchange: venue.into(),
                symbol: "BTCUSDT".into(),
                exchange_ts: ts,
                receive_ts: ts,
                sequence_id: seq + 1,
                data: EventData::Delta {
                    first: seq,
                    previous: seq,
                    bids: vec![],
                    asks: vec![],
                },
            });
        }
    }
    e.ingest(MarketEvent {
        exchange: "clock".into(),
        symbol: String::new(),
        exchange_ts: ts,
        receive_ts: ts,
        sequence_id: 0,
        data: EventData::Clock,
    });
}
#[test]
fn signal_invalidation_is_explicit_and_excluded_from_first_hit_calibration() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    let s = e.symbols.get_mut("BTCUSDT").unwrap();
    s.chart.clear();
    for v in s.venues.values_mut() {
        v.trades.clear();
        v.prices.clear();
    }
    for i in 8..12 {
        stationary_book(&mut e, 1_000_000 + i * 1000, p.start_price);
    }
    let o = e
        .outcomes
        .iter()
        .find(|o| o.prediction.prediction_id == p.prediction_id)
        .unwrap();
    assert_eq!(o.outcome_type, "SIGNAL_INVALIDATED");
    assert!(o.prediction.episode.first_hit.is_none());
    assert!(o.prediction.episode.first_hit_at.is_none());
    assert!(!o.learning_eligible);
    assert!(o.prediction.episode.closed_at.is_some());
}
#[test]
fn horizon_return_is_observed_separately_without_changing_target_truth() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    stationary_book(
        &mut e,
        1_008_000,
        p.start_price * (1. + p.sign() * p.target * 1.1),
    );
    let o = e
        .outcomes
        .iter()
        .find(|o| o.prediction.prediction_id == p.prediction_id)
        .unwrap();
    assert_eq!(o.outcome_type, "TARGET_HIT");
    let hit = o.first_hit_ts;
    for ts in (1_009_000..=p.ts + p.horizon * 1000).step_by(1000) {
        stationary_book(&mut e, ts, p.start_price * (1. - p.sign() * 0.0005));
    }
    let o = e
        .outcomes
        .iter()
        .find(|o| o.prediction.prediction_id == p.prediction_id)
        .unwrap();
    assert_eq!(o.outcome_type, "TARGET_HIT");
    assert_eq!(o.first_hit_ts, hit);
    assert_eq!(o.prediction.episode.first_hit_at, Some(hit));
    assert!(o.prediction.episode.first_hit.is_some());
    let horizon = o.horizon_observation.as_ref().unwrap();
    assert_eq!(horizon.status, "OBSERVED");
    assert!((horizon.final_move_at_timeout.unwrap() + 0.0005).abs() < 1e-10);
    assert_eq!(
        e.records
            .iter()
            .filter(|r| r["kind"] == "outcome"
                && r["payload"]["prediction"]["prediction_id"] == p.prediction_id)
            .count(),
        1
    );
    assert_eq!(
        e.records
            .iter()
            .filter(|r| r["kind"] == "outcome_horizon"
                && r["payload"]["prediction_id"] == p.prediction_id)
            .count(),
        1
    );
}

#[test]
fn alpha_and_qualification_cadence_are_independently_configurable() {
    let mut e = warm();
    e.config.alpha_eval_interval_ms = 2000;
    e.config.qualification_eval_interval_ms = 1000;
    let before = e.integrity.realtime_evaluations;
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    stationary_book(&mut e, 1_008_000, p.start_price);
    let ev = e.symbols["BTCUSDT"].evaluation.as_ref().unwrap();
    assert_eq!(ev.evaluated_at, 1_007_000);
    assert_eq!(ev.qualification_evaluated_at, 1_008_000);
    assert_eq!(e.integrity.realtime_evaluations, before);
    stationary_book(&mut e, 1_009_000, p.start_price);
    assert_eq!(
        e.symbols["BTCUSDT"]
            .evaluation
            .as_ref()
            .unwrap()
            .evaluated_at,
        1_009_000
    );
    assert_eq!(
        e.symbols["BTCUSDT"].active.as_ref().unwrap().prediction_id,
        p.prediction_id
    );
}
