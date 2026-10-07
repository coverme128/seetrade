use micro_alpha_engine::review::{archive_previous_day, build, date_at, ReviewQuery};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
const START: u64 = 1_791_302_400_000; // 2026-10-07 00:00 Asia/Shanghai
const DAY: u64 = 86_400_000;
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    dir: PathBuf,
    db: Connection,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "seetrade-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Connection::open(dir.join("live.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TABLE records(id INTEGER PRIMARY KEY,kind TEXT,ts INTEGER,payload TEXT)",
        )
        .unwrap();
        Self { dir, db }
    }
    fn add(&self, kind: &str, source: &str, ts: u64, payload: Value) {
        self.db
            .execute(
                "INSERT INTO records(kind,ts,payload) VALUES(?1,?2,?3)",
                params![
                    kind,
                    ts,
                    json!({"kind":kind,"source":source,"ts":ts,"payload":payload}).to_string()
                ],
            )
            .unwrap();
    }
    fn report(&self, q: ReviewQuery) -> Value {
        build(&self.dir.join("live.sqlite"), &q, START + DAY * 2).unwrap()
    }
    fn daily(&self) -> Value {
        self.report(ReviewQuery {
            date: Some("2026-10-07".into()),
            ..Default::default()
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn p(id: &str, ts: u64) -> Value {
    json!({"prediction_id":id,"ts":ts,"symbol":"BTCUSDT","model_version":"model-a","config_version":35,"direction":"LONG","horizon":60,"target":0.003,"stop":0.001,"calibrated_probabilities":[0.7,0.2,0.1],"qualification":{"prediction_id":id,"status":"QUALIFIED","reasons":[],"net_edge":0.001,"gates":[{"code":"calibration","pass":true},{"code":"model_trust","pass":true,"detail":"HIGH"}]},"episode":{"highest_qualification":"QUALIFIED"},"plan":{"plan_id":format!("plan-{id}-{ts}")}})
}
fn outcome(p: Value, ts: u64, kind: &str, valid: bool) -> Value {
    json!({"prediction":p,"first_hit_ts":ts,"outcome_type":kind,"learning_eligible":valid,"prediction_error":if valid{""}else{"confirmed_signal_change"},"observed_net_edge":0.002,"horizon_observation":null})
}
fn paper(id: &str, closed: u64, pnl: f64) -> Value {
    json!({"exit_ts":closed,"exit_reason":"timeout","net_pnl":pnl,"expected_cost":2.,"realized_cost":3.,"position":{"plan":{"plan_id":format!("plan-{id}"),"prediction_id":id,"symbol":"BTCUSDT"},"execution_venue":"binance"}})
}
fn order(id: &str, pred: &str, status: &str, filled: f64) -> Value {
    json!({"client_id":id,"action":"ENTRY","status":status,"filled_qty":filled,"reason":"entry","plan":{"prediction_id":pred,"symbol":"BTCUSDT"}})
}
#[test]
fn daily_boundaries_pending_and_close_day_are_distinct() {
    let f = Fixture::new();
    assert_eq!(date_at(START - 1).unwrap(), "2026-10-06");
    f.add("prediction", "live", START - 1, p("old", START - 1));
    f.add("prediction", "live", START, p("today", START));
    f.add("trade", "live", START + 100, paper("old", START + 100, 10.));
    f.add(
        "prediction",
        "live",
        START + DAY,
        p("tomorrow", START + DAY),
    );
    f.add(
        "outcome",
        "live",
        START + DAY + 1,
        outcome(p("today", START), START + DAY + 1, "TARGET_HIT", true),
    );
    let r = f.daily();
    assert_eq!(r["summary"]["opportunities"], 1);
    assert_eq!(r["summary"]["pending"], 1);
    assert_eq!(r["trading"][0]["metrics"]["net_pnl"], 10.);
    let all = f.report(ReviewQuery {
        date: Some("all".into()),
        ..Default::default()
    });
    assert_eq!(all["summary"]["eligible"], 1);
}
#[test]
fn deduplicates_predictions_and_preserves_opening_grade_and_horizon_labels() {
    let f = Fixture::new();
    let mut open = p("p", START);
    open["calibrated_probabilities"] = Value::Null;
    f.add("prediction", "live", START, open.clone());
    f.add("prediction", "live", START, open.clone());
    let mut end = outcome(p("p", START), START + 1, "SIGNAL_INVALIDATED", false);
    end["prediction"]["episode"]["highest_qualification"] = json!("QUALIFIED");
    f.add("outcome", "live", START + 1, end.clone());
    f.add("outcome", "live", START + 2, end);
    f.add("outcome_horizon","live",START+60000,json!({"prediction_id":"p","observation":{"status":"OBSERVED","final_move_at_timeout":0.01}}));
    let r = f.daily();
    assert_eq!(r["summary"]["opportunities"], 1);
    assert_eq!(r["summary"]["invalid"], 1);
    assert_eq!(r["rows"][0]["opening_grade"], "unverified");
    assert!(r["summary"]["observed_net_edge"].is_null());
    assert_eq!(r["rows"][0]["horizon_observation"]["status"], "OBSERVED");
    assert_eq!(r["rows"][0]["outcome"], "SIGNAL_INVALIDATED");
}
#[test]
fn previews_and_unfilled_orders_do_not_become_trades() {
    let f = Fixture::new();
    f.add("mode", "live", START, json!({"mode":"OBSERVE"}));
    f.add("prediction", "live", START, p("p", START));
    f.add("auto_admission","live",START+1,json!({"episode_id":"p","context":"SHADOW","decision":"SKIPPED","reason_codes":["NO_BUDGET"]}));
    let r = f.daily();
    assert_eq!(r["rows"][0]["execution"], "OBSERVE_ONLY");
    assert_eq!(r["trading"], json!([]));
    assert_eq!(r["rows"][0]["reasons"], json!([]));
    f.add(
        "broker_order",
        "live",
        START + 2,
        order("o", "p", "NEW", 0.),
    );
    assert_eq!(f.daily()["rows"][0]["execution"], "ORDER_PENDING");
    f.add(
        "broker_order",
        "live",
        START + 3,
        order("o", "p", "CANCELED", 0.),
    );
    let r = f.daily();
    assert_eq!(r["rows"][0]["execution"], "UNFILLED");
    assert_eq!(r["summary"]["exchange_filled_opportunities"], 0);
}
#[test]
fn full_history_and_pagination_share_a_pinned_snapshot() {
    let f = Fixture::new();
    for i in 0..610 {
        let mut pred = p(&format!("p{i}"), START + i);
        pred["config_version"] = json!(if i % 2 == 0 { 35 } else { 36 });
        f.add("prediction", "live", START + i, pred);
    }
    let first = f.daily();
    assert_eq!(first["summary"]["opportunities"], 610);
    assert_eq!(first["rows"].as_array().unwrap().len(), 50);
    assert_eq!(first["cohorts"].as_array().unwrap().len(), 2);
    f.add("prediction", "live", START + 700, p("new", START + 700));
    let frozen = f.report(ReviewQuery {
        date: Some("2026-10-07".into()),
        through_id: first["through_id"].as_u64(),
        as_of: first["as_of"].as_u64(),
        export: Some(true),
        ..Default::default()
    });
    assert_eq!(frozen["rows"].as_array().unwrap().len(), 610);
    assert_eq!(f.daily()["summary"]["opportunities"], 611);
}
#[test]
fn exchange_environment_is_frozen_and_funding_adjustments_replace_not_add() {
    let f = Fixture::new();
    f.add("prediction", "live", START, p("p", START));
    f.add(
        "broker_config",
        "live",
        START,
        json!({"environment":"testnet","venue":"okx"}),
    );
    f.add(
        "live_risk_decision",
        "live",
        START + 1,
        json!({"status":"PASS","intent":order("o","p","QUEUED",0.)}),
    );
    f.add(
        "broker_order",
        "live",
        START + 2,
        order("o", "p", "PARTIALLY_FILLED", 1.),
    );
    let trade = json!({"parent_id":"o","prediction_id":"p","symbol":"BTCUSDT","closed_ts":START+3,"net_pnl":5.,"feedback":{"model_version":"m","config_version":35,"expected_cost":1.,"realized_cost":2.}});
    f.add("live_trade", "live", START + 3, trade.clone());
    f.add(
        "broker_config",
        "live",
        START + 4,
        json!({"environment":"live","venue":"binance"}),
    );
    let mut revised = trade;
    revised["net_pnl"] = json!(7.);
    f.add(
        "live_trade_adjustment",
        "live",
        START + 5,
        json!({"trade":revised}),
    );
    f.add(
        "broker_order",
        "live",
        START + 6,
        order("o", "p", "CANCELED", 1.),
    );
    let r = f.daily();
    assert_eq!(r["trading"][0]["channel"], "TESTNET");
    assert_eq!(r["trading"][0]["venue"], "okx");
    assert_eq!(r["trading"][0]["metrics"]["closed_trades"], 1);
    assert_eq!(r["trading"][0]["metrics"]["net_pnl"], 7.);
    assert_eq!(r["summary"]["exchange_filled_opportunities"], 1);
}
#[test]
fn source_and_execution_buckets_are_never_combined() {
    let f = Fixture::new();
    for source in ["live", "demo"] {
        f.add("prediction", source, START, p("same-id", START));
        f.add("trade", source, START + 1, paper("same-id", START + 1, 5.));
    }
    f.add("live_trade","live",START+2,json!({"parent_id":"legacy","prediction_id":"same-id","symbol":"BTCUSDT","closed_ts":START+2,"net_pnl":100.}));
    let r = f.daily();
    assert_eq!(r["summary"]["opportunities"], 2);
    assert_eq!(r["trading"].as_array().unwrap().len(), 3);
    assert!(r["trading"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["channel"] == "UNKNOWN"));
    let demo = f.report(ReviewQuery {
        date: Some("all".into()),
        source: Some("demo".into()),
        ..Default::default()
    });
    assert_eq!(demo["trading"].as_array().unwrap().len(), 1);
    assert_eq!(demo["summary"]["opportunities"], 1);
}
#[test]
fn realized_curve_and_costs_use_closed_trades_only() {
    let f = Fixture::new();
    for (i, pnl) in [20., -5., -10.].iter().enumerate() {
        f.add(
            "trade",
            "live",
            START + i as u64,
            paper(&i.to_string(), START + i as u64, *pnl),
        );
    }
    let r = f.daily();
    let m = &r["trading"][0]["metrics"];
    assert_eq!(m["net_pnl"], 5.);
    assert_eq!(m["realized_pnl_drawdown"], 15.);
    assert_eq!(m["max_consecutive_losses"], 2);
    assert_eq!(m["mean_cost_overrun"], 1.);
}
#[test]
fn blocked_plan_reasons_and_manual_skip_are_auditable() {
    let f = Fixture::new();
    f.add("mode", "live", START, json!({"mode":"PAPER"}));
    f.add("prediction", "live", START, p("p", START));
    f.add("portfolio_gate_decision","live",START+1,json!({"trade_plan_id":format!("plan-p-{}",START+1),"decision":"BLOCK","enforced":true,"reason_codes":["PORTFOLIO_LIMIT"]}));
    let r = f.daily();
    assert_eq!(r["rows"][0]["execution"], "EXECUTION_BLOCKED");
    assert!(r["rows"][0]["reasons"][0]
        .as_str()
        .unwrap()
        .contains("PORTFOLIO_LIMIT"));
    f.add("skip", "live", START + 2, json!({"prediction_id":"p"}));
    assert_eq!(f.daily()["rows"][0]["execution"], "MANUAL_SKIP");
}
#[test]
fn no_trade_is_not_zero_return_and_calibration_uses_valid_calibrated_predictions() {
    let f = Fixture::new();
    f.add("prediction", "live", START, p("p", START));
    f.add(
        "outcome",
        "live",
        START + 1,
        outcome(p("p", START), START + 1, "TARGET_HIT", true),
    );
    let mut raw = p("raw", START + 2);
    raw["calibrated_probabilities"] = Value::Null;
    f.add(
        "outcome",
        "live",
        START + 3,
        outcome(raw, START + 3, "STOP_HIT", true),
    );
    let r = f.daily();
    assert_eq!(r["trading"], json!([]));
    assert_eq!(r["summary"]["calibration_samples"], 1);
    assert!((r["summary"]["calibration_ece"].as_f64().unwrap() - 0.3).abs() < 1e-9);
    assert!(r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "NO_CLOSED_TRADES"));
}
#[test]
fn daily_archive_is_idempotent_and_queries_do_not_mutate_database() {
    let f = Fixture::new();
    f.add("prediction", "live", START, p("p", START));
    let db = f.dir.join("live.sqlite");
    let before = std::fs::read(&db).unwrap();
    let archive = archive_previous_day(&db, &f.dir.join("reviews"), START + DAY + 100)
        .unwrap()
        .unwrap();
    let bytes = std::fs::read(&archive).unwrap();
    assert!(
        archive_previous_day(&db, &f.dir.join("reviews"), START + DAY + 200)
            .unwrap()
            .is_none()
    );
    assert_eq!(bytes, std::fs::read(archive).unwrap());
    assert_eq!(before, std::fs::read(db).unwrap());
    for date in ["2026-02-30", "../bad", "2027-01-01"] {
        assert!(ReviewQuery {
            date: Some(date.into()),
            ..Default::default()
        }
        .validate(START)
        .is_err());
    }
}

#[test]
fn real_admission_failure_takes_precedence_over_observation_at_open() {
    let f = Fixture::new();
    f.add("mode", "live", START, json!({"mode":"OBSERVE"}));
    f.add("prediction", "live", START, p("p", START));
    f.add("mode", "live", START + 1, json!({"mode":"AUTO"}));
    f.add("auto_admission", "live", START+2, json!({"episode_id":"p","context":"LIVE","decision":"SKIPPED","reason_codes":["NO_BUDGET"]}));
    assert_eq!(f.daily()["rows"][0]["execution"], "ADMISSION_BLOCKED");
}
