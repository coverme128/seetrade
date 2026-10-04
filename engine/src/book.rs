use crate::types::{EventData, MarketEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SCALE: f64 = 1_000_000.;
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Book {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_scale: Option<u64>,
    pub bids: BTreeMap<i64, f64>,
    pub asks: BTreeMap<i64, f64>,
    pub sequence: i64,
    pub valid: bool,
    pub bridged: bool,
    pub receive_ts: u64,
    pub exchange_ts: u64,
    pub gaps: u64,
    pub error: String,
    pub ofi: f64,
}
impl Book {
    pub fn scale(&self) -> f64 {
        self.price_scale.map_or(SCALE, |x| x as f64)
    }
    pub fn slippage_bps(&self, notional: f64) -> Option<f64> {
        let mid = self.mid()?;
        let wanted = notional / mid;
        let mut worst: f64 = 0.;
        for buy in [true, false] {
            let levels: Vec<_> = if buy {
                self.asks
                    .iter()
                    .map(|(p, q)| (*p as f64 / self.scale(), *q))
                    .collect()
            } else {
                self.bids
                    .iter()
                    .rev()
                    .map(|(p, q)| (*p as f64 / self.scale(), *q))
                    .collect()
            };
            let mut remain = wanted;
            let mut cost = 0.;
            for (p, q) in levels {
                let take = remain.min(q);
                cost += p * take;
                remain -= take;
                if remain <= 1e-10 {
                    break;
                }
            }
            if remain > 1e-8 {
                return None;
            }
            worst = worst.max((cost / wanted / mid - 1.).abs() * 10000.);
        }
        Some(worst)
    }
    pub fn top(&self) -> Option<(f64, f64, f64, f64)> {
        let (b, bq) = self.bids.last_key_value()?;
        let (a, aq) = self.asks.first_key_value()?;
        Some((*b as f64 / self.scale(), *a as f64 / self.scale(), *bq, *aq))
    }
    pub fn mid(&self) -> Option<f64> {
        self.top().map(|(b, a, _, _)| (b + a) / 2.)
    }
    pub fn depth(&self) -> (f64, f64) {
        (
            self.bids
                .iter()
                .rev()
                .take(20)
                .map(|(p, q)| *p as f64 / self.scale() * q)
                .sum(),
            self.asks
                .iter()
                .take(20)
                .map(|(p, q)| *p as f64 / self.scale() * q)
                .sum(),
        )
    }
    fn levels(
        side: &mut BTreeMap<i64, f64>,
        levels: &[[f64; 2]],
        scale: f64,
    ) -> Result<(), String> {
        for [p, q] in levels {
            if !p.is_finite() || !q.is_finite() || *p <= 0. || *p > 1e9 || *q < 0. {
                return Err("invalid_level".into());
            }
            if p * scale < 0.5 || p * scale >= i64::MAX as f64 {
                return Err("price_precision_unsupported".into());
            }
            let key = (p * scale).round() as i64;
            if *q == 0. {
                side.remove(&key);
            } else {
                side.insert(key, *q);
            }
        }
        Ok(())
    }
    pub fn apply(&mut self, e: &MarketEvent) -> Result<bool, String> {
        let old = self.top();
        let result = self.apply_inner(e);
        if let Err(reason) = &result {
            self.valid = false;
            self.gaps += 1;
            self.error = reason.clone();
        }
        if result == Ok(true) {
            if let (Some((b, a, bq, aq)), Some((nb, na, nbq, naq))) = (old, self.top()) {
                let bid_flow = if nb >= b { nbq } else { 0. } - if nb <= b { bq } else { 0. };
                let ask_flow = if na <= a { naq } else { 0. } - if na >= a { aq } else { 0. };
                self.ofi = (0.8 * self.ofi + 0.2 * (bid_flow - ask_flow) / (bq + aq).max(1e-9))
                    .clamp(-1., 1.);
            }
        }
        result
    }
    fn apply_inner(&mut self, e: &MarketEvent) -> Result<bool, String> {
        match &e.data {
            EventData::Snapshot { bids, asks } => {
                self.price_scale = if bids.first().is_some_and(|l| l[0] < 1.) {
                    Some(1_000_000_000_000)
                } else {
                    None
                };
                self.bids.clear();
                self.asks.clear();
                self.ofi = 0.;
                let scale = self.scale();
                Self::levels(&mut self.bids, bids, scale)?;
                Self::levels(&mut self.asks, asks, scale)?;
                self.bridged = e.exchange != "binance";
            }
            EventData::Delta {
                first,
                previous,
                bids,
                asks,
            } => {
                if !self.valid {
                    return Err("snapshot_required".into());
                }
                if e.exchange == "binance" {
                    if e.sequence_id < self.sequence
                        || (self.bridged && e.sequence_id == self.sequence)
                    {
                        return Ok(false);
                    }
                    if !self.bridged {
                        if *first > self.sequence || e.sequence_id < self.sequence {
                            return Err("snapshot_bridge_gap".into());
                        }
                    } else if *previous != self.sequence {
                        return Err("sequence_gap".into());
                    }
                } else if *previous != self.sequence {
                    return Err("sequence_gap".into());
                }
                // OKX may reset seqId lower; matching prevSeqId remains authoritative.
                let scale = self.scale();
                Self::levels(&mut self.bids, bids, scale)?;
                Self::levels(&mut self.asks, asks, scale)?;
                self.bridged = true;
            }
            _ => return Ok(false),
        }
        let (b, a, _, _) = self.top().ok_or("empty_book")?;
        if b >= a {
            return Err("crossed_book".into());
        }
        self.sequence = e.sequence_id;
        self.receive_ts = e.receive_ts;
        self.exchange_ts = e.exchange_ts;
        self.valid = true;
        self.error.clear();
        Ok(true)
    }
    pub fn ready(&self, now: u64, stale: u64) -> bool {
        self.valid
            && self.bridged
            && now.saturating_sub(self.receive_ts) <= stale
            && now.abs_diff(self.exchange_ts) <= stale
    }
}
