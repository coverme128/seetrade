use micro_alpha_engine::{core::Engine, feeds::demo_events, storage::Journal};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("data/demo-fixture.jsonl");
    let steps = args
        .get(2)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(240);
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(path)?);
    let mut engine = Engine::new_reference(true);
    serde_json::to_writer(
        &mut writer,
        &Journal::Start {
            engine: Box::new(engine.clone()),
        },
    )?;
    writeln!(writer)?;
    for i in 0..steps {
        for event in demo_events(i, 1_000_000 + i * 1000) {
            engine.ingest(event.clone());
            serde_json::to_writer(&mut writer, &Journal::Event { event })?;
            writeln!(writer)?;
        }
        if i == 30 {
            engine
                .command("mode", json!({"mode":"PAPER"}))
                .map_err(anyhow::Error::msg)?;
            serde_json::to_writer(
                &mut writer,
                &Journal::Command {
                    command: "mode".into(),
                    payload: json!({"mode":"PAPER"}),
                },
            )?;
            writeln!(writer)?;
        }
        engine.records.clear();
    }
    writer.flush()?;
    println!(
        "{}",
        json!({"path":path,"state_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&engine)?)),"outcomes":engine.outcomes.len(),"trades":engine.trades.len()})
    );
    Ok(())
}
