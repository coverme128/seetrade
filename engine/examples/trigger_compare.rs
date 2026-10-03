fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: trigger_compare <complete journal.jsonl>"))?;
    let report = micro_alpha_engine::replay_compare::compare(std::path::Path::new(&path))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
