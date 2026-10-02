//! One sizing calculation, with an explicit paper or exchange-account context.
use crate::{core::Engine, types::Prediction};
pub(crate) struct RiskContext {
    pub equity: f64,
    pub daily_remaining: f64,
    pub other_notional: f64,
    pub notional_cap: f64,
    pub available_notional: f64,
    pub available: bool,
    pub live: bool,
}
impl Engine {
    pub(crate) fn qualification_risk(&self, prediction: &Prediction) -> RiskContext {
        let c = &self.config;
        let live = ["CONFIRM", "AUTO"].contains(&self.mode.as_str())
            || self.live.orders.values().any(|o| o.pending())
            || ["BTCUSDT", "ETHUSDT"]
                .iter()
                .any(|s| self.tracked_live_quantity(s).abs() > 1e-10)
            || self
                .live
                .account
                .as_ref()
                .is_some_and(|a| a.positions.values().any(|p| p.quantity.abs() > 1e-10));
        if live {
            let Some(a) = self.live.account.as_ref() else {
                return RiskContext {
                    equity: 0.,
                    daily_remaining: 0.,
                    other_notional: 0.,
                    notional_cap: 0.,
                    available_notional: 0.,
                    available: false,
                    live,
                };
            };
            let mut other_notional = 0.;
            let mut other_risk = 0.;
            let mut current_notional = 0.;
            let mut pending_notional = 0.;
            for (symbol, position) in &a.positions {
                let notional = position.quantity.abs() * position.mark_price;
                if *symbol == prediction.symbol {
                    current_notional = notional;
                    continue;
                }
                other_notional += notional;
                let stop = self
                    .live
                    .orders
                    .values()
                    .filter(|o| {
                        o.plan.symbol == *symbol && o.action == "ENTRY" && o.filled_qty > 0.
                    })
                    .max_by_key(|o| o.created_ts)
                    .map(|o| o.plan.sl);
                let cost = (2. * c.fee_bps
                    + 2. * c.slippage_bps
                    + c.impact_bps
                    + c.adverse_selection_bps
                    + c.funding_bps)
                    / 10000.;
                other_risk += stop.map_or(notional, |stop| {
                    position.quantity.abs()
                        * ((position.mark_price - stop) * position.quantity.signum()).max(0.)
                        + notional * cost
                });
            }
            for order in self
                .live
                .orders
                .values()
                .filter(|o| o.pending() && !o.reduce_only())
            {
                // The target is the whole symbol position, including its pending increase.
                // Other symbols reserve both portfolio exposure and available cash.
                if order.plan.symbol == prediction.symbol {
                    continue;
                }
                let reserved = order.quantity * order.limit_price;
                other_notional += reserved;
                pending_notional += reserved;
                other_risk += order.plan.planned_max_loss;
            }
            RiskContext {
                equity: a.equity.max(0.),
                daily_remaining: (c.max_daily_loss + a.daily_pnl - other_risk).max(0.),
                other_notional,
                notional_cap: self.live.cap.min(c.max_position_notional),
                available_notional: current_notional
                    + (a.available.max(0.) * 0.95 - pending_notional).max(0.),
                available: self.live_ready().is_ok(),
                live,
            }
        } else {
            let equity = self.account_equity().max(0.);
            let other_notional = self
                .positions
                .iter()
                .filter(|(s, _)| **s != prediction.symbol)
                .map(|(_, p)| p.qty * p.current_price)
                .sum();
            let other_risk = self
                .positions
                .iter()
                .filter(|(s, _)| **s != prediction.symbol)
                .map(|(_, p)| p.plan.planned_max_loss)
                .sum::<f64>();
            let unrealized = self
                .positions
                .values()
                .map(|p| {
                    (p.current_price - p.entry_price)
                        * p.qty
                        * if p.plan.direction == "LONG" { 1. } else { -1. }
                        + p.realized_pnl
                        - p.fees
                        - p.funding
                })
                .sum::<f64>()
                .min(0.);
            let daily = c.max_daily_loss + self.daily_pnl + unrealized - other_risk;
            RiskContext {
                equity,
                daily_remaining: daily.max(0.),
                other_notional,
                notional_cap: c.max_position_notional,
                available_notional: equity * c.max_effective_leverage,
                available: !self.kill_switch
                    && self.storage_healthy
                    && equity > 0.
                    && daily > 0.
                    && (self.equity_peak - equity) / self.equity_peak.max(1.) < c.max_drawdown,
                live,
            }
        }
    }
}
