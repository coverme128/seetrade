//! Terminal first-hit truth is immutable; horizon return is a separate later observation.
use crate::{core::Engine, types::Prediction};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HorizonObservation {
    pub deadline_ts: u64,
    pub observed_price_ts: Option<u64>,
    pub final_move_at_timeout: Option<f64>,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HorizonTracker {
    pub symbol: String,
    pub start_price: f64,
    pub sign: f64,
    pub deadline_ts: u64,
    pub last_price: f64,
    pub last_price_ts: u64,
    pub invalidated: bool,
}
impl Engine {
    pub(crate) fn watch_horizon(
        &mut self,
        p: &Prediction,
        kind: &str,
    ) -> Option<HorizonObservation> {
        let deadline = p.ts + p.horizon * 1000;
        if kind.ends_with("INVALIDATED") && kind != "SIGNAL_INVALIDATED" {
            return Some(HorizonObservation {
                deadline_ts: deadline,
                observed_price_ts: None,
                final_move_at_timeout: None,
                status: "DATA_INVALIDATED".into(),
            });
        }
        let last_ts = if p.episode.last_price_ts == 0 {
            p.ts
        } else {
            p.episode.last_price_ts
        };
        if self.now >= deadline {
            let valid = deadline.saturating_sub(last_ts) <= self.config.max_stale_ms;
            return Some(HorizonObservation {
                deadline_ts: deadline,
                observed_price_ts: valid.then_some(last_ts),
                final_move_at_timeout: valid
                    .then_some((p.current_price / p.start_price - 1.) * p.sign()),
                status: if valid {
                    "OBSERVED"
                } else {
                    "DATA_INVALIDATED"
                }
                .into(),
            });
        }
        self.horizon_pending.insert(
            p.prediction_id.clone(),
            HorizonTracker {
                symbol: p.symbol.clone(),
                start_price: p.start_price,
                sign: p.sign(),
                deadline_ts: deadline,
                last_price: p.current_price,
                last_price_ts: last_ts,
                invalidated: false,
            },
        );
        None
    }
    pub(crate) fn update_horizons(&mut self) {
        let mut finished = vec![];
        for (id, tracker) in &mut self.horizon_pending {
            let venue = &self.symbols[&tracker.symbol].venues["binance"];
            if !venue.connected
                || !venue.book.valid
                || self.now.saturating_sub(venue.book.receive_ts) > self.config.max_stale_ms
                || self.now.abs_diff(venue.book.exchange_ts) > self.config.max_stale_ms
            {
                tracker.invalidated = true;
            }
            if !tracker.invalidated
                && venue.book.bridged
                && venue.book.receive_ts <= tracker.deadline_ts
                && venue.book.receive_ts >= tracker.last_price_ts
            {
                if let Some(price) = venue.book.mid() {
                    tracker.last_price = price;
                    tracker.last_price_ts = venue.book.receive_ts;
                }
            }
            if self.now >= tracker.deadline_ts || tracker.invalidated {
                let valid = !tracker.invalidated
                    && tracker.deadline_ts.saturating_sub(tracker.last_price_ts)
                        <= self.config.max_stale_ms;
                finished.push((
                    id.clone(),
                    HorizonObservation {
                        deadline_ts: tracker.deadline_ts,
                        observed_price_ts: valid.then_some(tracker.last_price_ts),
                        final_move_at_timeout: valid.then_some(
                            (tracker.last_price / tracker.start_price - 1.) * tracker.sign,
                        ),
                        status: if valid {
                            "OBSERVED"
                        } else {
                            "DATA_INVALIDATED"
                        }
                        .into(),
                    },
                ));
            }
        }
        for (id, observation) in finished {
            self.horizon_pending.remove(&id);
            if let Some(o) = self
                .outcomes
                .iter_mut()
                .find(|o| o.prediction.prediction_id == id)
            {
                o.horizon_observation = Some(observation.clone());
            }
            for s in self.symbols.values_mut() {
                if let Some(o) = s
                    .last_result
                    .as_mut()
                    .filter(|o| o.prediction.prediction_id == id)
                {
                    o.horizon_observation = Some(observation.clone());
                }
            }
            self.log(
                "outcome_horizon",
                json!({"prediction_id":id,"observation":observation}),
            );
        }
    }
}
