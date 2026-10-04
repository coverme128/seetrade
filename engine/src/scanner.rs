//! Slow, bounded public-data scanner. Runs outside the hot engine lock.
use crate::{core::Engine, universe::*};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
fn n(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|x| x.is_finite())
}
async fn get(client: &reqwest::Client, url: &str) -> Result<Value> {
    let v: Value = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if v.get("code").is_some_and(|x| x != "0" && x != 0) {
        anyhow::bail!("exchange error {}", v["code"]);
    }
    Ok(v)
}
pub fn instruments(
    venue: &str,
    metadata: &Value,
    tickers: &Value,
    now: u64,
) -> Result<Vec<Observation>> {
    let rows = if venue == "binance" {
        metadata["symbols"].as_array()
    } else {
        metadata["data"].as_array()
    }
    .context("missing instrument list")?;
    anyhow::ensure!(!rows.is_empty(), "empty metadata list; source incomplete");
    let ticks = if venue == "binance" {
        tickers.as_array()
    } else {
        tickers["data"].as_array()
    }
    .context("missing tickers")?;
    let map: BTreeMap<_, _> = ticks
        .iter()
        .filter_map(|r| {
            Some((
                r[if venue == "binance" {
                    "symbol"
                } else {
                    "instId"
                }]
                .as_str()?,
                r,
            ))
        })
        .collect();
    let mut out = vec![];
    for r in rows {
        let (symbol, id, active, listed, value) = if venue == "binance" {
            if r["contractType"] != "PERPETUAL"
                || r["quoteAsset"] != "USDT"
                || r["marginAsset"] != "USDT"
            {
                continue;
            }
            let id = r["symbol"].as_str().context("missing symbol")?;
            (
                normalize(id).map_err(anyhow::Error::msg)?,
                id.to_owned(),
                r["status"] == "TRADING",
                r["onboardDate"].as_u64().unwrap_or(0),
                1.,
            )
        } else {
            if r["settleCcy"] != "USDT" || r["ctType"] != "linear" {
                continue;
            }
            let id = r["instId"].as_str().context("missing instId")?;
            let symbol = normalize(id).map_err(anyhow::Error::msg)?;
            if r["ctValCcy"] != symbol.trim_end_matches("USDT") {
                continue;
            }
            (
                symbol,
                id.to_owned(),
                r["state"] == "live",
                r["listTime"]
                    .as_str()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0),
                n(&r["ctVal"])
                    .filter(|x| *x > 0.)
                    .context("invalid contract value")?,
            )
        };
        let t = map.get(id.as_str()).copied().unwrap_or(&Value::Null);
        let quote_volume = if venue == "binance" {
            n(&t["quoteVolume"])
        } else {
            n(&t["volCcy24h"]).zip(n(&t["last"])).map(|(v, p)| v * p)
        };
        let trade_count = if venue == "binance" {
            t["count"].as_u64()
        } else {
            None
        };
        let metrics = Metrics {
            quote_volume,
            trade_count,
            ..Default::default()
        };
        // A ticker spread is a single observation, never mislabeled as a distribution.
        out.push(Observation {
            asset_key: symbol.clone(),
            instruments: vec![Instrument {
                venue: venue.into(),
                symbol,
                instrument_id: id,
                contract_type: "PERPETUAL".into(),
                active,
                listed_at: listed,
                contract_value: value,
                observed_at: now,
            }],
            metrics,
        });
    }
    Ok(out)
}
async fn venue(client: &reqwest::Client, name: &str, now: u64) -> Result<Vec<Observation>> {
    let (meta, tick) = if name == "binance" {
        (
            "https://fapi.binance.com/fapi/v1/exchangeInfo",
            "https://fapi.binance.com/fapi/v1/ticker/24hr",
        )
    } else {
        (
            "https://www.okx.com/api/v5/public/instruments?instType=SWAP",
            "https://www.okx.com/api/v5/market/tickers?instType=SWAP",
        )
    };
    let (m, t) = tokio::try_join!(get(client, meta), get(client, tick))?;
    instruments(name, &m, &t, now)
}
fn book_metrics(bids: &Value, asks: &Value, multiplier: f64) -> Result<(f64, f64)> {
    let levels = |v: &Value| -> Result<Vec<(f64, f64)>> {
        v.as_array()
            .context("missing levels")?
            .iter()
            .map(|l| {
                let p = n(&l[0]).filter(|x| *x > 0.).context("invalid price")?;
                let q = n(&l[1]).filter(|x| *x >= 0.).context("invalid quantity")?;
                Ok((p, q * multiplier))
            })
            .collect()
    };
    let b = levels(bids)?;
    let a = levels(asks)?;
    let mid = (b.first().context("empty bids")?.0 + a.first().context("empty asks")?.0) / 2.;
    if b[0].0 >= a[0].0 {
        anyhow::bail!("crossed order book");
    }
    let depth = b
        .iter()
        .map(|(p, q)| p * q)
        .sum::<f64>()
        .min(a.iter().map(|(p, q)| p * q).sum::<f64>());
    let mut slip: f64 = 0.;
    for notional in [100., 500., 1000.] {
        for (side, buy) in [(&a, true), (&b, false)] {
            let wanted = notional / mid;
            let mut remain = wanted;
            let mut cost = 0.;
            for (p, q) in side {
                let take = remain.min(*q);
                cost += take * p;
                remain -= take;
                if remain <= 1e-10 {
                    break;
                }
            }
            if remain > 1e-8 {
                anyhow::bail!("insufficient depth at test notional {notional}");
            }
            let price = cost / wanted;
            slip = slip.max(if buy {
                price / mid - 1.
            } else {
                1. - price / mid
            });
        }
    }
    Ok((depth, slip * 10000.))
}
pub fn local_metrics(e: &Engine, symbol: &str) -> Metrics {
    let Some(s) = e.symbols.get(symbol) else {
        return Metrics::default();
    };
    let mut m = Metrics::default();
    let p = e.universe.as_ref().and_then(|u| u.profiles.get(symbol));
    m.clean_since = p.and_then(|p| p.clean_since);
    m.resets_last_hour = p.filter(|p| p.last_reset_count.is_some()).map(|p| {
        p.reset_history
            .iter()
            .filter(|(ts, _)| e.now.saturating_sub(*ts) <= 3_600_000)
            .map(|(_, n)| n)
            .sum()
    });
    m.resets = s.venues.values().map(|v| v.book.gaps + v.reconnects).sum();
    if let Some(f) = e.features(symbol) {
        if f.data_quality.is_empty() {
            m.depth = Some(f.depth);
            m.slippage_bps = s.venues["binance"].book.slippage_bps(1000.);
            m.funding_rate = f.funding_rate;
        }
    }
    if let Some(samples) = e
        .universe
        .as_ref()
        .and_then(|u| u.spread_samples.get(symbol))
    {
        let mut spreads: Vec<_> = samples
            .iter()
            .filter(|(ts, _)| e.now.saturating_sub(*ts) <= 3600000)
            .map(|(_, x)| *x)
            .collect();
        spreads.sort_by(f64::total_cmp);
        if spreads.len() >= 30 {
            m.median_spread_bps = Some(spreads[spreads.len() / 2]);
            m.tail_spread_bps = Some(spreads[spreads.len() * 95 / 100]);
        }
    }
    let returns: Vec<_> = s
        .chart
        .iter()
        .zip(s.chart.iter().skip(1))
        .filter(|(a, b)| b.0 - a.0 <= 2000)
        .map(|(a, b)| (b.1 / a.1).ln())
        .collect();
    if returns.len() >= 30 {
        m.volatility =
            Some((returns.iter().map(|x| x * x).sum::<f64>() / returns.len() as f64).sqrt());
        m.tail_gap = Some(returns.iter().map(|x| x.abs()).fold(0., f64::max));
    }
    m.samples = s.chart.len();
    m
}
pub async fn scan(e: &Engine, id: String, target: Option<String>) -> ScannerRun {
    let u = e.universe.as_ref().expect("migrated universe");
    let start = crate::feeds::now_ms();
    let mut errors = BTreeMap::new();
    let mut all: BTreeMap<String, Observation> = BTreeMap::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("TLS client");
    for name in ["binance", "okx"] {
        match venue(&client, name, start).await {
            Ok(rows) => {
                for row in rows {
                    let o = all
                        .entry(row.asset_key.clone())
                        .or_insert_with(|| Observation {
                            asset_key: row.asset_key.clone(),
                            instruments: vec![],
                            metrics: Metrics::default(),
                        });
                    o.instruments.extend(row.instruments);
                    o.metrics.quote_volume =
                        match (o.metrics.quote_volume, row.metrics.quote_volume) {
                            (Some(a), Some(b)) => Some(a + b),
                            (a, b) => a.or(b),
                        };
                    if row.metrics.trade_count.is_some() {
                        o.metrics.trade_count = row.metrics.trade_count;
                    }
                }
            }
            Err(err) => {
                errors.insert(name.into(), err.to_string());
            }
        }
    }
    // Missing venues remain unknown; never treat an outage as a delisting.
    for o in all.values_mut() {
        for name in errors.keys() {
            if let Some(old) = u
                .instruments
                .get(&format!("{name}:{}:PERPETUAL", o.asset_key))
            {
                o.instruments.push(old.clone());
            }
        }
        let local = local_metrics(e, &o.asset_key);
        let volume = o.metrics.quote_volume;
        let trades = o.metrics.trade_count;
        o.metrics = local;
        o.metrics.quote_volume = volume;
        o.metrics.trade_count = trades;
    }
    if errors.is_empty() {
        for (symbol, p) in &u.profiles {
            if !all.contains_key(symbol) && p.observation.is_some() {
                let mut o = p.observation.clone().unwrap();
                for i in &mut o.instruments {
                    i.active = false;
                    i.observed_at = start;
                }
                o.metrics = Default::default();
                all.insert(symbol.clone(), o);
            }
        }
    }
    let considered = all.len();
    if let Some(t) = &target {
        all.retain(|s, _| s == t);
        if all.is_empty() {
            errors.insert(
                "manual_target".into(),
                format!("{t}: 未从成功返回的交易所元数据中确认可用 USDT 永续合约"),
            );
        }
    }
    let mut selected: Vec<_> = all
        .values()
        .filter(|o| o.instruments.iter().any(|i| i.active))
        .map(|o| (o.asset_key.clone(), o.metrics.quote_volume.unwrap_or(0.)))
        .collect();
    selected.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    selected.truncate(u.config.candidate_l2_slots);
    // Snapshot probes, at most one in flight. No full-universe websocket subscriptions.
    for (symbol, _) in selected {
        let o = all.get_mut(&symbol).unwrap();
        let Some(i) = o
            .instruments
            .iter()
            .find(|i| i.active && i.venue == "binance")
            .or_else(|| o.instruments.iter().find(|i| i.active))
        else {
            continue;
        };
        let url = if i.venue == "binance" {
            format!("https://fapi.binance.com/fapi/v1/depth?symbol={symbol}&limit=20")
        } else {
            format!(
                "https://www.okx.com/api/v5/market/books?instId={}&sz=20",
                i.instrument_id
            )
        };
        match get(&client, &url).await.and_then(|v| {
            let v = if i.venue == "binance" {
                v
            } else {
                v["data"][0].clone()
            };
            book_metrics(&v["bids"], &v["asks"], i.contract_value)
        }) {
            Ok((depth, slippage)) => {
                o.metrics.depth = Some(depth);
                o.metrics.slippage_bps = Some(slippage);
            }
            Err(err) => {
                errors.insert(format!("depth:{symbol}"), err.to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    ScannerRun {
        id,
        started_at: start,
        completed_at: crate::feeds::now_ms(),
        status: if errors.is_empty() {
            "COMPLETED"
        } else if considered == 0 {
            "FAILED"
        } else {
            "PARTIAL"
        }
        .into(),
        config_version: u.config_version,
        target,
        considered,
        errors,
        observations: all.into_values().collect(),
    }
}
