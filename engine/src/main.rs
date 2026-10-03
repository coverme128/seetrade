use anyhow::{Context, Result};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use micro_alpha_engine::{
    broker::Broker,
    core::Engine,
    feeds,
    storage::{Batch, Journal, Store},
    types::*,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
use tokio::sync::{mpsc, Mutex};

struct App {
    engine: Mutex<Engine>,
    store: Store,
}
type Shared = Arc<App>;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--replay") {
        return replay(args.get(i + 1).context("--replay needs file")?);
    }
    micro_alpha_engine::assist::from_env().map_err(anyhow::Error::msg)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787")
        .await
        .context("engine port 8787 unavailable")?;
    let demo = args.iter().any(|a| a == "--demo");
    let source = if demo { "demo" } else { "live" };
    let dir = std::env::var("MICRO_ALPHA_DATA").unwrap_or_else(|_| "data".into());
    let (store, restored) = Store::open(std::path::Path::new(&dir), source)?;
    let mut engine = restored.unwrap_or_else(|| Engine::new(demo));
    // Research reference policy can never be restored as a production runtime switch.
    engine.reference_runtime = false;
    if engine.now > 0 {
        engine.restart();
    }
    if args.iter().any(|a| a == "--model") {
        anyhow::bail!("直接 --model 替换已停用；请用 --challenger <模型> --report <报告> 开始影子验证，在界面通过检查后晋升。已有 checkpoint 模型仍可恢复。");
    }
    if let Some(i) = args.iter().position(|a| a == "--challenger") {
        let j = args
            .iter()
            .position(|a| a == "--report")
            .context("--challenger requires --report")?;
        let artifact =
            std::fs::read_to_string(args.get(i + 1).context("--challenger needs file")?)?;
        let report: Value = serde_json::from_slice(&std::fs::read(
            args.get(j + 1).context("--report needs file")?,
        )?)?;
        engine.now = engine.now.max(feeds::now_ms());
        engine
            .command("shadow_start", json!({"artifact":artifact,"report":report}))
            .map_err(anyhow::Error::msg)?;
    }
    let broker = Broker::from_env()?;
    let broker_config = broker
        .as_ref()
        .map(Broker::config)
        .unwrap_or_else(|| json!({"configured":false}));
    if engine.live.configured
        && !engine.live.orders.is_empty()
        && (broker_config["venue"] != engine.live.venue
            || broker_config["environment"] != engine.live.environment
            || broker_config["account_id"] != engine.live.account_id)
    {
        anyhow::bail!("存在未对账订单，不能切换执行交易所或账户环境");
    }
    engine
        .command("broker_config", broker_config)
        .map_err(anyhow::Error::msg)?;
    let initial = serde_json::to_string(&engine)?;
    store
        .tx
        .send(Batch {
            journal: Journal::Start {
                engine: Box::new(engine.clone()),
            },
            records: std::mem::take(&mut engine.records),
            checkpoint: Some(initial),
            committed: None,
        })
        .await?;
    tracing::info!(journal=%store.journal_path.display(),"session recording started");
    let app = Arc::new(App {
        engine: Mutex::new(engine),
        store,
    });
    let research_worker = tokio::spawn(research_worker(app.clone(), PathBuf::from(&dir)));
    let execution_worker = broker.map(|b| tokio::spawn(broker_worker(app.clone(), b)));
    let (tx, mut rx) = mpsc::channel(8192);
    if demo {
        let demo_tx = tx.clone();
        tokio::spawn(async move {
            let mut step = 0;
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                for e in feeds::demo_events(step, feeds::now_ms()) {
                    if demo_tx.send(e).await.is_err() {
                        return;
                    }
                }
                step += 1;
            }
        });
    } else {
        feeds::spawn_live(tx.clone());
        let clock_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let ts = feeds::now_ms();
                if clock_tx
                    .send(MarketEvent {
                        exchange: "clock".into(),
                        symbol: String::new(),
                        exchange_ts: ts,
                        receive_ts: ts,
                        sequence_id: 0,
                        data: EventData::Clock,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    let runtime = app.clone();
    let worker = tokio::spawn(async move {
        let mut last_checkpoint = std::time::Instant::now();
        while let Some(mut e) = rx.recv().await {
            let mut engine = runtime.engine.lock().await;
            engine.telemetry.lock().unwrap().record(
                "ingest_queue_delay",
                feeds::now_ms().saturating_sub(e.receive_ts) * 1000,
            );
            // Serialize receive order across independent venue sockets before recording/replay.
            e.receive_ts = e.receive_ts.max(engine.now);
            // Journal every input, but avoid serializing the entire history at 10 Hz.
            // Derived records and broker/control commands still force durable checkpoints.
            let checkpoint = matches!(e.data, EventData::Clock)
                && last_checkpoint.elapsed() >= std::time::Duration::from_secs(1);
            if checkpoint {
                last_checkpoint = std::time::Instant::now();
            }
            engine.storage_healthy = runtime.store.healthy.load(Ordering::SeqCst);
            if !engine.storage_healthy {
                engine.kill_switch = true;
                engine.mode = "OBSERVE".into();
            }
            let started = std::time::Instant::now();
            engine.ingest(e.clone());
            engine
                .telemetry
                .lock()
                .unwrap()
                .record("core_ingest", started.elapsed().as_micros() as u64);
            persist(
                &runtime,
                &mut engine,
                Journal::Event { event: e },
                checkpoint,
            );
        }
    });
    let router = Router::new()
        .route("/api/state", get(state))
        .route("/api/health", get(health))
        .route("/api/trigger-analytics", get(trigger_analytics))
        .route("/api/assist", post(assist))
        .route("/api/history", get(history))
        .route("/api/control/{command}", post(control))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(app.clone());
    tracing::info!("Micro Alpha running at 127.0.0.1:8787 · source={source} · mode=OBSERVE");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    research_worker.abort();
    let _ = research_worker.await;
    if let Some(w) = execution_worker {
        w.abort();
        let _ = w.await;
    }
    worker.abort();
    let _ = worker.await;
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    {
        let mut engine = app.engine.lock().await;
        engine
            .command("mode", json!({"mode":"OBSERVE"}))
            .map_err(anyhow::Error::msg)?;
        let checkpoint = serde_json::to_string(&*engine)?;
        let records = std::mem::take(&mut engine.records);
        app.store
            .tx
            .send(Batch {
                journal: Journal::Command {
                    command: "mode".into(),
                    payload: json!({"mode":"OBSERVE"}),
                },
                records,
                checkpoint: Some(checkpoint),
                committed: Some(commit_tx),
            })
            .await?;
    }
    commit_rx.await?.map_err(anyhow::Error::msg)?;
    Ok(())
}
fn persist(app: &App, engine: &mut Engine, journal: Journal, checkpoint: bool) {
    let started = std::time::Instant::now();
    let snapshot = if checkpoint || !engine.records.is_empty() {
        let snapshot = serde_json::to_string(&*engine).expect("finite engine state");
        engine
            .telemetry
            .lock()
            .unwrap()
            .record("checkpoint_serialize", started.elapsed().as_micros() as u64);
        Some(snapshot)
    } else {
        None
    };
    let records = std::mem::take(&mut engine.records);
    if app
        .store
        .tx
        .try_send(Batch {
            journal,
            records,
            checkpoint: snapshot,
            committed: None,
        })
        .is_err()
    {
        engine.storage_healthy = false;
        engine.kill_switch = true;
        engine.mode = "OBSERVE".into();
        app.store.healthy.store(false, Ordering::SeqCst);
        tracing::error!("recorder queue full or disconnected; new paper orders halted");
    }
}
async fn state(State(app): State<Shared>) -> Json<Value> {
    let mut snapshot = app.engine.lock().await.view();
    snapshot["ai_mode"] = json!(if micro_alpha_engine::assist::from_env()
        .ok()
        .flatten()
        .is_some()
    {
        "ASSIST"
    } else {
        "OFF"
    });
    Json(snapshot)
}
async fn trigger_analytics(State(app): State<Shared>) -> Json<Value> {
    let e = app.engine.lock().await;
    let mut links: Vec<_> = e.trigger_analytics.links.values().collect();
    links.sort_by(|a, b| {
        b.ts.cmp(&a.ts)
            .then_with(|| b.trigger_id.cmp(&a.trigger_id))
    });
    links.truncate(100);
    Json(json!({"report": e.trigger_analytics.report(), "recent_links":links}))
}
async fn health(State(app): State<Shared>) -> Json<Value> {
    let e = app.engine.lock().await;
    Json(json!({"ok":e.storage_healthy,"source":e.source,"live_enabled":e.live_ready().is_ok()}))
}
async fn control(
    State(app): State<Shared>,
    Path(command): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let err = |s: StatusCode, m: String| (s, Json(json!({"error":m})));
    if headers.get("x-micro-alpha").and_then(|v| v.to_str().ok()) != Some("local") {
        return Err(err(StatusCode::FORBIDDEN, "缺少本地控制请求标记".into()));
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if ![
            "http://127.0.0.1:3000",
            "http://localhost:3000",
            "http://127.0.0.1:8787",
        ]
        .contains(&origin)
        {
            return Err(err(StatusCode::FORBIDDEN, "仅允许本机界面控制".into()));
        }
    }
    if command.starts_with("broker_")
        || ["research_started", "research_finished"].contains(&command.as_str())
    {
        return Err(err(
            StatusCode::FORBIDDEN,
            "执行回报只能由本机适配器写入".into(),
        ));
    }
    let mut engine = app.engine.lock().await;
    if !app.store.healthy.load(Ordering::SeqCst) {
        return Err(err(
            StatusCode::SERVICE_UNAVAILABLE,
            "存储不可用，控制操作已暂停".into(),
        ));
    }
    // Reserve recording capacity before accepting a mutation.
    let permit = app.store.tx.try_reserve().map_err(|_| {
        err(
            StatusCode::SERVICE_UNAVAILABLE,
            "记录队列繁忙，请稍后重试".into(),
        )
    })?;
    let command_result = engine.command(&command, payload.clone());
    let journal = match &command_result {
        Ok(()) => Journal::Command { command, payload },
        Err(error) => Journal::RejectedCommand {
            command,
            payload,
            error: error.clone(),
        },
    };
    let checkpoint = serde_json::to_string(&*engine)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    permit.send(Batch {
        journal,
        records: std::mem::take(&mut engine.records),
        checkpoint: Some(checkpoint),
        committed: Some(commit_tx),
    });
    let view = engine.view();
    drop(engine);
    // Durable confirmation happens outside the trading-core lock.
    commit_rx
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "存储服务已停止".into()))?
        .map_err(|e| err(StatusCode::SERVICE_UNAVAILABLE, e))?;
    command_result.map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(view))
}
#[derive(Deserialize)]
struct HistoryQuery {
    kind: Option<String>,
    limit: Option<usize>,
}
async fn history(
    State(app): State<Shared>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    let path = app.store.path.clone();
    let kind = q.kind.unwrap_or_else(|| "outcome".into());
    let limit = q.limit.unwrap_or(100);
    let rows = tokio::task::spawn_blocking(move || Store::history(&path, &kind, limit))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(rows)))
}
fn replay(path: &str) -> Result<()> {
    let file = BufReader::new(File::open(PathBuf::from(path))?);
    let mut engine = None;
    let mut count = 0;
    for line in file.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Journal>(&line)
            .with_context(|| format!("invalid journal line {}", count + 1))?
        {
            Journal::Start { engine: mut e } => {
                // Legacy recordings contain no trigger version and used the fixed-step policy.
                if e.trigger_config_version == 0 {
                    e.reference_runtime = true;
                }
                engine = Some(*e);
            }
            Journal::Event { event } => engine
                .as_mut()
                .context("journal missing start")?
                .ingest(event),
            Journal::RejectedCommand {
                command,
                payload,
                error,
            } => {
                let result = engine
                    .as_mut()
                    .context("journal missing start")?
                    .command(&command, payload);
                anyhow::ensure!(result == Err(error), "rejected command replay mismatch");
            }
            Journal::Command { command, payload } => engine
                .as_mut()
                .context("journal missing start")?
                .command(&command, payload)
                .map_err(anyhow::Error::msg)?,
        }
        if let Some(e) = &mut engine {
            e.records.clear();
        }
        count += 1;
    }
    let engine = engine.context("empty recording")?;
    let bytes = serde_json::to_vec(&engine)?;
    println!(
        "{}",
        json!({"records":count,"state_sha256":format!("{:x}",Sha256::digest(bytes)),"outcomes":engine.outcomes.len(),"trades":engine.trades.len(),"source":engine.source})
    );
    Ok(())
}

async fn broker_commit(app: &App, command: &str, payload: Value) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let command_result;
    {
        let mut e = app.engine.lock().await;
        if !app.store.healthy.load(Ordering::SeqCst) {
            anyhow::bail!("storage unavailable");
        }
        let permit = app.store.tx.reserve().await?;
        command_result = e.command(command, payload.clone());
        let journal = match &command_result {
            Ok(()) => Journal::Command {
                command: command.into(),
                payload,
            },
            Err(error) => Journal::RejectedCommand {
                command: command.into(),
                payload,
                error: error.clone(),
            },
        };
        permit.send(Batch {
            journal,
            records: std::mem::take(&mut e.records),
            checkpoint: Some(serde_json::to_string(&*e)?),
            committed: Some(tx),
        });
    }
    rx.await?.map_err(anyhow::Error::msg)?;
    command_result.map_err(anyhow::Error::msg)
}
trait ExecutionGateway: Clone + Send + Sync + 'static {
    fn prepare(
        &self,
        order: &micro_alpha_engine::live::Intent,
    ) -> Result<micro_alpha_engine::live::Intent>;
    fn account(
        &mut self,
    ) -> impl std::future::Future<Output = Result<micro_alpha_engine::live::AccountSnapshot>> + Send;
    fn submit(
        &mut self,
        order: &micro_alpha_engine::live::Intent,
    ) -> impl std::future::Future<Output = Result<micro_alpha_engine::live::Intent>> + Send;
    fn reconcile(
        &self,
        order: &micro_alpha_engine::live::Intent,
    ) -> impl std::future::Future<Output = Result<micro_alpha_engine::live::Intent>> + Send;
    fn cancel(
        &self,
        order: &micro_alpha_engine::live::Intent,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}
impl ExecutionGateway for Broker {
    fn prepare(
        &self,
        order: &micro_alpha_engine::live::Intent,
    ) -> Result<micro_alpha_engine::live::Intent> {
        Broker::prepare(self, order)
    }
    async fn account(&mut self) -> Result<micro_alpha_engine::live::AccountSnapshot> {
        Broker::account(self).await
    }
    async fn submit(
        &mut self,
        order: &micro_alpha_engine::live::Intent,
    ) -> Result<micro_alpha_engine::live::Intent> {
        Broker::submit(self, order).await
    }
    async fn reconcile(
        &self,
        order: &micro_alpha_engine::live::Intent,
    ) -> Result<micro_alpha_engine::live::Intent> {
        Broker::reconcile(self, order).await
    }
    async fn cancel(&self, order: &micro_alpha_engine::live::Intent) -> Result<()> {
        Broker::cancel(self, order).await
    }
}
async fn account_worker<G: ExecutionGateway>(app: Shared, rules: tokio::sync::watch::Receiver<G>) {
    loop {
        let mut broker = rules.borrow().clone();
        match broker.account().await {
            Ok(a) => {
                if broker_commit(&app, "broker_snapshot", json!(a))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(err) => {
                if broker_commit(&app, "broker_error", json!({"error":err.to_string()}))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
// Each task owns its network requests. Dropping the JoinSet aborts every child at shutdown.
async fn broker_worker(app: Shared, broker: Broker) {
    let (rules_tx, rules_rx) = tokio::sync::watch::channel(broker.clone());
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async move {
        let mut broker = broker;
        loop {
            if broker.refresh_rules().await.is_ok() {
                rules_tx.send_replace(broker.clone());
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    tasks.spawn(account_worker(app.clone(), rules_rx.clone()));
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        for protective in [true, false] {
            tasks.spawn(order_worker(
                app.clone(),
                rules_rx.clone(),
                symbol,
                protective,
            ));
        }
        tasks.spawn(cancel_worker(app.clone(), rules_rx.clone(), symbol));
    }
    // An unexpectedly stopped execution task is a fail-closed event for new risk.
    tasks.join_next().await;
    let _ = broker_commit(
        &app,
        "broker_error",
        json!({"error":"执行任务停止，请重启并对账"}),
    )
    .await;
}
async fn cancel_worker<G: ExecutionGateway>(
    app: Shared,
    rules: tokio::sync::watch::Receiver<G>,
    symbol: &str,
) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let orders = {
            let e = app.engine.lock().await;
            e.live
                .orders
                .values()
                .filter(|o| {
                    o.plan.symbol == symbol
                        && o.pending()
                        && !o.reduce_only()
                        && o.status != "QUEUED"
                        && (e.kill_switch
                            || feeds::now_ms() >= o.deadline_ts
                            || feeds::now_ms().abs_diff(e.now) > e.config.max_stale_ms
                            || e.mode != o.execution_mode
                            || e.live.protection.contains_key(symbol))
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let broker = rules.borrow().clone();
        for o in orders {
            // Cancellation is advisory until the order query confirms fills and final status.
            // A submit may still be in flight; repeat cancellation with the same client ID.
            if let Err(err) = broker.cancel(&o).await {
                let _ = broker_commit(
                    &app,
                    "broker_error",
                    json!({"error":format!("撤单待核实: {err}")}),
                )
                .await;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    }
}
fn runtime_risk_expired(
    e: &Engine,
    order: &micro_alpha_engine::live::Intent,
    wall_now: u64,
) -> bool {
    !order.reduce_only()
        && (wall_now.abs_diff(e.now) > e.config.max_stale_ms
            || wall_now >= order.deadline_ts
            || wall_now.saturating_sub(order.created_ts) > 5000)
}

async fn order_worker<G: ExecutionGateway>(
    app: Shared,
    rules: tokio::sync::watch::Receiver<G>,
    symbol: &str,
    protective: bool,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_reconcile = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        interval.tick().await;
        if !app.store.healthy.load(Ordering::SeqCst) {
            continue;
        }
        let mut broker = rules.borrow().clone();
        if last_reconcile.elapsed() >= std::time::Duration::from_secs(1) {
            let orders = {
                let e = app.engine.lock().await;
                e.live
                    .orders
                    .values()
                    .filter(|o| {
                        o.plan.symbol == symbol
                            && o.reduce_only() == protective
                            && o.pending()
                            && o.status != "QUEUED"
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };
            for mut o in orders {
                match broker.reconcile(&o).await {
                    Ok(u) => o = u,
                    Err(err) => {
                        o.status = "UNKNOWN".into();
                        o.reason = err.to_string();
                    }
                }
                if broker_commit(&app, "broker_order", json!(o)).await.is_err() {
                    return;
                }
            }
            last_reconcile = std::time::Instant::now();
        }
        let queued = {
            let e = app.engine.lock().await;
            e.live
                .orders
                .values()
                .filter(|o| {
                    o.plan.symbol == symbol && o.reduce_only() == protective && o.status == "QUEUED"
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        for mut o in queued {
            match broker.prepare(&o) {
                Ok(prepared) => o = prepared,
                Err(err) => {
                    o.status = "REJECTED".into();
                    o.reason = err.to_string();
                    let _ = broker_commit(&app, "broker_order", json!(o)).await;
                    continue;
                }
            }
            let reject = {
                let e = app.engine.lock().await;
                if o.reduce_only() {
                    None
                } else if runtime_risk_expired(&e, &o, feeds::now_ms())
                    || !["CONFIRM", "AUTO"].contains(&e.mode.as_str())
                {
                    Some("订单意图过期或模式已变更".to_string())
                } else {
                    e.live_ready().err().or_else(|| {
                        let s = &e.symbols[&o.plan.symbol];
                        (s.health.trust == "LOW"
                            || s.qualification
                                .as_ref()
                                .is_none_or(|q| q.status != "QUALIFIED")
                            || s.features.as_ref().is_none_or(|f| {
                                !f.data_quality.is_empty()
                                    || e.now.saturating_sub(f.ts) > e.config.max_stale_ms
                            }))
                        .then(|| "发送前风险复核失败".into())
                    })
                }
            };
            if let Some(reason) = reject {
                o.status = "CANCELED".into();
                o.reason = reason;
                let _ = broker_commit(&app, "broker_order", json!(o)).await;
                continue;
            }
            o.status = "SUBMITTING".into();
            o.send_ts = feeds::now_ms();
            if let Err(err) = broker_commit(&app, "broker_order", json!(o)).await {
                if !app.store.healthy.load(Ordering::SeqCst) {
                    return;
                }
                o.status = "CANCELED".into();
                o.reason = err.to_string();
                if broker_commit(&app, "broker_order", json!(o)).await.is_err() {
                    return;
                }
                continue;
            }
            // Recheck authorization after the durable write, immediately before network I/O.
            let canceled = {
                let e = app.engine.lock().await;
                !o.reduce_only()
                    && (e.kill_switch
                        || e.mode != o.execution_mode
                        || runtime_risk_expired(&e, &o, feeds::now_ms())
                        || e.live.protection.contains_key(&o.plan.symbol))
            };
            if canceled {
                o.status = "CANCELED".into();
                o.reason = "发送前授权已撤销".into();
                if broker_commit(&app, "broker_order", json!(o)).await.is_err() {
                    return;
                }
                continue;
            }
            // A crash from this point is uncertain, never an instruction to resubmit.
            match broker.submit(&o).await {
                Ok(u) => {
                    if broker_commit(&app, "broker_order", json!(u)).await.is_err() {
                        return;
                    }
                }
                Err(err) => {
                    o.status = if err
                        .downcast_ref::<micro_alpha_engine::broker::Rejected>()
                        .is_some()
                    {
                        "REJECTED"
                    } else {
                        "UNKNOWN"
                    }
                    .into();
                    o.reason = err.to_string();
                    if broker_commit(&app, "broker_order", json!(o)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

async fn research_worker(app: Shared, dir: PathBuf) {
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        timer.tick().await;
        let (job, source) = {
            let e = app.engine.lock().await;
            (e.research.clone(), e.source.clone())
        };
        if job.status != "QUEUED" {
            continue;
        }
        let output = dir.join("research").join(&job.job_id);
        if broker_commit(
            &app,
            "research_started",
            json!({"job_id":job.job_id,"output":output}),
        )
        .await
        .is_err()
        {
            continue;
        }
        let python =
            std::env::var("MICRO_ALPHA_PYTHON").unwrap_or_else(|_| ".venv/bin/python".into());
        let result = tokio::process::Command::new(python)
            .args(["research/train.py", "--db"])
            .arg(&app.store.path)
            .args(["--source", &source, "--model", &job.model, "--output"])
            .arg(&output)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .output();
        let (ok, value) =
            match tokio::time::timeout(std::time::Duration::from_secs(600), result).await {
                Ok(Ok(r)) => research_result(
                    r.status.success(),
                    std::fs::read(output.join("report.json"))
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok()),
                    &String::from_utf8_lossy(&r.stderr),
                ),
                Ok(Err(_)) => (
                    false,
                    json!({"error":"无法启动 Python；请安装 .venv 与研究依赖"}),
                ),
                Err(_) => (
                    false,
                    json!({"error":"研究任务超过 10 分钟，已终止；Champion 未改变"}),
                ),
            };
        let _ = broker_commit(
            &app,
            "research_finished",
            json!({"job_id":job.job_id,"ok":ok,"result":value}),
        )
        .await;
    }
}

fn research_result(success: bool, report: Option<Value>, stderr: &str) -> (bool, Value) {
    let report = report.filter(Value::is_object);
    if success {
        return match report {
            Some(v) => (true, v),
            None => (false, json!({"error":"训练进程没有生成有效报告"})),
        };
    }
    let mut value = report.unwrap_or_else(|| json!({}));
    value["error"] = json!(stderr.chars().take(1000).collect::<String>());
    (false, value)
}

#[cfg(test)]
mod research_result_tests {
    use super::*;

    #[test]
    fn failed_training_preserves_dataset_evidence_without_becoming_success() {
        let evidence = json!({"status":"REJECTED","dataset":{"eligible_outcomes":0}});
        let (ok, report) = research_result(false, Some(evidence), "insufficient samples");
        assert!(!ok);
        assert_eq!(report["dataset"]["eligible_outcomes"], 0);
        assert_eq!(report["error"], "insufficient samples");
        assert!(!research_result(true, None, "").0);
        assert!(!research_result(true, Some(json!(null)), "").0);
        assert!(!research_result(false, Some(json!({"status":"PASS"})), "failure").0);
    }
}

async fn assist(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if headers.get("x-micro-alpha").and_then(|v| v.to_str().ok()) != Some("local") {
        return Err(StatusCode::FORBIDDEN);
    }
    if headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|o| {
            ![
                "http://localhost:3000",
                "http://127.0.0.1:3000",
                "http://127.0.0.1:8787",
            ]
            .contains(&o)
        })
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let symbol = payload["symbol"].as_str().unwrap_or("BTCUSDT");
    let (brief, evidence) = {
        let e = app.engine.lock().await;
        let s = e.symbols.get(symbol).ok_or(StatusCode::BAD_REQUEST)?;
        let mut text=format!("{} · {}\n信号方向：{}；模型可信度：{}。\n资格阻断：{}。\n近期可比样本 {} 条，校准误差 {:.2}%。\n{}\n解释只读取冻结输出，不修改概率、仓位或订单。",
            symbol,e.mode,s.candidate_direction,s.health.trust,s.qualification.as_ref().map(|q|q.reasons.join("、")).filter(|s|!s.is_empty()).unwrap_or_else(||if s.qualification.is_some(){"当前资格通过，仍须执行前复核".into()}else{"暂无有效预测 / 资格".into()}),
            s.health.samples,s.health.calibration_error*100.,if s.health.retrain_recommended {"模型健康下降，建议创建候选模型并验证。"}else{"当前没有健康状态触发的重训建议。"});
        text.push_str(&format!(
            "\n今日自动结果 {} 条；本地 PAPER 当日净收益 {:.2} USDT。",
            e.outcomes
                .iter()
                .filter(|o| o.first_hit_ts / 86_400_000 == e.now / 86_400_000)
                .count(),
            e.daily_pnl
        ));
        (
            text,
            json!({"daily_summary":{"paper_pnl":e.daily_pnl,"exchange_pnl":e.live.account.as_ref().map(|a|a.daily_pnl)},"recent_paper_trades":e.trades.iter().rev().take(5).collect::<Vec<_>>(),"recent_exchange_trades":e.live.trades.iter().rev().take(5).collect::<Vec<_>>(),"features":s.features,"prediction":s.active,"qualification":s.qualification,"health":s.health,"last_result":s.last_result,"research":e.research.result}),
        )
    };
    let Some(model) =
        micro_alpha_engine::assist::from_env().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    else {
        return Ok(Json(json!({"provider":"deterministic-brief","text":brief})));
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result=client.post("http://127.0.0.1:11434/api/chat").json(&json!({"model":model,"stream":false,"options":{"temperature":0,"num_predict":600},"messages":[{"role":"system","content":"用中文解释提供的量化证据，说明信号驱动、拒绝原因、结果复盘、漂移与研究结论。不得创造数字或提出修改概率/门槛/下单的指令。没有证据则明确缺失。仅返回简短说明。"},{"role":"user","content":evidence.to_string()}]})).send().await;
    let generated = match result {
        Ok(r) if r.status().is_success() => r.json::<Value>().await.ok().and_then(|v| {
            v["message"]["content"]
                .as_str()
                .map(|s| s.chars().take(4000).collect::<String>())
        }),
        _ => None,
    };
    Ok(Json(match generated {
        Some(text) => {
            json!({"provider":"local-ollama","text":format!("本地 AI 解释（不参与决策）\n{text}\n\n原始状态摘要\n{brief}")})
        }
        None => {
            json!({"provider":"deterministic-brief","text":format!("本地 AI 暂不可用。\n{brief}")})
        }
    }))
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[tokio::test]
    async fn rejected_admission_is_durable_and_replays_its_state_changes() {
        let dir = std::env::temp_dir().join(format!(
            "admission-journal-{}-{}",
            std::process::id(),
            feeds::now_ms()
        ));
        let (store, _) = Store::open(&dir, "live").unwrap();
        let mut e = Engine::new_reference(true);
        for i in 0..8 {
            for event in feeds::demo_events(i, 1_000_000 + i * 1000) {
                e.ingest(event);
            }
        }
        e.source = "live".into();
        e.mode = "AUTO".into();
        e.config.min_net_pnl = 0.01;
        e.config.auto_required_model_trust = "NORMAL".into();
        e.config.auto_min_net_edge = e.config.min_net_edge;
        e.live.configured = true;
        e.live.healthy = true;
        e.live.cap = 500.;
        e.live.auto_allowed = true;
        e.live.account = Some(micro_alpha_engine::live::AccountSnapshot {
            ts: e.now,
            equity: 10000.,
            available: 10000.,
            one_way: true,
            ..Default::default()
        });
        for s in e.symbols.values_mut() {
            s.health.trust = "NORMAL".into();
            s.health.samples = 100;
            s.health.calibration_error = 0.01;
        }
        let id = e.symbols["BTCUSDT"]
            .active
            .as_ref()
            .unwrap()
            .prediction_id
            .clone();
        e.records.clear();
        let mut replayed = e.clone();
        let path = store.journal_path.clone();
        let initial = Journal::Start {
            engine: Box::new(e.clone()),
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        store
            .tx
            .send(Batch {
                journal: initial,
                records: vec![],
                checkpoint: Some(serde_json::to_string(&e).unwrap()),
                committed: Some(tx),
            })
            .await
            .unwrap();
        rx.await.unwrap().unwrap();
        let app = Arc::new(App {
            engine: Mutex::new(e),
            store,
        });
        let err = broker_commit(
            &app,
            "execute",
            json!({"symbol":"BTCUSDT","prediction_id":id}),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("AUTO_LIVE_EVIDENCE_REQUIRED"),
            "{err}"
        );
        let actual = app.engine.lock().await.clone();
        assert!(actual.integrity.realtime_evaluations > replayed.integrity.realtime_evaluations);
        assert!(actual.live.orders.is_empty());
        let history = Store::history(&app.store.path, "auto_admission", 10).unwrap();
        assert_eq!(history[0]["payload"]["decision"], "SKIPPED");
        let lines = std::fs::read_to_string(&path).unwrap();
        match serde_json::from_str::<Journal>(lines.lines().last().unwrap()).unwrap() {
            Journal::RejectedCommand {
                command,
                payload,
                error,
            } => assert_eq!(replayed.command(&command, payload), Err(error)),
            _ => panic!("rejected command must be journaled"),
        }
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(replayed).unwrap()
        );
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod execution_tests {
    use super::*;
    use micro_alpha_engine::live::{AccountSnapshot, Intent};
    use std::sync::atomic::AtomicUsize;
    #[derive(Clone)]
    struct Gateway {
        account_started: Arc<tokio::sync::Notify>,
        account_release: Arc<tokio::sync::Notify>,
        submitted: Arc<tokio::sync::Notify>,
        canceled: Arc<tokio::sync::Notify>,
        count: Arc<AtomicUsize>,
        journal: PathBuf,
    }
    impl ExecutionGateway for Gateway {
        fn prepare(&self, o: &Intent) -> Result<Intent> {
            Ok(o.clone())
        }
        async fn account(&mut self) -> Result<AccountSnapshot> {
            self.account_started.notify_one();
            self.account_release.notified().await;
            anyhow::bail!("simulated account unavailable")
        }
        async fn submit(&mut self, o: &Intent) -> Result<Intent> {
            assert!(o.reduce_only());
            assert_eq!(o.status, "SUBMITTING");
            let journal = std::fs::read_to_string(&self.journal).unwrap();
            assert!(journal.lines().any(|line| matches!(serde_json::from_str::<Journal>(line).unwrap(),
                Journal::Command { command, payload } if command == "broker_order" && payload["client_id"] == o.client_id && payload["status"] == "SUBMITTING")));
            self.count.fetch_add(1, Ordering::SeqCst);
            self.submitted.notify_one();
            anyhow::bail!("simulated ambiguous network timeout")
        }
        async fn reconcile(&self, _: &Intent) -> Result<Intent> {
            anyhow::bail!("still unknown")
        }
        async fn cancel(&self, _: &Intent) -> Result<()> {
            self.canceled.notify_one();
            Ok(())
        }
    }
    #[tokio::test]
    async fn account_stall_does_not_block_protection_or_cancel_and_unknown_is_not_resubmitted() {
        let dir = std::env::temp_dir().join(format!(
            "execution-isolation-{}-{}",
            std::process::id(),
            feeds::now_ms()
        ));
        let (store, _) = Store::open(&dir, "live").unwrap();
        let mut e = Engine::new_reference(true);
        for i in 0..8 {
            for event in feeds::demo_events(i, 1000000 + i * 1000) {
                e.ingest(event);
            }
        }
        let mut prediction = e.symbols["BTCUSDT"].active.clone().unwrap();
        prediction.direction = "LONG".into();
        prediction.calibrated_probabilities = Some([0.9, 0.05, 0.05]);
        let mut health = e.symbols["BTCUSDT"].health.clone();
        health.trust = "NORMAL".into();
        health.samples = 100;
        e.config.min_net_pnl = 0.01;
        let (q, plan) = e.qualify(
            &prediction,
            e.symbols["BTCUSDT"].features.as_ref().unwrap(),
            &health,
        );
        let mut entry = Intent {
            feedback: None,
            execution_mode: "CONFIRM".into(),
            admission: None,
            client_id: "entry-isolation".into(),
            parent_id: None,
            action: "ENTRY".into(),
            plan: plan.unwrap(),
            qualification: q,
            created_ts: e.now,
            deadline_ts: e.now + 60000,
            quantity: 0.001,
            limit_price: 100.,
            status: "PARTIALLY_FILLED".into(),
            exchange_order_id: None,
            filled_qty: 0.0005,
            average_price: 100.,
            fee: 0.,
            send_ts: e.now,
            ack_ts: e.now,
            fill_ts: e.now,
            reason: String::new(),
            alpha_id: None,
        };
        // Even when all event-time checks appear fresh, a stalled runtime cannot
        // submit new risk. Reductions must remain available during that stall.
        assert!(!runtime_risk_expired(&e, &entry, e.now));
        assert!(runtime_risk_expired(
            &e,
            &entry,
            e.now + e.config.max_stale_ms + 1
        ));
        let mut protection = entry.clone();
        protection.action = "EXIT".into();
        assert!(!runtime_risk_expired(&e, &protection, e.now + 60_001));
        e.live.orders.insert(entry.client_id.clone(), entry.clone());
        let mut blocked = entry.clone();
        blocked.client_id = "blocked-entry".into();
        blocked.plan.symbol = "ETHUSDT".into();
        blocked.status = "QUEUED".into();
        blocked.filled_qty = 0.;
        e.live.orders.insert(blocked.client_id.clone(), blocked);
        entry.client_id = "exit-isolation".into();
        entry.parent_id = Some("entry-isolation".into());
        entry.action = "EXIT".into();
        entry.quantity = 0.0005;
        entry.filled_qty = 0.;
        entry.status = "QUEUED".into();
        e.live.orders.insert(entry.client_id.clone(), entry);
        e.live
            .protection
            .insert("BTCUSDT".into(), "test protection".into());
        e.kill_switch = true;
        let app = Arc::new(App {
            engine: Mutex::new(e),
            store,
        });
        let gateway = Gateway {
            account_started: Arc::new(tokio::sync::Notify::new()),
            account_release: Arc::new(tokio::sync::Notify::new()),
            submitted: Arc::new(tokio::sync::Notify::new()),
            canceled: Arc::new(tokio::sync::Notify::new()),
            count: Arc::new(AtomicUsize::new(0)),
            journal: app.store.journal_path.clone(),
        };
        let (_tx, rx) = tokio::sync::watch::channel(gateway.clone());
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(account_worker(app.clone(), rx.clone()));
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            gateway.account_started.notified(),
        )
        .await
        .unwrap();
        tasks.spawn(order_worker(app.clone(), rx.clone(), "BTCUSDT", true));
        tasks.spawn(order_worker(app.clone(), rx.clone(), "ETHUSDT", false));
        tasks.spawn(cancel_worker(app.clone(), rx, "BTCUSDT"));
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            gateway.submitted.notified(),
        )
        .await
        .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            gateway.canceled.notified(),
        )
        .await
        .unwrap();
        gateway.account_release.notify_one();
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(gateway.count.load(Ordering::SeqCst), 1);
        let e = app.engine.lock().await;
        assert_eq!(e.live.orders["exit-isolation"].status, "UNKNOWN");
        assert_eq!(e.live.orders["blocked-entry"].status, "CANCELED");
        assert!(!e.live.healthy);
        drop(e);
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}
