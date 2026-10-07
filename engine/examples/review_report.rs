//! Offline daily/cumulative review using exactly the HTTP aggregation path.
use anyhow::{Context, Result};
use micro_alpha_engine::{
    feeds,
    review::{build, ReviewQuery},
};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let db = args
        .get(1)
        .context("usage: review_report <database> <YYYY-MM-DD|all> <output.json>")?;
    let date = args.get(2).context("missing date")?;
    let output = args.get(3).context("missing output.json")?;
    let report = build(
        Path::new(db),
        &ReviewQuery {
            date: Some(date.clone()),
            export: Some(true),
            ..Default::default()
        },
        feeds::now_ms(),
    )?;
    std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} opportunities; report saved to {}",
        report["summary"]["opportunities"], output
    );
    Ok(())
}
