//! Runtime scheduling and protection; existing qualification/risk remain the decision authority.
use crate::{core::Engine, market_pulse::*, types::EventData};
use serde_json::{json, Value};

impl Engine {
    pub fn has_runtime_position(&self, symbol: &str) -> bool {
        self.positions.contains_key(symbol)
            || self.tracked_live_quantity(symbol).abs() > 1e-10
            || self.live.account.as_ref().is_some_and(|a| {
                a.positions
                    .get(symbol)
                    .is_some_and(|p| p.quantity.abs() > 1e-10)
            })
    }
    pub(crate) fn sync_runtime(&mut self, symbol: &str) {
        if self.reference_runtime {
            return;
        }
        let position = self.has_runtime_position(symbol);
        let active = self.symbols[symbol].active.is_some();
        let r = &mut self.symbols.get_mut(symbol).unwrap().runtime;
        let previous = r.state;
        r.state = if position {
            RuntimeState::InPosition
        } else if active {
            RuntimeState::Active
        } else if previous == RuntimeState::Candidate && self.now < r.candidate_until {
            RuntimeState::Candidate
        } else {
            RuntimeState::Idle
        };
        if r.state == RuntimeState::Idle && previous != RuntimeState::Idle {
            let id = r.origin.take().map(|o| o.trigger_id);
            if let Some(id) = id {
                self.log(
                    "trigger_candidate_closed",
                    json!({"trigger_id":id,"symbol":symbol,"ts":self.now}),
                );
            }
        }
    }
    pub(crate) fn runtime_due(&mut self, symbol: &str, force: bool) -> bool {
        if self.reference_runtime {
            return true;
        }
        self.sync_runtime(symbol);
        let r = &self.symbols[symbol].runtime;
        if r.state == RuntimeState::Idle {
            return false;
        }
        let interval = match r.state {
            RuntimeState::Candidate => self.trigger_config.candidate_full_eval_interval_ms,
            RuntimeState::InPosition => self.trigger_config.position_full_eval_interval_ms,
            _ => self.trigger_config.active_full_eval_interval_ms,
        };
        force || self.now.saturating_sub(r.last_full_eval) >= interval
    }
    pub(crate) fn record_runtime_evaluation(&mut self, symbol: &str) {
        let r = &mut self.symbols.get_mut(symbol).unwrap().runtime;
        r.last_full_eval = self.now;
        r.full_evaluations += 1;
        let id = r.origin.as_ref().map(|o| o.trigger_id.clone());
        if let Some(id) = id {
            self.log(
                "trigger_evaluation",
                json!({"trigger_id":id,"symbol":symbol,
            "alpha_evaluation_id":format!("eval-{symbol}-{}",self.now),"evaluated_at":self.now}),
            );
        }
    }
    pub(crate) fn runtime_market_event(&mut self, event: &crate::types::MarketEvent) {
        let symbol = &event.symbol;
        self.sync_runtime(symbol);
        if matches!(
            event.data,
            EventData::Snapshot { .. } | EventData::Delta { .. } | EventData::Trade { .. }
        ) {
            self.symbols
                .get_mut(symbol)
                .unwrap()
                .runtime
                .event_freshness
                .insert(
                    event.exchange.clone(),
                    event.receive_ts.abs_diff(event.exchange_ts) <= self.config.max_stale_ms,
                );
        }
        let (blocked, protection) = self.protection_check(symbol);
        if blocked {
            return;
        }
        if protection {
            self.evaluate_symbol(symbol, true);
            self.live_tick();
            self.protection_result(symbol);
        }
        if event.exchange != "binance" {
            return;
        }
        let s = &mut self.symbols.get_mut(symbol).unwrap();
        let book = &s.venues["binance"].book;
        if let EventData::Trade {
            price,
            quantity,
            buy,
        } = event.data
        {
            if event.receive_ts.abs_diff(event.exchange_ts) <= self.config.max_stale_ms
                && price.is_finite()
                && quantity.is_finite()
                && (price * quantity).is_finite()
                && price > 0.
                && quantity >= 0.
            {
                s.runtime.pulse.trade(self.now, price * quantity, buy);
            }
        }
        let Some((bid, ask, _, _)) = book.top() else {
            return;
        };
        let mid = (bid + ask) / 2.;
        let (bd, ad) = book.depth();
        let input = PulseInput {
            symbol,
            ts: self.now,
            exchange_ts: event.exchange_ts,
            config_version: self.trigger_config_version,
            mid,
            ofi: book.ofi,
            bid_depth: bd,
            ask_depth: ad,
            spread_bps: (ask - bid) / mid * 10000.,
        };
        let start = std::time::Instant::now();
        let snapshot = s.runtime.pulse.sample(input, &self.trigger_config);
        self.telemetry
            .lock()
            .unwrap()
            .record("market_pulse", start.elapsed().as_micros() as u64);
        let Some(snapshot) = snapshot else { return };
        let decision = snapshot.decision();
        let triggered = decision.triggered;
        self.symbols.get_mut(symbol).unwrap().runtime.decision = Some(decision);
        if !triggered {
            self.symbols
                .get_mut(symbol)
                .unwrap()
                .runtime
                .trigger_latched = false;
            return;
        }
        let r = &self.symbols[symbol].runtime;
        let stronger = r.origin.as_ref().is_some_and(|o| {
            snapshot
                .severity_by_family
                .iter()
                .any(|(f, s)| *s > *o.severity_by_family.get(f).unwrap_or(&Severity::Normal))
        });
        // A continuous candidate/episode retains its original ID; only stronger evidence wakes it early.
        if r.origin.is_some() && !stronger {
            return;
        }
        if r.origin.is_none() && r.trigger_latched {
            return;
        }
        if r.origin.is_none()
            && self.now.saturating_sub(r.last_trigger_ts) < self.trigger_config.debounce_ms
        {
            return;
        }
        let before = r.state;
        let id = r
            .origin
            .as_ref()
            .map(|o| o.trigger_id.clone())
            .unwrap_or_else(|| {
                format!(
                    "trigger-{symbol}-{}-{}",
                    self.now, self.trigger_config_version
                )
            });
        let mut severity = r
            .origin
            .as_ref()
            .map(|o| o.severity_by_family.clone())
            .unwrap_or_default();
        for (f, v) in &snapshot.severity_by_family {
            let old = severity.entry(f.clone()).or_default();
            *old = (*old).max(*v);
        }
        let trigger = MarketPulseTriggerEvent {
            trigger_id: id,
            symbol: symbol.clone(),
            exchange_scope: "BINANCE_PRIMARY".into(),
            occurred_at_exchange_ts: snapshot.exchange_ts,
            received_at_local_ts: self.now,
            trigger_families: severity
                .iter()
                .filter(|(_, s)| **s > Severity::Normal)
                .map(|(f, _)| f.clone())
                .collect(),
            severity_by_family: severity,
            normalized_metric_snapshot: snapshot,
            normalization_config_version: self.trigger_config_version,
            runtime_state_before: before,
            action: "FULL_ALPHA_EVALUATION".into(),
        };
        let r = &mut self.symbols.get_mut(symbol).unwrap().runtime;
        if r.state == RuntimeState::Idle {
            r.state = RuntimeState::Candidate;
            r.candidate_until = self.now + self.trigger_config.candidate_ttl_ms;
        }
        r.trigger_latched = true;
        r.last_trigger_ts = self.now;
        r.origin = Some(trigger.clone());
        self.log("market_pulse_trigger", json!(trigger));
        self.evaluate_symbol(symbol, true);
        self.sync_runtime(symbol);
        self.live_tick();
    }
    /// Returns (data blocked, force risk re-evaluation). No independent exit rule is invented here.
    pub(crate) fn protection_check(&mut self, symbol: &str) -> (bool, bool) {
        if self.reference_runtime {
            return (false, false);
        }
        let position = self.has_runtime_position(symbol);
        let s = &self.symbols[symbol];
        let mut data = vec![];
        let mut risk = vec![];
        for (name, v) in &s.venues {
            if s.runtime.event_freshness.get(name) == Some(&false) {
                data.push(format!("{name}:EVENT_TIMESTAMP_SKEW"));
            }
            if !v.connected {
                data.push(format!("{name}:DISCONNECTED"));
            }
            if !v.book.valid {
                data.push(format!("{name}:BOOK_INVALID_OR_GAP"));
            }
            if self.now.saturating_sub(v.book.receive_ts) > self.config.max_stale_ms
                || self.now.abs_diff(v.book.exchange_ts) > self.config.max_stale_ms
            {
                data.push(format!("{name}:STALE_OR_TIMESTAMP_SKEW"));
            }
        }
        // A fresh valid REST snapshot waits for its first bridging delta. Block
        // new plans, retain the causal baseline, and let timeout/gap handling fail closed.
        if data.is_empty() && s.venues.values().any(|v| !v.book.bridged) {
            let s = self.symbols.get_mut(symbol).unwrap();
            s.qualification = None;
            s.plan = None;
            if let Some(ev) = s.evaluation.as_mut() {
                ev.qualification = None;
                ev.target_position = None;
            }
            return (true, false);
        }
        let book = &s.venues["binance"].book;
        let (bd, ad) = book.depth();
        let depth = bd + ad;
        let spread = book
            .top()
            .map(|(b, a, _, _)| (a - b) / ((a + b) / 2.) * 10000.)
            .unwrap_or(0.);
        let returns: Vec<_> = s
            .chart
            .iter()
            .zip(s.chart.iter().skip(1))
            .filter(|_| position)
            .map(|(a, b)| (b.1 / a.1).ln())
            .collect();
        let volatility = if returns.is_empty() {
            0.
        } else {
            (returns.iter().map(|v| v * v).sum::<f64>() / returns.len() as f64).sqrt()
        };
        let slippage = (spread / 2. + self.config.slippage_bps + self.config.impact_bps) / 10000.;
        if position {
            if self.live.orders.values().any(|o| {
                o.action == "ENTRY"
                    && o.plan.symbol == symbol
                    && o.filled_qty > 0.
                    && book.mid().is_some_and(|m| {
                        if o.plan.direction == "LONG" {
                            m <= o.plan.sl || m >= o.plan.tp
                        } else {
                            m >= o.plan.sl || m <= o.plan.tp
                        }
                    })
            }) {
                risk.push("POSITION_BARRIER".into());
            }
            if self.trigger_config.protection_spread && spread > self.config.max_spread_bps {
                risk.push("SPREAD_SPIKE".to_string());
            }
            if self.trigger_config.protection_depth
                && (depth < self.config.min_depth
                    || (s.runtime.pulse.previous_depth > 0.
                        && depth
                            < s.runtime.pulse.previous_depth
                                * (1. - self.trigger_config.depth_drop_fraction)))
            {
                risk.push("DEPTH_COLLAPSE".into());
            }
            if self.trigger_config.protection_slippage && slippage > self.config.max_slippage {
                risk.push("SLIPPAGE_SPIKE".into());
            }
            if self.trigger_config.protection_volatility
                && (volatility > self.config.max_volatility
                    || s.runtime.pulse.previous_mid > 0.
                        && book.mid().is_some_and(|m| {
                            (m / s.runtime.pulse.previous_mid - 1.).abs() > self.config.stop
                        }))
            {
                risk.push("VOLATILITY_OR_FAST_MOVE".into());
            }
            if self.live.configured
                && (!self.live.healthy
                    || self
                        .live
                        .account
                        .as_ref()
                        .is_none_or(|a| self.now.abs_diff(a.ts) > 10000))
            {
                risk.push("EXECUTION_HEALTH".into());
            }
        }
        let blocked = !data.is_empty();
        let mut reasons = data;
        reasons.extend(risk);
        reasons.sort();
        let changed = reasons != s.runtime.protection_reasons;
        let r = &mut self.symbols.get_mut(symbol).unwrap().runtime;
        r.protection_reasons = reasons.clone();
        if blocked {
            r.pulse = Pulse::default();
            let origin = r.origin.take().map(|o| o.trigger_id);
            if let Some(id) = origin {
                self.log(
                    "trigger_candidate_closed",
                    json!({"trigger_id":id,"reason":"data_invalidated"}),
                );
            }
            let r = &mut self.symbols.get_mut(symbol).unwrap().runtime;
            r.candidate_until = 0;
            self.invalidate(symbol, "market_data_integrity");
            let s = self.symbols.get_mut(symbol).unwrap();
            s.qualification = None;
            s.plan = None;
            if let Some(f) = s.features.as_mut() {
                if !f.data_quality.iter().any(|r| r == "RUNTIME_DATA_INVALID") {
                    f.data_quality.push("RUNTIME_DATA_INVALID".into());
                }
            }
            if let Some(ev) = s.evaluation.as_mut() {
                ev.qualification = None;
                ev.target_position = None;
            }
            if position {
                self.live_tick();
            }
        }
        if changed && !reasons.is_empty() {
            let id = self.id(&format!("protect-{symbol}"));
            self.symbols
                .get_mut(symbol)
                .unwrap()
                .runtime
                .last_protection_id = Some(id.clone());
            self.symbols
                .get_mut(symbol)
                .unwrap()
                .runtime
                .last_protection_ts = self.now;
            self.log("protection_trigger",json!({"id":id,"symbol":symbol,
                "occurred_at":self.now,"type":if blocked{"DATA"}else{"RISK"},"reason_codes":reasons,
                "metric_snapshot":{"spread_bps":spread,"depth":depth,"slippage_proxy":slippage,"realized_volatility":volatility},
                "position_open":position,"resulting_action":if blocked{"BLOCK_NEW"}else{"REEVALUATE_RISK"},"config_version":self.config_version}));
        }
        if blocked && position && changed {
            self.protection_result(symbol);
        }
        (
            blocked,
            position && !blocked && changed && !reasons.is_empty(),
        )
    }
    pub(crate) fn protection_result(&mut self, symbol: &str) {
        let action = match self.positions.get(symbol).map(|p| p.action.as_str()) {
            Some("REDUCE") => "REDUCE",
            Some("EXIT" | "EXIT_PENDING") => "EXIT",
            Some(_) => "NO_ACTION",
            None if self.live.protection.contains_key(symbol)
                || !self.has_runtime_position(symbol) =>
            {
                "EXIT"
            }
            None => "NO_ACTION",
        };
        self.log("protection_result",json!({"id":self.symbols[symbol].runtime.last_protection_id,"symbol":symbol,"resulting_action":action,"evaluated_at":self.now}));
    }
    pub(crate) fn set_trigger_config(&mut self, payload: Value) -> Result<(), String> {
        if self.mode != "OBSERVE"
            || self.symbols.keys().any(|s| self.has_runtime_position(s))
            || self.live.orders.values().any(|o| o.pending())
        {
            return Err("触发配置仅在 OBSERVE 且无持仓/待处理订单时修改".into());
        }
        let c: TriggerConfig = serde_json::from_value(payload).map_err(|e| e.to_string())?;
        c.validate()?;
        if self.shadow.as_ref().is_some_and(|r| r.status == "RUNNING") {
            self.pause_shadow("PAUSED_CONFIG");
        }
        self.trigger_config = c;
        self.trigger_config_version += 1;
        self.config_version += 1;
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            self.invalidate(symbol, "config_changed");
            if let Some(origin) = self.symbols.get_mut(symbol).unwrap().runtime.origin.take() {
                self.log(
                    "trigger_candidate_closed",
                    json!({"trigger_id":origin.trigger_id,"reason":"config_changed"}),
                );
            }
            self.symbols.get_mut(symbol).unwrap().runtime = Runtime::default();
        }
        Ok(())
    }
}
