use micro_alpha_engine::{core::Engine, feeds, types::VenueCosts};
use serde_json::json;
fn main() {
    let mut e = Engine::new_reference(true);
    e.initialize_universe();
    let before_version = e.config_version;
    let btc_config = e.asset_config("BTCUSDT").clone();
    let mut eth = e.asset_config("ETHUSDT").clone();
    eth.risk_per_trade = 0.001;
    e.command("asset_config", json!({"symbol":"ETHUSDT","config":eth}))
        .unwrap();
    let version_probe = json!({"global_version_before":before_version,"global_version_after":e.config_version,"btc_config_unchanged":btc_config==*e.asset_config("BTCUSDT")});
    let mut c = e.asset_config("BTCUSDT").clone();
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
        for event in feeds::demo_events(step, 1_000_000 + step * 1000) {
            e.ingest(event);
        }
    }
    let p = &e
        .outcomes
        .iter()
        .find(|o| o.prediction.symbol == "BTCUSDT")
        .unwrap()
        .prediction;
    let configured = e.asset_config("BTCUSDT").costs_for_venue("binance");
    let expected = 2. * configured.fee_bps
        + 2. * configured.slippage_bps
        + configured.impact_bps
        + configured.adverse_selection_bps
        + configured.funding_bps;
    println!(
        "{}",
        json!({"cross_asset_version":version_probe,"venue_costs":{"prediction_entry_fee_bps":p.qualification.costs.entry_fee*10000.,"frozen_research_cost_bps":p.cost_assumptions_bps,"configured_venue_round_trip_bps":expected},"transition_sample":e.universe.as_ref().unwrap().transitions.first()})
    );
}
