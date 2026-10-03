//! Quiet baseline, known book/aggressor burst, then a target-hit path. Synthetic only.
use micro_alpha_engine::{core::Engine, feeds::demo_events, storage::Journal, types::EventData};
use sha2::{Digest, Sha256};
use std::io::Write;
fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/trigger-fixture.jsonl".into());
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path)?);
    let mut e = Engine::new(true);
    writeln!(
        out,
        "{}",
        serde_json::to_string(&Journal::Start {
            engine: Box::new(e.clone())
        })?
    )?;
    for step in 0..960 {
        let pressure = if (80..320).contains(&step) { 0.8 } else { 0. };
        let movement = if step >= 160 { 0.004 } else { 0. };
        for mut event in demo_events(0, 1_000_000 + step * 250) {
            let base = if event.symbol == "BTCUSDT" {
                67420.
            } else {
                3520.
            };
            let mid = base * (1. + movement);
            match &mut event.data {
                EventData::Snapshot { bids, asks } => {
                    *bids = (1..=20)
                        .map(|i| [mid * (1. - i as f64 * 0.000001), 3. * (1. + pressure)])
                        .collect();
                    *asks = (1..=20)
                        .map(|i| [mid * (1. + i as f64 * 0.000001), 3. * (1. - pressure)])
                        .collect();
                }
                EventData::Trade {
                    price,
                    quantity,
                    buy,
                } => {
                    *price = mid;
                    *quantity = if pressure > 0. { 8. } else { 1. };
                    *buy = step % 2 == 0 || pressure > 0.;
                }
                EventData::Mark { mark, index, .. } => {
                    *mark = mid;
                    *index = mid;
                }
                _ => {}
            }
            e.ingest(event.clone());
            writeln!(out, "{}", serde_json::to_string(&Journal::Event { event })?)?;
        }
        e.records.clear();
    }
    out.flush()?;
    println!(
        "{}",
        serde_json::json!({"state_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&e)?)),"episodes":e.integrity.prediction_episodes_opened,"evaluations":e.integrity.realtime_evaluations,"outcomes":e.outcomes.len(),"trades":e.trades.len(),"funnel":e.trigger_analytics.report()})
    );
    Ok(())
}
