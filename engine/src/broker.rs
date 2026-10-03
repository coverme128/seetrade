//! Exchange credentials never enter Engine, JSON, logs, or the browser.
use crate::{
    feeds::now_ms,
    live::{AccountPosition, AccountSnapshot, FundingSettlement, Intent},
};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::{Client, Method};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug)]
pub struct Rejected(pub String);
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Rejected {}

#[derive(Clone)]
pub struct Broker {
    pub venue: String,
    pub environment: String,
    pub cap: f64,
    pub auto_allowed: bool,
    key: String,
    secret: String,
    passphrase: String,
    base: String,
    client: Client,
    rules: BTreeMap<String, Rule>,
    rules_ts: u64,
}
#[derive(Clone, Debug)]
pub struct Rule {
    pub step: f64,
    pub tick: f64,
    pub min_qty: f64,
    pub min_notional: f64,
    pub contract: f64,
}
fn number(v: &Value) -> Result<f64> {
    let n = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .context("missing numeric exchange field")?;
    if !n.is_finite() {
        bail!("non-finite exchange field");
    }
    Ok(n)
}
fn optional(v: &Value) -> f64 {
    number(v).unwrap_or(0.)
}
fn query(pairs: &[(String, String)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish()
}
pub fn signature(secret: &str, message: &str) -> Vec<u8> {
    let mut m =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC arbitrary key length");
    m.update(message.as_bytes());
    m.finalize().into_bytes().to_vec()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn inst(symbol: &str) -> Result<&'static str> {
    match symbol {
        "BTCUSDT" => Ok("BTC-USDT-SWAP"),
        "ETHUSDT" => Ok("ETH-USDT-SWAP"),
        _ => bail!("unsupported symbol"),
    }
}
fn symbol(inst: &str) -> Result<&'static str> {
    match inst {
        "BTC-USDT-SWAP" => Ok("BTCUSDT"),
        "ETH-USDT-SWAP" => Ok("ETHUSDT"),
        _ => bail!("external position in unsupported instrument"),
    }
}
impl Rule {
    pub fn order_values(&self, o: &Intent) -> Result<(String, String)> {
        let qty = (o.quantity / self.contract / self.step + 1e-10).floor() * self.step;
        let ticks = o.limit_price / self.tick;
        // Bound entry prices strictly by the risk-approved limit.
        let price = if o.buy() {
            ticks.floor() * self.tick
        } else {
            ticks.ceil() * self.tick
        };
        if qty < self.min_qty
            || qty <= 0.
            || (!o.reduce_only() && qty * self.contract * price < self.min_notional)
        {
            bail!("quantity below exchange minimum after rounding");
        }
        Ok((
            format!("{qty:.12}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .into(),
            format!("{price:.12}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .into(),
        ))
    }
}
impl Broker {
    pub fn from_env() -> Result<Option<Self>> {
        let Ok(venue) = std::env::var("MICRO_ALPHA_EXECUTION") else {
            return Ok(None);
        };
        if venue.is_empty() {
            return Ok(None);
        }
        if !["binance", "okx"].contains(&venue.as_str()) {
            bail!("MICRO_ALPHA_EXECUTION must be binance or okx");
        }
        let environment =
            std::env::var("MICRO_ALPHA_EXECUTION_ENV").unwrap_or_else(|_| "testnet".into());
        if !["testnet", "live"].contains(&environment.as_str()) {
            bail!("execution environment must be testnet or live");
        }
        if environment == "live" && std::env::var("MICRO_ALPHA_ALLOW_LIVE").as_deref() != Ok("YES")
        {
            bail!("funded trading requires MICRO_ALPHA_ALLOW_LIVE=YES");
        }
        let cap = std::env::var("MICRO_ALPHA_LIVE_CAP")
            .unwrap_or_else(|_| "100".into())
            .parse::<f64>()?;
        if !(10.0..=500.0).contains(&cap) {
            bail!("personal live cap must be 10..500 USDT");
        }
        let key = std::env::var("MICRO_ALPHA_API_KEY").context("missing MICRO_ALPHA_API_KEY")?;
        let secret =
            std::env::var("MICRO_ALPHA_API_SECRET").context("missing MICRO_ALPHA_API_SECRET")?;
        let passphrase = std::env::var("MICRO_ALPHA_API_PASSPHRASE").unwrap_or_default();
        if key.is_empty() || secret.is_empty() || (venue == "okx" && passphrase.is_empty()) {
            bail!("empty exchange credentials");
        }
        let base = if venue == "okx" {
            "https://www.okx.com"
        } else if environment == "live" {
            "https://fapi.binance.com"
        } else {
            "https://demo-fapi.binance.com"
        }
        .into();
        Ok(Some(Self {
            venue,
            environment,
            cap,
            auto_allowed: std::env::var("MICRO_ALPHA_ALLOW_AUTO").as_deref() == Ok("YES"),
            key,
            secret,
            passphrase,
            base,
            client: Client::builder()
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            rules: BTreeMap::new(),
            rules_ts: 0,
        }))
    }
    pub fn config(&self) -> Value {
        json!({"configured":true,"account_id":hex(&Sha256::digest(self.key.as_bytes())),"venue":self.venue,"environment":self.environment,"cap":self.cap,"auto_allowed":self.auto_allowed})
    }
    async fn request(
        &self,
        method: Method,
        path: &str,
        params: Value,
        signed: bool,
    ) -> Result<Value> {
        let pairs: Vec<_> = params
            .as_object()
            .context("expected request object")?
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                )
            })
            .collect();
        let mut request;
        if self.venue == "binance" {
            let mut pairs = pairs;
            if signed {
                pairs.push(("timestamp".into(), now_ms().to_string()));
                pairs.push(("recvWindow".into(), "5000".into()));
            }
            let mut q = query(&pairs);
            if signed {
                q.push_str(&format!("&signature={}", hex(&signature(&self.secret, &q))));
            }
            request = self
                .client
                .request(method, format!("{}{}?{}", self.base, path, q));
            if signed {
                request = request.header("X-MBX-APIKEY", &self.key);
            }
        } else {
            let q = query(&pairs);
            let request_path = if method == Method::GET && !q.is_empty() {
                format!("{path}?{q}")
            } else {
                path.into()
            };
            let body = if method == Method::GET {
                String::new()
            } else {
                serde_json::to_string(&params)?
            };
            request = self
                .client
                .request(method.clone(), format!("{}{}", self.base, request_path));
            if signed {
                let stamp = time::OffsetDateTime::now_utc().format(
                    &time::format_description::parse_borrowed::<2>(
                        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
                    )?,
                )?;
                let sign = STANDARD.encode(signature(
                    &self.secret,
                    &format!("{stamp}{}{request_path}{body}", method.as_str()),
                ));
                request = request
                    .header("OK-ACCESS-KEY", &self.key)
                    .header("OK-ACCESS-SIGN", sign)
                    .header("OK-ACCESS-TIMESTAMP", stamp)
                    .header("OK-ACCESS-PASSPHRASE", &self.passphrase);
                if self.environment == "testnet" {
                    request = request.header("x-simulated-trading", "1");
                }
            }
            if !body.is_empty() {
                request = request
                    .header("Content-Type", "application/json")
                    .body(body);
            }
        }
        // Never propagate reqwest URL errors: Binance signed query strings contain signatures.
        let response = request
            .send()
            .await
            .map_err(|_| anyhow!("exchange request failed or timed out; reconcile before retry"))?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|_| anyhow!("invalid exchange response"))?;
        if !status.is_success() {
            let code = value["code"].as_i64().unwrap_or(0);
            if status.is_client_error() && code < 0 && ![-1000, -1001, -1006, -1007].contains(&code)
            {
                return Err(Rejected(format!("exchange rejected code {code}")).into());
            }
            bail!("exchange HTTP {} code {}", status.as_u16(), value["code"]);
        }
        if self.venue == "okx" && value["code"] != "0" {
            return Err(Rejected(format!("OKX code {}", value["code"])).into());
        }
        Ok(value)
    }
    pub async fn refresh_rules(&mut self) -> Result<()> {
        if !self.rules.is_empty() && now_ms().saturating_sub(self.rules_ts) < 60_000 {
            return Ok(());
        }
        let mut rules = BTreeMap::new();
        if self.venue == "binance" {
            let v = self
                .request(Method::GET, "/fapi/v1/exchangeInfo", json!({}), false)
                .await?;
            for s in ["BTCUSDT", "ETHUSDT"] {
                let row = v["symbols"]
                    .as_array()
                    .context("missing instruments")?
                    .iter()
                    .find(|r| r["symbol"] == s)
                    .context("missing symbol")?;
                if row["status"] != "TRADING" || row["quoteAsset"] != "USDT" {
                    bail!("instrument unavailable");
                }
                let filters = row["filters"].as_array().context("missing filters")?;
                let f = |kind: &str| {
                    filters
                        .iter()
                        .find(|f| f["filterType"] == kind)
                        .ok_or_else(|| anyhow!("missing filter {kind}"))
                };
                rules.insert(
                    s.into(),
                    Rule {
                        step: number(&f("LOT_SIZE")?["stepSize"])?,
                        tick: number(&f("PRICE_FILTER")?["tickSize"])?,
                        min_qty: number(&f("LOT_SIZE")?["minQty"])?,
                        min_notional: number(&f("MIN_NOTIONAL")?["notional"])?,
                        contract: 1.,
                    },
                );
            }
        } else {
            for s in ["BTCUSDT", "ETHUSDT"] {
                let v = self
                    .request(
                        Method::GET,
                        "/api/v5/public/instruments",
                        json!({"instType":"SWAP","instId":inst(s)?}),
                        false,
                    )
                    .await?;
                let r = &v["data"][0];
                if r["state"] != "live"
                    || r["ctValCcy"] != s.trim_end_matches("USDT")
                    || r["settleCcy"] != "USDT"
                {
                    bail!("unsupported contract units");
                }
                rules.insert(
                    s.into(),
                    Rule {
                        step: number(&r["lotSz"])?,
                        tick: number(&r["tickSz"])?,
                        min_qty: number(&r["minSz"])?,
                        min_notional: 0.,
                        contract: number(&r["ctVal"])?,
                    },
                );
            }
        }
        if rules
            .values()
            .any(|r| r.step <= 0. || r.tick <= 0. || r.contract <= 0.)
        {
            bail!("invalid trading rules");
        }
        self.rules = rules;
        self.rules_ts = now_ms();
        Ok(())
    }
    pub async fn account(&mut self) -> Result<AccountSnapshot> {
        self.refresh_rules().await?;
        let mut a = AccountSnapshot {
            ts: now_ms(),
            ..Default::default()
        };
        if self.venue == "binance" {
            let account = self
                .request(Method::GET, "/fapi/v3/account", json!({}), true)
                .await?;
            let mode = self
                .request(Method::GET, "/fapi/v1/positionSide/dual", json!({}), true)
                .await?;
            let multi = self
                .request(Method::GET, "/fapi/v1/multiAssetsMargin", json!({}), true)
                .await?;
            a.one_way = mode["dualSidePosition"] == false && multi["multiAssetsMargin"] == false;
            let configs = self
                .request(Method::GET, "/fapi/v1/symbolConfig", json!({}), true)
                .await?;
            for symbol in ["BTCUSDT", "ETHUSDT"] {
                let config = configs
                    .as_array()
                    .context("symbol configuration missing")?
                    .iter()
                    .find(|r| r["symbol"] == symbol)
                    .context("symbol configuration missing")?;
                if config["marginType"] != "CROSSED" || number(&config["leverage"])? > 3. {
                    bail!("execution requires cross margin and exchange leverage <= 3 for BTC/ETH");
                }
            }

            a.equity = number(&account["totalMarginBalance"])?;
            a.available = number(&account["availableBalance"])?;
            let pos = self
                .request(Method::GET, "/fapi/v3/positionRisk", json!({}), true)
                .await?;
            for r in pos.as_array().context("positions missing")? {
                let qty = number(&r["positionAmt"])?;
                if qty.abs() < 1e-10 {
                    continue;
                }
                let s = r["symbol"].as_str().context("position symbol")?;
                if !self.rules.contains_key(s) || r["positionSide"] != "BOTH" {
                    bail!("unmanaged instrument or hedge account");
                }
                a.positions.insert(
                    s.into(),
                    AccountPosition {
                        quantity: qty,
                        entry_price: number(&r["entryPrice"])?,
                        mark_price: number(&r["markPrice"])?,
                        liquidation_price: number(&r["liquidationPrice"])?,
                        unrealized_pnl: number(&r["unRealizedProfit"])?,
                    },
                );
            }
            let orders = self
                .request(Method::GET, "/fapi/v1/openOrders", json!({}), true)
                .await?;
            a.open_order_ids = orders
                .as_array()
                .context("open orders")?
                .iter()
                .map(|v| v["clientOrderId"].as_str().unwrap_or("external").into())
                .collect();
            let income = self
                .request(
                    Method::GET,
                    "/fapi/v1/income",
                    json!({"startTime":now_ms()/86_400_000*86_400_000,"limit":1000}),
                    true,
                )
                .await?;
            let rows = income.as_array().context("income missing")?;
            if rows.len() >= 1000 {
                bail!("income pagination required: fail closed");
            }
            for r in rows {
                if ["REALIZED_PNL", "COMMISSION", "FUNDING_FEE"]
                    .contains(&r["incomeType"].as_str().unwrap_or(""))
                {
                    if r["asset"] != "USDT" {
                        bail!("non-USDT fees require conversion");
                    }
                    let income = number(&r["income"])?;
                    a.daily_pnl += income;
                    if r["incomeType"] == "FUNDING_FEE" {
                        a.funding.push(FundingSettlement {
                            symbol: r["symbol"].as_str().unwrap_or("").into(),
                            ts: r["time"].as_u64().unwrap_or(0),
                            income,
                        });
                    }
                }
            }
        } else {
            let mode = self
                .request(Method::GET, "/api/v5/account/config", json!({}), true)
                .await?;
            // Futures mode + net positions only; portfolio margin requires a different risk model.
            a.one_way =
                mode["data"][0]["posMode"] == "net_mode" && mode["data"][0]["acctLv"] == "2";
            for symbol in ["BTCUSDT", "ETHUSDT"] {
                let leverage = self
                    .request(
                        Method::GET,
                        "/api/v5/account/leverage-info",
                        json!({"instId":inst(symbol)?,"mgnMode":"cross"}),
                        true,
                    )
                    .await?;
                if number(&leverage["data"][0]["lever"])? > 3. {
                    bail!("execution requires exchange leverage <= 3 for BTC/ETH");
                }
            }
            let balance = self
                .request(
                    Method::GET,
                    "/api/v5/account/balance",
                    json!({"ccy":"USDT"}),
                    true,
                )
                .await?;
            let r = balance["data"][0]["details"]
                .as_array()
                .context("balance details")?
                .iter()
                .find(|r| r["ccy"] == "USDT")
                .context("USDT balance missing")?;
            a.equity = number(&r["eq"])?;
            a.available = number(&r["availEq"])?;
            let pos = self
                .request(Method::GET, "/api/v5/account/positions", json!({}), true)
                .await?;
            for r in pos["data"].as_array().context("positions missing")? {
                let qty = number(&r["pos"])?;
                if qty.abs() < 1e-10 {
                    continue;
                }
                let s = symbol(r["instId"].as_str().context("position instrument")?)?;
                if r["posSide"] != "net" {
                    bail!("hedge positions unsupported");
                }
                a.positions.insert(
                    s.into(),
                    AccountPosition {
                        quantity: qty * self.rules[s].contract,
                        entry_price: number(&r["avgPx"])?,
                        mark_price: number(&r["markPx"])?,
                        liquidation_price: optional(&r["liqPx"]),
                        unrealized_pnl: number(&r["upl"])?,
                    },
                );
            }
            let orders = self
                .request(Method::GET, "/api/v5/trade/orders-pending", json!({}), true)
                .await?;
            a.open_order_ids = orders["data"]
                .as_array()
                .context("open orders missing")?
                .iter()
                .map(|v| v["clOrdId"].as_str().unwrap_or("external").into())
                .collect();
            let bills = self
                .request(
                    Method::GET,
                    "/api/v5/account/bills",
                    json!({"begin":(now_ms()/86_400_000*86_400_000).to_string(),"limit":"100"}),
                    true,
                )
                .await?;
            let rows = bills["data"].as_array().context("bills missing")?;
            if rows.len() >= 100 {
                bail!("bill pagination required: fail closed");
            }
            for r in rows {
                if r["ccy"] == "USDT" && ["2", "8"].contains(&r["type"].as_str().unwrap_or("")) {
                    let income = number(&r["balChg"])?;
                    a.daily_pnl += income;
                    if r["type"] == "8" {
                        a.funding.push(FundingSettlement {
                            symbol: symbol(r["instId"].as_str().unwrap_or(""))?.into(),
                            ts: r["ts"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                            income,
                        });
                    }
                }
            }
        }
        a.daily_pnl += a
            .positions
            .values()
            .map(|p| p.unrealized_pnl.min(0.))
            .sum::<f64>();
        Ok(a)
    }
    pub fn prepare(&self, o: &Intent) -> Result<Intent> {
        if !o.reduce_only() && now_ms().saturating_sub(self.rules_ts) > 120_000 {
            bail!("instrument rules are stale; only risk reduction is allowed");
        }
        let rule = self
            .rules
            .get(&o.plan.symbol)
            .context("missing instrument rules")?;
        let (qty, px) = rule.order_values(o)?;
        let mut u = o.clone();
        u.quantity = qty.parse::<f64>()? * rule.contract;
        u.limit_price = px.parse()?;
        Ok(u)
    }
    pub async fn submit(&mut self, o: &Intent) -> Result<Intent> {
        // Rules are refreshed independently; protective orders never wait for account/rule HTTP.
        let rule = self
            .rules
            .get(&o.plan.symbol)
            .context("no instrument rule")?;
        let (qty, px) = rule.order_values(o)?;
        let mut updated = o.clone();
        updated.quantity = qty.parse::<f64>()? * rule.contract;
        updated.limit_price = px.parse()?;
        if self.venue == "binance" {
            let mut body = json!({"symbol":o.plan.symbol,"side":if o.buy(){"BUY"}else{"SELL"},"positionSide":"BOTH","type":if o.reduce_only(){"MARKET"}else{"LIMIT"},"quantity":qty,"reduceOnly":o.reduce_only(),"newClientOrderId":o.client_id,"newOrderRespType":"RESULT"});
            if !o.reduce_only() {
                body["timeInForce"] = json!("IOC");
                body["price"] = json!(px);
            }
            let r = self
                .request(Method::POST, "/fapi/v1/order", body, true)
                .await?;
            updated.exchange_order_id = Some(r["orderId"].to_string());
        } else {
            let mut body = json!({"instId":inst(&o.plan.symbol)?,"tdMode":"cross","posSide":"net","clOrdId":o.client_id,"side":if o.buy(){"buy"}else{"sell"},"ordType":if o.reduce_only(){"market"}else{"ioc"},"sz":qty,"reduceOnly":o.reduce_only()});
            if !o.reduce_only() {
                body["px"] = json!(px);
            }
            let r = self
                .request(Method::POST, "/api/v5/trade/order", body, true)
                .await?;
            if r["data"][0]["sCode"] != "0" {
                return Err(
                    Rejected(format!("OKX order rejected code {}", r["data"][0]["sCode"])).into(),
                );
            }
            updated.exchange_order_id = Some(
                r["data"][0]["ordId"]
                    .as_str()
                    .context("missing order id")?
                    .into(),
            );
        }
        updated.ack_ts = now_ms();
        updated.status = "NEW".into();
        // Ack is not fill. Query response is authoritative, including partial IOC fills.
        Ok(updated)
    }
    pub async fn reconcile(&self, o: &Intent) -> Result<Intent> {
        let mut u = o.clone();
        if self.venue == "binance" {
            let r = self
                .request(
                    Method::GET,
                    "/fapi/v1/order",
                    json!({"symbol":o.plan.symbol,"origClientOrderId":o.client_id}),
                    true,
                )
                .await?;
            u.exchange_order_id = Some(r["orderId"].to_string());
            u.status = r["status"].as_str().context("order status")?.into();
            u.filled_qty = number(&r["executedQty"])?;
            u.average_price = number(&r["avgPrice"])?;
            u.fill_ts = r["updateTime"].as_u64().unwrap_or(0);
            if u.filled_qty > 0. {
                let fills = self
                    .request(
                        Method::GET,
                        "/fapi/v1/userTrades",
                        json!({"symbol":o.plan.symbol,"orderId":r["orderId"],"limit":1000}),
                        true,
                    )
                    .await?;
                let rows = fills.as_array().context("fills missing")?;
                if rows.len() >= 1000 {
                    bail!("fill pagination required");
                }
                let total = rows.iter().map(|r| optional(&r["qty"])).sum::<f64>();
                if (total - u.filled_qty).abs() > 1e-8 {
                    bail!("fills not reconciled yet");
                }
                u.fee = 0.;
                for f in rows {
                    if f["commissionAsset"] != "USDT" {
                        bail!("non-USDT commission requires conversion");
                    }
                    u.fee += number(&f["commission"])?;
                }
            }
        } else {
            let r = self
                .request(
                    Method::GET,
                    "/api/v5/trade/order",
                    json!({"instId":inst(&o.plan.symbol)?,"clOrdId":o.client_id}),
                    true,
                )
                .await?;
            let r = &r["data"][0];
            u.exchange_order_id = Some(r["ordId"].as_str().context("order id")?.into());
            u.status = match r["state"].as_str().context("order state")? {
                "live" => "NEW",
                "partially_filled" => "PARTIALLY_FILLED",
                "filled" => "FILLED",
                "canceled" | "mmp_canceled" => "CANCELED",
                _ => "UNKNOWN",
            }
            .into();
            u.filled_qty = number(&r["accFillSz"])? * self.rules[&o.plan.symbol].contract;
            u.average_price = optional(&r["avgPx"]);
            u.fill_ts = r["uTime"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if u.filled_qty > 0. && r["feeCcy"] != "USDT" {
                bail!("non-USDT commission requires conversion");
            }
            u.fee = -optional(&r["fee"]);
        }
        if u.ack_ts == 0 {
            u.ack_ts = now_ms();
        }
        u.reason.clear();
        Ok(u)
    }
    pub async fn cancel(&self, o: &Intent) -> Result<()> {
        if self.venue == "binance" {
            self.request(
                Method::DELETE,
                "/fapi/v1/order",
                json!({"symbol":o.plan.symbol,"origClientOrderId":o.client_id}),
                true,
            )
            .await?;
        } else {
            let r = self
                .request(
                    Method::POST,
                    "/api/v5/trade/cancel-order",
                    json!({"instId":inst(&o.plan.symbol)?,"clOrdId":o.client_id}),
                    true,
                )
                .await?;
            if r["data"][0]["sCode"] != "0" {
                bail!("cancel rejected");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::Engine, feeds::demo_events};
    fn intent() -> Intent {
        let mut e = Engine::new_reference(true);
        for i in 0..8 {
            for ev in demo_events(i, 1_000_000 + i * 1000) {
                e.ingest(ev);
            }
        }
        let mut p = e.symbols["BTCUSDT"].active.clone().unwrap();
        p.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        p.direction = "LONG".into();
        let mut h = e.symbols["BTCUSDT"].health.clone();
        h.trust = "NORMAL".into();
        h.samples = 100;
        h.calibration_error = 0.;
        let (q, plan) = e.qualify(&p, e.symbols["BTCUSDT"].features.as_ref().unwrap(), &h);
        Intent {
            feedback: None,
            execution_mode: "CONFIRM".into(),
            admission: None,
            client_id: "ma123".into(),
            parent_id: None,
            action: "ENTRY".into(),
            plan: plan.unwrap(),
            qualification: q,
            created_ts: 1,
            deadline_ts: u64::MAX,
            quantity: 0.01357,
            limit_price: 100.17,
            status: "SUBMITTING".into(),
            exchange_order_id: None,
            filled_qty: 0.,
            average_price: 0.,
            fee: 0.,
            send_ts: now_ms(),
            ack_ts: 0,
            fill_ts: 0,
            reason: String::new(),
            alpha_id: None,
        }
    }
    #[test]
    fn signature_rfc4231_and_contract_rounding() {
        assert_eq!(
            hex(&signature("Jefe", "what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let rule = Rule {
            step: 1.,
            tick: 0.1,
            min_qty: 1.,
            min_notional: 0.,
            contract: 0.01,
        };
        let mut o = intent();
        let (qty, px) = rule.order_values(&o).unwrap();
        assert_eq!(qty.parse::<f64>().unwrap(), 1.);
        assert_eq!(px.parse::<f64>().unwrap(), 100.1);
        o.plan.direction = "SHORT".into();
        assert_eq!(
            rule.order_values(&o).unwrap().1.parse::<f64>().unwrap(),
            100.2
        );
        o.quantity = 0.009;
        assert!(rule.order_values(&o).is_err());
    }
    #[tokio::test]
    async fn mock_binance_ack_is_not_fill_and_reconcile_never_resubmits() {
        use axum::{
            extract::{Query, State},
            http::{HeaderMap, Method, StatusCode, Uri},
            routing::any,
            Json, Router,
        };
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        async fn order(
            State(n): State<Arc<AtomicUsize>>,
            m: Method,
            h: HeaderMap,
            Query(q): Query<BTreeMap<String, String>>,
        ) -> (StatusCode, Json<Value>) {
            assert_eq!(h["x-mbx-apikey"], "test-key");
            assert!(q.contains_key("signature"));
            if m == Method::POST {
                if q["newClientOrderId"] == "reject" {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"code":-2019,"msg":"margin insufficient"})),
                    );
                }
                if q["reduceOnly"] == "true" {
                    assert_eq!(q["type"], "MARKET");
                    assert_eq!(q["side"], "SELL");
                    assert!(!q.contains_key("timeInForce"));
                } else {
                    assert_eq!(q["timeInForce"], "IOC");
                }
                n.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, Json(json!({"orderId":7})))
            } else {
                assert_eq!(q["origClientOrderId"], "ma123");
                (
                    StatusCode::OK,
                    Json(
                        json!({"orderId":7,"status":"EXPIRED","executedQty":"0.001","avgPrice":"100.1","updateTime":123}),
                    ),
                )
            }
        }
        async fn fills() -> Json<Value> {
            Json(json!([{"qty":"0.001","commission":"0.00004","commissionAsset":"USDT"}]))
        }
        async fn account(h: HeaderMap, uri: Uri) -> Json<Value> {
            assert_eq!(h["x-mbx-apikey"], "test-key");
            assert!(uri.query().unwrap().contains("signature="));
            Json(match uri.path() {
                "/fapi/v3/account" => json!({"totalMarginBalance":"250","availableBalance":"200"}),
                "/fapi/v1/positionSide/dual" => json!({"dualSidePosition":false}),
                "/fapi/v1/multiAssetsMargin" => json!({"multiAssetsMargin":false}),
                "/fapi/v1/symbolConfig" => {
                    json!([{"symbol":"BTCUSDT","marginType":"CROSSED","leverage":2},{"symbol":"ETHUSDT","marginType":"CROSSED","leverage":2}])
                }
                "/fapi/v3/positionRisk" => {
                    json!([{"symbol":"BTCUSDT","positionAmt":"0.001","positionSide":"BOTH","entryPrice":"100","markPrice":"99.8","liquidationPrice":"50","unRealizedProfit":"-0.2"}])
                }
                "/fapi/v1/openOrders" => json!([{"clientOrderId":"ma123"}]),
                "/fapi/v1/income" => {
                    json!([{"incomeType":"REALIZED_PNL","asset":"USDT","income":"5"},{"incomeType":"COMMISSION","asset":"USDT","income":"-0.1"},{"incomeType":"FUNDING_FEE","asset":"USDT","income":"-0.02","symbol":"BTCUSDT","time":123}])
                }
                _ => panic!("unexpected account route: {uri}"),
            })
        }
        let count = Arc::new(AtomicUsize::new(0));
        let router = Router::new()
            .route("/fapi/v1/order", any(order))
            .route("/fapi/v1/userTrades", any(fills))
            .fallback(account)
            .with_state(count.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut b = Broker {
            venue: "binance".into(),
            environment: "testnet".into(),
            cap: 100.,
            auto_allowed: false,
            key: "test-key".into(),
            secret: "test-secret".into(),
            passphrase: String::new(),
            base: format!("http://{address}"),
            client: Client::new(),
            rules: BTreeMap::from([(
                "BTCUSDT".into(),
                Rule {
                    step: 0.001,
                    tick: 0.1,
                    min_qty: 0.001,
                    min_notional: 0.,
                    contract: 1.,
                },
            )]),
            rules_ts: now_ms(),
        };
        let o = b.submit(&intent()).await.unwrap();
        assert_eq!(o.status, "NEW");
        assert_eq!(o.filled_qty, 0.);
        let u = b.reconcile(&o).await.unwrap();
        assert_eq!(u.status, "EXPIRED");
        assert_eq!(u.filled_qty, 0.001);
        assert_eq!(u.fee, 0.00004);
        b.reconcile(&o).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        b.cancel(&o).await.unwrap();
        let account = b.account().await.unwrap();
        assert!(account.one_way);
        assert_eq!(account.equity, 250.);
        assert_eq!(account.positions["BTCUSDT"].quantity, 0.001);
        assert!((account.daily_pnl - 4.68).abs() < 1e-10);
        assert_eq!(account.funding.len(), 1);
        assert_eq!(account.open_order_ids, vec!["ma123"]);
        let mut exit = o.clone();
        exit.action = "EXIT".into();
        b.rules_ts = 0;
        assert!(b.prepare(&o).is_err());
        assert!(b.prepare(&exit).is_ok());
        b.submit(&exit).await.unwrap();
        let mut rejected = o;
        rejected.client_id = "reject".into();
        assert!(b
            .submit(&rejected)
            .await
            .unwrap_err()
            .downcast_ref::<Rejected>()
            .is_some());
        assert_eq!(count.load(Ordering::SeqCst), 2);
        task.abort();
    }
}

#[cfg(test)]
mod okx_tests {
    use super::*;
    #[tokio::test]
    async fn okx_signing_demo_header_and_contract_to_base_reconciliation() {
        use axum::{
            http::{HeaderMap, Uri},
            routing::{get, post},
            Json, Router,
        };
        async fn order(h: HeaderMap, uri: Uri) -> Json<Value> {
            assert_eq!(h["x-simulated-trading"], "1");
            assert_eq!(h["ok-access-key"], "test-key");
            let stamp = h["ok-access-timestamp"].to_str().unwrap();
            let expected = STANDARD.encode(signature("test-secret", &format!("{stamp}GET{uri}")));
            assert_eq!(h["ok-access-sign"], expected);
            Json(
                json!({"code":"0","data":[{"ordId":"7","state":"canceled","accFillSz":"2","avgPx":"100","uTime":"123","fee":"-0.03","feeCcy":"USDT"}]}),
            )
        }
        async fn submit(h: HeaderMap, uri: Uri, body: String) -> Json<Value> {
            let stamp = h["ok-access-timestamp"].to_str().unwrap();
            assert_eq!(
                h["ok-access-sign"],
                STANDARD.encode(signature("test-secret", &format!("{stamp}POST{uri}{body}")))
            );
            assert_eq!(h["x-simulated-trading"], "1");
            let p: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(p["instId"], "BTC-USDT-SWAP");
            if uri.path().ends_with("cancel-order") {
                return Json(json!({"code":"0","data":[{"sCode":"0"}]}));
            }
            if p["clOrdId"] == "reject" {
                return Json(json!({"code":"0","data":[{"sCode":"51008"}]}));
            }
            assert_eq!(p["tdMode"], "cross");
            assert_eq!(p["posSide"], "net");
            if p["reduceOnly"] == true {
                assert_eq!(p["ordType"], "market");
                assert_eq!(p["side"], "sell");
                assert_eq!(p["sz"], "2");
            } else {
                assert_eq!(p["ordType"], "ioc");
                assert_eq!(p["sz"], "10");
            }
            Json(json!({"code":"0","data":[{"sCode":"0","ordId":"7"}]}))
        }
        async fn account(h: HeaderMap, uri: Uri) -> Json<Value> {
            assert_eq!(h["x-simulated-trading"], "1");
            let data = match uri.path() {
                "/api/v5/account/config" => json!([{"posMode":"net_mode","acctLv":"2"}]),
                "/api/v5/account/leverage-info" => json!([{"lever":"2"}]),
                "/api/v5/account/balance" => {
                    json!([{"details":[{"ccy":"USDT","eq":"250","availEq":"200"}]}])
                }
                "/api/v5/account/positions" => {
                    json!([{"instId":"BTC-USDT-SWAP","pos":"2","posSide":"net","avgPx":"100","markPx":"99.8","liqPx":"50","upl":"-0.2"}])
                }
                "/api/v5/trade/orders-pending" => json!([{"clOrdId":"ma1"}]),
                "/api/v5/account/bills" => {
                    json!([{"ccy":"USDT","type":"2","balChg":"4.9"},{"ccy":"USDT","type":"8","balChg":"-0.02","instId":"BTC-USDT-SWAP","ts":"123"}])
                }
                _ => panic!("unexpected route: {uri}"),
            };
            Json(json!({"code":"0","data":data}))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/api/v5/trade/order", get(order).post(submit))
                    .route("/api/v5/trade/cancel-order", post(submit))
                    .fallback(account),
            )
            .await
            .unwrap();
        });
        // Reuse a real engine-generated, risk-approved intent via serde fixture construction.
        let mut e = crate::core::Engine::new_reference(true);
        for i in 0..8 {
            for v in crate::feeds::demo_events(i, 1_000_000 + i * 1000) {
                e.ingest(v);
            }
        }
        let p = e.symbols["BTCUSDT"].active.as_ref().unwrap();
        let plan = json!({"plan_id":"plan","prediction_id":p.prediction_id,"symbol":"BTCUSDT","direction":"LONG","position_notional":100.,"effective_leverage":0.1,"expected_net_pnl":1.,"expected_loss":0.1,"planned_max_loss":1.,"break_even_probability":0.5,"expected_mae":0.,"liquidity_capacity":1000.,"liquidation_buffer":1.,"tp":101.,"sl":99.,"horizon":60,"entry_type":"LIMIT_IOC","exit_rules":[]});
        let o:Intent=serde_json::from_value(json!({"client_id":"ma1","parent_id":null,"action":"ENTRY","plan":plan,"qualification":p.qualification,"created_ts":1,"deadline_ts":60001,"quantity":0.1,"limit_price":100.,"status":"UNKNOWN","exchange_order_id":null,"filled_qty":0.,"average_price":0.,"fee":0.,"send_ts":1,"ack_ts":0,"fill_ts":0,"reason":"","alpha_id":null})).unwrap();
        let mut b = Broker {
            venue: "okx".into(),
            environment: "testnet".into(),
            cap: 100.,
            auto_allowed: false,
            key: "test-key".into(),
            secret: "test-secret".into(),
            passphrase: "test-passphrase".into(),
            base: format!("http://{address}"),
            client: Client::new(),
            rules: BTreeMap::from([(
                "BTCUSDT".into(),
                Rule {
                    step: 1.,
                    tick: 0.1,
                    min_qty: 1.,
                    min_notional: 0.,
                    contract: 0.01,
                },
            )]),
            rules_ts: now_ms(),
        };
        let result = b.reconcile(&o).await.unwrap();
        assert_eq!(result.filled_qty, 0.02);
        assert_eq!(result.status, "CANCELED");
        assert_eq!(result.fee, 0.03);
        let account = b.account().await.unwrap();
        assert!(account.one_way);
        assert_eq!(account.equity, 250.);
        assert_eq!(account.positions["BTCUSDT"].quantity, 0.02);
        assert!((account.daily_pnl - 4.68).abs() < 1e-10);
        assert_eq!(account.funding.len(), 1);
        let entry = b.submit(&o).await.unwrap();
        assert_eq!(entry.status, "NEW");
        assert_eq!(entry.filled_qty, 0.);
        b.cancel(&entry).await.unwrap();
        let mut exit = o.clone();
        exit.action = "EXIT".into();
        exit.quantity = result.filled_qty;
        b.submit(&exit).await.unwrap();
        let mut rejected = o;
        rejected.client_id = "reject".into();
        assert!(b
            .submit(&rejected)
            .await
            .unwrap_err()
            .downcast_ref::<Rejected>()
            .is_some());
        server.abort();
    }
}
