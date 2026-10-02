use crate::types::{EventData, MarketEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SCALE: f64 = 1_000_000.;
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Book {
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
    pub fn top(&self) -> Option<(f64, f64, f64, f64)> {
        let (b, bq) = self.bids.last_key_value()?;
        let (a, aq) = self.asks.first_key_value()?;
        Some((*b as f64 / SCALE, *a as f64 / SCALE, *bq, *aq))
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
                .map(|(p, q)| *p as f64 / SCALE * q)
                .sum(),
            self.asks
                .iter()
                .take(20)
                .map(|(p, q)| *p as f64 / SCALE * q)
                .sum(),
        )
    }
    fn levels(side: &mut BTreeMap<i64, f64>, levels: &[[f64; 2]]) -> Result<(), String> {
        for [p, q] in levels {
            if !p.is_finite() || !q.is_finite() || *p <= 0. || *p > 1e9 || *q < 0. {
                return Err("invalid_level".into());
            }
            let key = (p * SCALE).round() as i64;
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
                self.bids.clear();
                self.asks.clear();
                self.ofi = 0.;
                Self::levels(&mut self.bids, bids)?;
                Self::levels(&mut self.asks, asks)?;
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
                Self::levels(&mut self.bids, bids)?;
                Self::levels(&mut self.asks, asks)?;
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
