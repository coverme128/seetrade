//! Public endpoints only; no subscriptions, account credentials or private orders.
use micro_alpha_engine::{core::Engine, feeds, scanner};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut e = Engine::new(false);
    e.now = feeds::now_ms();
    e.initialize_universe();
    let target = std::env::args().nth(1);
    let run = scanner::scan(&e, format!("public-probe-{}", e.now), target).await;
    println!("{}", serde_json::to_string_pretty(&run)?);
    Ok(())
}
