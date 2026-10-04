use micro_alpha_engine::{
    core::Engine,
    scanner,
    storage::{Batch, Journal, Store},
    types::*,
    universe::*,
};
use serde_json::json;
fn obs(s: &str) -> Observation {
    Observation {
        asset_key: s.into(),
        instruments: ["binance", "okx"]
            .iter()
            .map(|venue| Instrument {
                venue: (*venue).into(),
                symbol: s.into(),
                instrument_id: if *venue == "okx" {
                    format!("{}-USDT-SWAP", s.trim_end_matches("USDT"))
                } else {
                    s.into()
                },
                contract_type: "PERPETUAL".into(),
                active: true,
                listed_at: 1,
                contract_value: 1.,
                observed_at: 1000000,
            })
            .collect(),
        metrics: Metrics {
            quote_volume: Some(200_000_000.),
            ..Default::default()
        },
    }
}
fn scan(e: &mut Engine, rows: Vec<Observation>, id: &str) {
    e.command("scanner_started", json!({"id":id})).unwrap();
    e.command(
        "scanner_result",
        json!(ScannerRun {
            id: id.into(),
            started_at: e.now,
            completed_at: e.now,
            status: "COMPLETED".into(),
            config_version: 1,
            target: None,
            considered: rows.len(),
            errors: Default::default(),
            observations: rows
        }),
    )
    .unwrap();
}
fn engine() -> Engine {
    let mut e = Engine::new(true);
    e.now = 1_000_000;
    e.initialize_universe();
    e
}
#[test]
fn migration_is_conservative_idempotent_and_keeps_history() {
    let mut e = engine();
    let first = e.universe.clone().unwrap();
    e.initialize_universe();
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(e.universe.as_ref().unwrap()).unwrap()
    );
    assert!(!e.universe.as_ref().unwrap().config.enabled);
    assert!(!e.asset_mode_allowed("BTCUSDT", "AUTO"));
    assert!(e.asset_mode_allowed("BTCUSDT", "PAPER"));
}
#[test]
fn scanner_priority_is_not_qualification_or_live_admission() {
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT")], "one");
    let p = &e.universe.as_ref().unwrap().profiles["SOLUSDT"];
    assert_eq!(p.qualification.as_ref().unwrap().research_priority, "HIGH");
    assert_eq!(
        p.qualification.as_ref().unwrap().overall_state,
        "CONDITIONAL"
    );
    assert_eq!(
        p.qualification.as_ref().unwrap().components["continuity"].state,
        "UNKNOWN"
    );
    assert!(!e.symbols.contains_key("SOLUSDT"));
    assert!(!e.asset_mode_allowed("SOLUSDT", "AUTO"));
    assert!(e
        .command("asset_review", json!({"symbol":"SOLUSDT"}))
        .is_err());
    assert!(e
        .command(
            "asset_permissions",
            json!({"symbol":"SOLUSDT","modes":["OBSERVE","AUTO"]})
        )
        .is_err());
    scan(&mut e, vec![obs("SOLUSDT")], "two");
    assert_eq!(e.universe.as_ref().unwrap().profiles.len(), 3);
}
#[test]
fn bounded_collection_and_rollback_preserve_dynamic_positions() {
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT"), obs("XRPUSDT")], "one");
    e.universe.as_mut().unwrap().config.candidate_l2_slots = 3;
    e.command("asset_collect", json!({"symbol":"SOLUSDT","enabled":true}))
        .unwrap();
    assert!(e.collection_symbols().contains(&"SOLUSDT".into()));
    assert!(e
        .command("asset_collect", json!({"symbol":"XRPUSDT","enabled":true}))
        .is_err());
    e.command("universe_rollback", json!({"frozen":true}))
        .unwrap();
    assert_eq!(e.collection_symbols(), vec!["BTCUSDT", "ETHUSDT"]);
    assert!(e
        .universe
        .as_ref()
        .unwrap()
        .profiles
        .contains_key("SOLUSDT"));
    e.live.account = Some(micro_alpha_engine::live::AccountSnapshot {
        positions: std::collections::BTreeMap::from([(
            "SOLUSDT".into(),
            micro_alpha_engine::live::AccountPosition {
                quantity: 1.,
                mark_price: 100.,
                ..Default::default()
            },
        )]),
        ..Default::default()
    });
    assert!(e.collection_symbols().contains(&"SOLUSDT".into()));
}
#[test]
fn qualification_rejects_cost_and_delisting_not_high_volume() {
    let mut o = obs("SOLUSDT");
    o.metrics.slippage_bps = Some(500.);
    let q = qualify(&o, &Default::default(), 1, 1000000);
    assert_eq!(q.overall_state, "FAIL");
    assert_eq!(q.research_priority, "REJECT");
    o.instruments.iter_mut().for_each(|i| i.active = false);
    let q = qualify(&o, &Default::default(), 1, 1000000);
    assert!(q.blocking_reasons.contains(&"exchange_available".into()));
}
#[test]
fn metadata_normalizes_contracts_and_rejects_wrong_quote() {
    let meta = json!({"symbols":[{"symbol":"SOLUSDT","contractType":"PERPETUAL","quoteAsset":"USDT","marginAsset":"USDT","status":"TRADING"},{"symbol":"SOLUSDC","contractType":"PERPETUAL","quoteAsset":"USDC","marginAsset":"USDC"}]});
    let rows = scanner::instruments(
        "binance",
        &meta,
        &json!([{"symbol":"SOLUSDT","quoteVolume":"10000000","count":100}]),
        100,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].asset_key, "SOLUSDT");
    assert_eq!(normalize("sol-usdt-swap").unwrap(), "SOLUSDT");
    assert!(normalize("../../bad").is_err());
    assert_eq!(normalize("币安人生USDT").unwrap(), "币安人生USDT");
    assert!(rows[0].metrics.depth.is_none());
}
fn plan(e: &Engine) -> TradePlan {
    let mut legacy = Engine::new_reference(true);
    legacy.config.min_samples = 1;
    legacy.config.high_trust_samples = 1;
    for step in 0..8 {
        for event in micro_alpha_engine::feeds::demo_events(step, 1_000_000 + step * 1000) {
            legacy.ingest(event);
        }
    }
    let mut p = legacy.symbols["BTCUSDT"].active.as_ref().unwrap().plan.clone().unwrap_or_else(||serde_json::from_value(json!({"plan_id":"test-plan","prediction_id":"test-prediction","symbol":"BTCUSDT","direction":"LONG","position_notional":100.,"effective_leverage":0.1,"expected_net_pnl":1.,"expected_loss":1.,"planned_max_loss":1.,"break_even_probability":0.5,"expected_mae":0.01,"liquidity_capacity":10000.,"liquidation_buffer":1.,"tp":70000.,"sl":67000.,"horizon":60,"entry_type":"PAPER_MARKET_IOC","exit_rules":[]})).unwrap());
    p.position_notional = e.config.equity;
    p.planned_max_loss = 20.;
    p
}
#[test]
fn portfolio_reduces_never_increases_and_live_disabled_blocks() {
    let mut e = engine();
    let mut p = plan(&e);
    let requested = p.position_notional;
    e.apply_portfolio(&mut p, false, None).unwrap();
    assert!(p.position_notional < requested);
    assert!(p.portfolio_decision_id.is_some());
    let d = e.portfolio_decision(&p, true, None).unwrap();
    assert_eq!(d.decision, "BLOCK");
    assert!(d
        .reason_codes
        .contains(&"PORTFOLIO_LIVE_DISABLED_OR_NO_EQUITY".into()));
    assert_eq!(d.approved_notional, 0.);
}
#[test]
fn independent_profiles_and_unknown_correlation_fail_conservatively() {
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT")], "one");
    e.command("asset_collect", json!({"symbol":"SOLUSDT","enabled":true}))
        .unwrap();
    let btc = e.asset_trigger("BTCUSDT").0.normalization_window_ms;
    e.universe
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("SOLUSDT")
        .unwrap()
        .trigger
        .config
        .normalization_window_ms = 60000;
    assert_eq!(e.asset_trigger("SOLUSDT").0.normalization_window_ms, 60000);
    assert_eq!(e.asset_trigger("BTCUSDT").0.normalization_window_ms, btc);
    assert_eq!(e.asset_correlation("BTCUSDT", "SOLUSDT"), None);
    let mut c = e.config.clone();
    c.target = 0.005;
    e.command("asset_config", json!({"symbol":"SOLUSDT","config":c}))
        .unwrap();
    assert_eq!(e.asset_config("SOLUSDT").target, 0.005);
    assert_ne!(e.asset_config("BTCUSDT").target, 0.005);
}
#[tokio::test]
async fn migration_projections_and_append_only_transitions_survive_restore() {
    let path = std::env::temp_dir().join(format!(
        "universe-{}-{}",
        std::process::id(),
        micro_alpha_engine::feeds::now_ms()
    ));
    let (store, _) = Store::open(&path, "demo").unwrap();
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT")], "one");
    e.command("asset_collect", json!({"symbol":"SOLUSDT","enabled":true}))
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
    let db = rusqlite::Connection::open(&store.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM universe_membership", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM instrument_registry WHERE normalized_symbol='SOLUSDT'",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM asset_state_transition WHERE asset_key='SOLUSDT'",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        1
    );
    let state: String = db
        .query_row("SELECT state FROM checkpoint", [], |r| r.get(0))
        .unwrap();
    let restored: Engine = serde_json::from_str(&state).unwrap();
    assert_eq!(restored.collection_symbols(), e.collection_symbols());
    drop(db);
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn dynamic_asset_trigger_episode_outcome_and_idle_budget() {
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT")], "one");
    e.command("asset_collect", json!({"symbol":"SOLUSDT","enabled":true}))
        .unwrap();
    let mut trigger_profile_seen = false;
    let mut idle_evaluations = 0;
    for step in 0..480 {
        let pressure = if (80..320).contains(&step) { 0.8 } else { 0. };
        let movement = if step >= 160 { 0.004 } else { 0. };
        for mut event in micro_alpha_engine::feeds::demo_events(0, 1_000_000 + step * 250) {
            if event.symbol == "ETHUSDT" {
                continue;
            }
            if event.symbol == "BTCUSDT" {
                event.symbol = "SOLUSDT".into();
            }
            let mid = 150. * (1. + movement);
            match &mut event.data {
                EventData::Snapshot { bids, asks } => {
                    *bids = (1..=20)
                        .map(|i| [mid * (1. - i as f64 * 0.000001), 300. * (1. + pressure)])
                        .collect();
                    *asks = (1..=20)
                        .map(|i| [mid * (1. + i as f64 * 0.000001), 300. * (1. - pressure)])
                        .collect();
                }
                EventData::Trade {
                    price,
                    quantity,
                    buy,
                } => {
                    *price = mid;
                    *quantity = if pressure > 0. { 800. } else { 100. };
                    *buy = step % 2 == 0 || pressure > 0.;
                }
                EventData::Mark { mark, index, .. } => {
                    *mark = mid;
                    *index = mid;
                }
                _ => {}
            }
            e.ingest(event);
        }
        if step == 70 {
            idle_evaluations = e.symbols["SOLUSDT"].runtime.full_evaluations;
        }
        trigger_profile_seen |= e.records.iter().any(|r| {
            r["kind"] == "market_pulse_trigger"
                && r["payload"]["asset_trigger_profile_id"] == "trigger-profile-SOLUSDT-1"
        });
        e.records.clear();
    }
    assert_eq!(idle_evaluations, 0);
    assert!(trigger_profile_seen);
    assert!(e
        .outcomes
        .iter()
        .any(|o| o.prediction.symbol == "SOLUSDT" && o.outcome_type == "TARGET_HIT"));
    assert!(e.positions.is_empty());
    assert!(e.symbols["SOLUSDT"].runtime.full_evaluations < 120);
}

#[test]
fn parameter_grid_reuses_recording_without_inventing_outcomes_or_orders() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("grid-{}.jsonl", std::process::id()));
    let e = Engine::new(true);
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "{}",
        serde_json::to_string(&Journal::Start {
            engine: Box::new(e)
        })
        .unwrap()
    )
    .unwrap();
    for step in 0..8 {
        for event in micro_alpha_engine::feeds::demo_events(step, 1_000_000 + step * 1000) {
            writeln!(
                file,
                "{}",
                serde_json::to_string(&Journal::Event { event }).unwrap()
            )
            .unwrap();
        }
    }
    drop(file);
    let first = micro_alpha_engine::asset_grid::replay(&path, "BTCUSDT").unwrap();
    let second = micro_alpha_engine::asset_grid::replay(&path, "BTCUSDT").unwrap();
    assert_eq!(first, second);
    assert_eq!(first["candidates"].as_array().unwrap().len(), 24);
    assert_eq!(first["selection"], "RESEARCH_ONLY_NO_AUTOMATIC_SELECTION");
    assert!(first["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["samples"] == 0 && v["observed_net_edge"].is_null()));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn micro_price_books_keep_sub_micro_dollar_ticks_and_roundtrip() {
    let mut b = micro_alpha_engine::book::Book::default();
    let event = MarketEvent {
        exchange: "okx".into(),
        symbol: "SHIBUSDT".into(),
        exchange_ts: 1000,
        receive_ts: 1000,
        sequence_id: 1,
        data: EventData::Snapshot {
            bids: vec![[0.00001231, 1e9]],
            asks: vec![[0.00001232, 1e9]],
        },
    };
    b.apply(&event).unwrap();
    let (bid, ask, _, _) = b.top().unwrap();
    assert!((bid - 0.00001231).abs() < 1e-15);
    assert!((ask - 0.00001232).abs() < 1e-15);
    assert!(b.slippage_bps(1000.).unwrap() > 0.);
    let restored: micro_alpha_engine::book::Book =
        serde_json::from_value(serde_json::to_value(&b).unwrap()).unwrap();
    assert_eq!(restored.top(), b.top());
}

#[test]
fn soft_demotion_requires_separated_scans_and_hard_failure_is_immediate() {
    let mut e = engine();
    e.now = 10 * 86_400_000;
    let mut o = obs("SOLUSDT");
    o.metrics = Metrics {
        quote_volume: Some(200_000_000.),
        median_spread_bps: Some(1.),
        tail_spread_bps: Some(2.),
        depth: Some(500000.),
        slippage_bps: Some(1.),
        volatility: Some(0.001),
        tail_gap: Some(0.001),
        clean_since: Some(e.now - 24 * 3600000),
        samples: 180,
        resets_last_hour: Some(0),
        ..Default::default()
    };
    scan(&mut e, vec![o.clone()], "pass");
    e.universe
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("SOLUSDT")
        .unwrap()
        .lifecycle_state = "ELIGIBLE".into();
    o.metrics.quote_volume = Some(1.);
    for i in 0..3 {
        e.now += 300000;
        scan(&mut e, vec![o.clone()], &format!("warn-{i}"));
        assert_eq!(
            e.universe.as_ref().unwrap().profiles["SOLUSDT"].lifecycle_state,
            if i == 2 { "DEGRADED" } else { "ELIGIBLE" }
        );
    }
    o.metrics.slippage_bps = Some(100.);
    scan(&mut e, vec![o], "hard");
    assert_eq!(
        e.universe.as_ref().unwrap().profiles["SOLUSDT"].lifecycle_state,
        "SUSPENDED"
    );
}
#[test]
fn trigger_calibration_needs_clean_history_and_audits_asset_specific_thresholds() {
    let mut e = engine();
    scan(&mut e, vec![obs("SOLUSDT")], "one");
    e.command("asset_collect", json!({"symbol":"SOLUSDT","enabled":true}))
        .unwrap();
    assert!(e
        .command("asset_trigger_calibrate", json!({"symbol":"SOLUSDT"}))
        .is_err());
    e.now = 2 * 86_400_000;
    e.universe
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("SOLUSDT")
        .unwrap()
        .clean_since = Some(1);
    e.symbols.get_mut("SOLUSDT").unwrap().runtime.pulse.baseline = (0..100)
        .map(|i| (e.now - (100 - i) * 250, [1. + (i % 7) as f64 * 0.01; 8]))
        .collect();
    e.command("asset_trigger_calibrate", json!({"symbol":"SOLUSDT"}))
        .unwrap();
    let p = &e.universe.as_ref().unwrap().profiles["SOLUSDT"];
    assert!(p.trigger.calibrated);
    assert_eq!(p.trigger.config.family_thresholds.len(), 4);
    assert_eq!(p.trigger.metric_distribution_version.len(), 64);
    assert!(e.asset_trigger("BTCUSDT").0.family_thresholds.is_empty());
    assert!(!e.asset_mode_allowed("SOLUSDT", "AUTO"));
    assert!(e.records.iter().any(
        |r| r["kind"] == "asset_trigger_calibration" && r["payload"]["asset_key"] == "SOLUSDT"
    ));
}

#[test]
fn asset_venue_costs_follow_execution_venue_and_reject_invalid_assumptions() {
    let mut e = Engine::new_reference(true);
    e.initialize_universe();
    for step in 0..8 {
        for event in micro_alpha_engine::feeds::demo_events(step, 1_000_000 + step * 1000) {
            e.ingest(event);
        }
    }
    let f = e.symbols["BTCUSDT"].features.clone().unwrap();
    let base = e.costs(&f);
    let mut config = e.config.clone();
    config.venue_costs.insert(
        "okx".into(),
        VenueCosts {
            fee_bps: 8.,
            slippage_bps: 3.,
            impact_bps: 2.,
            adverse_selection_bps: 1.,
            funding_bps: 0.7,
        },
    );
    e.command("asset_config", json!({"symbol":"BTCUSDT", "config":config}))
        .unwrap();
    assert_eq!(e.costs(&f).entry_fee, base.entry_fee);
    e.live.configured = true;
    e.live.venue = "okx".into();
    assert_eq!(e.costs(&f).entry_fee, 0.0008);
    assert_eq!(e.costs(&f).slippage, 0.0006);
    let eth = e.symbols["ETHUSDT"].features.clone().unwrap();
    assert_eq!(e.costs(&eth).entry_fee, base.entry_fee);
    config.venue_costs.get_mut("okx").unwrap().fee_bps = -1.;
    assert!(e
        .command("asset_config", json!({"symbol":"BTCUSDT", "config":config}))
        .is_err());
}

#[test]
fn asset_revisions_preserve_other_asset_samples_and_global_changes_supersede_both() {
    let mut e = engine();
    let btc = e.asset_config_version("BTCUSDT");
    let mut c = e.config.clone();
    c.risk_per_trade = 0.001;
    e.command("asset_config", json!({"symbol":"ETHUSDT","config":c}))
        .unwrap();
    let eth = e.asset_config_version("ETHUSDT");
    assert_eq!(e.asset_config_version("BTCUSDT"), btc);
    assert!(eth > btc);
    let restored: Engine = serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
    assert_eq!(restored.asset_config_version("ETHUSDT"), eth);
    let transition = e.universe.as_ref().unwrap().transitions.last().unwrap();
    assert_eq!(transition.config_version, eth);
    assert!(transition.evidence.get("scanner_rules").is_some());
    e.command("config", json!(e.config)).unwrap();
    assert!(e.asset_config_version("BTCUSDT") > eth);
    assert_eq!(
        e.asset_config_version("BTCUSDT"),
        e.asset_config_version("ETHUSDT")
    );
}

#[test]
fn venue_costs_are_frozen_in_prediction_research_context() {
    let mut e = Engine::new_reference(true);
    e.initialize_universe();
    let mut c = e.config.clone();
    c.venue_costs.insert(
        "binance".into(),
        VenueCosts {
            fee_bps: 8.,
            slippage_bps: 3.,
            impact_bps: 2.,
            adverse_selection_bps: 1.,
            funding_bps: 0.7,
        },
    );
    e.command("asset_config", json!({"symbol":"BTCUSDT","config":c}))
        .unwrap();
    for step in 0..240 {
        for ev in micro_alpha_engine::feeds::demo_events(step, 1_000_000 + step * 1000) {
            e.ingest(ev);
        }
    }
    let p = &e
        .outcomes
        .iter()
        .find(|o| o.prediction.symbol == "BTCUSDT")
        .unwrap()
        .prediction;
    assert_eq!(p.cost_assumptions_bps, Some(25.7));
    assert_eq!(p.qualification.costs.entry_fee, 0.0008);
    assert_eq!(p.config_version, e.asset_config_version("BTCUSDT"));
    assert_ne!(p.config_version, e.asset_config_version("ETHUSDT"));
}

#[test]
fn excessive_recent_resets_are_auditable_hard_qualification_failure() {
    let mut row = obs("SOLUSDT");
    row.metrics.resets_last_hour = Some(4);
    let q = qualify(&row, &UniverseConfig::default(), 2, 1_000_000);
    assert_eq!(q.components["book_stability"].state, "FAIL");
    assert!(q.blocking_reasons.contains(&"book_stability".into()));
    row.metrics.resets_last_hour = Some(0);
    assert_eq!(
        qualify(&row, &UniverseConfig::default(), 2, 1_000_000).components["book_stability"].state,
        "PASS"
    );
}

#[test]
fn shared_model_rollback_versions_profiles_without_overwriting_independent_asset() {
    let mut e = engine();
    let old = e.universe.as_ref().unwrap().profiles["ETHUSDT"].profile_version;
    let mut c = e.config.clone();
    c.target = 0.005;
    e.command("asset_config", json!({"symbol":"BTCUSDT","config":c}))
        .unwrap();
    let btc = e.asset_model("BTCUSDT").clone();
    let mut previous = e.model.clone();
    previous.model_version = "rollback-fixture".into();
    previous.calibration_version = "rollback-calibration".into();
    e.previous_model = Some(previous);
    e.command(
        "model_rollback",
        json!({"current_hash":e.model.artifact_hash}),
    )
    .unwrap();
    let eth = &e.universe.as_ref().unwrap().profiles["ETHUSDT"];
    assert!(eth.profile_version > old);
    assert_eq!(
        eth.model.as_ref().unwrap().model_version,
        "rollback-fixture"
    );
    assert_eq!(
        eth.calibration_artifact_id.as_deref(),
        Some("rollback-calibration")
    );
    assert_eq!(e.asset_model("BTCUSDT").model_version, btc.model_version);
    assert!(e.records.iter().any(|r| r["kind"] == "asset_profile"
        && r["payload"]["model"]["model_version"] == "rollback-fixture"));
}
