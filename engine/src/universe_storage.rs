//! Queryable projections share the recorder transaction with input journal and checkpoint.
use rusqlite::{params, Connection, Result};
use serde_json::Value;
pub fn migrate(c: &Connection) -> Result<()> {
    c.execute_batch(include_str!("../migrations/001_universe.sql"))?;
    c.execute_batch(include_str!("../migrations/002_trigger_profile.sql"))?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS instrument_registry(venue TEXT NOT NULL, normalized_symbol TEXT NOT NULL, contract_type TEXT NOT NULL, instrument_json TEXT NOT NULL, PRIMARY KEY(venue,normalized_symbol,contract_type)); CREATE TABLE IF NOT EXISTS schema_migration(version TEXT PRIMARY KEY, applied_at TEXT NOT NULL); INSERT OR IGNORE INTO schema_migration VALUES('v1.1',strftime('%Y-%m-%dT%H:%M:%SZ','now'));")
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
pub fn project(c: &Connection, record: &Value) -> Result<()> {
    let p = &record["payload"];
    let ts = record["ts"].as_u64().unwrap_or(0).to_string();
    match s(record, "kind") {
        "asset_profile" => {
            c.execute(
                "INSERT OR IGNORE INTO asset_profile VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    s(p, "id"),
                    s(p, "asset_key"),
                    p["profile_version"].as_u64(),
                    s(p, "lifecycle_state"),
                    p.to_string(),
                    ts
                ],
            )?;
            c.execute("INSERT INTO universe_membership VALUES(?1,?2,?3,?4,?5) ON CONFLICT(asset_key) DO UPDATE SET lifecycle_state=excluded.lifecycle_state,mode_permissions_json=excluded.mode_permissions_json,current_asset_profile_id=excluded.current_asset_profile_id,updated_at=excluded.updated_at",params![s(p,"asset_key"),s(p,"lifecycle_state"),p["mode_permissions"].to_string(),s(p,"id"),ts])?;
        }
        "asset_trigger_profile" => {
            c.execute(
                "INSERT OR IGNORE INTO asset_trigger_profile VALUES(?1,?2,?3,?4,?5,?6,?6)",
                params![
                    s(p, "id"),
                    s(p, "asset_key"),
                    p["profile_version"].as_u64(),
                    p["config"].to_string(),
                    s(p, "metric_distribution_version"),
                    ts
                ],
            )?;
        }
        "scanner_run_record" => {
            c.execute(
                "INSERT OR REPLACE INTO scanner_run VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    s(p, "id"),
                    p["started_at"].to_string(),
                    p["completed_at"].to_string(),
                    s(p, "status"),
                    p["config_version"].to_string(),
                    p["errors"].to_string()
                ],
            )?;
        }
        "scanner_observation" => {
            let o = &p["observation"];
            let q = &p["qualification"];
            c.execute(
                "INSERT OR IGNORE INTO scanner_observation VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    format!("{}:{}", s(p, "scanner_run_id"), s(o, "asset_key")),
                    s(p, "scanner_run_id"),
                    s(o, "asset_key"),
                    o["metrics"].to_string(),
                    s(q, "research_priority"),
                    ts
                ],
            )?;
            if let Some(rows) = o["instruments"].as_array() {
                for i in rows {
                    c.execute("INSERT INTO instrument_registry VALUES(?1,?2,?3,?4) ON CONFLICT(venue,normalized_symbol,contract_type) DO UPDATE SET instrument_json=excluded.instrument_json",params![s(i,"venue"),s(i,"symbol"),s(i,"contract_type"),i.to_string()])?;
                }
            }
        }
        "asset_qualification" => {
            c.execute(
                "INSERT OR IGNORE INTO asset_qualification VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    s(p, "id"),
                    s(p, "asset_key"),
                    p["rule_version"].to_string(),
                    s(p, "overall_state"),
                    p["components"].to_string(),
                    p["blocking_reasons"].to_string(),
                    p["evaluated_at"].to_string()
                ],
            )?;
        }
        "asset_state_transition" => {
            c.execute(
                "INSERT OR IGNORE INTO asset_state_transition VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    s(p, "id"),
                    s(p, "asset_key"),
                    s(p, "from_state"),
                    s(p, "to_state"),
                    s(p, "reason_code"),
                    p["evidence"].to_string(),
                    s(p, "initiated_by"),
                    ts
                ],
            )?;
        }
        "portfolio_gate_decision" => {
            c.execute(
                "INSERT OR IGNORE INTO portfolio_gate_decision VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    s(p, "id"),
                    s(p, "trade_plan_id"),
                    s(p, "decision"),
                    p["requested_notional"].as_f64(),
                    p["approved_notional"].as_f64(),
                    p["reason_codes"].to_string(),
                    p["snapshot"].to_string(),
                    ts
                ],
            )?;
        }
        _ => {}
    }
    Ok(())
}
