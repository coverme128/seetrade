//! Final portfolio veto; the approved size never exceeds the individual plan.
use crate::{core::Engine, types::TradePlan};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PortfolioConfig {
    pub live_enabled: bool,
    pub enforce_paper: bool,
    pub max_positions: usize,
    pub max_total_leverage: f64,
    pub max_same_direction_leverage: f64,
    pub max_risk_fraction: f64,
    pub max_asset_leverage: f64,
    pub max_venue_leverage: f64,
    pub correlation_threshold: f64,
    pub max_correlated_leverage: f64,
}
impl Default for PortfolioConfig {
    fn default() -> Self {
        Self {
            live_enabled: false,
            enforce_paper: true,
            max_positions: 3,
            max_total_leverage: 1.,
            max_same_direction_leverage: 0.75,
            max_risk_fraction: 0.005,
            max_asset_leverage: 0.5,
            max_venue_leverage: 1.,
            correlation_threshold: 0.8,
            max_correlated_leverage: 0.5,
        }
    }
}
impl PortfolioConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=12).contains(&self.max_positions)
            || [
                self.max_total_leverage,
                self.max_same_direction_leverage,
                self.max_asset_leverage,
                self.max_venue_leverage,
                self.max_correlated_leverage,
            ]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0. || *x > 3.)
            || !self.max_risk_fraction.is_finite()
            || !(0.0001..=0.02).contains(&self.max_risk_fraction)
            || !self.correlation_threshold.is_finite()
            || !(0.5..=0.99).contains(&self.correlation_threshold)
        {
            return Err("组合限制无效".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioDecision {
    pub id: String,
    pub trade_plan_id: String,
    pub asset_key: String,
    pub decision: String,
    pub enforced: bool,
    pub requested_notional: f64,
    pub approved_notional: f64,
    pub approved_leverage: f64,
    pub reason_codes: Vec<String>,
    pub snapshot: Value,
    pub created_at: u64,
}
impl Engine {
    pub fn portfolio_is_live(&self) -> bool {
        ["AUTO", "CONFIRM"].contains(&self.mode.as_str())
            || self.live.orders.values().any(|o| o.pending())
            || self
                .live
                .account
                .as_ref()
                .is_some_and(|a| a.positions.values().any(|p| p.quantity.abs() > 1e-10))
    }
    pub fn portfolio_snapshot(&self, live: bool) -> Value {
        let (equity, rows) = self.portfolio_rows(live, None);
        let gross = rows.iter().map(|r| r.1).sum::<f64>();
        let long = rows.iter().filter(|r| r.2).map(|r| r.1).sum::<f64>();
        json!({"equity":equity,"gross_notional":gross,"long_notional":long,"short_notional":gross-long,"risk_used":rows.iter().map(|r|r.3).sum::<f64>(),"positions":rows.iter().filter(|r| r.1 > 0.).map(|r| &r.0).collect::<std::collections::BTreeSet<_>>().len(),"effective_leverage":gross/equity.max(1.),"venue":if live{self.live.venue.as_str()}else{"paper"},"rows":rows})
    }
    fn portfolio_rows(
        &self,
        live: bool,
        ignore: Option<&str>,
    ) -> (f64, Vec<(String, f64, bool, f64)>) {
        let mut rows = vec![];
        if live {
            if let Some(a) = &self.live.account {
                for (s, p) in &a.positions {
                    if p.quantity.abs() < 1e-10 {
                        continue;
                    }
                    let n = p.quantity.abs() * p.mark_price;
                    let risk = self
                        .live
                        .orders
                        .values()
                        .filter(|o| o.plan.symbol == *s && o.action == "ENTRY" && o.filled_qty > 0.)
                        .max_by_key(|o| o.created_ts)
                        .map_or(n, |o| {
                            p.quantity.abs()
                                * ((p.mark_price - o.plan.sl) * p.quantity.signum()).max(0.)
                                + n * o.qualification.costs.total
                        });
                    rows.push((s.clone(), n, p.quantity > 0., risk));
                }
                for o in self.live.orders.values().filter(|o| {
                    o.pending() && !o.reduce_only() && Some(o.client_id.as_str()) != ignore
                }) {
                    let n = (o.quantity - o.filled_qty).max(0.) * o.limit_price;
                    rows.push((o.plan.symbol.clone(), n, o.buy(), o.plan.planned_max_loss));
                }
                return (a.equity, rows);
            }
            (0., rows)
        } else {
            for (s, p) in &self.positions {
                rows.push((
                    s.clone(),
                    p.qty * p.current_price,
                    p.plan.direction == "LONG",
                    p.qty * (p.current_price - p.plan.sl).abs()
                        + p.qty * p.current_price * p.expected_cost / p.cost_basis.max(1.),
                ));
            }
            (self.account_equity(), rows)
        }
    }
    pub fn asset_correlation(&self, a: &str, b: &str) -> Option<f64> {
        let x = &self.symbols.get(a)?.chart;
        let y = &self.symbols.get(b)?.chart;
        let mut pairs = vec![];
        for w in x.iter().zip(x.iter().skip(1)).rev().take(120) {
            let y0 = y
                .iter()
                .rev()
                .find(|(t, _)| *t <= w.0 .0 && w.0 .0 - *t <= 1000)?;
            let y1 = y
                .iter()
                .rev()
                .find(|(t, _)| *t <= w.1 .0 && w.1 .0 - *t <= 1000)?;
            if y1.0 > y0.0 {
                pairs.push(((w.1 .1 / w.0 .1).ln(), (y1.1 / y0.1).ln()));
            }
        }
        if pairs.len() < 30 {
            return None;
        }
        let n = pairs.len() as f64;
        let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
        let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
        let cov = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>();
        let den = (pairs.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>()
            * pairs.iter().map(|p| (p.1 - my).powi(2)).sum::<f64>())
        .sqrt();
        (den > 1e-16).then(|| (cov / den).clamp(-1., 1.))
    }
    pub fn portfolio_decision(
        &self,
        plan: &TradePlan,
        live: bool,
        ignore: Option<&str>,
    ) -> Option<PortfolioDecision> {
        let u = self.universe.as_ref()?;
        let c = &u.portfolio;
        let (eq, rows) = self.portfolio_rows(live, ignore);
        let long = plan.direction == "LONG";
        let others: Vec<_> = rows.iter().filter(|r| r.0 != plan.symbol).collect();
        let gross = others.iter().map(|r| r.1).sum::<f64>();
        let same = others
            .iter()
            .filter(|r| r.2 == long)
            .map(|r| r.1)
            .sum::<f64>();
        let risk = others.iter().map(|r| r.3).sum::<f64>();
        let correlated = others
            .iter()
            .filter(|r| {
                r.2 == long
                    && self
                        .asset_correlation(&plan.symbol, &r.0)
                        .is_none_or(|x| x >= c.correlation_threshold)
            })
            .map(|r| r.1)
            .sum::<f64>();
        let pending_self = rows
            .iter()
            .filter(|r| r.0 == plan.symbol)
            .map(|r| r.1)
            .sum::<f64>();
        let mut approved = plan.position_notional;
        let mut reasons = vec![];
        let ratio = (plan.planned_max_loss / plan.position_notional.max(1.)).max(0.0001);
        for (code, cap) in [
            ("TOTAL_NOTIONAL", eq * c.max_total_leverage - gross),
            ("SAME_DIRECTION", eq * c.max_same_direction_leverage - same),
            ("RISK_BUDGET", (eq * c.max_risk_fraction - risk) / ratio),
            ("ASSET_CAP", eq * c.max_asset_leverage),
            ("VENUE_CONCENTRATION", eq * c.max_venue_leverage - gross),
            (
                "CORRELATED_EXPOSURE",
                eq * c.max_correlated_leverage - correlated,
            ),
        ] {
            if cap < approved {
                approved = cap.max(0.);
                reasons.push(code.into());
            }
        }
        let count = others
            .iter()
            .map(|r| &r.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if count >= c.max_positions && pending_self == 0. {
            approved = 0.;
            reasons.push("MAX_POSITIONS".into());
        }
        if !eq.is_finite() || eq <= 0. || (live && !c.live_enabled) {
            approved = 0.;
            reasons.push("PORTFOLIO_LIVE_DISABLED_OR_NO_EQUITY".into());
        }
        if !self.asset_mode_allowed(&plan.symbol, if live { &self.mode } else { "PAPER" }) {
            approved = 0.;
            reasons.push("ASSET_MODE_NOT_PERMITTED".into());
        }
        if approved < 10. {
            approved = 0.;
        }
        Some(PortfolioDecision {
            id: format!("portfolio-{}-{}", plan.plan_id, self.now),
            trade_plan_id: plan.plan_id.clone(),
            asset_key: plan.symbol.clone(),
            decision: if approved == 0. {
                "BLOCK"
            } else if approved + 1e-8 < plan.position_notional {
                "REDUCE"
            } else {
                "PASS"
            }
            .into(),
            enforced: live || c.enforce_paper,
            requested_notional: plan.position_notional,
            approved_notional: approved.min(plan.position_notional),
            approved_leverage: approved / eq.max(1.),
            reason_codes: reasons,
            snapshot: json!({"equity":eq,"other_gross":gross,"other_same_direction":same,"other_risk":risk,"correlated_notional":correlated,"correlation_unknown_policy":"COMMON_DIRECTION_CONSERVATIVE","rows":rows,"limits":c,"ignore_intent":ignore}),
            created_at: self.now,
        })
    }
    pub fn apply_portfolio(
        &mut self,
        plan: &mut TradePlan,
        live: bool,
        ignore: Option<&str>,
    ) -> Result<(), String> {
        let Some(mut d) = self.portfolio_decision(plan, live, ignore) else {
            return Ok(());
        };
        d.id = self.id("portfolio");
        self.universe
            .as_mut()
            .unwrap()
            .portfolio_latest
            .insert(plan.symbol.clone(), d.clone());
        self.log("portfolio_gate_decision", json!(d));
        plan.portfolio_decision_id = Some(d.id.clone());
        if !d.enforced {
            return Ok(());
        }
        if d.decision == "BLOCK" {
            return Err(format!("PORTFOLIO_BLOCK: {}", d.reason_codes.join(",")));
        }
        let scale = d.approved_notional / plan.position_notional.max(1e-12);
        plan.position_notional = d.approved_notional;
        plan.effective_leverage = d.approved_leverage;
        plan.expected_net_pnl *= scale;
        plan.expected_loss *= scale;
        plan.planned_max_loss *= scale;
        Ok(())
    }
}
