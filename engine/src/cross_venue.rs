//! Receive-time lead/lag context, not a causal claim or an input to micro-v1's model.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
const STEP: u64 = 500;
const MAX_AGE: u64 = 750;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeadLag {
    pub clock: String,
    pub leader: String,
    pub lag_ms: u64,
    pub correlation: f64,
    pub zero_lag_correlation: f64,
    pub samples: usize,
}
pub fn asof(prices: &VecDeque<(u64, f64)>, ts: u64, max_age: u64) -> Option<f64> {
    prices
        .iter()
        .rev()
        .find(|(t, _)| *t <= ts)
        .filter(|(t, p)| ts - *t <= max_age && p.is_finite() && *p > 0.)
        .map(|x| x.1)
}
fn correlation(rows: &[(f64, f64)]) -> Option<f64> {
    if rows.len() < 60 {
        return None;
    }
    let n = rows.len() as f64;
    let x = rows.iter().map(|p| p.0).sum::<f64>() / n;
    let y = rows.iter().map(|p| p.1).sum::<f64>() / n;
    let xx = rows.iter().map(|p| (p.0 - x).powi(2)).sum::<f64>();
    let yy = rows.iter().map(|p| (p.1 - y).powi(2)).sum::<f64>();
    if xx < 1e-16 || yy < 1e-16 {
        return None;
    }
    Some(
        (rows.iter().map(|p| (p.0 - x) * (p.1 - y)).sum::<f64>() / (xx * yy).sqrt()).clamp(-1., 1.),
    )
}
pub fn estimate(a: &VecDeque<(u64, f64)>, b: &VecDeque<(u64, f64)>, now: u64) -> Option<LeadLag> {
    asof(a, now, MAX_AGE)?;
    asof(b, now, MAX_AGE)?;
    let end = now / STEP * STEP;
    let start = end.checked_sub(60_000)?;
    let returns = |prices: &VecDeque<(u64, f64)>| -> Vec<Option<f64>> {
        let mut input = prices.iter().peekable();
        let mut last: Option<(u64, f64)> = None;
        let mut previous: Option<f64> = None;
        let mut result = Vec::with_capacity(120);
        for i in 0..=120 {
            let ts = start + i * STEP;
            while input.peek().is_some_and(|(t, _)| *t <= ts) {
                last = input.next().copied();
            }
            let price = last
                .filter(|(t, p)| ts - *t <= MAX_AGE && p.is_finite() && *p > 0.)
                .map(|x| x.1);
            if i > 0 {
                result.push(price.zip(previous).map(|(p, prev)| (p / prev).ln()));
            }
            previous = price;
        }
        result
    };
    let ar = returns(a);
    let br = returns(b);
    // Positive lag pairs Binance's earlier return with OKX's later return.
    let score = |lag: i32| {
        let rows: Vec<_> = (0..120i32)
            .filter_map(|i| {
                let j = i + lag;
                if !(0..120).contains(&j) {
                    return None;
                }
                Some((ar[i as usize]?, br[j as usize]?))
            })
            .collect();
        correlation(&rows).map(|c| (c, rows.len()))
    };
    let (zero, count) = score(0)?;
    let mut best = (0, zero, count);
    for lag in [-4, -3, -2, -1, 1, 2, 3, 4] {
        if let Some((c, n)) = score(lag) {
            if c > best.1 {
                best = (lag, c, n);
            }
        }
    }
    if best.1 < 0.5 {
        return None;
    }
    if best.1 - zero < 0.05 {
        best = (0, zero, count);
    }
    if best.1 < 0.5 {
        return None;
    }
    Some(LeadLag {
        clock: "receive_time_500ms_60s_v1".into(),
        leader: match best.0.cmp(&0) {
            std::cmp::Ordering::Greater => "binance",
            std::cmp::Ordering::Less => "okx",
            _ => "synchronous",
        }
        .into(),
        lag_ms: best.0.unsigned_abs() as u64 * STEP,
        correlation: best.1,
        zero_lag_correlation: zero,
        samples: best.2,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn history(delay: u64) -> VecDeque<(u64, f64)> {
        let mut price = 100.;
        (0..130)
            .map(|i| {
                price *= 1. + (((i * 73 + i * i * 19) % 97) as f64 - 48.) * 0.00001;
                (100_000 + i * STEP + delay, price)
            })
            .collect()
    }
    #[test]
    fn detects_both_leaders_and_synchronous_prices_without_future_data() {
        let a = history(0);
        let b = history(1000);
        let result = estimate(&a, &b, 164500).unwrap();
        assert_eq!(result.leader, "binance");
        assert_eq!(result.lag_ms, 1000);
        assert!(result.correlation > 0.99);
        assert_eq!(estimate(&b, &a, 164500).unwrap().leader, "okx");
        assert_eq!(estimate(&a, &a, 164500).unwrap().lag_ms, 0);
        let mut future = a.clone();
        future.push_back((200000, 999.));
        assert_eq!(
            serde_json::to_value(estimate(&future, &b, 164500)).unwrap(),
            serde_json::to_value(Some(result)).unwrap()
        );
    }
    #[test]
    fn stale_sparse_flat_and_missing_history_are_unavailable() {
        let a = history(0);
        assert!(estimate(&a, &a, 166000).is_none());
        let flat = a.iter().map(|(t, _)| (*t, 100.)).collect();
        assert!(estimate(&flat, &flat, 164500).is_none());
        let sparse = a.iter().step_by(8).copied().collect();
        assert!(estimate(&a, &sparse, 164000).is_none());
        assert!(estimate(&a, &VecDeque::new(), 164500).is_none());
        assert!(asof(&a, 99999, MAX_AGE).is_none());
    }
}
