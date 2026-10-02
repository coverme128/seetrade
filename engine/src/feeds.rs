use crate::{book::Book, types::*};
use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn number(v: &Value) -> Result<f64> {
    let n = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| anyhow!("invalid number"))?;
    if !n.is_finite() {
        return Err(anyhow!("non-finite number"));
    }
    Ok(n)
}
fn integer(v: &Value) -> Result<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| anyhow!("invalid sequence/timestamp"))
}
fn levels(v: &Value, multiplier: f64) -> Result<Vec<[f64; 2]>> {
    v.as_array()
        .context("missing levels")?
        .iter()
        .map(|v| Ok([number(&v[0])?, number(&v[1])? * multiplier]))
        .collect()
}
async fn send(
    tx: &mpsc::Sender<MarketEvent>,
    exchange: &str,
    symbol: &str,
    ts: u64,
    seq: i64,
    data: EventData,
) -> Result<()> {
    tx.send(MarketEvent {
        exchange: exchange.into(),
        symbol: symbol.into(),
        exchange_ts: ts,
        receive_ts: now_ms(),
        sequence_id: seq,
        data,
    })
    .await
    .map_err(|_| anyhow!("engine stopped"))
}
pub fn spawn_live(tx: mpsc::Sender<MarketEvent>) {
    for s in ["BTCUSDT", "ETHUSDT"] {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut backoff = 1;
            loop {
                let started = Instant::now();
                let result = binance(&tx, s).await;
                let reason = result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "disconnected".into());
                tracing::warn!(exchange="binance",symbol=s,%reason);
                if send(
                    &tx,
                    "binance",
                    s,
                    now_ms(),
                    0,
                    EventData::Health {
                        connected: false,
                        reason,
                    },
                )
                .await
                .is_err()
                {
                    break;
                }
                if started.elapsed() > Duration::from_secs(60) {
                    backoff = 1;
                }
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
            }
        });
    }
    tokio::spawn(async move {
        let mut backoff = 1;
        loop {
            let started = Instant::now();
            let reason = okx(&tx)
                .await
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "disconnected".into());
            tracing::warn!(exchange="okx",%reason);
            for s in ["BTCUSDT", "ETHUSDT"] {
                if send(
                    &tx,
                    "okx",
                    s,
                    now_ms(),
                    0,
                    EventData::Health {
                        connected: false,
                        reason: reason.clone(),
                    },
                )
                .await
                .is_err()
                {
                    return;
                }
            }
            if started.elapsed() > Duration::from_secs(60) {
                backoff = 1;
            }
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    });
}
async fn binance(tx: &mpsc::Sender<MarketEvent>, symbol: &str) -> Result<()> {
    let lower = symbol.to_lowercase();
    let (mut depth, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async(format!(
            "wss://fstream.binance.com/public/ws/{lower}@depth@100ms"
        )),
    )
    .await??;
    let (mut market, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async(format!(
            "wss://fstream.binance.com/market/stream?streams={lower}@aggTrade/{lower}@markPrice@1s"
        )),
    )
    .await??;
    // WebSocket is established first. Frames buffered by the socket are bridged to this REST snapshot.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let snapshot: Value = client
        .get(format!(
            "https://fapi.binance.com/fapi/v1/depth?symbol={symbol}&limit=1000"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let snapshot = MarketEvent {
        exchange: "binance".into(),
        symbol: symbol.into(),
        exchange_ts: snapshot["E"].as_u64().unwrap_or_else(now_ms),
        receive_ts: now_ms(),
        sequence_id: integer(&snapshot["lastUpdateId"])?,
        data: EventData::Snapshot {
            bids: levels(&snapshot["bids"], 1.)?,
            asks: levels(&snapshot["asks"], 1.)?,
        },
    };
    let mut book = Book::default();
    book.apply(&snapshot).map_err(|e| anyhow!(e))?;
    tx.send(snapshot).await?;
    let mut health_timer = tokio::time::interval(Duration::from_secs(5));
    let mut last_depth = Instant::now();
    let mut last_market = Instant::now();
    loop {
        tokio::select! {
            message=depth.next()=>{
                match message.context("depth closed")?? {
                    Message::Ping(p)=>depth.send(Message::Pong(p)).await?,
                    Message::Close(_)=>return Err(anyhow!("depth disconnected")),
                    Message::Text(t)=>{
                        let v:Value=serde_json::from_str(&t)?;
                        let e=MarketEvent{exchange:"binance".into(),symbol:symbol.into(),exchange_ts:integer(&v["E"])? as u64,receive_ts:now_ms(),sequence_id:integer(&v["u"])?,
                            data:EventData::Delta{first:integer(&v["U"])?,previous:integer(&v["pu"])?,bids:levels(&v["b"],1.)?,asks:levels(&v["a"],1.)?}};
                        if book.apply(&e).map_err(|e|anyhow!(e))? {tx.send(e).await?;}last_depth=Instant::now();
                    },_=>{}
                }
            }
            message=market.next()=>{
                match message.context("market closed")?? {
                    Message::Ping(p)=>market.send(Message::Pong(p)).await?,
                    Message::Close(_)=>return Err(anyhow!("market disconnected")),
                    Message::Text(t)=>{
                        let wrapper:Value=serde_json::from_str(&t)?;let v=wrapper.get("data").unwrap_or(&wrapper);last_market=Instant::now();
                        let ts=integer(&v["E"])? as u64;
                        match v["e"].as_str(){
                            Some("aggTrade")=>send(tx,"binance",symbol,ts,integer(&v["a"])?,EventData::Trade{price:number(&v["p"])?,quantity:number(&v["q"])?,buy:!v["m"].as_bool().context("missing maker side")?}).await?,
                            Some("markPriceUpdate")=>send(tx,"binance",symbol,ts,0,EventData::Mark{mark:number(&v["p"])?,index:number(&v["i"])?,funding:number(&v["r"])?}).await?,_=>{}
                        }
                    },_=>{}
                }
            }
            _=health_timer.tick()=>{if last_depth.elapsed()>Duration::from_secs(10)||last_market.elapsed()>Duration::from_secs(15){return Err(anyhow!("feed stalled"));}}
        }
    }
}
async fn okx(tx: &mpsc::Sender<MarketEvent>) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut multipliers = BTreeMap::new();
    for (inst, base) in [("BTC-USDT-SWAP", "BTC"), ("ETH-USDT-SWAP", "ETH")] {
        let v: Value = client
            .get(format!(
                "https://www.okx.com/api/v5/public/instruments?instType=SWAP&instId={inst}"
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let row = &v["data"][0];
        if row["ctValCcy"].as_str() != Some(base) {
            return Err(anyhow!("unsupported contract value currency"));
        }
        let multiplier = number(&row["ctVal"])?;
        if multiplier <= 0. {
            return Err(anyhow!("invalid contract multiplier"));
        }
        multipliers.insert(inst, multiplier);
    }
    let (mut ws, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async("wss://ws.okx.com:8443/ws/v5/public"),
    )
    .await??;
    ws.send(Message::Text(json!({"op":"subscribe","args":[
        {"channel":"books","instId":"BTC-USDT-SWAP"},{"channel":"books","instId":"ETH-USDT-SWAP"},
        {"channel":"trades","instId":"BTC-USDT-SWAP"},{"channel":"trades","instId":"ETH-USDT-SWAP"}
    ]}).to_string().into())).await?;
    let mut books: BTreeMap<String, Book> = BTreeMap::new();
    let mut ping = tokio::time::interval(Duration::from_secs(10));
    let mut last = Instant::now();
    loop {
        tokio::select! {
            _=ping.tick()=>{if last.elapsed()>Duration::from_secs(30){return Err(anyhow!("OKX heartbeat timeout"));}ws.send(Message::Text("ping".into())).await?;}
            message=ws.next()=>{match message.context("OKX closed")?? {
                Message::Ping(p)=>ws.send(Message::Pong(p)).await?,Message::Close(_)=>return Err(anyhow!("OKX disconnected")),
                Message::Text(t)=>{
                    last=Instant::now();if t=="pong"{continue;}
                    let v:Value=serde_json::from_str(&t)?;if v["event"]=="error"{return Err(anyhow!("OKX subscription: {}",v["msg"]));}
                    let inst=v["arg"]["instId"].as_str().unwrap_or("");
                    let symbol=match inst{"BTC-USDT-SWAP"=>"BTCUSDT","ETH-USDT-SWAP"=>"ETHUSDT",_=>continue};
                    let Some(rows)=v["data"].as_array()else{continue;};
                    for row in rows {
                        let ts=integer(&row["ts"])? as u64;
                        match v["arg"]["channel"].as_str(){
                            Some("books")=>{
                                let bids=levels(&row["bids"],multipliers[inst])?;let asks=levels(&row["asks"],multipliers[inst])?;
                                let data=if v["action"]=="snapshot"{EventData::Snapshot{bids,asks}}else{EventData::Delta{first:integer(&row["seqId"])?,previous:integer(&row["prevSeqId"])?,bids,asks}};
                                let e=MarketEvent{exchange:"okx".into(),symbol:symbol.into(),exchange_ts:ts,receive_ts:now_ms(),sequence_id:integer(&row["seqId"])?,data};
                                books.entry(symbol.into()).or_default().apply(&e).map_err(|e|anyhow!(e))?;tx.send(e).await?;
                            }
                            Some("trades")=>send(tx,"okx",symbol,ts,0,EventData::Trade{price:number(&row["px"])?,quantity:number(&row["sz"])?*multipliers[inst],buy:row["side"]=="buy"}).await?,_=>{}
                        }
                    }
                },_=>{}
            }}
        }
    }
}

/// Synthetic deterministic fixture. Never reported as live market data.
pub fn demo_events(step: u64, ts: u64) -> Vec<MarketEvent> {
    let mut events = vec![];
    for (symbol, base, offset) in [("BTCUSDT", 67420., 0.), ("ETHUSDT", 3520., 0.8)] {
        let t = step as f64;
        let phase = t / 25. + offset;
        let price = base * (1. + 0.0028 * phase.sin() + 0.0004 * (t * 0.37).sin());
        let pressure = phase.cos() * 0.72;
        for exchange in ["okx", "binance"] {
            let mid = price
                * (1.
                    + if exchange == "okx" {
                        0.00001 * (t / 9.).sin()
                    } else {
                        0.
                    });
            let qty = if symbol == "BTCUSDT" { 1.4 } else { 25. };
            let tick = if symbol == "BTCUSDT" { 0.1 } else { 0.01 };
            let bids = (1..=20)
                .map(|i| [mid - i as f64 * tick, qty * (1. + pressure)])
                .collect();
            let asks = (1..=20)
                .map(|i| [mid + i as f64 * tick, qty * (1. - pressure)])
                .collect();
            let seq = (step * 2 + 1) as i64;
            events.push(MarketEvent {
                exchange: exchange.into(),
                symbol: symbol.into(),
                exchange_ts: ts,
                receive_ts: ts,
                sequence_id: seq,
                data: EventData::Snapshot { bids, asks },
            });
            if exchange == "binance" {
                events.push(MarketEvent {
                    exchange: exchange.into(),
                    symbol: symbol.into(),
                    exchange_ts: ts,
                    receive_ts: ts,
                    sequence_id: seq + 1,
                    data: EventData::Delta {
                        first: seq,
                        previous: seq,
                        bids: vec![],
                        asks: vec![],
                    },
                });
            }
            events.push(MarketEvent {
                exchange: exchange.into(),
                symbol: symbol.into(),
                exchange_ts: ts,
                receive_ts: ts,
                sequence_id: seq,
                data: EventData::Trade {
                    price: mid,
                    quantity: qty,
                    buy: pressure > 0.,
                },
            });
            if exchange == "binance" {
                events.push(MarketEvent {
                    exchange: exchange.into(),
                    symbol: symbol.into(),
                    exchange_ts: ts,
                    receive_ts: ts,
                    sequence_id: seq,
                    data: EventData::Mark {
                        mark: mid,
                        index: mid * 0.99999,
                        funding: 0.0001,
                    },
                });
            }
        }
    }
    events.push(MarketEvent {
        exchange: "clock".into(),
        symbol: String::new(),
        exchange_ts: ts,
        receive_ts: ts,
        sequence_id: 0,
        data: EventData::Clock,
    });
    events
}
