use micro_alpha_engine::{
    book::Book,
    core::{unrealized, Engine},
    feeds::demo_events,
    model::{self, Model},
    types::*,
};
use serde_json::json;
fn event(ts: u64, seq: i64, data: EventData) -> MarketEvent {
    MarketEvent {
        exchange: "binance".into(),
        symbol: "BTCUSDT".into(),
        exchange_ts: ts,
        receive_ts: ts,
        sequence_id: seq,
        data,
    }
}
fn snapshot(ts: u64, price: f64) -> Vec<MarketEvent> {
    vec![
        event(
            ts,
            10,
            EventData::Snapshot {
                bids: vec![[price - 0.1, 100.]],
                asks: vec![[price + 0.1, 100.]],
            },
        ),
        event(
            ts,
            11,
            EventData::Delta {
                first: 10,
                previous: 10,
                bids: vec![],
                asks: vec![],
            },
        ),
    ]
}
fn tick(e: &mut Engine, ts: u64) {
    e.ingest(event(ts, 0, EventData::Clock));
}
fn warm() -> Engine {
    let mut e = Engine::new(true);
    for i in 0..8 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            e.ingest(event);
        }
    }
    assert!(e.symbols["BTCUSDT"].active.is_some());
    e
}
fn healthy(e: &Engine) -> ModelHealth {
    let mut h = model::health(&[], &[], &e.config, false);
    h.trust = "NORMAL".into();
    h.samples = 50;
    h.calibration_error = 0.02;
    h
}
fn strong_prediction(e: &Engine) -> Prediction {
    let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
    p.direction = "LONG".into();
    p.raw_probabilities = [0.8, 0.1, 0.1];
    p.calibrated_probabilities = Some([0.8, 0.1, 0.1]);
    p
}

#[test]
fn book_bridges_snapshot_and_detects_gap() {
    let mut b = Book::default();
    let initial = snapshot(1000, 100.);
    b.apply(&initial[0]).unwrap();
    assert!(!b.ready(1000, 3000));
    b.apply(&initial[1]).unwrap();
    assert!(b.ready(1000, 3000));
    let delta = event(
        1100,
        12,
        EventData::Delta {
            first: 12,
            previous: 11,
            bids: vec![[99.9, 0.], [99.8, 5.]],
            asks: vec![],
        },
    );
    b.apply(&delta).unwrap();
    assert_eq!(b.top().unwrap().0, 99.8);
    let gap = event(
        1200,
        14,
        EventData::Delta {
            first: 14,
            previous: 13,
            bids: vec![],
            asks: vec![],
        },
    );
    assert!(b.apply(&gap).is_err());
    assert!(!b.valid);
    assert_eq!(b.gaps, 1);
    for e in snapshot(1300, 101.) {
        b.apply(&e).unwrap();
    }
    assert!(b.ready(1300, 3000));
}
#[test]
fn stale_crossed_and_malformed_book_fail_closed() {
    let mut b = Book::default();
    for e in snapshot(1000, 100.) {
        b.apply(&e).unwrap();
    }
    assert!(!b.ready(5001, 3000));
    assert!(b
        .apply(&event(
            1001,
            12,
            EventData::Delta {
                first: 12,
                previous: 11,
                bids: vec![[200., 1.]],
                asks: vec![]
            }
        ))
        .is_err());
    assert!(!b.valid);
    assert!(b
        .apply(&event(
            2000,
            1,
            EventData::Snapshot {
                bids: vec![[f64::NAN, 1.]],
                asks: vec![[100., 1.]]
            }
        ))
        .is_err());
}
#[test]
fn okx_sequence_reset_requires_matching_previous() {
    let mut b = Book::default();
    let mut e = snapshot(1000, 100.).remove(0);
    e.exchange = "okx".into();
    b.apply(&e).unwrap();
    e.sequence_id = 2;
    e.data = EventData::Delta {
        first: 2,
        previous: 10,
        bids: vec![],
        asks: vec![],
    };
    assert!(b.apply(&e).is_ok());
    assert_eq!(b.sequence, 2);
    e.data = EventData::Delta {
        first: 3,
        previous: 8,
        bids: vec![],
        asks: vec![],
    };
    assert!(b.apply(&e).is_err());
}
#[test]
fn first_hit_closes_once_and_cannot_be_rewritten() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    let price = p.start_price * (1. + p.sign() * p.target * 1.1);
    for x in snapshot(e.now + 100, price) {
        e.ingest(x);
    }
    let result = e.outcomes.last().unwrap();
    assert_eq!(result.outcome_type, "TARGET_HIT");
    assert_eq!(result.prediction.prediction_id, p.prediction_id);
    let n = e.outcomes.len();
    for x in snapshot(e.now + 100, p.start_price * (1. - p.sign() * p.stop * 3.)) {
        e.ingest(x);
    }
    assert_eq!(e.outcomes.len(), n);
}
#[test]
fn late_target_cannot_override_timeout() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    for x in snapshot(
        p.ts + p.horizon * 1000 + 1,
        p.start_price * (1. + p.sign() * p.target * 2.),
    ) {
        e.ingest(x);
    }
    assert!(e
        .outcomes
        .last()
        .unwrap()
        .outcome_type
        .starts_with("TIMEOUT"));
}
#[test]
fn stop_gap_records_actual_tail_loss() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    for x in snapshot(e.now + 1, p.start_price * (1. - p.sign() * p.stop * 3.)) {
        e.ingest(x);
    }
    let o = e.outcomes.last().unwrap();
    assert_eq!(o.outcome_type, "STRONG_OPPOSITE_MOVE");
    assert!(o.mae > p.stop * 2.);
}
#[test]
fn stale_data_invalidates_not_market_loss() {
    let mut e = warm();
    tick(&mut e, 1_020_000);
    let invalid: Vec<_> = e
        .outcomes
        .iter()
        .filter(|o| o.outcome_type == "INVALIDATED")
        .collect();
    assert_eq!(invalid.len(), 2);
    assert!(invalid.iter().all(|o| !o.learning_eligible));
    assert!(e.symbols["BTCUSDT"].active.is_none());
}
#[test]
fn positive_probability_negative_net_ev_rejected() {
    let mut e = warm();
    let p = strong_prediction(&e);
    e.config.fee_bps = 30.;
    let (q, plan) = e.qualify(
        &p,
        e.symbols["BTCUSDT"].features.as_ref().unwrap(),
        &healthy(&e),
    );
    assert!(q.net_edge < 0.);
    assert!(q.reasons.contains(&"net_ev".into()));
    assert!(plan.is_none());
}
#[test]
fn low_trust_and_missing_calibration_are_hard_gates() {
    let e = warm();
    let mut p = strong_prediction(&e);
    let mut h = healthy(&e);
    h.trust = "LOW".into();
    let f = e.symbols["BTCUSDT"].features.as_ref().unwrap();
    let (q, plan) = e.qualify(&p, f, &h);
    assert!(q.reasons.contains(&"model_trust".into()));
    assert!(plan.is_none());
    h.trust = "NORMAL".into();
    p.calibrated_probabilities = None;
    let (q, plan) = e.qualify(&p, f, &h);
    assert!(q.reasons.contains(&"calibration".into()));
    assert!(plan.is_none());
}
#[test]
fn plan_obeys_notional_leverage_risk_and_liquidity_caps() {
    let e = warm();
    let p = strong_prediction(&e);
    let f = e.symbols["BTCUSDT"].features.as_ref().unwrap();
    let (q, plan) = e.qualify(&p, f, &healthy(&e));
    let plan = plan.unwrap_or_else(|| panic!("{:?}", q.reasons));
    assert!(plan.position_notional <= e.config.max_position_notional);
    assert!(plan.effective_leverage <= e.config.max_effective_leverage);
    assert!(plan.planned_max_loss <= e.config.equity * e.config.risk_per_trade + 1e-9);
    assert!(plan.position_notional <= f.depth * e.config.depth_participation);
}
#[test]
fn daily_loss_and_drawdown_and_kill_block_new_risk() {
    let mut e = warm();
    let p = strong_prediction(&e);
    let f = e.symbols["BTCUSDT"].features.clone().unwrap();
    let h = healthy(&e);
    e.daily_pnl = -e.config.max_daily_loss;
    assert!(e.qualify(&p, &f, &h).1.is_none());
    e.daily_pnl = 0.;
    e.realized_pnl = -e.config.equity * e.config.max_drawdown;
    assert!(e.qualify(&p, &f, &h).1.is_none());
    e.realized_pnl = 0.;
    e.command("kill", json!({"active":true})).unwrap();
    assert!(e.qualify(&p, &f, &h).1.is_none());
    assert!(e.command("mode", json!({"mode":"PAPER"})).is_err());
}
#[test]
fn live_modes_cannot_be_enabled_by_api() {
    let mut e = warm();
    for mode in ["CONFIRM", "AUTO", "LIVE"] {
        assert!(e.command("mode", json!({"mode":mode})).is_err());
    }
    assert_eq!(e.mode, "OBSERVE");
}
#[test]
fn calibration_health_downgrades_without_weight_changes() {
    let mut e = warm();
    let p = strong_prediction(&e);
    for i in 0..40 {
        let mut p = p.clone();
        p.prediction_id = format!("p{i}");
        e.outcomes.push(Outcome {
            prediction: p,
            outcome_type: "STOP_HIT".into(),
            first_hit_ts: e.now,
            final_move: -0.001,
            mfe: 0.,
            mae: 0.001,
            prediction_error: "WRONG_WAY".into(),
            brier: 1.46,
            observed_net_edge: -0.002,
            learning_eligible: true,
            annotation: None,
        });
    }
    let before = serde_json::to_string(&e.model).unwrap();
    let h = model::health(&e.outcomes, &[], &e.config, false);
    assert_eq!(h.trust, "LOW");
    assert!(h.retrain_recommended);
    assert!(h.calibration_error > 0.7);
    assert_eq!(serde_json::to_string(&e.model).unwrap(), before);
}
#[test]
fn baseline_uncalibrated_and_demo_model_cannot_load_live() {
    let e = Engine::new(false);
    let f = Features::default();
    assert!(e.model.probabilities(&f, &e.config).1.is_none());
    let demo = Model::baseline(true, &e.config);
    assert!(demo.validate(false).is_err());
    let mut c = e.config.clone();
    c.horizon = 15;
    assert!(demo.probabilities(&f, &c).1.is_none());
}
#[test]
fn invalid_config_does_not_mutate_state() {
    let mut e = warm();
    let old = e.config.clone();
    let version = e.config_version;
    let mut c = old.clone();
    c.max_effective_leverage = 100.;
    assert!(e.command("config", json!(c)).is_err());
    assert_eq!(e.config, old);
    assert_eq!(e.config_version, version);
}
#[test]
fn replay_is_identical_with_commands_and_checkpoints() {
    let mut a = Engine::new(true);
    let mut b = Engine::new(true);
    for i in 0..240 {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            a.ingest(event.clone());
            let serialized = serde_json::to_string(&event).unwrap();
            b.ingest(serde_json::from_str(&serialized).unwrap());
        }
        if i == 30 {
            for e in [&mut a, &mut b] {
                e.command("mode", json!({"mode":"PAPER"})).unwrap();
            }
        }
        if i == 100 {
            b = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        }
        a.records.clear();
        b.records.clear();
    }
    assert!(!a.outcomes.is_empty());
    let a = serde_json::to_value(a).unwrap();
    let b = serde_json::to_value(b).unwrap();
    fn compare(a: &serde_json::Value, b: &serde_json::Value, path: &str) {
        match (a, b) {
            (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (k, v) in a {
                    compare(v, &b[k], &format!("{path}/{k}"));
                }
            }
            (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    compare(a, b, &format!("{path}/{i}"));
                }
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
    compare(&a, &b, "");
}
#[test]
fn annotation_cannot_change_ground_truth() {
    let mut e = warm();
    tick(&mut e, 1_020_000);
    let id = e.outcomes[0].prediction.prediction_id.clone();
    let truth = e.outcomes[0].outcome_type.clone();
    e.command(
        "annotate",
        json!({"prediction_id":id,"annotation":"market-wide reversal"}),
    )
    .unwrap();
    assert_eq!(truth, e.outcomes[0].outcome_type);
    assert!(!e.outcomes[0].learning_eligible);
}

fn paper_position(e: &mut Engine) {
    let p = strong_prediction(e);
    let (q, plan) = e.qualify(
        &p,
        e.symbols["BTCUSDT"].features.as_ref().unwrap(),
        &healthy(e),
    );
    let plan = plan.unwrap_or_else(|| panic!("{:?}", q.reasons));
    let price = p.current_price;
    e.positions.insert(
        "BTCUSDT".into(),
        Position {
            plan,
            qty: 0.01,
            entry_price: price,
            current_price: price,
            fees: 0.3,
            funding: 0.,
            realized_pnl: 0.,
            ts: e.now,
            last_rebalance: e.now,
            adjustments: 0,
            action: "HOLD".into(),
            reason: String::new(),
            target_notional: price * 0.01,
            executions: vec![],
            config: e.config.clone(),
            expected_cost: 1.,
            expected_edge: 0.001,
            cost_basis: price * 0.01,
        },
    );
}
#[test]
fn paper_exit_has_fees_and_fill_telemetry() {
    let mut e = warm();
    paper_position(&mut e);
    e.close_position("BTCUSDT", "TEST_EXIT");
    assert!(e.positions.is_empty());
    let t = e.trades.last().unwrap();
    assert!(t.position.fees > 0.3);
    assert_eq!(t.position.executions.len(), 1);
    assert!(t.net_pnl < 0.);
    assert_eq!(e.realized_pnl, t.net_pnl);
}
#[test]
fn stale_quotes_never_fill_and_pending_exit_recovers() {
    let mut e = warm();
    paper_position(&mut e);
    let before = e.positions["BTCUSDT"].qty;
    e.now += 5000;
    e.close_position("BTCUSDT", "MANUAL_EXIT");
    assert_eq!(e.positions["BTCUSDT"].action, "EXIT_PENDING");
    assert_eq!(e.positions["BTCUSDT"].qty, before);
    assert!(e.trades.is_empty());
    let price = e.positions["BTCUSDT"].current_price;
    for x in snapshot(e.now, price) {
        e.ingest(x);
    }
    let now = e.now + 1;
    tick(&mut e, now);
    assert!(e.positions.is_empty());
    assert_eq!(e.trades.len(), 1);
}
#[test]
fn loss_cannot_increase_position() {
    let mut e = warm();
    paper_position(&mut e);
    let qty = e.positions["BTCUSDT"].qty;
    let price = e.positions["BTCUSDT"].entry_price * 0.9997;
    for x in snapshot(e.now + 1, price) {
        e.ingest(x);
    }
    tick(&mut e, 1_008_000);
    assert!(e.positions.get("BTCUSDT").is_none_or(|p| p.qty <= qty));
}
#[test]
fn kill_hold_and_flatten_are_explicit() {
    let mut e = warm();
    paper_position(&mut e);
    e.command("kill", json!({"active":true})).unwrap();
    assert!(!e.positions.is_empty());
    e.command("kill", json!({"active":false})).unwrap();
    e.config.kill_behavior = "flatten".into();
    e.command("kill", json!({"active":true})).unwrap();
    assert!(e.positions.is_empty());
    assert_eq!(e.trades[0].exit_reason, "KILL_FLATTEN");
}
#[test]
fn config_locked_with_position_and_equity_includes_fees() {
    let mut e = warm();
    paper_position(&mut e);
    let p = &e.positions["BTCUSDT"];
    assert_eq!(unrealized(p), -0.3);
    assert!((e.account_equity() - (e.config.equity - 0.3)).abs() < 1e-7);
    assert!(e.command("config", json!(e.config)).is_err());
}

#[test]
fn aggregate_positions_share_account_cap() {
    let mut e = warm();
    paper_position(&mut e);
    let mut p = e.positions.remove("BTCUSDT").unwrap();
    p.plan.symbol = "ETHUSDT".into();
    p.qty = 4500. / p.current_price;
    e.positions.insert("ETHUSDT".into(), p);
    let p = strong_prediction(&e);
    let (q, plan) = e.qualify(
        &p,
        e.symbols["BTCUSDT"].features.as_ref().unwrap(),
        &healthy(&e),
    );
    let plan = plan.unwrap_or_else(|| panic!("{:?}", q.reasons));
    assert!(plan.position_notional <= 500. + 1e-8);
}

#[test]
fn thin_book_exit_is_partial_and_remaining_quantity_is_preserved() {
    let mut e = warm();
    paper_position(&mut e);
    let venue = e
        .symbols
        .get_mut("BTCUSDT")
        .unwrap()
        .venues
        .get_mut("binance")
        .unwrap();
    let bid = *venue.book.bids.last_key_value().unwrap().0;
    venue.book.bids.clear();
    venue.book.bids.insert(bid, 0.002);
    e.close_position("BTCUSDT", "MANUAL_EXIT");
    let p = &e.positions["BTCUSDT"];
    assert!((p.qty - 0.008).abs() < 1e-9);
    assert_eq!(p.action, "EXIT_PENDING");
    assert!(p.executions[0].liquidity_limited);
    assert!(e.trades.is_empty());
}

#[test]
fn paper_mode_executes_only_new_qualified_prediction_and_records_chain() {
    let mut e = warm();
    e.config.min_samples = 20;
    e.model.temperature = 1.;
    let mut weights = vec![vec![0.; 10]; 3];
    weights[0][0] = 0.8_f64.ln();
    weights[1][0] = 0.1_f64.ln();
    weights[2][0] = 0.1_f64.ln();
    e.model.weights = Some(weights);
    let p = strong_prediction(&e);
    for i in 0..20 {
        let mut p = p.clone();
        p.prediction_id = format!("history-{i}");
        p.qualification.net_edge = 0.001;
        e.outcomes.push(Outcome {
            prediction: p,
            outcome_type: if i < 16 { "TARGET_HIT" } else { "STOP_HIT" }.into(),
            first_hit_ts: e.now,
            final_move: if i < 16 { 0.003 } else { -0.001 },
            mfe: 0.003,
            mae: 0.0004,
            prediction_error: String::new(),
            brier: 0.1,
            observed_net_edge: 0.001,
            learning_eligible: true,
            annotation: None,
        });
    }
    let s = e.symbols.get_mut("BTCUSDT").unwrap();
    s.active = None;
    s.last_prediction_ts = 0;
    e.command("mode", json!({"mode":"PAPER"})).unwrap();
    for x in demo_events(8, 1_008_000) {
        e.ingest(x);
    }
    let p = e
        .positions
        .get("BTCUSDT")
        .expect("qualified PAPER signal should execute");
    assert_eq!(
        p.plan.prediction_id,
        e.symbols["BTCUSDT"].active.as_ref().unwrap().prediction_id
    );
    assert_eq!(p.executions.len(), 1);
    assert!(p.cost_basis <= e.config.max_position_notional);
    for state in ["QUALIFIED", "TRADE_READY", "TRACKING"] {
        assert!(e
            .records
            .iter()
            .any(|r| r["kind"] == "prediction_state" && r["payload"]["to"] == state));
    }
    for kind in ["prediction", "qualification", "plan", "execution"] {
        assert!(e.records.iter().any(|r| r["kind"] == kind));
    }
}

#[tokio::test]
async fn durable_checkpoint_restores_risk_and_annotations_without_rewriting_ground_truth() {
    use micro_alpha_engine::storage::{Batch, Journal, Store};
    let dir = std::env::temp_dir().join(format!(
        "micro-alpha-test-{}-{}",
        std::process::id(),
        micro_alpha_engine::feeds::now_ms()
    ));
    let (store, restored) = Store::open(&dir, "demo").unwrap();
    assert!(restored.is_none());
    let mut e = warm();
    tick(&mut e, 1_020_000);
    e.daily_pnl = -42.;
    let id = e.outcomes[0].prediction.prediction_id.clone();
    e.command(
        "annotate",
        json!({"prediction_id":id,"annotation":"exchange anomaly"}),
    )
    .unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    store
        .tx
        .send(Batch {
            journal: Journal::Start {
                engine: Box::new(e.clone()),
            },
            records: std::mem::take(&mut e.records),
            checkpoint: Some(serde_json::to_string(&e).unwrap()),
            committed: Some(tx),
        })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    let history = Store::history(&store.path, "outcome", 100).unwrap();
    let o = history
        .iter()
        .find(|r| r["payload"]["prediction"]["prediction_id"] == id)
        .unwrap();
    assert_eq!(o["payload"]["annotation"], "exchange anomaly");
    assert_eq!(o["payload"]["outcome_type"], "INVALIDATED");
    drop(store);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let (store, restored) = Store::open(&dir, "demo").unwrap();
    let mut restored = restored.unwrap();
    restored.restart();
    assert_eq!(restored.daily_pnl, -42.);
    assert_eq!(restored.mode, "OBSERVE");
    assert!(!restored.symbols["BTCUSDT"].venues["binance"].book.valid);
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn neutral_signal_and_hysteresis_prevent_arbitrary_direction() {
    use micro_alpha_engine::core::signal_direction;
    let c = Config::default();
    assert_eq!(signal_direction(0., "FLAT", &c), "FLAT");
    assert_eq!(signal_direction(0.25, "FLAT", &c), "FLAT");
    assert_eq!(signal_direction(0.25, "LONG", &c), "LONG");
    assert_eq!(signal_direction(0.1, "LONG", &c), "FLAT");
    assert_eq!(signal_direction(-0.25, "SHORT", &c), "SHORT");
    assert_eq!(signal_direction(-0.4, "LONG", &c), "SHORT");
    let mut e = Engine::new(true);
    for i in 0..10 {
        let ts = 1_000_000 + i * 1000;
        for x in snapshot(ts, 100_000.) {
            e.ingest(x);
        }
        tick(&mut e, ts);
    }
    let s = &e.symbols["BTCUSDT"];
    assert_eq!(s.candidate_direction, "FLAT");
    assert_eq!(s.lifecycle, "NO_SIGNAL");
    assert!(s.active.is_none());
    assert!(e.outcomes.is_empty());
}

#[test]
fn completed_prediction_stays_on_original_card_during_cooldown() {
    let mut e = warm();
    let p = e.symbols["BTCUSDT"].active.clone().unwrap();
    let ts = e.now + 100;
    let price = p.start_price * (1. + p.sign() * p.target * 1.1);
    for x in snapshot(ts, price) {
        e.ingest(x);
    }
    assert_eq!(e.symbols["BTCUSDT"].lifecycle, "COMPLETED");
    for i in 0..4 {
        let now = ts + i * 1000;
        let mut xs = snapshot(now, price);
        if let EventData::Snapshot { bids, .. } = &mut xs[0].data {
            bids[0][1] = 300.;
        }
        for x in xs {
            e.ingest(x);
        }
        tick(&mut e, now);
        let s = &e.symbols["BTCUSDT"];
        assert!(s.active.is_none());
        assert!(s.plan.is_none());
        assert_eq!(s.lifecycle, "COMPLETED");
        let last = s.last_result.as_ref().unwrap();
        assert_eq!(last.prediction.prediction_id, p.prediction_id);
        assert_eq!(last.prediction.raw_probabilities, p.raw_probabilities);
        assert_eq!(last.outcome_type, "TARGET_HIT");
    }
    let states: Vec<_> = e
        .records
        .iter()
        .filter(|r| r["kind"] == "prediction_state" && r["payload"]["symbol"] == "BTCUSDT")
        .filter_map(|r| r["payload"]["to"].as_str())
        .collect();
    for state in ["WATCH", "EMERGING", "TRACKING", "COMPLETED"] {
        assert!(states.contains(&state));
    }
    let view = e.view();
    assert_eq!(
        view["symbols"]["BTCUSDT"]["last_result"]["prediction"]["prediction_id"],
        p.prediction_id
    );
}

#[test]
fn checkpoint_without_new_fields_restores_and_restarts_safely() {
    let mut original = warm();
    original.daily_pnl = -17.;
    let mut value = serde_json::to_value(original).unwrap();
    for key in [
        "signal_enter",
        "signal_exit",
        "signal_confirm_ms",
        "result_hold_ms",
        "max_opposite_rate",
        "max_mae_stop_multiple",
        "high_trust_samples",
    ] {
        value["config"].as_object_mut().unwrap().remove(key);
    }
    for key in ["distributions", "regime_calibrations"] {
        value["model"].as_object_mut().unwrap().remove(key);
    }
    for symbol in value["symbols"].as_object_mut().unwrap().values_mut() {
        for key in [
            "lifecycle",
            "lifecycle_since",
            "last_result",
            "completed_at",
        ] {
            symbol.as_object_mut().unwrap().remove(key);
        }
        for key in [
            "regime",
            "regime_samples",
            "regime_calibration_error",
            "distribution_score",
            "distribution_status",
        ] {
            symbol["health"].as_object_mut().unwrap().remove(key);
        }
    }
    let mut e: Engine = serde_json::from_value(value).unwrap();
    e.restart();
    assert_eq!(e.daily_pnl, -17.);
    assert_eq!(e.mode, "OBSERVE");
    assert_eq!(e.config.signal_enter, 0.35);
    assert_eq!(
        e.symbols["BTCUSDT"]
            .last_result
            .as_ref()
            .unwrap()
            .outcome_type,
        "INVALIDATED"
    );
    assert!(e.symbols["BTCUSDT"].active.is_none());
}

#[test]
fn training_distribution_and_sparse_regime_block_trade_qualification() {
    use micro_alpha_engine::model::{Distribution, RegimeCalibration};
    let mut e = warm();
    let f = e.symbols["BTCUSDT"].features.clone().unwrap();
    e.model.source = "live".into();
    let d = Distribution {
        mean: Model::vector(&f),
        scale: vec![0.1; 10],
        max_z: 6.,
        volatility_boundary: f.volatility + 0.001,
        spread_boundary: f.spread_bps + 2.,
        samples: 100,
    };
    e.model.distributions.insert("BTCUSDT".into(), d.clone());
    e.model.distributions.insert("ETHUSDT".into(), d);
    e.model.regime_calibrations.insert(
        "BTCUSDT:NORMAL".into(),
        RegimeCalibration {
            temperature: 2.,
            samples: 50,
        },
    );
    assert!(e.model.validate(false).is_ok());
    assert_eq!(e.model.distribution_score(&f), Some(0.));
    assert!(e.model.probabilities(&f, &e.config).1.is_some());
    let mut unusual = f.clone();
    unusual.momentum += 0.1;
    let h = e.model.health(&[], &[], &e.config, &unusual);
    assert_eq!(h.distribution_status, "OUT_OF_DISTRIBUTION");
    assert!(h.regime_drift);
    let (q, plan) = e.qualify(&strong_prediction(&e), &unusual, &h);
    assert!(q.reasons.iter().any(|s| s == "regime"));
    assert!(plan.is_none());
    unusual = f.clone();
    unusual.volatility += 0.002;
    assert_eq!(e.model.regime(&unusual), "BTCUSDT:HIGH_VOL");
    assert!(e.model.probabilities(&unusual, &e.config).1.is_none());
    assert_eq!(e.model.health(&[], &[], &e.config, &unusual).trust, "LOW");
    e.model.distributions.get_mut("BTCUSDT").unwrap().scale[1] = 0.;
    assert!(e.model.validate(false).is_err());
    e.model.distributions.clear();
    assert!(e.model.validate(false).is_err());
    assert_eq!(
        e.model.health(&[], &[], &e.config, &f).distribution_status,
        "UNAVAILABLE"
    );
    assert!(
        e.model.probabilities(&f, &e.config).1.is_none(),
        "missing profile must not reuse old calibration"
    );
}

#[test]
fn healthy_aggregate_cannot_hide_deterioration_in_current_regime() {
    use micro_alpha_engine::model::{Distribution, RegimeCalibration};
    let e = warm();
    let f = e.symbols["BTCUSDT"].features.clone().unwrap();
    let mut m = e.model.clone();
    m.source = "live".into();
    m.distributions.insert(
        "BTCUSDT".into(),
        Distribution {
            mean: Model::vector(&f),
            scale: vec![1.; 10],
            max_z: 6.,
            volatility_boundary: f.volatility + 0.0001,
            spread_boundary: f.spread_bps + 2.,
            samples: 100,
        },
    );
    m.regime_calibrations.insert(
        "BTCUSDT:NORMAL".into(),
        RegimeCalibration {
            temperature: 1.,
            samples: 50,
        },
    );
    let rows: Vec<_> = (0..100)
        .map(|i| {
            let mut p = strong_prediction(&e);
            p.features = f.clone();
            p.qualification.net_edge = 0.001;
            if i >= 30 {
                p.features.volatility += 0.0002;
            }
            // Current regime hits 60%, other regime 80%, against predicted 80%.
            let win = if i < 30 { i < 18 } else { i < 86 };
            Outcome {
                prediction: p,
                outcome_type: if win { "TARGET_HIT" } else { "STOP_HIT" }.into(),
                first_hit_ts: e.now,
                final_move: 0.,
                mfe: 0.,
                mae: 0.0001,
                prediction_error: String::new(),
                brier: 0.1,
                observed_net_edge: 0.001,
                learning_eligible: true,
                annotation: None,
            }
        })
        .collect();
    let h = m.health(&rows, &[], &e.config, &f);
    assert!(h.calibration_error < e.config.calibration_warning);
    assert_eq!(h.regime_samples, 30);
    assert!((h.regime_calibration_error.unwrap() - 0.2).abs() < 1e-10);
    assert_eq!(h.trust, "DEGRADED");
    assert!(h.retrain_recommended);
}
