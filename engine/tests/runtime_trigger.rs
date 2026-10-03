use micro_alpha_engine::{core::Engine, market_pulse::*, storage::Journal, types::*};
use serde_json::json;
use std::collections::BTreeMap;

fn frame(e: &mut Engine, step: u64, pressure: f64) {
    let ts = 1_000_000 + step * 250;
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        for venue in ["okx", "binance"] {
            let seq = e.symbols[symbol].venues[venue].book.sequence;
            let bids = vec![[9999.9, 10. * (1. + pressure)]];
            let asks = vec![[10000.1, 10. * (1. - pressure)]];
            let data = if step == 0 {
                EventData::Snapshot { bids, asks }
            } else {
                EventData::Delta {
                    first: seq,
                    previous: seq,
                    bids,
                    asks,
                }
            };
            e.ingest(MarketEvent {
                exchange: venue.into(),
                symbol: symbol.into(),
                exchange_ts: ts,
                receive_ts: ts,
                sequence_id: seq + 1,
                data,
            });
            if step == 0 && venue == "binance" {
                let seq = e.symbols[symbol].venues[venue].book.sequence;
                e.ingest(MarketEvent {
                    exchange: venue.into(),
                    symbol: symbol.into(),
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
    }
    e.ingest(MarketEvent {
        exchange: "clock".into(),
        symbol: "".into(),
        exchange_ts: ts,
        receive_ts: ts,
        sequence_id: 0,
        data: EventData::Clock,
    });
}
fn warm() -> Engine {
    let mut e = Engine::new(true);
    for i in 0..80 {
        frame(&mut e, i, 0.);
    }
    e.records.clear();
    e
}
fn burst() -> Engine {
    let mut e = warm();
    for i in 80..100 {
        frame(&mut e, i, 0.8);
    }
    e
}

#[test]
fn quiet_market_has_zero_full_evaluations_with_continuous_books_and_chart() {
    let mut e = Engine::new(true);
    for i in 0..1200 {
        frame(&mut e, i, 0.);
        e.records.clear();
    }
    assert_eq!(e.integrity.realtime_evaluations, 0);
    assert_eq!(e.integrity.prediction_episodes_opened, 0);
    assert!(e
        .symbols
        .values()
        .all(|s| s.runtime.state == RuntimeState::Idle
            && s.venues.values().all(|v| v.book.ready(e.now, 3000))));
    assert!(e.symbols["BTCUSDT"].chart.len() > 100);
    assert!(e.positions.is_empty() && e.live.orders.is_empty());
}
#[test]
fn any_extreme_or_two_distinct_abnormal_only() {
    let mut values = BTreeMap::from([(FAMILIES[0].into(), Severity::Abnormal)]);
    assert!(!combined(&values));
    values.insert(FAMILIES[1].into(), Severity::Abnormal);
    assert!(combined(&values));
    values.clear();
    values.insert(FAMILIES[3].into(), Severity::Extreme);
    assert!(combined(&values));
}
#[test]
fn robust_normalization_is_scale_invariant_and_uses_past_only() {
    let history = [1., 2., 3., 4., 5.];
    assert!((robust_z(12., &history) - robust_z(120., &history.map(|v| v * 10.))).abs() < 1e-9);
    assert_eq!(robust_z(1., &[1.; 32]), 0.);
    assert!(robust_z(2., &[1.; 32]) > 6.);
}
#[test]
fn all_four_families_have_normal_abnormal_extreme_fixtures() {
    let c = TriggerConfig::default();
    for (family, _) in FAMILIES.iter().enumerate() {
        for (value, expected) in [
            (1., Severity::Normal),
            (1.2, Severity::Abnormal),
            (1.5, Severity::Extreme),
        ] {
            let mut p = Pulse::default();
            p.snapshot.ts = 1000;
            p.previous_mid = 100.;
            p.previous_depth = 100.;
            p.previous_ofi = 1.;
            p.baseline = (0..32).map(|i| (i, [1.; 8])).collect();
            let input = PulseInput {
                symbol: "BTCUSDT",
                ts: 2000,
                exchange_ts: 2000,
                config_version: 1,
                mid: 100.,
                ofi: 1.,
                bid_depth: 50.,
                ask_depth: 50.,
                spread_bps: 1.,
            };
            // Choose the causal historical level that produces the requested standardized surprise.
            let mut input = input;
            match family {
                0 => input.ofi = value,
                1 => {
                    p.trade(2000, value.exp() - 1., true);
                }
                2 => {
                    input.bid_depth = 50. * (1. + value / 2.);
                    input.ask_depth = 50. * (1. - value / 2.);
                    p.baseline.iter_mut().for_each(|(_, v)| v[4] = 0.5);
                }
                3 => {
                    input.mid = 100. * (1. + value);
                }
                _ => unreachable!(),
            }
            let result = p.sample(input, &c).unwrap();
            assert_eq!(
                result.severity_by_family[FAMILIES[family]], expected,
                "family {family}, {value}: {:?}",
                result.normalized_metrics
            );
        }
    }
}
#[test]
fn trigger_evaluates_immediately_and_deduplicates_candidate_and_episode() {
    let mut e = warm();
    let before = e.integrity.realtime_evaluations;
    frame(&mut e, 80, 0.8);
    assert!(e.integrity.realtime_evaluations > before);
    assert!(e.symbols["BTCUSDT"].runtime.origin.is_some());
    assert_eq!(e.symbols["BTCUSDT"].runtime.last_full_eval, e.now);
    assert!(e.positions.is_empty());
    let id = e.symbols["BTCUSDT"]
        .runtime
        .origin
        .as_ref()
        .unwrap()
        .trigger_id
        .clone();
    for i in 81..100 {
        frame(&mut e, i, 0.8);
    }
    let p = e.symbols["BTCUSDT"]
        .active
        .as_ref()
        .expect("confirmed directional episode");
    assert_eq!(p.episode.origin_trigger_id.as_ref(), Some(&id));
    assert_eq!(e.trigger_analytics.links[&id].episode_ids.len(), 1);
    assert_eq!(e.symbols["BTCUSDT"].runtime.state, RuntimeState::Active);
    let n = e.symbols["BTCUSDT"].runtime.full_evaluations;
    for i in 100..104 {
        frame(&mut e, i, 0.8);
    }
    assert_eq!(e.symbols["BTCUSDT"].runtime.full_evaluations, n + 1);
    assert!(e.live.orders.is_empty());
}
#[test]
fn no_edge_candidate_expires_and_is_counted_without_creating_episode() {
    let mut e = warm();
    e.config.signal_enter = 2.9;
    frame(&mut e, 80, 0.2);
    assert_eq!(e.symbols["BTCUSDT"].runtime.state, RuntimeState::Candidate);
    let id = e.symbols["BTCUSDT"]
        .runtime
        .origin
        .as_ref()
        .unwrap()
        .trigger_id
        .clone();
    for i in 81..125 {
        frame(&mut e, i, 0.2);
    }
    assert!(e.trigger_analytics.links[&id].closed_without_episode);
    assert!(e.trigger_analytics.links[&id].episode_ids.is_empty());
}
#[test]
fn data_failures_preempt_cadence_and_clear_qualification_immediately() {
    for failure in ["gap", "invalid", "disconnect", "skew", "stale"] {
        let mut e = burst();
        let before = e.integrity.realtime_evaluations;
        let ts = e.now + 1;
        let seq = e.symbols["BTCUSDT"].venues["binance"].book.sequence;
        let data = match failure {
            "gap" => EventData::Delta {
                first: seq + 5,
                previous: seq + 4,
                bids: vec![],
                asks: vec![],
            },
            "invalid" => EventData::Snapshot {
                bids: vec![[10001., 1.]],
                asks: vec![[10000., 1.]],
            },
            "disconnect" => EventData::Health {
                connected: false,
                reason: "fixture".into(),
            },
            "skew" => EventData::Delta {
                first: seq,
                previous: seq,
                bids: vec![],
                asks: vec![],
            },
            _ => EventData::Clock,
        };
        let ts = if failure == "stale" { ts + 4000 } else { ts };
        e.ingest(MarketEvent {
            symbol: "BTCUSDT".into(),
            exchange: "binance".into(),
            receive_ts: ts,
            exchange_ts: if failure == "skew" { ts - 4000 } else { ts },
            sequence_id: seq + 1,
            data,
        });
        assert!(e.symbols["BTCUSDT"].active.is_none(), "{failure}");
        assert!(e.symbols["BTCUSDT"].qualification.is_none());
        assert_eq!(e.integrity.realtime_evaluations, before);
        assert!(
            e.records
                .iter()
                .any(|r| r["kind"] == "protection_trigger" && r["payload"]["type"] == "DATA"),
            "{failure}"
        );
        assert!(e.outcomes.iter().any(|o| !o.learning_eligible));
    }
}
#[test]
fn trigger_config_is_versioned_validated_and_not_a_runtime_mode_switch() {
    let mut e = warm();
    frame(&mut e, 80, 0.8);
    assert!(!e.trigger_analytics.links.is_empty());
    let mut c = e.trigger_config.clone();
    c.extreme_z = 7.;
    e.command("trigger_config", json!(c)).unwrap();
    assert_eq!(e.trigger_config_version, 2);
    assert_eq!(e.config_version, 2);
    assert!(e
        .trigger_analytics
        .links
        .values()
        .all(|l| l.closed && !l.closed_without_episode));
    let mut bad = json!(c);
    bad["idle_mode"] = json!("fixed_step");
    assert!(e.command("trigger_config", bad).is_err());
    c.protection_data_stale = false;
    assert!(e.command("trigger_config", json!(c)).is_err());
    e.mode = "PAPER".into();
    assert!(e
        .command("trigger_config", json!(TriggerConfig::default()))
        .is_err());
}
#[test]
fn production_trigger_state_and_analytics_roundtrip_and_replay_exactly() {
    let mut e = burst();
    let restored: Engine = serde_json::from_slice(&serde_json::to_vec(&e).unwrap()).unwrap();
    let mut restored = restored;
    for i in 100..360 {
        frame(&mut e, i, 0.8);
        frame(&mut restored, i, 0.8);
    }
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert!(e
        .outcomes
        .iter()
        .any(|o| o.outcome_type.starts_with("TIMEOUT")));
    assert!(e
        .trigger_analytics
        .links
        .values()
        .any(|l| !l.outcomes.is_empty()));
    let mut restarted = e.clone();
    restarted.restart();
    assert_eq!(restarted.mode, "OBSERVE");
    assert!(restarted
        .symbols
        .values()
        .all(|s| s.runtime.pulse.baseline.is_empty()));
}
#[test]
fn replay_comparison_is_deterministic_and_censors_incomplete_episodes() {
    use std::io::Write;
    let path =
        std::env::temp_dir().join(format!("trigger-comparison-{}.jsonl", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap();
    let start = Engine::new(true);
    writeln!(
        f,
        "{}",
        serde_json::to_string(&Journal::Start {
            engine: Box::new(start)
        })
        .unwrap()
    )
    .unwrap();
    for i in 0..240 {
        for event in micro_alpha_engine::feeds::demo_events(i, 1_000_000 + i * 1000) {
            writeln!(
                f,
                "{}",
                serde_json::to_string(&Journal::Event { event }).unwrap()
            )
            .unwrap();
        }
    }
    drop(f);
    let a = micro_alpha_engine::replay_compare::compare(&path).unwrap();
    let b = micro_alpha_engine::replay_compare::compare(&path).unwrap();
    assert_eq!(a, b);
    assert!(a["fixed_step_reference"]["evaluations"].as_u64().unwrap() > 0);
    assert!(a["potentially_missed_episodes"].is_array());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn risk_spikes_interrupt_position_cadence_and_keep_existing_risk_authority() {
    use micro_alpha_engine::live::{AccountPosition, AccountSnapshot};
    for (failure, reason) in [
        ("spread", "SPREAD_SPIKE"),
        ("depth", "DEPTH_COLLAPSE"),
        ("slippage", "SLIPPAGE_SPIKE"),
        ("volatility", "VOLATILITY_OR_FAST_MOVE"),
        ("fast_move", "VOLATILITY_OR_FAST_MOVE"),
        ("execution", "EXECUTION_HEALTH"),
    ] {
        let mut e = burst();
        e.live.account = Some(AccountSnapshot {
            positions: BTreeMap::from([(
                "BTCUSDT".into(),
                AccountPosition {
                    quantity: 1.,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        });
        let before = e.integrity.realtime_evaluations;
        e.symbols.get_mut("BTCUSDT").unwrap().runtime.last_full_eval = e.now;
        if failure == "slippage" {
            e.config.max_slippage = 0.00001;
        }
        if failure == "volatility" {
            e.symbols.get_mut("BTCUSDT").unwrap().chart =
                std::collections::VecDeque::from([(e.now - 1000, 9000.), (e.now, 10000.)]);
        }
        if failure == "execution" {
            e.live.configured = true;
            e.command("broker_error", json!({"error":"fixture"}))
                .unwrap();
        } else {
            let seq = e.symbols["BTCUSDT"].venues["binance"].book.sequence;
            let ts = e.now + 1;
            let (bid, ask, quantity) = match failure {
                "spread" => (9990., 10010., 10.),
                "depth" => (9999.9, 10000.1, 0.001),
                "fast_move" => (10029.9, 10030.1, 10.),
                _ => (9999.9, 10000.1, 10.),
            };
            e.ingest(MarketEvent {
                exchange: "binance".into(),
                symbol: "BTCUSDT".into(),
                exchange_ts: ts,
                receive_ts: ts,
                sequence_id: seq + 1,
                data: EventData::Delta {
                    first: seq,
                    previous: seq,
                    bids: vec![[9999.9, 0.], [bid, quantity]],
                    asks: vec![[10000.1, 0.], [ask, quantity]],
                },
            });
        }
        assert!(e.integrity.realtime_evaluations > before, "{failure}");
        assert!(
            e.records.iter().any(|r| r["kind"] == "protection_trigger"
                && r["payload"]["reason_codes"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(reason))),
            "{failure}"
        );
        // An external position is not adopted or traded by a trigger. Existing account/LOW gates win.
        assert!(e.live.orders.is_empty());
    }
}

#[test]
fn analytics_counts_confirmed_fills_and_keeps_paper_live_and_funding_separate() {
    use micro_alpha_engine::trigger_analytics::TriggerAnalytics;
    let mut a = TriggerAnalytics::default();
    let trigger = json!({"trigger_id":"t","symbol":"BTCUSDT","trigger_families":["PRICE_VELOCITY"],"received_at_local_ts":1});
    a.observe("market_pulse_trigger", &trigger);
    a.observe("market_pulse_trigger", &trigger);
    a.observe("prediction",&json!({"prediction_id":"p","episode":{"origin_trigger_id":"t"},"qualification":{"status":"QUALIFIED"}}));
    a.observe(
        "auto_admission",
        &json!({"episode_id":"p","decision":"ADMITTED","context":"SHADOW"}),
    );
    assert!(a.links["t"].admitted.is_empty());
    a.observe(
        "auto_admission",
        &json!({"episode_id":"p","decision":"ADMITTED","context":"LIVE"}),
    );
    a.observe(
        "broker_order",
        &json!({"plan":{"prediction_id":"p"},"filled_qty":0}),
    );
    assert!(a.links["t"].executed.is_empty());
    a.observe(
        "broker_order",
        &json!({"plan":{"prediction_id":"p"},"filled_qty":0.1}),
    );
    a.observe(
        "trade",
        &json!({"position":{"plan":{"prediction_id":"p"}},"net_pnl":2.}),
    );
    a.observe(
        "live_trade",
        &json!({"prediction_id":"p","parent_id":"entry","net_pnl":3.}),
    );
    a.observe(
        "live_trade_adjustment",
        &json!({"trade":{"prediction_id":"p","parent_id":"entry","net_pnl":4.}}),
    );
    let report = a.report();
    let r = &report["groups"]["family:PRICE_VELOCITY"];
    assert_eq!(r["trigger_count"], 1);
    assert_eq!(r["auto_admitted_count"], 1);
    assert_eq!(r["executed_count"], 1);
    assert_eq!(r["paper_realized_net_pnl"], 2.);
    assert_eq!(r["live_realized_net_pnl"], 4.);
}

#[test]
fn normalization_warmup_and_disabled_families_cannot_trigger() {
    let mut e = Engine::new(true);
    e.trigger_config.enabled_families = [false; 4];
    for i in 0..400 {
        frame(&mut e, i, if i % 20 < 10 { 0.8 } else { -0.8 });
    }
    assert_eq!(e.integrity.realtime_evaluations, 0);
    assert!(e.trigger_analytics.links.is_empty());
    let mut e = Engine::new(true);
    for i in 0..8 {
        frame(&mut e, i, if i % 2 == 0 { 0.8 } else { -0.8 });
    }
    assert_eq!(e.integrity.realtime_evaluations, 0);
}

#[test]
fn production_trigger_cannot_bypass_low_trust_cost_or_auto_admission() {
    let mut e = burst();
    let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
    p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
    let mut h = e.symbols["BTCUSDT"].health.clone();
    h.trust = "LOW".into();
    let f = e.symbols["BTCUSDT"].features.clone().unwrap();
    assert!(e.qualify(&p, &f, &h).1.is_none());
    h.trust = "HIGH".into();
    h.samples = 100;
    h.calibration_error = 0.;
    e.config.min_net_pnl = 0.01;
    e.config.min_depth = 10_000.; // The single-level fixture has only $200k of depth.
    let (q, plan) = e.qualify(&p, &f, &h);
    assert_eq!(q.status, "QUALIFIED", "{:?}", q.reasons);
    let s = e.symbols.get_mut("BTCUSDT").unwrap();
    s.health = h.clone();
    s.qualification = Some(q.clone());
    s.plan = plan.clone();
    let ev = s.evaluation.as_mut().unwrap();
    ev.qualification = Some(q);
    ev.target_position = plan;
    ev.calibrated_probabilities = p.calibrated_probabilities;
    e.config.auto_min_net_edge = 0.5;
    assert!(e
        .admission_decisions(false, None)
        .iter()
        .all(|d| d.decision == "SKIPPED"));
    assert!(e.live.orders.is_empty());
    e.config.fee_bps = 30.;
    assert!(e.qualify(&p, &f, &h).1.is_none());
}

#[test]
fn recording_start_restores_nonempty_books_from_a_previous_session() {
    let engine = burst();
    assert!(!engine.symbols["BTCUSDT"].venues["binance"]
        .book
        .bids
        .is_empty());
    let encoded = serde_json::to_string(&Journal::Start {
        engine: Box::new(engine.clone()),
    })
    .unwrap();
    let Journal::Start { engine: restored } = serde_json::from_str(&encoded).unwrap() else {
        panic!("expected checkpoint");
    };
    assert_eq!(
        serde_json::to_value(engine).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}

#[test]
fn legacy_checkpoint_migration_preserves_history_and_advances_sampling_config() {
    let e = burst();
    let count = e.outcomes.len();
    let version = e.config_version;
    let mut value = serde_json::to_value(&e).unwrap();
    for k in [
        "trigger_config",
        "trigger_config_version",
        "trigger_analytics",
        "reference_runtime",
    ] {
        value.as_object_mut().unwrap().remove(k);
    }
    for s in value["symbols"].as_object_mut().unwrap().values_mut() {
        s.as_object_mut().unwrap().remove("runtime");
    }
    let mut restored: Engine = serde_json::from_value(value).unwrap();
    restored.restart();
    assert_eq!(restored.trigger_config_version, 1);
    assert_eq!(restored.config_version, version + 1);
    assert!(restored.outcomes.len() >= count);
    assert!(!restored.reference_runtime);
    assert_eq!(restored.mode, "OBSERVE");
}
